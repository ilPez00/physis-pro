#!/usr/bin/env python3
"""E3 — does DENSITY fix what E2 diagnosed?

E2's verdict was MAPPER HYPOTHESIS NOT SUPPORTED, with a stated diagnosis:
658 short entries over 70 cells leaves L1/L2 too sparse to estimate and gives
label propagation almost no unlabelled substrate. That was an argument. This
measures it, by running the physis codebase through physis and reusing the
volume two ways:

  LEG A  relational density -- estimate PPMI over the union corpus (labelled
         entries + codebase chunks) instead of over 658 entries alone, then
         build the SAME 658 entry representations from it.
  LEG B  node density -- add the codebase chunks to the propagation graph as
         UNLABELLED vertices. Anchors are still drawn only from the labelled
         train pool; the test set is unchanged.

WHAT IS NOT DONE, AND WHY IT WOULD BE CIRCULAR
----------------------------------------------
physis's own cell assignments on the codebase are MACHINE-PRODUCED. They are
never evaluated against and never used as anchors. Scoring against them would
be scoring physis against itself. Evaluation stays on the 658 human-authored
cells with E2's identical held-out split and identical metrics.
"""
import argparse, json, os, sys
from collections import Counter
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import (QwenDictionary, build_counts, ppmi, svd_reduce, l2,
                    grounding_curve, propagation_curve, build_graph,
                    propagate_predict, score)

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "─" * 78); print(t); print("─" * 78)


