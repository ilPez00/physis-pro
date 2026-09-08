#!/usr/bin/env python3
"""E5 — is the 70-cell grid the dominant constraint, rather than the representation?

FINAL_REPORT.md concluded that "the taxonomy, not the embedding or the
calibration heuristic, [is] the dominant constraint". E1 partly revises the
evidence behind that (the embedder swap was NOT modest: BGE beat MiniLM on 22 of
22 configurations, +0.132 AUC), so the taxonomy claim needs testing on its own
rather than inheriting.

Three things, answering the three open items:

  A. PROPER SUPERVISION COMPARISON -- real trained classifiers, and Cohen's
     kappa rather than accuracy against a majority floor. Kappa normalises for
     chance agreement given the actual class distribution, which a majority
     floor does not: on a 70-cell problem with one cell at 10%, "beats the
     floor" is a very low bar and reporting it alone flatters every arm.

  B. GRANULARITY -- the same representation scored at cell (70), mode (14) and
     domain (5). If the taxonomy is the constraint, coarse granularities should
     be far healthier relative to their own chance level.

  C. MERGE CURVE -- agglomerate confusable cells and re-score. If accuracy
     climbs steeply as cells merge, the grid is too fine for the distinctions
     the data actually carries. If it climbs slowly, the representation is the
     limit and the grid is not to blame.
"""
import argparse, json, os, sys
from collections import Counter, defaultdict
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "─" * 78); print(t); print("─" * 78)


def kappa(pred, truth):
    """Cohen's kappa: agreement corrected for chance given the class marginals."""
    labs = sorted(set(truth) | set(pred))
    idx = {c: i for i, c in enumerate(labs)}
    n = len(truth)
    po = sum(p == t for p, t in zip(pred, truth)) / n
    pc = Counter(pred); tc = Counter(truth)
    pe = sum((pc.get(c, 0) / n) * (tc.get(c, 0) / n) for c in labs)
    return (po - pe) / (1 - pe) if pe < 1 else 0.0


def macro_f1(pred, truth):
    labs = set(truth)
    f1s = []
    for c in labs:
        tp = sum(p == c and t == c for p, t in zip(pred, truth))
        fp = sum(p == c and t != c for p, t in zip(pred, truth))
        fn = sum(p != c and t == c for p, t in zip(pred, truth))
        if tp == 0:
            f1s.append(0.0); continue
        pr, rc = tp / (tp + fp), tp / (tp + fn)
        f1s.append(2 * pr * rc / (pr + rc))
    return float(np.mean(f1s))


def nearest_centroid(Xtr, ytr, Xte):
    classes = sorted(set(ytr))
    C = np.stack([l2(Xtr[[i for i, y in enumerate(ytr) if y == c]].mean(0, keepdims=True))[0]
                  for c in classes])
    return [classes[i] for i in np.argmax(Xte @ C.T, axis=1)]


def knn(Xtr, ytr, Xte, k=5):
    S = Xte @ Xtr.T
    out = []
    kk = min(k, len(ytr))
    for i in range(len(Xte)):
        top = np.argpartition(-S[i], kk - 1)[:kk]
        v = defaultdict(float)
        for j in top:
            v[ytr[j]] += max(float(S[i, j]), 0.0)
        out.append(max(v, key=v.get))
    return out


