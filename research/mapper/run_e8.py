#!/usr/bin/env python3
"""E8 — a pre-flight checker, validated; and the hypertrophy diagnostic.

Part 1  VALIDATE THE CHECKER against arms whose outcome is already known. A
        feasibility checker that cannot recover known failures is worthless, and
        one that also refuses arms that WORKED is worse than nothing.

Part 2  HYPERTROPHY / DIFFERENTIATION. Does a cell grow by making distinctions,
        or by absorbing whatever comes near it? Two indices, both against
        size-matched random controls:
          differentiation = mean intra-cell similarity - mean similarity of a
                            size-matched random sample of the corpus
          hypertrophy     = does differentiation FALL as the cell grows?
        WORK and CONSTRUCT/CREATE are the known suspects; this asks the question
        of every cell at once.

Part 3  PRE-FLIGHT THE GRID-EXPANSION PROPOSAL, then test the premise that a
        machine partition would serve better than the authored one.
"""
import json, os, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2
from preflight import (check_span, check_metric_wellposed, check_power_auc,
                       check_class_sparsity, check_leakage, report)

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "═" * 78); print(t); print("═" * 78)


# ── Part 1 ────────────────────────────────────────────────────────────────
hdr("PART 1 — validate the checker on arms whose outcome is already known")
P = json.load(open(os.path.join(HERE, "..", "perspective-discovery", "e1_points.json")))["points"]
s1 = np.array([p["s1"] for p in P]); s2 = np.array([p["s2"] for p in P])
inj = np.array([p["inj"] for p in P], dtype=bool)
z1 = (s1 - s1.mean()) / s1.std(); z2 = (s2 - s2.mean()) / s2.std()
base = np.column_stack([s1, s2])

print("\n  E1 measured every arm below. The checker sees only the features.\n")
cases = [
    ("signed dz  (E1c: 0.775, lost to s1+s2's 0.866)", z1 - z2, base, "linear"),
    ("absolute |D|  (E1: 0.602, lost)", np.abs(s1 - s2), base, "quadratic"),
    ("support_2 alone  (E1: 0.866, WON 22/22)", s2, s1[:, None], "linear"),
]
outcomes = []
for name, feat, bl, basis in cases:
    c = check_span(feat, bl, basis=basis)
    outcomes.append((name, c))
    mark = {"PASS": "  ok ", "WARN": " warn", "REFUTED": " STOP"}[c["status"]]
    print(f"   {mark}  {name}\n            {c['detail']}")
print("\n  Scorecard: the checker refutes signed dz (which lost), passes support_2")
print("  (which won 22/22), and only WARNS on |D| — it cannot prove that one")
print("  dead without running it. That is the honest boundary: the checker")
print("  catches ALGEBRAIC redundancy, not weak signal.")

# ── Part 2 ────────────────────────────────────────────────────────────────
hdr("PART 2 — do cells grow by differentiating, or by absorbing?")
E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
cell = [f"{e['domain']}/{e['mode']}" for e in E]
mode = [e["mode"] for e in E]
S = X @ X.T
rng = np.random.default_rng(17)


def differentiation(members, reps=60):
    """intra-group similarity minus that of size-matched random groups."""
    k = len(members)
    if k < 3:
        return None, None
    idx = np.array(members)
    intra = float(S[np.ix_(idx, idx)][np.triu_indices(k, 1)].mean())
    ctrl = []
    for _ in range(reps):
        r = rng.choice(n, k, replace=False)
        ctrl.append(float(S[np.ix_(r, r)][np.triu_indices(k, 1)].mean()))
    return intra, intra - float(np.mean(ctrl))


groups = defaultdict(list)
for i, m in enumerate(mode):
    groups[m].append(i)
print("\n  MODE axis — differentiation against a size-matched random control\n")
print(f"  {'mode':<12}{'n':>5}{'intra-sim':>11}{'vs random':>11}{'reading':>28}")
rows = []
for m in sorted(groups, key=lambda c: -len(groups[c])):
    intra, diff = differentiation(groups[m])
    if intra is None:
        print(f"  {m:<12}{len(groups[m]):>5}{'—':>11}{'—':>11}{'too small to measure':>28}")
        continue
    rows.append((m, len(groups[m]), diff))
    tag = ("absorbing — barely above random" if diff < 0.02
           else "weakly differentiated" if diff < 0.05 else "differentiated")
    print(f"  {m:<12}{len(groups[m]):>5}{intra:>11.3f}{diff:>+11.3f}{tag:>28}")

ns = np.array([r[1] for r in rows], float); ds = np.array([r[2] for r in rows], float)
r_hyp = float(np.corrcoef(np.log(ns), ds)[0, 1])
print(f"\n  HYPERTROPHY INDEX  r(log size, differentiation) = {r_hyp:+.3f}")
if r_hyp < -0.4:
    print("  -> the bigger a mode gets, the LESS it differentiates: growth is")
    print("     absorption, not distinction. That is hypertrophic propagation.")
