#!/usr/bin/env python3
"""E21-XR — is the proposer signal rater-specific or shared semantic structure?

E21 trained centroids on ONE rater's (Claude) cell labels and tested containment
of that SAME rater's labels. That cannot separate "geometry carries semantic
structure" from "geometry memorises this rater's policy". This script adds a
second rater (deepseek-chat, research/p4_rater2_deepseek.json) and crosses them:

    A->A   train Claude labels   test Claude labels
    B->B   train DeepSeek labels test DeepSeek labels
    A->B   train Claude labels   test DeepSeek labels   (cross-rater transfer)
    B->A   train DeepSeek labels test Claude labels     (cross-rater transfer)

Same 582 canonical entries, identical deterministic splits for every arm.
Every arm/fraction has a matched permutation null that shuffles ONLY the train
labels, so the split, the train label marginals and the train cell coverage are
preserved exactly.

NOTES ON THE DATA (why this joins on `n`, not on name)
  refiling_gate0.tsv has 661 data rows: 585 blind + 76 HINT-LEAKS-CELL. Of the
  585 blind, 582 have a bge vector in mapper/corpus.json -- that is the
  manuscript's 582. Names are NOT unique in the tsv (585 blind lines, 573
  distinct names; e.g. 'Incident Response' x3), so a name-keyed join silently
  merges entries and loses each row's own new_cell. The tsv row index (column
  `n`) is unique and is also carried by the rater-2 file, so the join is on `n`.

  Reused nothing from mapper/run_e21.py: its load() drops the tsv row index
  (the only sound join key) and collapses duplicate-name rows, so importing it
  would have been the bug, not the shortcut. The centroid/rank math below is the
  same construction as run_e21.topk(..., mode='geom').

Usage:
  python3 research/papers/e21_cross_rater.py [--splits 40] [--perms 200]

Writes research/e21_cross_rater_results.json. verify_paper4.py reads that file.
"""
import argparse
import json
import os
import random
import sys

import numpy as np
from scipy import stats

HERE = os.path.dirname(os.path.abspath(__file__))
RESEARCH = os.path.normpath(os.path.join(HERE, ".."))
GT = os.path.join(RESEARCH, "perspective-discovery", "refiling_gate0.tsv")
CORPUS = os.path.join(RESEARCH, "mapper", "corpus.json")
RATER2 = os.path.join(RESEARCH, "p4_rater2_deepseek.json")
DEST = os.path.join(RESEARCH, "e21_cross_rater_results.json")

FRACS = (0.10, 0.25, 0.50, 0.75)
KS = (1, 3, 5)
ARMS = (("A->A", "claude", "claude"), ("B->B", "deepseek", "deepseek"),
        ("A->B", "claude", "deepseek"), ("B->A", "deepseek", "claude"))
ABSENT = 10 ** 6
# test entries bucketed by how much TRAIN support their own true cell has
BUCKETS = {"supported": lambda s: s >= 2, "thin": lambda s: s == 1,
           "unseen": lambda s: s == 0}


def load():
    """Return X (n,768) L2-normalised and per-rater label arrays.

    Order is tsv row order -- deterministic, and identical for every arm.
    """
    vecs = {e["name"]: e["vec"] for e in json.load(open(CORPUS))["entries"]}
    rater2 = {r["n"]: r["rater2_cell"] for r in json.load(open(RATER2))["rows"]}

    ids, claude, deepseek, names = [], [], [], []
    for line in open(GT):
        if line.startswith("#") or line.startswith("n\t"):
            continue
        f = line.rstrip("\n").split("\t")
        if len(f) < 7 or f[6] != "blind":          # 76 HINT-LEAKS-CELL dropped
            continue
        n, name, new = int(f[0]), f[2], f[4]
        if name not in vecs or n not in rater2:     # canonical 582
            continue
        ids.append(n)
        names.append(name)
        claude.append(new)
        deepseek.append(rater2[n])
    if len(ids) != 582:
        print(f"WARNING: canonical set is {len(ids)}, manuscript says 582", file=sys.stderr)

    X = np.asarray([vecs[n] for n in names], dtype=np.float32)
    X /= np.linalg.norm(X, axis=1, keepdims=True) + 1e-12
    return X, {"claude": np.asarray(claude), "deepseek": np.asarray(deepseek)}, ids


