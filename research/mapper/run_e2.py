#!/usr/bin/env python3
"""E2 driver — builds every arm, runs the ladder, prints the report."""
import argparse
import json
import os
import sys
from collections import Counter
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import (QwenDictionary, build_counts, ppmi, svd_reduce, l2,
                    grounding_curve, predict, score, build_graph,
                    propagate_predict, propagation_curve)

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "─" * 78)
    print(t)
    print("─" * 78)


def char_ngrams(s, lo=3, hi=5):
    s = " " + s.lower() + " "
    return [s[i:i + k] for k in range(lo, hi + 1) for i in range(len(s) - k + 1)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tokenizer", required=True)
    ap.add_argument("--dim", type=int, default=256)
    ap.add_argument("--reps", type=int, default=12)
    ap.add_argument("--knn", type=int, default=5)
    ap.add_argument("--seed", type=int, default=17)
    a = ap.parse_args()

    corpus = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    n = len(corpus)
    labels = [f"{e['domain']}/{e['mode']}" for e in corpus]
    texts = [" ".join([e["name"]] + e["hints"]) for e in corpus]
    names = [e["name"] for e in corpus]
    BGE = l2(np.array([e["vec"] for e in corpus], dtype=np.float32))
    BGE_NAME = l2(np.array([e["vec_name"] for e in corpus], dtype=np.float32))

    print("E2 — the mapper hypothesis")
    cc = Counter(labels)
    maj = cc.most_common(1)[0]
    print(f"\ncorpus: {n} entries, {len(cc)} occupied cells of 70")
    print(f"majority cell {maj[0]} at {maj[1]} ({maj[1]/n:.3f}) — the trivial floor")

    hdr("A. The Qwen3-8B n-gram representation dictionary")
    D = QwenDictionary(a.tokenizer)
    print(f"  vocabulary       : {len(D.vocab):,} byte-level BPE tokens")
    print(f"  merge rules      : {D.n_merges:,}  (the derivation DAG)")
    print(f"  byte roots       : {len(D.vocab)-D.n_merges:,} tokens with no parents")
    print(f"  alphabet / unit  : bytes / one BPE subword")
    print(f"  stored per entry : token id, surface, merge rank, merge parents")
    print(f"  lookup returns   : token ids + their merge-DAG ancestors")
    print( "  NOTE: tokenizer only — no weights loaded, no inference run.")

    from tokenizers import Tokenizer
    tk = Tokenizer.from_file(a.tokenizer)
    qtoks = [tk.encode(t).ids for t in texts]
    qtoks_name = [tk.encode(t).ids for t in names]
    tl = [len(x) for x in qtoks]
    print(f"\n  corpus -> {sum(tl):,} tokens, {len(set(t for x in qtoks for t in x)):,} "
          f"distinct, mean {np.mean(tl):.1f}/entry")

    hdr("B. The map — each level, its definition and its pruning criterion")
    M_q, keep_q, idx_q = build_counts(qtoks, min_df=2)
    print(f"  L0 nodes : {len(keep_q):,} Qwen tokens at df>=2")
    print( "             (pruned because co-occurrence is undefined for a token")
    print( "              seen in a single entry)")
    M_c, keep_c, _ = build_counts([char_ngrams(t) for t in texts], min_df=2)
    print(f"  ordinary n-gram control: {len(keep_c):,} char 3-5 grams, no Qwen")

    w = np.array([D.rank_weight(t) for t in keep_q], dtype=np.float32)
    M_anc = np.zeros_like(M_q)
    for i, toks in enumerate(qtoks):
        for t in toks:
            for aid in D.ancestors(t, max_depth=4):
                j = idx_q.get(aid)
                if j is not None:
                    M_anc[i, j] += 0.5
    print(f"  merge-DAG ancestors: {int((M_anc>0).sum()):,} (entry, ancestor) incidences")

    P1 = ppmi(M_q)
    print(f"  L1 edges : {int((P1>0).sum()):,} positive-PPMI token pairs")
    R = l2(P1)
    P2 = np.maximum(R @ R.T, 0.0).astype(np.float32)
    np.fill_diagonal(P2, 0.0)
    print(f"  L2 edges : {int((P2>0.05).sum()):,} pairs with second-order sim > 0.05")

    arms = {}
    arms["A tokenizer-only (Qwen vocab, no merges)"] = l2(svd_reduce(l2(M_q), a.dim, a.seed))
    arms["B ordinary char n-gram (no Qwen)"]         = l2(svd_reduce(l2(M_c), a.dim, a.seed))
    arms["C Qwen dict: L0 rank-weighted"]            = l2(svd_reduce(l2(M_q*w), a.dim, a.seed))
    arms["C+ Qwen dict: L0 + merge-DAG ancestors"]   = l2(svd_reduce(l2(M_q*w + M_anc*w), a.dim, a.seed))
    arms["L1 Qwen dict + PPMI relations"]            = l2(svd_reduce(l2((M_q*w) @ P1), a.dim, a.seed))
    arms["L2 relations between relations"]           = l2(svd_reduce(l2((M_q*w) @ P1 @ P2), a.dim, a.seed))
    arms["E existing embedding (BGE-base)"]          = BGE
    rng = np.random.default_rng(a.seed)
    arms["CONTROL shuffled map"] = arms["C Qwen dict: L0 rank-weighted"][rng.permutation(n)]
    arms["CONTROL random gaussian"] = l2(rng.standard_normal((n, a.dim)).astype(np.float32))
    M_qn, keep_qn, _ = build_counts(qtoks_name, min_df=2)
    wn = np.array([D.rank_weight(t) for t in keep_qn], dtype=np.float32)
    arms["FORMAT CONTROL Qwen L0, names only"] = l2(svd_reduce(l2(M_qn*wn), a.dim, a.seed))
    arms["FORMAT CONTROL BGE, names only"] = BGE_NAME

    rs = np.random.default_rng(a.seed)
    order = rs.permutation(n)
    test_idx = np.array(sorted(order[:int(0.30*n)]))
    train_pool = np.array(sorted(order[int(0.30*n):]))
    floor = float(np.mean([labels[i] == maj[0] for i in test_idx]))
    classes = sorted(set(labels))
    ks = [1, 2, 5, 10, 20, 40, 80, 160, 320, len(train_pool)]

    hdr("C. Sparse grounding, propagation = cosine kNN to the anchors")
    print(f"  train pool {len(train_pool)} · held out {len(test_idx)} · "
          f"majority floor on the test half {floor:.3f}\n")
    print(f"  {'arm':<44}" + "".join(f"{k:>7}" for k in ks))
    knn_res = {}
    for name, rep in arms.items():
        rows = grounding_curve(rep, labels, train_pool, test_idx, ks,
                               reps=a.reps, knn=a.knn, seed=a.seed)
        knn_res[name] = rows
        print(f"  {name:<44}" + "".join(f"{r[1]:>7.3f}" for r in rows))

    hdr("D. Label propagation over the map graph — the hypothesis's own mechanism")
    print("  kNN-to-anchors ignores the map entirely. This pins the anchors and")
    print("  diffuses their labels through the UNLABELLED structure, which is")
    print("  'pinning data points' stated literally. Zhou normalized-Laplacian,")
    print("  alpha=0.9, 10-NN graph.\n")
    print(f"  {'arm':<44}" + "".join(f"{k:>7}" for k in ks))
    prop = {}
    for name, rep in arms.items():
        rows = propagation_curve(rep, labels, train_pool, test_idx, ks, classes,
                                 reps=a.reps, knn=10, seed=a.seed)
        prop[name] = rows
        print(f"  {name:<44}" + "".join(f"{r[1]:>7.3f}" for r in rows))
    print(f"\n  {'majority-class floor':<44}" + f"{floor:>7.3f}")

    hdr("E. Does the map's topology add anything over plain kNN?")
    print(f"  {'arm':<44}{'kNN':>8}{'propagation':>13}{'delta':>8}")
    for name in arms:
        k = {r[0]: r[1] for r in knn_res[name]}
        p = {r[0]: r[1] for r in prop[name]}
        kf, pf = k[max(k)], p[max(p)]
        print(f"  {name:<44}{kf:>8.3f}{pf:>13.3f}{pf-kf:>+8.3f}")

    hdr("F. THE MAPPER QUESTION — are sparse gains disproportionate?")
    print("  Fraction of the full-supervision gain captured by the first 20 anchors.")
    print("  The mapper hypothesis predicts this is LARGE. An ordinary learning")
    print("  curve predicts it tracks the label fraction.\n")
    print(f"  {'arm':<44}{'k=20':>8}{'k=full':>8}{'captured':>11}")
    for name, rows in prop.items():
        d = {r[0]: r[1] for r in rows}
        k20, kf = d.get(20), d[max(d)]
        if k20 is None or kf - floor <= 1e-9:
            print(f"  {name:<44}{(k20 or 0):>8.3f}{kf:>8.3f}{'n/a (at floor)':>11}")
            continue
        print(f"  {name:<44}{k20:>8.3f}{kf:>8.3f}{(k20-floor)/(kf-floor):>10.1%}")
    print(f"\n  proportional reference: 20 of {len(train_pool)} anchors = "
          f"{20/len(train_pool):.1%} of the labels")

    json.dump({"floor": floor,
               "knn": {k: [list(map(float, r)) for r in v] for k, v in knn_res.items()},
               "prop": {k: [list(map(float, r)) for r in v] for k, v in prop.items()}},
              open(os.path.join(HERE, "e2_curves.json"), "w"), indent=1)
    print(f"\nwrote {os.path.join(HERE,'e2_curves.json')}")


if __name__ == "__main__":
    main()
