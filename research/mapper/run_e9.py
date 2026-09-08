#!/usr/bin/env python3
"""E9 — is there a discrete SIGNIFIER structure worth formalising?

The proposal: stop deciding cell membership by geometry (cosine to a centroid --
continuous, unstable, and E8 showed it sits +0.003 above random for the largest
cells) and give each entry a VALUE instead. Membership becomes a decidable
predicate on that value. The value is a signifier, not the thing. Tokens are the
obvious alphabet.

That is a real proposal and Lean would earn its keep on it -- consistency of an
assignment is machine-checkable, propagation becomes inference with provable
properties. But formalising noise is worse than not formalising, so the prior
question is empirical:

  DOES A DISCRETE ADDRESS CARRY THE ONTOLOGY BETTER THAN GEOMETRY DOES?

Three address schemes, all discrete, all decidable:

  DYADIC   recursive bisection of the representation space. Each entry gets a
           bit-path; membership at any level is a PREFIX TEST. Self-similar by
           construction, which is what "fractal" would mean operationally.
  TOKEN    the entry's rarest Qwen tokens by merge rank, used directly as the
           code. No geometry anywhere -- signifiers straight from the text.
  RANDOM   the control. Any scheme must beat a random address of equal size.

And the "fractal" guess makes a falsifiable prediction: a self-similar structure
yields roughly CONSTANT information per level. If the information saturates after
two or three bits, the structure is shallow and a deep address is decoration.
"""
import json, os, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2, QwenDictionary

HERE = os.path.dirname(os.path.abspath(__file__))
TOK = sys.argv[1] if len(sys.argv) > 1 else None


def hdr(t):
    print("\n" + "═" * 78); print(t); print("═" * 78)


def mutual_info(codes, labels):
    """I(code ; label) in bits, and normalised by H(label)."""
    n = len(labels)
    jc = Counter(zip(codes, labels)); cc = Counter(codes); lc = Counter(labels)
    mi = 0.0
    for (c, l), k in jc.items():
        pxy = k / n
        mi += pxy * np.log2(pxy / ((cc[c] / n) * (lc[l] / n)))
    hl = -sum((v / n) * np.log2(v / n) for v in lc.values())
    return mi, (mi / hl if hl > 0 else 0.0)


def heldout_predict(codes, labels, tr, te):
    """Does an address predict the cell of an entry it has NEVER SEEN?

    In-sample MI is worthless here: an address with one entry per group scores
    a perfect 1.0 by memorising. This assigns each address group its TRAIN
    majority cell and scores held-out entries, so a code that names a single
    entry generalises to nothing and is measured as such.
    """
    maj = {}
    g = defaultdict(list)
    for i in tr:
        g[codes[i]].append(labels[i])
    for c, v in g.items():
        maj[c] = Counter(v).most_common(1)[0][0]
    hit = miss = unseen = 0
    for i in te:
        c = codes[i]
        if c not in maj:
            unseen += 1
        elif maj[c] == labels[i]:
            hit += 1
        else:
            miss += 1
    return hit / len(te), unseen / len(te)


def purity(codes, labels):
    """Fraction of entries whose code-group is cell-pure (the decidable win)."""
    g = defaultdict(list)
    for c, l in zip(codes, labels):
        g[c].append(l)
    pure = sum(len(v) for v in g.values() if len(set(v)) == 1)
    return pure / len(labels), len(g)


E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
cell = [f"{e['domain']}/{e['mode']}" for e in E]
mode = [e["mode"] for e in E]
texts = [" ".join([e["name"]] + e["hints"]) for e in E]
rng = np.random.default_rng(17)

print("E9 — is there a discrete signifier structure worth formalising?\n")
hl = -sum((v / n) * np.log2(v / n) for v in Counter(cell).values())
hm = -sum((v / n) * np.log2(v / n) for v in Counter(mode).values())
print(f"  {n} entries · H(cell) = {hl:.2f} bits · H(mode) = {hm:.2f} bits")
print(f"  a perfect address would recover all of it; geometry (BGE k-NN) reaches")
print(f"  0.447 cell accuracy, which is the standard to beat.")

# ── DYADIC: recursive bisection, a prefix code by construction ───────────
hdr("A. Dyadic address — recursive bisection, membership is a prefix test")


def dyadic(Xs, depth, seed=0):
    """Bit-path per entry. Each split is the top principal direction of the
    node's own members, so the partition is self-similar in construction."""
    codes = [""] * len(Xs)
    nodes = [list(range(len(Xs)))]
    for d in range(depth):
        nxt = []
        for grp in nodes:
            if len(grp) < 4:
                for i in grp:
                    codes[i] += "0"
                nxt.append(grp)
                continue
            M = Xs[grp] - Xs[grp].mean(0)
            # power iteration for the top singular direction: deterministic
            v = np.random.default_rng(seed + d).standard_normal(M.shape[1])
            for _ in range(24):
                v = M.T @ (M @ v)
                v /= np.linalg.norm(v) + 1e-12
            proj = M @ v
            med = np.median(proj)
            a = [grp[i] for i in range(len(grp)) if proj[i] <= med]
            b = [grp[i] for i in range(len(grp)) if proj[i] > med]
            for i in a:
                codes[i] += "0"
            for i in b:
                codes[i] += "1"
            if a:
                nxt.append(a)
            if b:
                nxt.append(b)
        nodes = nxt
    return codes