def split_indices(n, splits):
    """Deterministic per-split index permutations -- shared by all arms.

    Uses stdlib random.Random(s).shuffle exactly as mapper/run_e21.py topk() does,
    so split s is E21's split s: with --splits 5 the A->A arm at frac 0.75 must
    reproduce the published GEOMETRY row (0.438 / 0.667 / 0.755). That equality is
    the integrity check on this whole pipeline.
    """
    idx = np.empty((splits, n), dtype=int)
    for s in range(splits):
        row = list(range(n))
        random.Random(s).shuffle(row)
        idx[s] = row
    return idx


def rank_test(Xtr, ytr, Xte, yte):
    """Centroid classification. Returns (rank of each test entry's own label,
    number of candidate cells). Rank = ABSENT when the label is not a candidate.

    Indicator-matrix centroid (BLAS matmul) instead of np.add.at: the null loop
    calls this ~20k times, and np.add.at dominated the runtime.
    """
    cells, inv = np.unique(ytr, return_inverse=True)
    I = np.zeros((len(ytr), len(cells)), dtype=np.float32)
    I[np.arange(len(ytr)), inv] = 1.0
    C = I.T @ Xtr
    C /= np.linalg.norm(C, axis=1, keepdims=True) + 1e-12
    sims = Xte @ C.T
    j = np.searchsorted(cells, yte)
    jc = np.clip(j, 0, len(cells) - 1)
    valid = cells[jc] == yte
    truth = np.where(valid, jc, 0)
    rank = (sims > sims[np.arange(len(yte)), truth][:, None]).sum(axis=1) + 1
    rank[~valid] = ABSENT
    return rank, len(cells)


def majority_hits(ytr, ks=KS):
    """top-k containment of the single most frequent train cell: constant over
    the test set, so it is the cumulative share of the k largest train cells."""
    _, cnt = np.unique(ytr, return_counts=True)
    share = np.cumsum(np.sort(cnt)[::-1]) / cnt.sum()
    return {k: float(share[min(k, len(share)) - 1]) for k in ks}


