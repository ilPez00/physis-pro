#!/usr/bin/env python3
"""E16 — the same battery on REAL misfilings, not injected ones.

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


print(f"\n  {'scorer':<13}{'AUC':>8}{'n':>7}{'pos':>6}   vs COSINE (95% CI)")
base = S["COSINE"]
for k in ["COSINE", "NAME-ONLY", "CONSTRAINT", "ANCHORVOC", "ANCESTRY"]:
    v = S[k]
    if not v:
        continue
    a = auc(v)
    if k == "COSINE":
        print(f"  {k:<13}{a:>8.3f}{len(v):>7}{sum(1 for x in v if x[1]):>6}   --")
    else:
        d0, lo, hi = boot(v, base)
        star = "  significant" if lo > 0 else ""
        print(f"  {k:<13}{a:>8.3f}{len(v):>7}{sum(1 for x in v if x[1]):>6}   "
              f"{d0:+.3f} [{lo:+.3f}, {hi:+.3f}]{star}")

print("\n  For reference, the SYNTHETIC results (E14/E15):")
print("    random injection   COSINE 0.882   CONSTRAINT 0.675")
print("    nearest injection  COSINE 0.506   CONSTRAINT 0.612")
print("    Stage 9 headline on real misfilings:  0.410")
