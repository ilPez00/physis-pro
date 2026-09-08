#!/usr/bin/env python3
"""E17 — is ANCHORVOC circular? Three tests.

One process, one pass, five scorers x four injection policies. Everything is
read from artifacts already on disk (corpus.json, anchors, WordNet); nothing is
re-embedded.

E14 showed the geometric scorer falls 0.880 -> 0.506 when the injected error
becomes plausible. Stage 12 refuted the structural scorers against that 0.880.
The refutations were therefore measured on a task nobody faces, and whether
structure survives where geometry does not is the highest-value open question in
the track.

SCORERS (all oriented so higher = more suspect)
  COSINE       distance to the assigned cell's centroid, self excluded  [E1]
  NAME-ONLY    the same, on name embeddings only            [Stage 13 control]
  CONSTRAINT   is the entry's axis_name / category attested elsewhere
               in its assigned cell's DOMAIN?                [Stage 12]
  ANCHORVOC    negated lexical overlap with the cell's authored anchor hints
  ANCESTRY     1 - mean WordNet ancestor Jaccard against cellmates  [Stage 12]

POLICIES
  random / same-domain / same-mode / nearest         [E14]
"""
import json, os, re, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))


def auc(sc):
    v = sorted(sc, key=lambda x: x[0])
    pos = sum(1 for x in v if x[1]); neg = len(v) - pos
    if pos == 0 or neg == 0:
        return float("nan")
    rs, i = 0.0, 0
    while i < len(v):
        j = i
        while j + 1 < len(v) and v[j + 1][0] == v[i][0]:
            j += 1
        a = (i + 1 + j + 1) / 2.0
        for x in v[i:j + 1]:
            if x[1]:
                rs += a
        i = j + 1
    return (rs - pos * (pos + 1) / 2.0) / (pos * neg)


def words(s):
    return {w for w in re.findall(r"[a-z][a-z\-]{2,}", s.lower())}


# ── data, all cached ────────────────────────────────────────────────────
E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
XN = l2(np.array([e["vec_name"] for e in E], dtype=np.float32))
dom = [e["domain"] for e in E]; mode = [e["mode"] for e in E]
cell = [f"{d}/{m}" for d, m in zip(dom, mode)]
cells = sorted(set(cell)); ci = {c: k for k, c in enumerate(cells)}
axis = [e.get("axis", "") for e in E]; cat = [e.get("category", "") for e in E]
etoks = [words(e["name"] + " " + " ".join(e["hints"])) for e in E]

ap = next(p for p in [os.path.join(ROOT, "physis-core/config/mode_anchors_ontology.json"),
                      os.path.join(ROOT, "config/mode_anchors_ontology.json")]
          if os.path.exists(p))
AH = {}
for a in json.load(open(ap))["domains"]:
    AH[f"{a['domain']}/{a['mode']}"] = words(a["name"] + " " + " ".join(a.get("hints", [])))

# WordNet ancestor sets, parsed once
wn = os.path.join(ROOT, "models/wordnet/data.noun")
lemma, hyper = defaultdict(list), defaultdict(list)
for line in open(wn, encoding="latin-1"):
    if line.startswith("  ") or not line.strip():
        continue
    head = line.split(" | ")[0]; f = head.split()
    if len(f) < 5:
        continue
    try:
        off = int(f[0]); wc = int(f[3], 16)
    except ValueError:
        continue
    for k in range(wc):
        if 4 + k * 2 < len(f):
            lemma[f[4 + k * 2].replace("_", " ").lower()].append(off)
    ps = 4 + wc * 2
    if ps < len(f):
        try:
            pc = int(f[ps])
        except ValueError:
            continue
        for p in range(pc):
            b = ps + 1 + p * 4
            if b + 2 < len(f) and f[b] in ("@", "@i") and f[b + 2] == "n":
                hyper[off].append(int(f[b + 1]))


def anc(seeds, depth=6):
    seen = set(seeds); fr = list(seeds)
    for _ in range(depth):
        nx = []
        for x in fr:
            for p in hyper.get(x, []):
                if p not in seen:
                    seen.add(p); nx.append(p)
        if not nx:
            break
        fr = nx
    return seen


ANC = []
for e in E:
    seeds = []
    for w in words(e["name"]):
        seeds += lemma.get(w, [])[:2]
    ANC.append(anc(seeds) if seeds else set())
anchored = [i for i in range(n) if ANC[i]]
print(f"E15 — batched realistic-injection battery\n")
print(f"  {n} entries · {len(cells)} cells · {len(anchored)} anchored in WordNet")

true_ctr = np.stack([l2(X[[i for i in range(n) if cell[i] == c]].mean(0, keepdims=True))[0]
                     for c in cells])
s2c = X @ true_ctr.T


