#!/usr/bin/env python3
"""E90 — does the rank intervention the literature prescribes move the slope?

E88 left machinery behind: per-embedder anisotropy (0.527-0.562) with fit ARI
against true cells (0.062-0.080) moving opposite anisotropy, plus a
cross-embedder ARI band of 0.108-0.178 that reproduces the settled refutation.
The prescription in the sweep (PLAN.md E4, R1 — Parupudi: rank-based metrics
beat cosine under anisotropy) is an intervention: replace raw cosine with a
rank metric and watch whether fit ARI rises and the embedder gap narrows.

The intervention is the smallest one with a name: spherical k-means in
rank space. Each vector is replaced by its within-row ranks, standardized;
assignment then runs on rank correlation (Spearman) instead of raw cosine.
Same corpus, same k=57, same seed, same ARI machinery as E88.

Prediction, preregistered: fit ARI rises for all three sets and rises MOST
for the most anisotropic set (bge nameonly, 0.562); the cross-embedder ARI
band widens or shifts upward rather than sitting at 0.108-0.178.

    python3 research/mapper/e90_rank_intervention.py
"""
import json, os, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import e88_anisotropy_covariate as e88


def rank_standardize(v):
    """Within-row ranks, standardized — cosine in this space is Spearman."""
    r = np.argsort(np.argsort(v, axis=1), axis=1).astype(np.float64)
    m = r.mean(axis=1, keepdims=True)
    s = r.std(axis=1, keepdims=True)
    s[s == 0] = 1.0
    return ((r - m) / s).astype(np.float32)


def main():
    path_of = lambda nm: dict(e88.SETS)[nm]
    corpus = json.load(open(os.path.join(HERE, "corpus.json")))
    names = [e["name"] for e in corpus["entries"]]
    cells = e88.true_cells(corpus)
    uniq = sorted(set(cells))
    k = len(uniq)
    truth = np.array([uniq.index(c) for c in cells])

    arms = [("cosine (E88)", lambda v: e88.l2(v)),
            ("rank (E90)", lambda v: rank_standardize(v))]
    table = {}
    assignments = {}
    fits = {}
    subs = {}
    print("")
    for arm_name, prep in arms:
        print("=== " + arm_name)
        runs, arm_fits, arm_subs = {}, {}, {}
        for name, path in e88.SETS:
            keep, raw = e88.load(path, names)
            idx = {n: i for i, n in enumerate(keep)}
            sel = [i for i, n in enumerate(names) if n in idx]
            order = np.array([idx[n] for n in names if n in idx], dtype=int)
            sub_truth = truth[sel]
            prepared = prep(raw[order])
            n = np.linalg.norm(prepared, axis=1, keepdims=True)
            prepared = prepared / np.maximum(n, 1e-12)
            assign = e88.cosine_kmeans(prepared, k)
            fit = e88.ari(assign, sub_truth)
            runs[name] = assign
            arm_fits[name] = fit
            arm_subs[name] = sub_truth
            print(f"  {name:14} fit ARI {fit:.3f}")
        print("  cross-embedder ARI:")
        import itertools
        cross = {}
        for (n1, _), (n2, _) in itertools.combinations(e88.SETS, 2):
            s = e88.ari(runs[n1], runs[n2])
            cross[f"{n1} vs {n2}"] = round(s, 3)
            print(f"    {n1:14} vs {n2:14} {s:.3f}")
        assignments[arm_name] = runs
        fits[arm_name] = {n: round(f, 3) for n, f in arm_fits.items()}
        subs[arm_name] = arm_subs
        table[arm_name] = {**fits[arm_name], "cross": cross}

    print("")
    print("=== VERDICT — preregistered: fit rises most where anisotropy is highest")
    for n in ["bge namehints", "bge nameonly", "nomic-embed"]:
        c = fits["cosine (E88)"][n]
        r = fits["rank (E90)"][n]
        print(f"  {n:14} cosine {c:.3f} → rank {r:.3f}  (delta {r - c:+.3f})")
    json.dump(table, open(os.path.join(HERE, "e90_results.json"), "w"), indent=1)
    print("")
    print("wrote e90_results.json")


if __name__ == "__main__":
    main()