def evaluate(X, labels, tr_key, te_key, idx, splits, perms, fracs, rowsel=None):
    """One arm: geometry + matched null + coverage + seen/unseen + majority."""
    out = {}
    n = X.shape[0]
    for frac in fracs:
        all_cells = len(np.unique(labels[tr_key])) if rowsel is None else \
            len(np.unique(labels[tr_key][rowsel]))
        seed_bits = int(round(frac * 1000)) * 10 ** 6

        geo = {k: [] for k in KS}
        nul_split = {k: [] for k in KS}
        cov, present_rate = [], []
        bh = {b: {k: [] for k in KS} for b in BUCKETS}
        bn = {b: [] for b in BUCKETS}
        maj = {k: [] for k in KS}
        cells_mean = []
        n_train = 0

        for s in range(splits):
            order = idx[s] if rowsel is None else idx[s][np.isin(idx[s], rowsel)]
            n_train = max(2, int(frac * len(order)))
            tr, te = order[:n_train], order[n_train:]
            if len(te) == 0 or len(np.unique(labels[tr_key][tr])) < 1:
                continue
            ytr = labels[tr_key][tr]
            yte = labels[te_key][te]

            rank, ncell = rank_test(X[tr], ytr, X[te], yte)
            cov.append(ncell / all_cells)
            cells_mean.append(ncell)
            # how much training support the test entry's own cell has: 0 = the
            # cell is not even a candidate (containment is impossible, so the
            # ceiling matters more than the score), 1 = thin, >=2 = supported.
            ucells, ucounts = np.unique(ytr, return_counts=True)
            jj = np.clip(np.searchsorted(ucells, yte), 0, len(ucells) - 1)
            supp = np.where(ucells[jj] == yte, ucounts[jj], 0)
            present_rate.append(float(np.mean(supp > 0)))
            for b in BUCKETS:
                m = BUCKETS[b](supp)
                bn[b].append(float(np.mean(m)))
                for k in KS:
                    bh[b][k].append(float(np.mean(rank[m] <= k)) if m.any() else 0.0)
            for k in KS:
                geo[k].append(float(np.mean(rank <= k)))
                maj[k].append(majority_hits(ytr)[k])
            # matched null: shuffle ONLY the train labels -- split, train label
            # marginals and train cell coverage are untouched by construction
            run_nul = {k: [] for k in KS}
            for p in range(perms):
                rng = np.random.default_rng([seed_bits, s, p])
                nrank, _ = rank_test(X[tr], rng.permutation(ytr), X[te], yte)
                for k in KS:
                    run_nul[k].append(float(np.mean(nrank <= k)))
            for k in KS:
                nul_split[k].append(float(np.mean(run_nul[k])))

        def ci(v):
            v = np.asarray(v, dtype=float)
            sem = float(stats.sem(v)) if len(v) > 1 else 0.0
            lo, hi = stats.t.interval(0.95, len(v) - 1, loc=v.mean(), scale=sem or 1e-12)
            return [float(lo), float(hi)]

        per_k = {}
        for k in KS:
            g, nv = float(np.mean(geo[k])), float(np.mean(nul_split[k]))
            per_k[f"top{k}"] = g
            per_k[f"top{k}_ci95"] = ci(geo[k])
            per_k[f"per_split_top{k}"] = [round(x, 4) for x in geo[k]]
            per_k[f"first5_top{k}"] = float(np.mean(geo[k][:5]))
            per_k[f"null_top{k}"] = nv
            per_k[f"null_top{k}_ci95"] = ci(nul_split[k])
            per_k[f"margin_top{k}"] = g - nv
            per_k[f"majority_top{k}"] = float(np.mean(maj[k]))
            for b in BUCKETS:
                per_k[f"{b}_top{k}"] = float(np.mean(bh[b][k]))
                per_k[f"{b}_n"] = float(np.mean(bn[b]))

        out[f"{frac:.2f}"] = dict(
            n_train=int(n_train), n_test=int(n - n_train),
            **per_k,
            train_coverage=float(np.mean(cov)),
            train_cells_mean=float(np.mean(cells_mean)),
            label_present_rate=float(np.mean(present_rate)),
        )
    return out


def run_arms(X, labels, idx, splits, perms, fracs, rowsel=None):
    return {name: evaluate(X, labels, tk, ek, idx, splits, perms, fracs, rowsel)
            for name, tk, ek in ARMS}


