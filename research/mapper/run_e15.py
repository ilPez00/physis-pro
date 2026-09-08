#!/usr/bin/env python3
"""E15 — batched: does ANY scorer survive a realistic misfiling?

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


POLICIES = ["random", "same-domain", "same-mode", "nearest"]
NAMES = ["COSINE", "NAME-ONLY", "CONSTRAINT", "ANCHORVOC", "ANCESTRY"]
res = defaultdict(dict)
for pol in POLICIES:
    acc = defaultdict(list)
    for seed in range(6):
        asg, fl = inject(pol, np.random.default_rng(200 + seed))
        for k, v in scorers(asg, fl).items():
            a = auc(v)
            if a == a:
                acc[k].append(a)
    for k in NAMES:
        res[k][pol] = float(np.mean(acc[k])) if acc[k] else float("nan")

print("\n  AUC by scorer x injection policy (higher = better detection)\n")
print(f"  {'scorer':<13}" + "".join(f"{p:>13}" for p in POLICIES) + f"{'drop':>8}")
for k in NAMES:
    r = res[k]
    print(f"  {k:<13}" + "".join(f"{r[p]:>13.3f}" for p in POLICIES)
          + f"{r['nearest']-r['random']:>+8.3f}")
print(f"\n  Stage 9, the corpus's own real misfilings: 0.410")

# ── significance under the policy that matters ──────────────────────────
# `nearest` is deterministic, so the six seeds give one measurement. The
# uncertainty that matters is over ENTRIES, so bootstrap those.
asg, fl = inject("nearest", np.random.default_rng(0))
S = scorers(asg, fl)
rng = np.random.default_rng(7)
print("\n  ── bootstrap over entries, `nearest` injection, 2000 resamples ──\n")
print(f"  {'comparison':<34}{'diff':>8}{'95% CI':>20}")
base = S["COSINE"]
for k in ["NAME-ONLY", "CONSTRAINT", "ANCHORVOC", "ANCESTRY"]:
    other = S[k]
    m = min(len(base), len(other))
    if m < 30:
        continue
    d0 = auc(other[:m]) - auc(base[:m])
    ds = []
    for _ in range(2000):
        idx = rng.integers(0, m, m)
        a = [other[i] for i in idx]; b = [base[i] for i in idx]
        if sum(1 for x in a if x[1]) in (0, len(a)):
            continue
        ds.append(auc(a) - auc(b))
    ds.sort()
    lo, hi = ds[int(.025 * len(ds))], ds[int(.975 * len(ds))]
    star = "  significant" if lo > 0 else ""
    print(f"  {k + ' - COSINE':<34}{d0:>+8.3f}   [{lo:+.3f}, {hi:+.3f}]{star}")

print("\n  ── the question E14 opened ──")
cos_n = res["COSINE"]["nearest"]
best = max((res[k]["nearest"], k) for k in NAMES if k != "COSINE")
print(f"  cosine under realistic injection : {cos_n:.3f}")
print(f"  best non-geometric scorer        : {best[0]:.3f}  ({best[1]})")
if best[0] > cos_n + 0.03:
    print("  -> a structural scorer SURVIVES where geometry does not. Stage 12's")
    print("     refutations were an artifact of the injection policy and the")
    print("     certification question reopens.")
elif best[0] > 0.55:
    print("  -> structure does not beat geometry, but it clears chance where")
    print("     geometry does not. Worth a powered re-test.")
else:
    print("  -> nothing detects a plausible misfiling. Stage 12's refutations")
    print("     stand, and human-in-the-loop is the only remaining answer.")