order = rng.permutation(n)
te_i = sorted(order[: int(0.30 * n)]); tr_i = sorted(order[int(0.30 * n):])
print(f"\n  Held out {len(te_i)} entries. An address group is assigned its TRAIN")
print( "  majority cell; a held-out entry is scored against that. In-sample MI is")
print( "  reported alongside so the memorisation gap is visible.\n")
print(f"  {'depth':>6}{'groups':>8}{'I in-sample':>13}{'norm':>7}"
      f"{'bits/lvl':>10}{'HELD-OUT acc':>14}{'unseen':>8}")
prev = 0.0
dy_rows = []
for d in range(1, 13):
    codes = dyadic(X, d)
    mi, nmi = mutual_info(codes, cell)
    pu, ng = purity(codes, cell)
    ho, un = heldout_predict(codes, cell, tr_i, te_i)
    dy_rows.append((d, mi, nmi, pu, ng, ho))
    print(f"  {d:>6}{ng:>8}{mi:>13.3f}{nmi:>7.3f}{mi - prev:>10.3f}"
          f"{ho:>14.3f}{un:>8.2f}")
    prev = mi

# the fractal prediction: constant information per level
# Measure bits/level ONLY over unsaturated depths. The splitter stops dividing
# a node below 4 members, so past saturation the gain is structurally zero and
# averaging it in would measure the stopping rule rather than the structure.
sat = next((i for i in range(1, len(dy_rows)) if dy_rows[i][4] == dy_rows[i - 1][4]),
           len(dy_rows))
gains = [dy_rows[i][1] - (dy_rows[i - 1][1] if i else 0.0) for i in range(sat)]
print(f"\n  the splitter saturates at depth {sat} ({dy_rows[sat-1][4]} groups, "
      f"nodes below 4 members stop dividing)")
print(f"  bits per level over the {sat} UNSATURATED depths: "
      + ", ".join(f"{g:.2f}" for g in gains))
half = max(1, sat // 2)
early, late = float(np.mean(gains[:half])), float(np.mean(gains[half:]))
print(f"  first half {early:.3f}, second half {late:.3f}, ratio {late/max(early,1e-9):.2f}")
if late / max(early, 1e-9) > 0.6:
    print("  -> information per level is roughly CONSTANT: the structure is")
    print("     self-similar, and a deep address is doing real work. The fractal")
    print("     guess is supported.")
else:
    print("  -> information per level DECAYS: the structure is shallow. Most of")
    print("     what an address can know is in its first few bits, and depth")
    print("     beyond that is decoration. The fractal guess is not supported.")

# random-address control at matched granularity
print("\n  control — random addresses at the same number of groups:")
for d in [4, 8, 12]:
    ng = dy_rows[d - 1][4]
    rc = [str(rng.integers(ng)) for _ in range(n)]
    mi, nmi = mutual_info(rc, cell)
    print(f"    depth {d:>2} ({ng:>3} groups): random I = {mi:.3f} "
          f"vs dyadic {dy_rows[d-1][1]:.3f}")

# ── TOKEN: signifiers straight from the text, no geometry ────────────────
if TOK and os.path.exists(TOK):
    hdr("B. Token address — the rarest Qwen tokens, used directly as the code")
    from tokenizers import Tokenizer
    D = QwenDictionary(TOK)
    tk = Tokenizer.from_file(TOK)
    ids = [tk.encode(t).ids for t in texts]
    print("\n  The code is the entry's k rarest tokens by MERGE RANK — Qwen's own")
    print("  frequency ordering, so 'rare' means rare in Qwen's training corpus,")
    print("  not in these 658 entries. No embedding is consulted.\n")
    print(f"  {'k tokens':>10}{'groups':>8}{'I in-sample':>13}{'norm':>7}"
          f"{'HELD-OUT acc':>14}{'unseen':>8}")
    for k in [1, 2, 3, 5]:
        codes = []
        for row in ids:
            rare = sorted(set(row), key=lambda t: -D.rank.get(t, 0))[:k]
            codes.append("|".join(str(x) for x in sorted(rare)))
        mi, nmi = mutual_info(codes, cell)
        pu, ng = purity(codes, cell)
        ho, un = heldout_predict(codes, cell, tr_i, te_i)
        print(f"  {k:>10}{ng:>8}{mi:>13.3f}{nmi:>7.3f}{ho:>14.3f}{un:>8.2f}")
    print("\n  The in-sample columns are a trap and are printed to show it: at")
    print("  k=2 there are 653 groups for 658 entries, so the code NAMES each")
    print("  entry and normalised MI reads 1.000. The held-out column is the")
    print("  real one, and 'unseen' is the fraction of held-out entries whose")
    print("  code never appeared in training — a code that generalises to")
    print("  nothing scores zero there by construction.")

hdr("C. What this says about formalising it")
best_d = max(dy_rows, key=lambda r: r[5])
print(f"\n  best dyadic address by HELD-OUT accuracy: depth {best_d[0]}, "
      f"{best_d[4]} groups, held-out {best_d[5]:.3f}")
print(f"  (its in-sample normalised MI is {best_d[2]:.3f} — the gap between those")
print(f"   two numbers is exactly the memorisation an address can fake)")
print(f"  geometry, for comparison: 0.447 cell accuracy, kappa 0.416")
print("\n  A decidable address is worth formalising only if it GENERALISES.")
print("  Compare the held-out column against geometry's 0.447 and decide")
print("  whether a Lean layer would encode structure or encode noise.")