def kappa(a, b):
    """Cohen's kappa + bootstrap CI over items (2000 resamples)."""
    a, b = np.asarray(a), np.asarray(b)
    cats = np.unique(np.concatenate([a, b]))
    index = {c: i for i, c in enumerate(cats)}
    M = np.zeros((len(cats), len(cats)))
    np.add.at(M, (np.fromiter((index[c] for c in a), int, len(a)),
                  np.fromiter((index[c] for c in b), int, len(b))), 1)

    def k_of(m, n):
        po = np.trace(m) / n
        pe = float((m.sum(1) / n) @ (m.sum(0) / n))
        return (po - pe) / (1 - pe)

    obs = float(k_of(M, len(a)))
    rng = np.random.default_rng(7)
    boot = []
    for _ in range(2000):
        s = rng.integers(0, len(a), len(a))
        B = np.zeros_like(M)
        np.add.at(B, (np.fromiter((index[c] for c in a[s]), int, len(s)),
                      np.fromiter((index[c] for c in b[s]), int, len(s))), 1)
        try:
            boot.append(k_of(B, len(s)))
        except ZeroDivisionError:
            pass
    lo, hi = np.percentile(boot, [2.5, 97.5])
    return {"cohen_kappa": obs, "ci95": [float(lo), float(hi)],
            "agreement": float(np.mean(a == b)),
            "agreement_ci95": [float(x) for x in
                               stats.t.interval(0.95, len(a) - 1,
                                                loc=np.mean(a == b),
                                                scale=stats.sem(a == b) or 1e-12)]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--splits", type=int, default=40)
    ap.add_argument("--perms", type=int, default=200)
    ap.add_argument("--light-splits", type=int, default=20)
    ap.add_argument("--light-perms", type=int, default=100)
    ap.add_argument("--out", default=DEST)
    a = ap.parse_args()

    X, labels, ids = load()
    n = X.shape[0]
    idx = split_indices(n, a.splits)
    cl, ds = set(labels["claude"]), set(labels["deepseek"])
    shared = cl & ds

    # lower-resolution views: the coarse axis pair, and the mutually-usable
    # vocabulary (entries both raters filed inside their shared cell set)
    axis = {"domain": {r: np.asarray([c.split("/")[0] for c in v])
                       for r, v in labels.items()},
            "mode": {r: np.asarray([c.split("/")[1] if "/" in c else "?"
                                    for c in v]) for r, v in labels.items()}}
    shared_sel = np.asarray([c in shared and d in shared
                             for c, d in zip(labels["claude"], labels["deepseek"])])

    print(f"n={n}  cells claude={len(cl)} deepseek={len(ds)} shared={len(shared)}  "
          f"shared-vocab rows={int(shared_sel.sum())}", file=sys.stderr)
    results = {
        "n": int(n),
        "n_cells": {"claude": len(cl), "deepseek": len(ds), "shared": len(shared)},
        "vocab": {"shared": sorted(shared), "claude_only": sorted(cl - ds),
                  "deepseek_only": sorted(ds - cl)},
        "config": {"splits": a.splits, "perms": a.perms,
                   "light_splits": a.light_splits, "light_perms": a.light_perms,
                   "fracs": list(FRACS), "split_seed": 20260924,
                   "join_key": "gate0.tsv column n",
                   "null": "train labels shuffled only; split + train marginals + train cell coverage preserved"},
        "arms": run_arms(X, labels, idx, a.splits, a.perms, FRACS),
        "shared_vocab": {
            "n_rows": int(shared_sel.sum()), "vocabulary_size": len(shared),
            "note": "rows both raters filed inside claude-n-deepseek; candidate set is that shared vocabulary",
            "arms": run_arms(X, labels, idx, a.light_splits, a.light_perms, FRACS,
                             rowsel=np.where(shared_sel)[0]),
        },
        "axis_transfer": {ax: {"claude_cells": len(set(axis[ax]["claude"])),
                               "deepseek_cells": len(set(axis[ax]["deepseek"])),
                               "arms": run_arms(X, axis[ax], idx, a.light_splits,
                                                a.light_perms, FRACS)}
                          for ax in ("domain", "mode")},
        "inter_rater": dict(
            n=int(n), claude_cells=len(cl), deepseek_cells=len(ds),
            **kappa(labels["claude"], labels["deepseek"])),
    }
    json.dump(results, open(a.out, "w"), indent=2)
    print(f"wrote {a.out}")
    print(f"inter-rater: agree={results['inter_rater']['agreement']:.4f} "
          f"kappa={results['inter_rater']['cohen_kappa']:.4f} "
          f"CI={[round(x, 3) for x in results['inter_rater']['ci95']]}")
    for name, r in results["arms"].items():
        print(f"  {name:5s} " + "  ".join(
            f"f{f}: t3={r[f]['top3']:.3f}{tuple(round(x,3) for x in r[f]['top3_ci95'])} "
            f"null={r[f]['null_top3']:.3f} m={r[f]['margin_top3']:+.3f} "
            f"sup={r[f]['supported_top3']:.3f} thin={r[f]['thin_top3']:.3f} "
            f"pres={r[f]['label_present_rate']:.3f}" for f in r))


if __name__ == "__main__":
    main()