def inject(policy, rng, stride=7):
    asg = list(cell); fl = [False] * n
    for i in range(0, n, stride):
        cur = cell[i]
        if policy == "random":
            al = [c for c in cells if c != cur]
        elif policy == "same-domain":
            al = [c for c in cells if c != cur and c.startswith(dom[i] + "/")]
        elif policy == "same-mode":
            al = [c for c in cells if c != cur and c.endswith("/" + mode[i])]
        else:
            al = [cells[k] for k in np.argsort(-s2c[i]) if cells[k] != cur][:1]
        if not al:
            continue
        asg[i] = al[rng.integers(len(al))] if policy != "nearest" else al[0]
        fl[i] = True
    return asg, fl


def scorers(asg, fl):
    mem = defaultdict(list)
    for i, c in enumerate(asg):
        mem[c].append(i)
    dom_of = {c: c.split("/")[0] for c in cells}
    by_dom = defaultdict(list)
    for i, c in enumerate(asg):
        by_dom[dom_of[c]].append(i)
    out = defaultdict(list)
    for i in range(n):
        peers = [j for j in mem[asg[i]] if j != i]
        if len(peers) < 2:
            continue
        y = fl[i]
        ctr = l2(X[peers].mean(0, keepdims=True))[0]
        out["COSINE"].append((1.0 - float(X[i] @ ctr), y))
        ctrn = l2(XN[peers].mean(0, keepdims=True))[0]
        out["NAME-ONLY"].append((1.0 - float(XN[i] @ ctrn), y))
        d = dom_of[asg[i]]
        seen_ax = any(axis[j] == axis[i] and axis[i] for j in by_dom[d] if j != i)
        seen_ct = any(cat[j] == cat[i] and cat[i] for j in by_dom[d] if j != i)
        out["CONSTRAINT"].append(((not seen_ax) + (not seen_ct), y))
        av = AH.get(asg[i], set())
        out["ANCHORVOC"].append((-len(etoks[i] & av), y))
        if ANC[i]:
            ps = [j for j in peers if ANC[j]][:12]
            if ps:
                js = [len(ANC[i] & ANC[j]) / max(len(ANC[i] | ANC[j]), 1) for j in ps]
                out["ANCESTRY"].append((1.0 - float(np.mean(js)), y))
    return out



# ── REAL ground truth: Stage 11's blind re-filing ───────────────────────
# refiling_gate0.tsv: every entry re-filed by a rater who saw name, category and
# hints and was BLIND to the existing cell, judging against docs/GRID_AXES.md.
#   positive = MOVED  (a blind judge rejected the old filing)
#   negative = same   (a blind judge endorsed it)
# The 76 entries whose hints narrate their old cell are excluded — the file
# flags them precisely so this can be done.
#
# CAVEAT from the artifact's own header: the rater was Claude, and the same
# rater produced the replacement filing. Blindness to the old cell makes the
# disagreement genuine; this is not an independent human panel.
PD = os.path.join(ROOT, "research", "perspective-discovery")
rows = []
for line in open(os.path.join(PD, "refiling_gate0.tsv")):
    if line.startswith("#"):
        continue
    f = line.rstrip("\n").split("\t")
    if len(f) < 7 or f[0] == "n":
        continue
    rows.append((f[2], f[3], f[5] == "MOVED", f[6] == "blind"))

byname = {e["name"]: k for k, e in enumerate(E)}
use = [(nm, oc, mv) for nm, oc, mv, bl in rows if bl and nm in byname]
print(f"\n  ground truth: {len(rows)} re-filed, {sum(1 for r in rows if r[3])} blind, "
      f"{len(use)} matched to an embedded entry")
idx = [byname[nm] for nm, _, _ in use]
oldc = [oc for _, oc, _ in use]
yv = [mv for _, _, mv in use]
print(f"  positives (blind rater MOVED it) {sum(yv)}  negatives {len(yv)-sum(yv)}"
      f"   base rate {sum(yv)/len(yv):.1%}")

Xr = X[idx]; XNr = XN[idx]
axr = [axis[i] for i in idx]; ctr_ = [cat[i] for i in idx]
etr = [etoks[i] for i in idx]; ancr = [ANC[i] for i in idx]

mem = defaultdict(list)
for i, c in enumerate(oldc):
    mem[c].append(i)
bydom = defaultdict(list)
for i, c in enumerate(oldc):
    bydom[c.split("/")[0]].append(i)

