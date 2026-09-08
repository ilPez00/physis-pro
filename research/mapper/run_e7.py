#!/usr/bin/env python3
"""E7 — would E6's proposed fixes actually work?

E6 ranked three remedies for the mode axis and said plainly that none of them
was tested. Recommending work without testing the recommendation is how a
research track accumulates plausible-sounding debt, so each is measured here
against the same held-out split.

  A. POPULATION. E6 blamed the F1 = 0.00 modes on starvation (r = +0.718 with
     log class size). If that is right, a mode that HAS data should collapse the
     same way when its training entries are subsampled down to 3 or 4. If a big
     mode still scores well at n = 3, starvation is not a sufficient explanation
     and BRAINSTORM's zero needs another one.

  B. THE SINK. Is WORK actively harming its neighbours, or merely failing
     itself? Remove WORK from the label space and re-score the other 13 modes on
     the same entries. If their F1 jumps, WORK is pulling entries away from
     cells they belong in.

  C. THE CLUSTER. Are SENSE / MAINTAIN / LEARN / PLAN separable from each other
     at all? Restrict to those four and measure, against size-matched random
     four-mode subsets as the control.
"""
import json, os, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "─" * 78); print(t); print("─" * 78)


def centroids(X, y, classes):
    return np.stack([
        l2(X[[i for i, v in enumerate(y) if v == c]].mean(0, keepdims=True))[0]
        if any(v == c for v in y) else np.zeros(X.shape[1], dtype=np.float32)
        for c in classes])


def predict(C, classes, Xte):
    return [classes[i] for i in np.argmax(Xte @ C.T, axis=1)]


def f1_of(pred, truth, c):
    tp = sum(p == c and t == c for p, t in zip(pred, truth))
    fp = sum(p == c and t != c for p, t in zip(pred, truth))
    fn = sum(p != c and t == c for p, t in zip(pred, truth))
    if tp == 0:
        return 0.0
    pr, rc = tp / (tp + fp), tp / (tp + fn)
    return 2 * pr * rc / (pr + rc)


def kappa(pred, truth):
    n = len(truth)
    po = sum(p == t for p, t in zip(pred, truth)) / n
    pc, tc = Counter(pred), Counter(truth)
    pe = sum((pc.get(c, 0) / n) * (tc.get(c, 0) / n) for c in set(truth) | set(pred))
    return (po - pe) / (1 - pe) if pe < 1 else 0.0


E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
mode = [e["mode"] for e in E]
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
rs = np.random.default_rng(17)
order = rs.permutation(n)
te = np.array(sorted(order[: int(0.30 * n)]))
tr = np.array(sorted(order[int(0.30 * n):]))
classes = sorted(set(mode))
truth = [mode[i] for i in te]
sizes = Counter(mode)

print("E7 — do E6's proposed fixes hold up?\n")
print(f"  {n} entries · train {len(tr)} · held out {len(te)} · 14 modes")

hdr("A. Would populating the starved modes help?")
print("  Each well-populated mode has its TRAIN entries subsampled to k, every")
print("  other mode keeps all of its own, and that mode's held-out F1 is")
print("  re-measured. 12 replicates. This is the F1(n) curve read off modes")
print("  that actually have data.\n")
big = [m for m in classes if sizes[m] >= 50]
ks = [3, 4, 6, 10, 20, 40, None]
print(f"  {'mode':<11}{'n':>5}" + "".join(f"{('all' if k is None else k):>7}" for k in ks))
curves = {}
for m in sorted(big, key=lambda c: -sizes[c]):
    tr_m = [i for i in tr if mode[i] == m]
    tr_other = [i for i in tr if mode[i] != m]
    row = []
    for k in ks:
        vals = []
        reps = 1 if k is None else 12
        for r in range(reps):
            if k is None:
                sel = tr_m
            else:
                if k > len(tr_m):
                    vals.append(np.nan); continue
                sel = list(np.random.default_rng(1000 + r).choice(tr_m, k, replace=False))
            idx = tr_other + list(sel)
            y = [mode[i] for i in idx]
            C = centroids(X[idx], y, classes)
            vals.append(f1_of(predict(C, classes, X[te]), truth, m))
        row.append(float(np.nanmean(vals)) if vals else float("nan"))
    curves[m] = row
    print(f"  {m:<11}{sizes[m]:>5}" + "".join(f"{v:>7.2f}" for v in row))