def prop_curve_extended(rep_all, n_lab, labels, train_pool, test_idx, ks, classes,
                        reps=10, knn=10, alpha=0.9, seed=17):
    """Propagation over a graph that includes unlabelled codebase nodes.

    Anchors and evaluation indices address the labelled block only (the first
    n_lab rows); the extra rows are substrate the labels may diffuse through.
    """
    W = build_graph(rep_all, knn=knn)
    rng = np.random.default_rng(seed)
    truth = [labels[i] for i in test_idx]
    rows = []
    for k in ks:
        if k > len(train_pool):
            continue
        accs = []
        for _ in range(reps):
            anchors = rng.choice(train_pool, size=k, replace=False)
            al = [labels[i] for i in anchors]
            pred = propagate_predict(W, anchors, al, test_idx, classes, alpha=alpha)
            accs.append(score(pred, truth)[0])
        rows.append((k, float(np.mean(accs))))
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tokenizer", required=True)
    ap.add_argument("--dim", type=int, default=256)
    ap.add_argument("--reps", type=int, default=10)
    ap.add_argument("--seed", type=int, default=17)
    a = ap.parse_args()

    lab = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    n = len(lab)
    labels = [f"{e['domain']}/{e['mode']}" for e in lab]
    texts = [" ".join([e["name"]] + e["hints"]) for e in lab]
    BGE = l2(np.array([e["vec"] for e in lab], dtype=np.float32))

    self_path = os.path.join(HERE, "selfcorpus.json")
    if not os.path.exists(self_path):
        sys.exit("selfcorpus.json missing — run mapper_selfcorpus_export first")
    sc = json.load(open(self_path))["chunks"]
    stexts = [c["text"] for c in sc]
    SV = np.fromfile(os.path.join(HERE, "selfcorpus_vecs.f32"), dtype=np.float32)
    SV = l2(SV.reshape(len(sc), -1))

    print("E3 — density, measured rather than argued\n")
    print(f"  labelled entries (evaluation) : {n}")
    print(f"  codebase chunks (substrate)   : {len(sc)}")
    print(f"  total text units              : {n + len(sc)}")
    mach = Counter(c["cell"] for c in sc)
    print(f"  physis's own cells on the codebase: {len(mach)} distinct, "
          f"top {mach.most_common(1)[0][0]} at {mach.most_common(1)[0][1]}")
    print( "  (machine-assigned; never evaluated against, never used as anchors)")

    from tokenizers import Tokenizer
    D = QwenDictionary(a.tokenizer)
    tk = Tokenizer.from_file(a.tokenizer)
    q_lab = [tk.encode(t).ids for t in texts]
    q_all = q_lab + [tk.encode(t).ids for t in stexts]

    hdr("LEG A — relational density: PPMI estimated on the union corpus")
    M_small, keep_s, _ = build_counts(q_lab, min_df=2)
    M_big_all, keep_b, _ = build_counts(q_all, min_df=2)
    M_big = M_big_all[:n]                      # the same 658 entries, big vocabulary
    print(f"  L0 vocabulary at df>=2 : {len(keep_s):,} (658 entries) -> "
          f"{len(keep_b):,} (union)   {len(keep_b)/max(len(keep_s),1):.1f}x")
    ws = np.array([D.rank_weight(t) for t in keep_s], dtype=np.float32)
    wb = np.array([D.rank_weight(t) for t in keep_b], dtype=np.float32)

    P1s = ppmi(M_small)
    P1b = ppmi(M_big_all)                      # estimated on EVERYTHING
    print(f"  L1 positive edges      : {int((P1s>0).sum()):,} -> {int((P1b>0).sum()):,}"
          f"   {int((P1b>0).sum())/max(int((P1s>0).sum()),1):.1f}x")
    dens_s = (P1s > 0).sum() / max(P1s.size, 1)
    dens_b = (P1b > 0).sum() / max(P1b.size, 1)
    print(f"  L1 matrix density      : {dens_s:.4f} -> {dens_b:.4f}")

    Rb = l2(P1b); P2b = np.maximum(Rb @ Rb.T, 0.0).astype(np.float32)
    np.fill_diagonal(P2b, 0.0)
    Rs = l2(P1s); P2s = np.maximum(Rs @ Rs.T, 0.0).astype(np.float32)
    np.fill_diagonal(P2s, 0.0)

    arms = {}
    arms["C  Qwen L0            (658-corpus)"] = l2(svd_reduce(l2(M_small*ws), a.dim, a.seed))
    arms["C  Qwen L0            (union)"]      = l2(svd_reduce(l2(M_big*wb), a.dim, a.seed))
    arms["L1 Qwen + PPMI        (658-corpus)"] = l2(svd_reduce(l2((M_small*ws)@P1s), a.dim, a.seed))
    arms["L1 Qwen + PPMI        (union)"]      = l2(svd_reduce(l2((M_big*wb)@P1b), a.dim, a.seed))
    arms["L2 rel-of-rel         (658-corpus)"] = l2(svd_reduce(l2((M_small*ws)@P1s@P2s), a.dim, a.seed))
    arms["L2 rel-of-rel         (union)"]      = l2(svd_reduce(l2((M_big*wb)@P1b@P2b), a.dim, a.seed))
    arms["E  BGE-base"]                        = BGE

    rs = np.random.default_rng(a.seed)
    order = rs.permutation(n)
    test_idx = np.array(sorted(order[:int(0.30*n)]))
    train_pool = np.array(sorted(order[int(0.30*n):]))
    floor = float(np.mean([labels[i] == Counter(labels).most_common(1)[0][0] for i in test_idx]))
    classes = sorted(set(labels))
    ks = [5, 10, 20, 40, 80, 160, 320, len(train_pool)]

    print(f"\n  held-out cell accuracy, cosine kNN (identical split to E2, floor {floor:.3f})\n")
    print(f"  {'arm':<38}" + "".join(f"{k:>7}" for k in ks))
    knn_res = {}
    for name, rep in arms.items():
        rows = grounding_curve(rep, labels, train_pool, test_idx, ks,
                               reps=a.reps, knn=5, seed=a.seed)
        knn_res[name] = rows
        print(f"  {name:<38}" + "".join(f"{r[1]:>7.3f}" for r in rows))

    hdr("LEG A verdict — did denser relations help?")
    print(f"  {'level':<24}{'658-corpus':>12}{'union':>10}{'delta':>9}")
    for lvl in ["C  Qwen L0", "L1 Qwen + PPMI", "L2 rel-of-rel"]:
        s_ = {r[0]: r[1] for r in knn_res[f"{lvl:<21} (658-corpus)"]}
        b_ = {r[0]: r[1] for r in knn_res[f"{lvl:<21} (union)"]}
        sv, bv = s_[max(s_)], b_[max(b_)]
        print(f"  {lvl:<24}{sv:>12.3f}{bv:>10.3f}{bv-sv:>+9.3f}")

    hdr("LEG B — node density: codebase chunks as unlabelled graph substrate")
    print("  Anchors are drawn only from the labelled train pool. The extra")
    print("  vertices are substrate the labels may diffuse through.\n")
    print(f"  {'arm':<38}" + "".join(f"{k:>7}" for k in ks))
    for name, rep_lab, rep_extra in [
        ("BGE, 658 nodes only",      BGE, None),
        ("BGE + codebase nodes",     BGE, SV),
    ]:
        if rep_extra is None:
            rows = propagation_curve(rep_lab, labels, train_pool, test_idx, ks, classes,
                                     reps=a.reps, knn=10, seed=a.seed)
            rows = [(r[0], r[1]) for r in rows]
        else:
            rep_all = np.vstack([rep_lab, rep_extra]).astype(np.float32)
            rows = prop_curve_extended(rep_all, n, labels, train_pool, test_idx, ks,
                                       classes, reps=a.reps, knn=10, seed=a.seed)
        print(f"  {name:<38}" + "".join(f"{v:>7.3f}" for _, v in rows))

    qb = arms["L1 Qwen + PPMI        (union)"]
    # union-vocabulary Qwen representation for the codebase chunks, so the
    # extra nodes live in the same space as the labelled ones
    M_chunks = M_big_all[n:]
    q_extra = l2(svd_reduce(l2((M_chunks*wb) @ P1b), a.dim, a.seed))
    for name, rep_lab, rep_extra in [
        ("L1 Qwen union, 658 nodes only", qb, None),
        ("L1 Qwen union + codebase nodes", qb, q_extra),
    ]:
        if rep_extra is None:
            rows = propagation_curve(rep_lab, labels, train_pool, test_idx, ks, classes,
                                     reps=a.reps, knn=10, seed=a.seed)
            rows = [(r[0], r[1]) for r in rows]
        else:
            rep_all = np.vstack([rep_lab, rep_extra]).astype(np.float32)
            rows = prop_curve_extended(rep_all, n, labels, train_pool, test_idx, ks,
                                       classes, reps=a.reps, knn=10, seed=a.seed)
        print(f"  {name:<38}" + "".join(f"{v:>7.3f}" for _, v in rows))
    print(f"\n  {'majority-class floor':<38}{floor:>7.3f}")

    json.dump({"floor": floor, "n_labelled": n, "n_chunks": len(sc),
               "legA": {k: [list(map(float, r)) for r in v] for k, v in knn_res.items()}},
              open(os.path.join(HERE, "e3_curves.json"), "w"), indent=1)
    print(f"\nwrote {os.path.join(HERE,'e3_curves.json')}")


if __name__ == "__main__":
    main()