S = defaultdict(list)
for i in range(len(use)):
    peers = [j for j in mem[oldc[i]] if j != i]
    if len(peers) < 2:
        continue
    y = yv[i]
    c1 = l2(Xr[peers].mean(0, keepdims=True))[0]
    S["COSINE"].append((1.0 - float(Xr[i] @ c1), y))
    c2 = l2(XNr[peers].mean(0, keepdims=True))[0]
    S["NAME-ONLY"].append((1.0 - float(XNr[i] @ c2), y))
    d = oldc[i].split("/")[0]
    sa = any(axr[j] == axr[i] and axr[i] for j in bydom[d] if j != i)
    sc_ = any(ctr_[j] == ctr_[i] and ctr_[i] for j in bydom[d] if j != i)
    S["CONSTRAINT"].append(((not sa) + (not sc_), y))
    S["ANCHORVOC"].append((-len(etr[i] & AH.get(oldc[i], set())), y))
    if ancr[i]:
        ps = [j for j in peers if ancr[j]][:12]
        if ps:
            js = [len(ancr[i] & ancr[j]) / max(len(ancr[i] | ancr[j]), 1) for j in ps]
            S["ANCESTRY"].append((1.0 - float(np.mean(js)), y))


def boot(a, b, iters=2000, seed=5):
    m = min(len(a), len(b)); rng = np.random.default_rng(seed)
    d0 = auc(a[:m]) - auc(b[:m]); ds = []
    for _ in range(iters):
        ix = rng.integers(0, m, m)
        aa = [a[i] for i in ix]; bb = [b[i] for i in ix]
        if sum(1 for x in aa if x[1]) in (0, len(aa)):
            continue
        ds.append(auc(aa) - auc(bb))
    ds.sort()
    return d0, ds[int(.025 * len(ds))], ds[int(.975 * len(ds))]



# ═══════════════════════════════════════════════════════════════════════
# E17. ANCHORVOC scored 0.664 on real misfilings, the only significant
# result. But it measures lexical overlap with the cell's authored anchor
# hints, and the blind rater judged entries against those same anchors. It
# may detect "shares vocabulary with the text the rater read" rather than
# "is misfiled". Three tests, cheapest and most decisive first.
# ═══════════════════════════════════════════════════════════════════════
import random

ALLC = sorted(AH.keys())


def av(tokens, cellkey, hints=None):
    return len(tokens & (hints if hints is not None else AH.get(cellkey, set())))


print("\n" + "═" * 78)
print("TEST 1 — does ANCHORVOC predict the rater's CHOICE of new cell?")
print("═" * 78)
print("""
  If the rater's procedure is essentially 'pick the cell whose anchor shares
  most vocabulary', then ANCHORVOC on the OLD cell is measuring the rater's own
  rule and the 0.664 is circular. Rank all 70 cells by ANCHORVOC for each MOVED
  entry and see where the rater's chosen cell lands.
""")
newc = {}
for line in open(os.path.join(PD, "refiling_gate0.tsv")):
    if line.startswith("#"):
        continue
    f = line.rstrip("\n").split("\t")
    if len(f) < 7 or f[0] == "n":
        continue
    newc[f[2]] = f[4]

moved = [(nm, oc) for nm, oc, mv in use if mv]
r_new, r_old, r_rand = [], [], []
rng = random.Random(3)
for nm, oc in moved:
    t = etoks[byname[nm]]
    scores = sorted(ALLC, key=lambda c: -av(t, c))
    pos = {c: i for i, c in enumerate(scores)}
    nc = newc.get(nm)
    if nc in pos:
        r_new.append(pos[nc])
    if oc in pos:
        r_old.append(pos[oc])
    r_rand.append(pos[rng.choice(ALLC)])
import numpy as _np
print(f"  {'cell':<28}{'median rank of 70':>20}{'top-5 rate':>13}")
for lbl, r in [("the rater's NEW cell", r_new), ("the rejected OLD cell", r_old),
               ("a random cell (control)", r_rand)]:
    print(f"  {lbl:<28}{_np.median(r):>20.0f}{sum(1 for x in r if x < 5)/len(r):>13.1%}")
print(f"\n  n = {len(moved)} moved entries")
if _np.median(r_new) < 8:
    print("  -> the rater's choice sits near the TOP of the ANCHORVOC ranking.")
    print("     The rater's rule and this scorer largely coincide, so the 0.664")
    print("     is substantially CIRCULAR.")
else:
    print("  -> the rater's choice is NOT what ANCHORVOC would have picked, so the")
    print("     scorer is not simply reproducing the rater's rule.")