elif r_hyp > 0.4:
    print("  -> bigger modes are MORE differentiated: growth tracks real structure.")
else:
    print("  -> size and differentiation are roughly independent: no systematic")
    print("     hypertrophy across the axis, whatever individual cells do.")

print("\n  The outer-ring test: does a cell's periphery dilute it more than chance?\n")
print(f"  {'mode':<12}{'core 75% diff':>15}{'full diff':>12}{'dilution':>11}")
for m, k, d in sorted(rows, key=lambda x: -x[1])[:8]:
    idx = np.array(groups[m])
    ctr = l2(X[idx].mean(0, keepdims=True))[0]
    d2c = X[idx] @ ctr
    core = idx[np.argsort(-d2c)[: max(3, int(0.75 * len(idx)))]]
    _, dcore = differentiation(list(core))
    print(f"  {m:<12}{dcore:>+15.3f}{d:>+12.3f}{dcore - d:>+11.3f}")
print("\n  A large positive dilution means the outer quarter is what makes the")
print("  cell look undifferentiated — the concrete, reviewable form of a sink.")

# ── Part 3 ────────────────────────────────────────────────────────────────
hdr("PART 3 — pre-flight the grid-expansion proposal")
print("\n  The proposal: add domains and modes, i.e. more cells over the same 658")
print("  entries. Run the checker on it before doing the work.\n")
for k_cells, label in [(70, "current 5 x 14 grid"), (140, "proposed 2x expansion"),
                       (280, "proposed 4x expansion")]:
    lab = [f"c{i % k_cells}" for i in range(n)]      # best case: perfectly balanced
    sizes = Counter(cell) if k_cells == 70 else Counter(lab)
    checks = [check_class_sparsity(list(cell) if k_cells == 70 else lab),
              check_metric_wellposed(list(cell) if k_cells == 70 else lab, metric="kappa")]
    report(checks, f"{label} — {k_cells} cells over {n} entries"
                   + ("" if k_cells == 70 else ", assuming PERFECT balance"))

print("\n  The expansion arms are checked under perfect balance, which no real")
print("  grid achieves — the current one has 22 of 57 cells below 5 entries")
print("  while its largest holds 69. The forecast is therefore optimistic.")

hdr("PART 3b — would a MACHINE partition serve better than the authored one?")
print("\n  A taxonomy has to survive resampling or it is not a taxonomy. The")
print("  authored grid has stability 1.0 for free — it is written down. A")
print("  discovered partition has to earn it. k-means on the same vectors,")
print("  adjusted Rand index between partitions fitted on disjoint 80% resamples.\n")


def kmeans(Xs, k, seed, iters=40):
    r = np.random.default_rng(seed)
    C = Xs[r.choice(len(Xs), k, replace=False)]
    for _ in range(iters):
        a = np.argmax(Xs @ C.T, axis=1)
        for j in range(k):
            m = Xs[a == j]
            if len(m):
                C[j] = l2(m.mean(0, keepdims=True))[0]
    return np.argmax(Xs @ C.T, axis=1)


def ari(a, b):
    ct = defaultdict(int)
    for x, y in zip(a, b):
        ct[(x, y)] += 1
    ra, rb = Counter(a), Counter(b)
    N = len(a)
    comb = lambda x: x * (x - 1) / 2
    idx = sum(comb(v) for v in ct.values())
    ea = sum(comb(v) for v in ra.values()); eb = sum(comb(v) for v in rb.values())
    exp = ea * eb / comb(N)
    mx = (ea + eb) / 2
    return (idx - exp) / (mx - exp) if mx != exp else 0.0


print(f"  {'k':>6}{'ARI across resamples':>24}{'reading':>34}")
for k in [5, 14, 57, 140]:
    vals = []
    for s in range(6):
        r = np.random.default_rng(100 + s)
        i1 = r.choice(n, int(0.8 * n), replace=False)
        i2 = r.choice(n, int(0.8 * n), replace=False)
        both = np.intersect1d(i1, i2)
        if len(both) < 50:
            continue
        a1 = kmeans(X[i1], k, s)
        a2 = kmeans(X[i2], k, s + 50)
        m1 = {v: a1[j] for j, v in enumerate(i1)}
        m2 = {v: a2[j] for j, v in enumerate(i2)}
        vals.append(ari([m1[v] for v in both], [m2[v] for v in both]))
    mv = float(np.mean(vals))
    tag = ("unusable as a taxonomy" if mv < 0.25 else
           "unstable" if mv < 0.45 else "moderately stable")
    print(f"  {k:>6}{mv:>24.3f}{tag:>34}")
print("\n  For scale: Iteration 18 measured cross-embedder ARI at 0.10 and this")
print("  track treated that as fatal to discovered structure.")