starved = {"BRAINSTORM": 3, "DESTROY": 4, "GUIDE": 14}
print(f"\n  For comparison, the starved modes at their ACTUAL sizes:")
for m, k in starved.items():
    tr_m = [i for i in tr if mode[i] == m]
    C = centroids(X[tr], [mode[i] for i in tr], classes)
    print(f"    {m:<11} n={sizes[m]:<3} train={len(tr_m):<3} F1 = "
          f"{f1_of(predict(C, classes, X[te]), truth, m):.2f}")
at3 = [curves[m][0] for m in curves if not np.isnan(curves[m][0])]
print(f"\n  Well-populated modes subsampled to n=3 score F1 = {np.mean(at3):.2f} on average.")
print( "  -> starvation is a SUFFICIENT explanation for a 0.00 only if this is near 0.")

hdr("B. Is WORK a sink that harms its neighbours, or just weak itself?")
keep = [i for i in range(n) if mode[i] != "WORK"]
tr_k = [i for i in tr if mode[i] != "WORK"]
te_k = [i for i in te if mode[i] != "WORK"]
cls13 = sorted(set(mode[i] for i in keep))
truth13 = [mode[i] for i in te_k]
# with WORK present as a distractor class, scored on the same non-WORK entries
C_all = centroids(X[tr], [mode[i] for i in tr], classes)
p_with = predict(C_all, classes, X[te_k])
# WORK removed from the label space entirely
C_13 = centroids(X[tr_k], [mode[i] for i in tr_k], cls13)
p_without = predict(C_13, cls13, X[te_k])
print(f"  scored on the {len(te_k)} held-out entries that are NOT WORK\n")
print(f"  {'':<28}{'accuracy':>10}{'kappa':>9}{'macro-F1':>10}")
for nm, p, cl in [("WORK present as a class", p_with, classes),
                  ("WORK removed entirely", p_without, cls13)]:
    acc = float(np.mean([a == b for a, b in zip(p, truth13)]))
    mf = float(np.mean([f1_of(p, truth13, c) for c in cls13]))
    print(f"  {nm:<28}{acc:>10.3f}{kappa(p, truth13):>9.3f}{mf:>10.3f}")
stolen = sum(1 for p, t in zip(p_with, truth13) if p == "WORK")
print(f"\n  entries of other modes that WORK captured: {stolen} of {len(te_k)} "
      f"({stolen/len(te_k):.1%})")

hdr("C. Is SENSE / MAINTAIN / LEARN / PLAN separable at all?")
cluster = ["SENSE", "MAINTAIN", "LEARN", "PLAN"]
def restricted(subset):
    tr_s = [i for i in tr if mode[i] in subset]
    te_s = [i for i in te if mode[i] in subset]
    if len(te_s) < 12 or len(set(mode[i] for i in tr_s)) < len(subset):
        return None
    C = centroids(X[tr_s], [mode[i] for i in tr_s], subset)
    p = predict(C, subset, X[te_s])
    t = [mode[i] for i in te_s]
    return (float(np.mean([a == b for a, b in zip(p, t)])), kappa(p, t), len(te_s))
r = restricted(cluster)
print(f"  {'subset':<44}{'accuracy':>10}{'kappa':>9}{'n test':>8}")
print(f"  {'SENSE/MAINTAIN/LEARN/PLAN':<44}{r[0]:>10.3f}{r[1]:>9.3f}{r[2]:>8}")
ctrl = []
pool = [m for m in classes if sizes[m] >= 25]
rng = np.random.default_rng(7)
for _ in range(40):
    s = sorted(rng.choice(pool, 4, replace=False).tolist())
    if s == sorted(cluster):
        continue
    v = restricted(s)
    if v:
        ctrl.append(v[1])
print(f"  {'random 4-mode subsets (control), mean kappa':<44}{'':>10}"
      f"{np.mean(ctrl):>9.3f}{'':>8}")
print(f"  {'  their range':<44}{'':>10}{f'{min(ctrl):.3f}–{max(ctrl):.3f}':>9}")
print(f"\n  If the cluster's kappa sits at the bottom of the control range, those")
print( "  four modes are genuinely one concept as the corpus uses them. If it sits")
print( "  inside the range, E6's cluster reading was pattern-matching.")
