#!/usr/bin/env python3
"""E29 — what the blur actually contains. Material for rewriting four definitions.

E22 localised the mode axis's failure: WORK, CREATE, MAINTAIN and SENSE hold 68%
of the corpus with ample population and do not separate. E27 closed off merging;
E28 closed off head-only representation and factorisation. What is left is
definitional, and that is a human call.

This does the part a machine can do: show what those entries GROUP INTO when
nothing is imposed on them, so the rewrite starts from the corpus rather than
from the intuition that produced the current four.

  - k-means in the alpha=0.5 space (E24/E28's shipping representation)
  - for each cluster: how it cuts across the current four labels, its most
    characteristic head words, and exemplars
  - and the honest check: is the discovered partition MORE COHERENT than the
    current four, by E26's compression criterion, held out?

It proposes. It does not rename anything.
"""
import json, os, random, statistics, sys
from collections import Counter, defaultdict
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run_e21 as E
from run_e26 import basis, nearest_centroid

BLUR = {"WORK", "CREATE", "MAINTAIN", "SENSE"}


def kmeans(X, k, seed=0, iters=60):
    rng = np.random.default_rng(seed)
    C = X[rng.choice(len(X), k, replace=False)].copy()
    for _ in range(iters):
        a = (X @ C.T).argmax(1)
        for j in range(k):
            m = X[a == j]
            if len(m):
                C[j] = m.mean(0) / (np.linalg.norm(m.mean(0)) + 1e-12)
    return (X @ C.T).argmax(1)


def compression(V, labels, seeds=16):
    out = []
    for s in range(seeds):
        rng = random.Random(13000 + s)
        idx = list(range(len(labels)))
        rng.shuffle(idx)
        n = int(0.75 * len(labels))
        tr, te = idx[:n], idx[n:]
        ytr = [labels[i] for i in tr]
        yte = [labels[i] for i in te]
        if len(set(ytr)) < 2:
            continue
        B = basis(V[tr], ytr)
        out.append(nearest_centroid(V[tr] @ B, ytr, V[te] @ B, yte)
                   - nearest_centroid(V[tr], ytr, V[te], yte))
    return statistics.mean(out)


def main():
    corpus = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    by = {e["name"]: e for e in corpus}
    rows = [r for r in E.load() if r["new"].split("/")[1] in BLUR]
    X = np.array([[n_ + 0.5 * (h - n_) for n_, h in
                   zip(by[r["name"]]["vec_name"], by[r["name"]]["vec"])] for r in rows])
    X /= (np.linalg.norm(X, axis=1, keepdims=True) + 1e-12)
    cur = [r["new"].split("/")[1] for r in rows]
    print(f"E29 — inside the blur.  {len(rows)} entries in {sorted(BLUR)}")
    print(f"  current four, held-out compression: {compression(X, cur):+.3f}")
    for k in (4, 5, 6):
        lab = [f"C{j}" for j in kmeans(X, k, seed=0)]
        print(f"  discovered k={k},               compression: {compression(X, lab):+.3f}")

    K = 5
    a = kmeans(X, K, seed=0)
    print(f"\n{'='*74}\nDISCOVERED GROUPS AT k={K} — proposals, not names\n")
    for j in range(K):
        mem = [i for i in range(len(rows)) if a[i] == j]
        if not mem:
            continue
        mix = Counter(cur[i] for i in mem)
        heads = Counter(rows[i]["name"].split()[-1] for i in mem)
        cent = X[mem].mean(0)
        cent /= np.linalg.norm(cent) + 1e-12
        near = sorted(mem, key=lambda i: -float(X[i] @ cent))[:6]
        print(f"  GROUP {j}  n={len(mem)}")
        print(f"    cuts across : " + ", ".join(f"{c} {n}" for c, n in mix.most_common()))
        print(f"    head words  : " + ", ".join(f"{h}({n})" for h, n in heads.most_common(6)))
        print(f"    closest to its own centre:")
        for i in near:
            print(f"        {rows[i]['name']:38s}  (currently {cur[i]})")
        print()
    print("""HOW TO USE THIS
  A group that cuts cleanly across the current four is a distinction the corpus
  makes that the grid does not. A group that matches one existing label is that
  label's real extent. Neither is a name — naming is the human half, and E26's
  compression figure above says whether a proposed carve is an object at all.""")


if __name__ == "__main__":
    main()
