#!/usr/bin/env python3
"""E27 — can the mode axis be made coherent? A merge search with a real stop rule.

E26 gave a criterion that is not a matter of taste. An axis is a COHERENT OBJECT
when it compresses into the span of its own class centroids without loss:
domain does (4-d, -0.003), mode does not (13-d, -0.034).

So: greedily merge the most-confused mode pair, and at every k measure

  KAPPA        chance-corrected, because merging raises raw accuracy for free
               and house rule 2 exists for this reason
  COMPRESSION  own (k-1)-dim subspace vs the full 768-d vector. THE STOP RULE.
               Near zero means the partition is a linear object.

Merging is driven by the held-out confusion matrix, never by the compression
score it is being judged on, so the criterion stays independent of the search.

WHAT WOULD FALSIFY THE WHOLE IDEA: if compression loss stays large at every k,
mode is not a linear structure at any granularity and no merge fixes it. That is
a real possible outcome and it is the one worth knowing.
"""
import json, os, random, statistics, sys
from collections import Counter, defaultdict
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run_e21 as E
from run_e26 import basis, nearest_centroid


def kappa(pred, true):
    n = len(true)
    po = sum(p == t for p, t in zip(pred, true)) / n
    cp, ct = Counter(pred), Counter(true)
    pe = sum(cp[c] / n * ct[c] / n for c in set(cp) | set(ct))
    return (po - pe) / (1 - pe) if pe < 1 else 0.0


def evaluate(V, labels, seeds=24):
    """held-out kappa, confusion, and compression loss for one labelling"""
    ks, conf, comp = [], Counter(), []
    n_all = len(labels)
    for s in range(seeds):
        rng = random.Random(9000 + s)
        idx = list(range(n_all))
        rng.shuffle(idx)
        n = int(0.75 * n_all)
        tr, te = idx[:n], idx[n:]
        ytr = [labels[i] for i in tr]
        yte = [labels[i] for i in te]
        if len(set(ytr)) < 2:
            continue
        cs = sorted(set(ytr))
        M = np.array([V[[i for i in tr if labels[i] == c]].mean(0) for c in cs])
        M /= (np.linalg.norm(M, axis=1, keepdims=True) + 1e-12)
        pred = [cs[i] for i in (V[te] @ M.T).argmax(1)]
        ks.append(kappa(pred, yte))
        for p, t in zip(pred, yte):
            conf[(t, p)] += 1
        B = basis(V[tr], ytr)
        full = nearest_centroid(V[tr], ytr, V[te], yte)
        sub = nearest_centroid(V[tr] @ B, ytr, V[te] @ B, yte) if B.shape[1] else 0.0
        comp.append(sub - full)
    return statistics.mean(ks), conf, statistics.mean(comp)


def main():
    rows = E.load()
    V = np.array([r["v"] for r in rows], dtype=float)
    V /= (np.linalg.norm(V, axis=1, keepdims=True) + 1e-12)
    labels = [r["new"].split("/")[1] for r in rows]

    print("E27 — merge search on the mode axis")
    print("  stop rule: compression loss ~ 0 means the partition is a linear object")
    print("  (domain, for reference: 5 classes, compression -0.003)\n")
    print(f"  {'k':>3s} {'kappa':>7s} {'compress':>9s}   merged this step")

    groups = {m: {m} for m in set(labels)}
    history = []
    while len(groups) >= 2:
        cur = {}
        for g, members in groups.items():
            for m in members:
                cur[m] = g
        lab = [cur[m] for m in labels]
        kap, conf, comp = evaluate(V, lab)
        history.append((len(groups), kap, comp, sorted(groups)))
        print(f"  {len(groups):3d} {kap:+7.3f} {comp:+9.3f}", end="")
        if len(groups) == 2:
            print()
            break
        # merge the most mutually confused pair
        tot = Counter()
        for (t, p), c in conf.items():
            tot[t] += c
        best, bestscore = None, -1
        gs = sorted(groups)
        for i in range(len(gs)):
            for j in range(i + 1, len(gs)):
                a, b = gs[i], gs[j]
                sc = (conf[(a, b)] / max(1, tot[a])) + (conf[(b, a)] / max(1, tot[b]))
                if sc > bestscore:
                    bestscore, best = sc, (a, b)
        a, b = best
        print(f"   {a} + {b}")
        groups[a + "|" + b] = groups.pop(a) | groups.pop(b)

    print("\n  BEST BY EACH CRITERION")
    bk = max(history, key=lambda h: h[1])
    bc = min(history, key=lambda h: abs(h[2]))
    print(f"    highest kappa      k={bk[0]}  kappa {bk[1]:+.3f}  compression {bk[2]:+.3f}")
    print(f"    most coherent      k={bc[0]}  kappa {bc[1]:+.3f}  compression {bc[2]:+.3f}")
    print(f"    classes at k={bc[0]}:")
    for g in bc[3]:
        print(f"      {g}")


if __name__ == "__main__":
    main()
