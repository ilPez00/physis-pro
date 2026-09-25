#!/usr/bin/env python3
"""Hostile consistency check on research/e21_cross_rater_results.json.

Adversarial on purpose: every check below is one that would fail if the numbers
had been mis-stored, mis-joined, or quietly fudged. Run after e21_cross_rater.py.

  python3 research/papers/check_e21_cross_rater.py

Exit code is non-zero if any HARD check fails.
"""
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
RESEARCH = os.path.normpath(os.path.join(HERE, ".."))
XR = os.path.join(RESEARCH, "e21_cross_rater_results.json")
if len(sys.argv) > 1:                      # optional path override, for dry runs
    XR = sys.argv[1]
E21 = os.path.join(RESEARCH, "e21_proposer_results.json")
GT = os.path.join(RESEARCH, "perspective-discovery", "refiling_gate0.tsv")
CORPUS = os.path.join(RESEARCH, "mapper", "corpus.json")
RATER2 = os.path.join(RESEARCH, "p4_rater2_deepseek.json")

fails, warns = [], []


def hard(ok, msg):
    print(f"  [{'ok ' if ok else 'FAIL'}] {msg}")
    if not ok:
        fails.append(msg)


def soft(ok, msg):
    print(f"  [{'ok ' if ok else 'warn'}] {msg}")
    if not ok:
        warns.append(msg)


