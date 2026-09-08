#!/usr/bin/env python3
"""E2 required output H — error propagation and emendation.

Protocol, stated before running:

  GROUNDING   an anchor is (entry index -> cell string). Nothing else. The
              human never edits the map, only pins a point.
  CORRUPTION  m anchors are given a deliberately WRONG cell, sampled uniformly
              from the other occupied cells.
  EMENDATION  a correction replaces one corrupted anchor's label with the true
              one. That is the whole intervention -- the same operation as a
              grounding, applied to a point already pinned.

Three things are measured, and they are different:
  REPAIR       accuracy recovered on the held-out entries nearest the corrected
               anchor (its local region)
  COLLATERAL   accuracy change on held-out entries FAR from the corrected
               anchor -- a correction should not disturb unrelated structure
  EFFICIENCY   held-out entries fixed per correction
"""
import json, os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import (QwenDictionary, build_counts, ppmi, svd_reduce, l2,
                    predict, score)

HERE = os.path.dirname(os.path.abspath(__file__))


def run(rep, labels, classes, train_pool, test_idx, n_corrupt=40, n_fix=(0, 5, 10, 20, 40),
        reps=10, knn=5, seed=17, n_anchor=160):
    rng = np.random.default_rng(seed)
    truth = [labels[i] for i in test_idx]
    # region membership: which held-out entries are nearest to which anchor
    out = []
    for nf in n_fix:
        accs, locals_, fars = [], [], []
        for _ in range(reps):
            anchors = rng.choice(train_pool, size=n_anchor, replace=False)
            lab = [labels[i] for i in anchors]
            bad = rng.choice(len(anchors), size=min(n_corrupt, len(anchors)), replace=False)
            for b in bad:
                alt = [c for c in classes if c != lab[b]]
                lab[b] = alt[rng.integers(len(alt))]
            fixed = bad[:nf]
            for b in fixed:
                lab[b] = labels[anchors[b]]
            pred = predict(rep, anchors, lab, test_idx, k=knn)
            accs.append(score(pred, truth)[0])
            # local = test entries whose nearest anchor is one of the still-corrupt
            # ones; far = the rest. Measures whether repair stays local.
            S = rep[test_idx] @ rep[anchors].T
            nearest = np.argmax(S, axis=1)
            still_bad = set(bad[nf:].tolist())
            loc = [j for j, na in enumerate(nearest) if na in still_bad]
            far = [j for j, na in enumerate(nearest) if na not in still_bad]
            locals_.append(np.mean([pred[j] == truth[j] for j in loc]) if loc else np.nan)
            fars.append(np.mean([pred[j] == truth[j] for j in far]) if far else np.nan)
        out.append((nf, np.mean(accs), np.nanmean(locals_), np.nanmean(fars)))
    return out


def main():
    tok = sys.argv[1]
    corpus = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    labels = [f"{e['domain']}/{e['mode']}" for e in corpus]
    classes = sorted(set(labels))
    texts = [" ".join([e["name"]] + e["hints"]) for e in corpus]
    n = len(corpus)
    BGE = l2(np.array([e["vec"] for e in corpus], dtype=np.float32))

    from tokenizers import Tokenizer
    D = QwenDictionary(tok)
    tk = Tokenizer.from_file(tok)
    qt = [tk.encode(t).ids for t in texts]
    M, keep, _ = build_counts(qt, min_df=2)
    w = np.array([D.rank_weight(t) for t in keep], dtype=np.float32)
    P1 = ppmi(M)
    reps_ = {
        "C Qwen dict L0": l2(svd_reduce(l2(M * w), 256, 17)),
        "L1 Qwen + PPMI": l2(svd_reduce(l2((M * w) @ P1), 256, 17)),
        "E BGE-base": BGE,
    }
    rs = np.random.default_rng(17)
    order = rs.permutation(n)
    test_idx = np.array(sorted(order[:int(0.30 * n)]))
    train_pool = np.array(sorted(order[int(0.30 * n):]))

    print("E2 output H — emendation: do sparse corrections repair the map?\n")
    print("  160 anchors, 40 of them deliberately mislabelled, then corrected one")
    print("  by one. 10 replicates. 'local' = held-out entries whose nearest anchor")
    print("  is still corrupt; 'far' = everything else.\n")
    for name, rep in reps_.items():
        rows = run(rep, labels, classes, train_pool, test_idx)
        base = rows[0][1]
        print(f"  {name}")
        print(f"    {'corrections':>12}{'held-out acc':>14}{'local':>9}{'far':>9}"
              f"{'fixed/corr':>12}")
        for nf, acc, loc, far in rows:
            eff = ((acc - base) * len(test_idx) / nf) if nf else float("nan")
            e = f"{eff:>12.2f}" if nf else f"{'—':>12}"
            print(f"    {nf:>12}{acc:>14.3f}{loc:>9.3f}{far:>9.3f}{e}")
        print()


if __name__ == "__main__":
    main()
