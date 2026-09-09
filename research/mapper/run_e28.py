#!/usr/bin/env python3
"""E28 — the split proposer: each axis read from the representation that suits it.

Assembles what E24-E27 measured, and tests it end to end on the thing that ships:
top-k containment of the rater's cell over 57 cells.

  DOMAIN  from the full entry at alpha = 0.5   (E24: hints halved beats shipped)
  MODE    from the HEAD WORD alone             (E25 mode lives in the head;
                                                E27 head-only halves mode's
                                                incoherence at no cost to domain)
  CELL    scored as the product of the two per-axis posteriors, restricted to
          cells actually seen in training.

BASELINE is E21's proposer: one representation, both axes, whole cell at once.

The axes are NOT merged and mode is NOT coarsened — E27 showed merging makes the
axis less of an object, and the kappa gain that argues for it is the free kind.
"""
import json, math, os, random, statistics, sys
from collections import Counter, defaultdict
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run_e21 as E


def unit(a):
    a = np.asarray(a, dtype=float)
    return a / (np.linalg.norm(a) + 1e-12)


def posterior(V, labels, X, temp=0.05):
    """softmax over cosine to class centroids -> comparable scores across spaces"""
    cs = sorted(set(labels))
    M = np.array([V[[i for i, l in enumerate(labels) if l == c]].mean(0) for c in cs])
    M /= (np.linalg.norm(M, axis=1, keepdims=True) + 1e-12)
    S = (X @ M.T) / temp
    S -= S.max(1, keepdims=True)
    P = np.exp(S)
    P /= P.sum(1, keepdims=True)
    return cs, P


def main():
    corpus = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    by = {e["name"]: e for e in corpus}
    W = json.load(open(os.path.join(HERE, "e25_word_vecs.json")))
    rows = E.load()

    # domain space: BGE, hints halved (E24)
    Vd = np.array([unit([n_ + 0.5 * (h - n_)
                         for n_, h in zip(by[r["name"]]["vec_name"], by[r["name"]]["vec"])])
                   for r in rows])
    # mode space: nomic head word; single-word entries fall back to the whole name
    def headvec(name):
        w = name.split()
        return unit(W[w[-1]]) if len(w) >= 2 and w[-1] in W else unit(W.get(name, W[w[-1]]))
    Vm = np.array([headvec(r["name"]) for r in rows])
    # baseline space: the shipped vector, as E21 used it
    Vb = np.array([unit(by[r["name"]]["vec"]) for r in rows])

    cell = [r["new"] for r in rows]
    dom = [c.split("/")[0] for c in cell]
    mod = [c.split("/")[1] for c in cell]
    n_single = sum(1 for r in rows if len(r["name"].split()) < 2)
    print(f"E28 — split proposer.  {len(rows)} entries, {len(set(cell))} cells, "
          f"{n_single} single-word (mode falls back to full name)\n")

    res = defaultdict(lambda: {1: [], 3: [], 5: []})
    for s in range(24):
        rng = random.Random(12000 + s)
        idx = list(range(len(rows)))
        rng.shuffle(idx)
        n = int(0.75 * len(rows))
        tr, te = idx[:n], idx[n:]
        seen = sorted({cell[i] for i in tr})

        # --- baseline: one space, cells directly
        cs, P = posterior(Vb[tr], [cell[i] for i in tr], Vb[te])
        order_b = [[cs[j] for j in np.argsort(-p)] for p in P]

        # --- split: per-axis posteriors, multiplied over seen cells
        dcs, Pd = posterior(Vd[tr], [dom[i] for i in tr], Vd[te])
        mcs, Pm = posterior(Vm[tr], [mod[i] for i in tr], Vm[te])
        di = {c: k for k, c in enumerate(dcs)}
        mi = {c: k for k, c in enumerate(mcs)}
        order_s = []
        for a in range(len(te)):
            sc = [(Pd[a][di[c.split('/')[0]]] * Pm[a][mi[c.split('/')[1]]], c)
                  for c in seen if c.split('/')[0] in di and c.split('/')[1] in mi]
            sc.sort(key=lambda t: -t[0])
            order_s.append([c for _, c in sc])

        for tag, order in (("baseline (E21)", order_b), ("split (E28)", order_s)):
            for k in (1, 3, 5):
                res[tag][k].append(
                    sum(cell[i] in o[:k] for i, o in zip(te, order)) / len(te))

    print(f"  {'proposer':18s} {'top-1':>8s} {'top-3':>8s} {'top-5':>8s}")
    for tag in ("baseline (E21)", "split (E28)"):
        r = res[tag]
        print(f"  {tag:18s} " + "".join(f"{statistics.mean(r[k]):8.3f}" for k in (1, 3, 5)))
    b, sp = res["baseline (E21)"], res["split (E28)"]
    print(f"  {'delta':18s} " +
          "".join(f"{statistics.mean(sp[k])-statistics.mean(b[k]):+8.3f}" for k in (1, 3, 5)))
    d3 = [x - y for x, y in zip(sp[3], b[3])]
    t = statistics.mean(d3) / (statistics.stdev(d3) / len(d3) ** 0.5)
    print(f"\n  paired over 24 identical splits: top-3 delta t = {t:+.2f}, "
          f"better in {sum(1 for x in d3 if x>0)}/24")


if __name__ == "__main__":
    main()