def main():
    d = json.load(open(XR))
    e21 = json.load(open(E21))
    arms = d["arms"]
    groups = {"arms": arms}
    if d.get("shared_vocab"):
        groups["shared_vocab"] = d["shared_vocab"]["arms"]
    for ax, t in (d.get("axis_transfer") or {}).items():
        groups[f"axis/{ax}"] = t["arms"]

    print("1. provenance / shape")
    hard(d["n"] == 582, f"n == 582 (got {d['n']})")
    hard(d["n_cells"]["claude"] == 57, f"claude cells == 57 (got {d['n_cells']['claude']})")
    # independent recount straight from the raw files
    vec = {e["name"] for e in json.load(open(CORPUS))["entries"]}
    r2 = {r["n"] for r in json.load(open(RATER2))["rows"]}
    gt = [l.rstrip("\n").split("\t") for l in open(GT)
          if not l.startswith(("#", "n\t"))]
    blind = [f for f in gt if len(f) >= 7 and f[6] == "blind"]
    canon = [f for f in blind if f[2] in vec and int(f[0]) in r2]
    hard(len(canon) == 582, f"canonical set recount == 582 (got {len(canon)} of "
                            f"{len(blind)} blind / {len(gt)} rows)")
    hard(len({f[2] for f in blind}) == 573,
         f"names are NOT unique in the tsv ({len({f[2] for f in blind})} distinct for "
         f"{len(blind)} blind rows) -> a name-keyed join would have collapsed rows; "
         "join must be on `n`")

    print("2. integrity with the published E21 pipeline")
    g = arms["A->A"]["0.75"]
    for k, want in ((1, 0.438), (3, 0.667), (5, 0.755)):
        hard(abs(g[f"first5_top{k}"] - want) < 5e-4,
             f"A->A @0.75 first-5 splits top-{k} == {want} (got {g[f'first5_top{k}']:.4f})")
    pub = e21["blocks"]["GEOMETRY"][-1]
    hard(abs(g["first5_top3"] - pub["top3"]) < 5e-4,
         f"matches e21_proposer_results.json GEOMETRY@436 top3={pub['top3']}")

    print("3. split geometry matches the manuscript")
    for f, (ntr, nte) in (("0.10", (58, 524)), ("0.25", (145, 437)),
                          ("0.50", (291, 291)), ("0.75", (436, 146))):
        r = arms["A->A"][f]
        hard((r["n_train"], r["n_test"]) == (ntr, nte),
             f"frac {f}: n_train/n_test == {ntr}/{nte} (got {r['n_train']}/{r['n_test']})")

    print("4. arithmetic and invariants, every arm/fraction")
    bad = []
    for gn, gg in groups.items():
        for an, fr in gg.items():
            for f, r in fr.items():
                for k in (1, 3, 5):
                    m = r[f"top{k}"] - r[f"null_top{k}"]
                    if abs(m - r[f"margin_top{k}"]) > 1e-6:
                        bad.append(f"{gn}/{an}/{f}/top{k} margin != top-null")
                    if not (r[f"top{k}_ci95"][0] <= r[f"top{k}"] <= r[f"top{k}_ci95"][1]):
                        bad.append(f"{gn}/{an}/{f}/top{k} CI excludes the estimate")
                    if not (r[f"null_top{k}_ci95"][0] <= r[f"null_top{k}"]
                            <= r[f"null_top{k}_ci95"][1]):
                        bad.append(f"{gn}/{an}/{f}/top{k} null CI excludes the null")
                    # containment can never exceed the fraction of test entries
                    # whose own label is even a candidate
                    if r[f"top{k}"] > r["label_present_rate"] + 0.02:
                        bad.append(f"{gn}/{an}/{f}/top{k}={r[f'top{k}']:.3f} exceeds "
                                   f"the candidate ceiling {r['label_present_rate']:.3f}")
                if not (r["majority_top1"] <= r["majority_top3"] + 1e-9
                        <= r["majority_top5"] + 1e-9):
                    bad.append(f"{gn}/{an}/{f} majority top-k not monotone")
                # bucket decomposition: top3 is the n-weighted average of the
                # supported / thin / unseen buckets (unseen is 0 by construction)
                rec = (r["supported_n"] * r["supported_top3"] + r["thin_n"] * r["thin_top3"]) \
                    / max(r["supported_n"] + r["thin_n"] + r["unseen_n"], 1e-9)
                if abs(rec - r["top3"]) > 0.03:
                    bad.append(f"{gn}/{an}/{f} bucket decomposition top3: "
                               f"{r['top3']:.3f} vs recomputed {rec:.3f}")
    hard(not bad, f"invariants hold across {sum(len(v) for g in groups.values() for v in g.values())} "
                  f"arm-fractions ({len(bad)} violations)")
    for b in bad[:8]:
        print(f"        - {b}")

    print("5. does any arm lose to the trivial baseline?")
    for gn, gg in groups.items():
        for an, fr in gg.items():
            for f, r in fr.items():
                if r["top3"] < r["majority_top3"]:
                    print(f"  [note] {gn}/{an} frac {f}: geometry top3 "
                          f"{r['top3']:.3f} < majority {r['majority_top3']:.3f}")

    print("6. inter-rater agreement, recomputed from raw files")
    cl = {f[0]: f[4] for f in canon}
    ds = {str(r["n"]): r["rater2_cell"] for r in json.load(open(RATER2))["rows"]}
    a = [cl[k] for k in cl]
    b = [ds[k] for k in cl]
    agree = np.mean([x == y for x, y in zip(a, b)])
    cats = sorted(set(a) | set(b))          # square contingency over the union:
    ci = {c: i for i, c in enumerate(cats)}  # the two raters do not share a vocabulary
    M = np.zeros((len(cats), len(cats)))
    for x, y in zip(a, b):
        M[ci[x], ci[y]] += 1
    po = np.trace(M) / len(a)
    pe = float((M.sum(1) / len(a)) @ (M.sum(0) / len(a)))
    kappa = (po - pe) / (1 - pe)
    ir = d["inter_rater"]
    hard(abs(ir["agreement"] - agree) < 1e-6,
         f"agreement == {agree:.4f} (stored {ir['agreement']:.4f})")
    hard(abs(ir["cohen_kappa"] - kappa) < 1e-6,
         f"kappa == {kappa:.4f} (stored {ir['cohen_kappa']:.4f})")
    hard(ir["n"] == 582, f"inter-rater n == 582 (got {ir['n']})")
    print(f"        observed {po:.4f}, chance {pe:.4f} -> "
          f"the raters agree on {po:.2f} of items but only {kappa:.2f} of chance-corrected agreement")

    print("7. the claim that is actually tested")
    for an in ("A->A", "B->B", "A->B", "B->A"):
        r = arms[an]["0.75"]
        lo, hi = r["top3_ci95"]
        nlo, nhi = r["null_top3_ci95"]
        print(f"        {an}: top3 {r['top3']:.3f} [{lo:.3f},{hi:.3f}]  "
              f"null {r['null_top3']:.3f} [{nlo:.3f},{nhi:.3f}]  "
              f"margin {r['margin_top3']:+.3f}  "
              f"{'SEPARATED' if lo > nhi else 'OVERLAPPING'}")

    print()
    print(f"HARD FAILURES: {len(fails)}   warnings: {len(warns)}")
    for f in fails:
        print(f"  FAIL {f}")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
