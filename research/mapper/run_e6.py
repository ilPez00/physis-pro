#!/usr/bin/env python3
"""E6 — localize the mode-axis weakness.

E5 found the two halves of the grid are not equally learnable: **domain kappa
0.552, mode kappa 0.418**. Stage 11 diagnosed the DOMAIN axis's problem exactly
-- five words nobody had defined -- wrote `docs/GRID_AXES.md`, and regenerated
the corpus against it. The mode axis never got that treatment.

This asks whether the mode axis has the same root cause, and where.

  A. Per-mode breakdown, and its top confusion partner.
  B. Population or definition? Correlate per-class F1 with class size. If the
     weakness tracks size, it is a population problem and writing definitions
     will not fix it.
  C. THE STAGE 11 TEST. Do the 70 authored anchors actually separate the axis
     they are supposed to define? For each entry, find the nearest anchor and
     ask whether its axis value matches. Run for BOTH axes: the domain axis has
     a written definition and is the control; if the mode axis scores markedly
     worse, its anchors do not define it, which is Stage 11's finding restated
     for the other half of the grid.
"""
import json, os, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "─" * 78); print(t); print("─" * 78)


def nearest_centroid(Xtr, ytr, Xte):
    classes = sorted(set(ytr))
    C = np.stack([l2(Xtr[[i for i, y in enumerate(ytr) if y == c]].mean(0, keepdims=True))[0]
                  for c in classes])
    return [classes[i] for i in np.argmax(Xte @ C.T, axis=1)]


def prf(pred, truth, c):
    tp = sum(p == c and t == c for p, t in zip(pred, truth))
    fp = sum(p == c and t != c for p, t in zip(pred, truth))
    fn = sum(p != c and t == c for p, t in zip(pred, truth))
    pr = tp / (tp + fp) if tp + fp else 0.0
    rc = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * pr * rc / (pr + rc) if pr + rc else 0.0
    return pr, rc, f1


E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
dom = [e["domain"] for e in E]
mode = [e["mode"] for e in E]
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
A = json.load(open(os.path.join(HERE, "anchors.json")))["anchors"]

rs = np.random.default_rng(17)
order = rs.permutation(n)
te = np.array(sorted(order[: int(0.30 * n)]))
tr = np.array(sorted(order[int(0.30 * n):]))

print("E6 — where is the mode axis weak, and why?\n")
print(f"  {n} entries · train {len(tr)} · held out {len(te)} · {len(A)} authored anchors")

hdr("A. Per-mode breakdown, held out")
pred = nearest_centroid(X[tr], [mode[i] for i in tr], X[te])
truth = [mode[i] for i in te]
conf = defaultdict(Counter)
for p, t in zip(pred, truth):
    if p != t:
        conf[t][p] += 1
sizes = Counter(mode)
print(f"  {'mode':<12}{'n':>5}{'prec':>7}{'recall':>8}{'F1':>7}   most confused with")
rows = []
for m in sorted(set(mode), key=lambda c: -sizes[c]):
    pr, rc, f1 = prf(pred, truth, m)
    top = conf[m].most_common(1)
    rows.append((m, sizes[m], f1))
    print(f"  {m:<12}{sizes[m]:>5}{pr:>7.2f}{rc:>8.2f}{f1:>7.2f}   "
          f"{(top[0][0] + ' x' + str(top[0][1])) if top else '—'}")

hdr("B. Population or definition? F1 against class size")
ns = np.array([r[1] for r in rows], dtype=float)
f1s = np.array([r[2] for r in rows], dtype=float)
r = float(np.corrcoef(np.log(ns), f1s)[0, 1])
print(f"  Pearson r(log class size, F1) = {r:+.3f}  over {len(rows)} modes")
print(f"  smallest modes: " + ", ".join(f"{m}({k})" for m, k, _ in sorted(rows, key=lambda x: x[1])[:5]))
if r > 0.55:
    print("  -> the weakness tracks POPULATION. Writing definitions will not fix it;")
    print("     those modes need entries.")
elif r < 0.3:
    print("  -> the weakness does NOT track population. Size is not the explanation,")
    print("     which leaves the definition.")
else:
    print("  -> partly population; not the whole story.")

hdr("C. The Stage 11 test — do the authored anchors define their axis?")
print("  For each entry, find the nearest of the 70 authored anchors and ask")
print("  whether that anchor's axis value matches the entry's. The DOMAIN axis")
print("  received a written definition in Stage 11 and is the control.\n")
for label, key in [("anchor text (name + hints)", "vec"), ("hints alone", "vec_hints")]:
    AV = l2(np.array([a[key] for a in A], dtype=np.float32))
    nearest = np.argmax(X @ AV.T, axis=1)
    d_hit = float(np.mean([A[nearest[i]]["domain"] == dom[i] for i in range(n)]))
    m_hit = float(np.mean([A[nearest[i]]["mode"] == mode[i] for i in range(n)]))
    d_ch = max(Counter(dom).values()) / n
    m_ch = max(Counter(mode).values()) / n
    print(f"  {label}")
    print(f"    domain recovered  {d_hit:.3f}   (largest class {d_ch:.3f})   "
          f"gain {(d_hit-d_ch)/(1-d_ch):>6.1%}")
    print(f"    mode   recovered  {m_hit:.3f}   (largest class {m_ch:.3f})   "
          f"gain {(m_hit-m_ch)/(1-m_ch):>6.1%}")
    print()

hdr("D. Which mode pairs actually collapse?")
pairs = Counter()
for p, t in zip(pred, truth):
    if p != t:
        pairs[tuple(sorted((p, t)))] += 1
print("  top confusion pairs on the held-out half:")
for (a_, b_), k in pairs.most_common(10):
    print(f"    {a_:<12} <-> {b_:<12} {k:>3}")
print("\n  A definition problem shows up as a few semantically adjacent pairs")
print("  swapping repeatedly. Diffuse confusion across unrelated pairs would")
print("  point at the representation instead.")