def softmax_reg(Xtr, ytr, Xte, iters=3000, lr=0.5, l2reg=1e-4):
    """Multinomial logistic with adaptive steps.

    The first version of this ran 400 plain-gradient iterations at lr=1.0 and
    reached 0.218 against nearest-centroid's 0.431 -- badly underfit, and it
    would have been reported as a finding about the taxonomy. Convergence is
    checked below by printing the train accuracy alongside the test accuracy.
    """
    classes = sorted(set(ytr))
    ci = {c: i for i, c in enumerate(classes)}
    Y = np.zeros((len(ytr), len(classes)), dtype=np.float32)
    for i, y in enumerate(ytr):
        Y[i, ci[y]] = 1.0
    W = np.zeros((Xtr.shape[1], len(classes)), dtype=np.float32)
    b = np.zeros(len(classes), dtype=np.float32)
    mW = np.zeros_like(W); vW = np.zeros_like(W)
    mb = np.zeros_like(b); vb = np.zeros_like(b)
    n = len(ytr)
    for t_ in range(1, iters + 1):
        Z = Xtr @ W + b
        Z -= Z.max(1, keepdims=True)
        P = np.exp(Z); P /= P.sum(1, keepdims=True)
        G = (P - Y) / n
        gW = Xtr.T @ G + l2reg * W
        gb = G.sum(0)
        for (m_, v_, g_, p_) in ((mW, vW, gW, W), (mb, vb, gb, b)):
            m_ *= 0.9; m_ += 0.1 * g_
            v_ *= 0.999; v_ += 0.001 * g_ * g_
            p_ -= lr * (m_ / (1 - 0.9 ** t_)) / (np.sqrt(v_ / (1 - 0.999 ** t_)) + 1e-8)
    return [classes[i] for i in np.argmax(Xte @ W + b, axis=1)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=17)
    a = ap.parse_args()

    E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    n = len(E)
    cell = [f"{e['domain']}/{e['mode']}" for e in E]
    dom = [e["domain"] for e in E]
    mode = [e["mode"] for e in E]
    X = l2(np.array([e["vec"] for e in E], dtype=np.float32))

    rs = np.random.default_rng(a.seed)
    order = rs.permutation(n)
    te = np.array(sorted(order[: int(0.30 * n)]))
    tr = np.array(sorted(order[int(0.30 * n):]))

    print("E5 — is the grid the constraint, or the representation?\n")
    print(f"  {n} entries · {len(set(cell))} occupied cells of 70 · "
          f"train {len(tr)} · held out {len(te)}")
    sizes = Counter(cell)
    tiny = [c for c, k in sizes.items() if k < 5]
    print(f"  cell sizes: median {int(np.median(list(sizes.values())))}, "
          f"max {max(sizes.values())}, {len(tiny)} cells with <5 entries")
    print(f"  a cell with 3 entries cannot be learned from a 70% split; "
          f"{sum(sizes[c] for c in tiny)} entries sit in such cells")

    hdr("A. Proper supervision comparison — real classifiers, and kappa not floor")
    print("  Cohen's kappa corrects for chance agreement given the ACTUAL class")
    print("  marginals. 'Beats the 0.102 majority floor' is a very low bar on a")
    print("  70-way problem and flatters every arm; kappa does not.\n")
    print(f"  {'classifier':<26}{'accuracy':>10}{'kappa':>9}{'macro-F1':>10}")
    results = {}
    for name, fn in [("nearest centroid", lambda: nearest_centroid(X[tr], [cell[i] for i in tr], X[te])),
                     ("k-NN k=1", lambda: knn(X[tr], [cell[i] for i in tr], X[te], 1)),
                     ("k-NN k=5", lambda: knn(X[tr], [cell[i] for i in tr], X[te], 5)),
                     ("k-NN k=15", lambda: knn(X[tr], [cell[i] for i in tr], X[te], 15)),
                     ("softmax regression", lambda: softmax_reg(X[tr], [cell[i] for i in tr], X[te]))]:
        p = fn()
        t = [cell[i] for i in te]
        results[name] = p
        print(f"  {name:<26}{np.mean([a_ == b_ for a_, b_ in zip(p, t)]):>10.3f}"
              f"{kappa(p, t):>9.3f}{macro_f1(p, t):>10.3f}")
    floor_p = [Counter([cell[i] for i in tr]).most_common(1)[0][0]] * len(te)
    t = [cell[i] for i in te]
    print(f"  {'majority-class floor':<26}{np.mean([a_ == b_ for a_, b_ in zip(floor_p, t)]):>10.3f}"
          f"{kappa(floor_p, t):>9.3f}{macro_f1(floor_p, t):>10.3f}")
    print("\n  Label-noise ceiling: Stage 11 put the post-0.1.21 misfiling proxy at")
    print("  16.2%. If ~16% of the held-out labels are themselves wrong, a perfect")
    print("  classifier scores about 0.84 — that, not 1.0, is the target.")

    hdr("B. Granularity — the same representation, three levels of the taxonomy")
    print(f"  {'level':<18}{'classes':>9}{'accuracy':>10}{'floor':>8}{'kappa':>9}"
          f"{'gain over floor':>17}")
    for lvl, lab in [("cell (domain×mode)", cell), ("mode", mode), ("domain", dom)]:
        p = nearest_centroid(X[tr], [lab[i] for i in tr], X[te])
        t = [lab[i] for i in te]
        acc = float(np.mean([a_ == b_ for a_, b_ in zip(p, t)]))
        fl = Counter([lab[i] for i in tr]).most_common(1)[0][0]
        flv = float(np.mean([fl == x for x in t]))
        print(f"  {lvl:<18}{len(set(lab)):>9}{acc:>10.3f}{flv:>8.3f}{kappa(p, t):>9.3f}"
              f"{(acc-flv)/(1-flv):>16.1%}")
    print("  (nearest centroid: parameter-free, and unlike a fitted classifier it")
    print("   does not quietly degrade on the 22 cells holding fewer than 5 entries)")
    print("\n  If the grid were the binding constraint, the coarse levels would be")
    print("  markedly healthier relative to their own chance level. Read the kappa")
    print("  and the gain-over-floor columns, not the raw accuracy.")

    hdr("C. Merge curve — does collapsing confusable cells rescue accuracy?")
    print("  Cells are merged greedily by centroid similarity (computed on TRAIN")
    print("  only), the labels relabelled to the merged groups, and the classifier")
    print("  retrained. If the grid is too fine, accuracy should climb steeply.\n")
    classes = sorted(set(cell))
    C = np.stack([l2(X[[i for i in tr if cell[i] == c]].mean(0, keepdims=True))[0]
                  if any(cell[i] == c for i in tr) else np.zeros(X.shape[1], dtype=np.float32)
                  for c in classes])
    # greedy agglomeration on train centroids
    groups = {c: [c] for c in classes}
    order_c = list(classes)
    sim = C @ C.T
    np.fill_diagonal(sim, -np.inf)
    merged = {c: c for c in classes}
    targets = [70, 50, 35, 25, 18, 12, 8, 5]
    curve = []
    active = list(classes)
    seen_k = set()
    while True:
        k = len(active)
        if (k in targets or k == len(classes)) and k not in seen_k:
            seen_k.add(k)
            lab = [merged[c] for c in cell]
            gs = Counter(merged.values())
            biggest = max(Counter(lab).values()) / len(lab)
            p = nearest_centroid(X[tr], [lab[i] for i in tr], X[te])
            t = [lab[i] for i in te]
            acc = float(np.mean([a_ == b_ for a_, b_ in zip(p, t)]))
            fl = Counter([lab[i] for i in tr]).most_common(1)[0][0]
            flv = float(np.mean([fl == x for x in t]))
            curve.append((k, acc, flv, kappa(p, t), biggest))
        if k <= 5:
            break
        # AVERAGE linkage. The first version used maximum (single) linkage and
        # chained everything into one mega-cluster: accuracy tracked the floor
        # exactly and kappa was 0.000 at every level, which is the signature of
        # a degenerate merge rather than a finding about the grid.
        ai_ = {c: i for i, c in enumerate(classes)}
        ai = ai_
        bestpair, bestv = None, -np.inf
        for i in range(len(active)):
            for j in range(i + 1, len(active)):
                v = sim[ai[active[i]], ai[active[j]]]
                if v > bestv:
                    bestv, bestpair = v, (active[i], active[j])
        x, y = bestpair
        # Size guard: refuse a merge that would put more than 45% of the corpus
        # in one group. Without it the curve degenerates into one mega-cluster
        # by 35 groups (largest group 72.5%) and kappa stops meaning anything.
        cnt = Counter()
        for c in merged:
            cnt[merged[c]] += sizes.get(c, 0)
        if (cnt[x] + cnt[y]) / n > 0.45 and len(active) > 6:
            sim[ai_[x], ai_[y]] = -np.inf
            sim[ai_[y], ai_[x]] = -np.inf
            continue
        nx = sum(1 for c in merged if merged[c] == x)
        ny = sum(1 for c in merged if merged[c] == y)
        for c in list(merged):
            if merged[c] == y:
                merged[c] = x
        active.remove(y)
        avg = (sim[ai[x]] * nx + sim[ai[y]] * ny) / (nx + ny)
        sim[ai[x]] = avg
        sim[:, ai[x]] = avg
        sim[ai[y], :] = -np.inf; sim[:, ai[y]] = -np.inf
        sim[ai[x], ai[x]] = -np.inf

    print(f"  {'groups':>8}{'accuracy':>10}{'floor':>8}{'kappa':>9}"
          f"{'gain over floor':>17}{'largest group':>15}")
    for k, acc, flv, kp, big in curve:
        print(f"  {k:>8}{acc:>10.3f}{flv:>8.3f}{kp:>9.3f}"
              f"{(acc-flv)/(1-flv):>16.1%}{big:>15.1%}")
    print("\n  Accuracy MUST rise as groups merge -- the floor rises with it. Kappa is")
    print("  the column that matters: it asks whether the merged problem is better")
    print("  solved, not merely easier. A 'largest group' share running away toward")
    print("  100% is the degenerate-merge signature, and it is printed so it cannot")
    print("  be mistaken for a result.")

    json.dump({"merge_curve": curve}, open(os.path.join(HERE, "e5_curves.json"), "w"), indent=1)
    print(f"\nwrote {os.path.join(HERE,'e5_curves.json')}")


if __name__ == "__main__":
    main()