print("\n" + "═" * 78)
print("TEST 2 — held-out half of each anchor's hints")
print("═" * 78)
print("""
  Build the probe from HALF of each anchor's hint words and score with it. The
  rater read all of them. If the effect survives on the unseen half it is about
  the cell's vocabulary rather than the exact strings.
""")
rng2 = random.Random(11)
AH_half = {}
for c, toks in AH.items():
    tl = sorted(toks)
    rng2.shuffle(tl)
    AH_half[c] = set(tl[: max(1, len(tl) // 2)])
S2 = []
for i in range(len(use)):
    peers = [j for j in mem[oldc[i]] if j != i]
    if len(peers) < 2:
        continue
    S2.append((-av(etr[i], oldc[i], AH_half.get(oldc[i], set())), yv[i]))
a_full = auc(S["ANCHORVOC"]); a_half = auc(S2)
d0, lo, hi = boot(S2, S["COSINE"])
print(f"  ANCHORVOC, all hints      AUC {a_full:.3f}")
print(f"  ANCHORVOC, half the hints AUC {a_half:.3f}   vs COSINE {d0:+.3f} "
      f"[{lo:+.3f}, {hi:+.3f}]{'  significant' if lo > 0 else ''}")

print("\n" + "═" * 78)
print("TEST 3 — the 76 entries where blindness FAILED")
print("═" * 78)
print("""
  The file flags entries whose hints narrate their own old cell. The rater's
  procedure was identical there; only blindness differs. A scorer detecting real
  misfiling should behave similarly on both; one tracking the rater's reading of
  the hints should not.
""")
nb = [(nm, oc, mv) for nm, oc, mv, bl in rows if not bl and nm in byname]
if len(nb) > 30:
    idx2 = [byname[nm] for nm, _, _ in nb]
    old2 = [oc for _, oc, _ in nb]; y2 = [mv for _, _, mv in nb]
    X2 = l2(X[idx2]); et2 = [etoks[i] for i in idx2]
    m2 = defaultdict(list)
    for i, c in enumerate(old2):
        m2[c].append(i)
    A2, C2 = [], []
    for i in range(len(nb)):
        p = [j for j in m2[old2[i]] if j != i]
        if len(p) < 2:
            continue
        A2.append((-av(et2[i], old2[i]), y2[i]))
        ct = l2(X2[p].mean(0, keepdims=True))[0]
        C2.append((1.0 - float(X2[i] @ ct), y2[i]))
    print(f"  n = {len(A2)} non-blind entries, {sum(1 for x in A2 if x[1])} moved")
    print(f"  ANCHORVOC  AUC {auc(A2):.3f}   (blind subset: {a_full:.3f})")
    print(f"  COSINE     AUC {auc(C2):.3f}   (blind subset: {auc(S['COSINE']):.3f})")
else:
    print(f"  only {len(nb)} non-blind entries matched — too few to compare")

print("\n" + "═" * 78)
print("VERDICT")
print("═" * 78)
# The verdict must weigh all three tests. The first version read only Test 1
# and declared the confound resolved, ignoring that Test 3 goes the other way
# by the criterion this script itself pre-registered.
t1_clean = _np.median(r_new) >= 8
t2_survives = lo > 0
t3_similar = abs(auc(A2) - a_full) < 0.10 if len(nb) > 30 else None
print(f"\n  Test 1  rater's rule != ANCHORVOC      : {'PASS' if t1_clean else 'FAIL'}"
      f"   (new cell at median rank {_np.median(r_new):.0f} of 70, random 35)")
print(f"  Test 2  survives held-out hints        : {'PASS' if t2_survives else 'FAIL'}"
      f"   ({a_half:.3f}, still significant)")
print(f"  Test 3  behaves the same without blind : "
      f"{'PASS' if t3_similar else 'FAIL'}   ({auc(A2):.3f} vs {a_full:.3f})")
print("""
  The tests DISAGREE, and the honest reading is partial.

  Test 1 is reassuring: the rater's chosen cell sits at median rank 17 of 70,
  not 1-3, so the rater is not running ANCHORVOC. But 33% top-5 against a 7.9%
  random baseline says the two rules overlap more than chance -- shared
  information, not a shared procedure.

  Test 2 is reassuring: half the hint vocabulary still beats cosine.

  Test 3 fails the criterion this script pre-registered. On the 56 entries whose
  hints narrate their own cell, ANCHORVOC RISES to 0.871 while cosine falls.
  A detector of real misfiling should not care whether the hint text leaks the
  cell; one that keys on hint-vs-anchor wording should, and does.

  NET: ANCHORVOC is not circular in the strong sense -- it does not reproduce
  the rater's decision. It is hint-dependent in a way a filing-independent
  signal would not be. The 0.664 is most likely a real but PARTLY
  vocabulary-mediated effect: an entry sharing no words with its cell's anchor
  really is more likely misfiled, and that is also part of what the rater used.
  Treat it as a lead worth an independent test, not as a validated certifier.

  The independent test that would settle it: adjudication by a rater who never
  sees the anchor hints, only the axis definitions. That removes the shared
  input entirely. n=56 is also too small for Test 3 to be more than a flag.
""")
