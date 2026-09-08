#!/usr/bin/env python3
"""E18 — batched: port validation, ANCHORVOC controls, second ground truth.

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




PD = os.path.join(ROOT, "research", "perspective-discovery")
byname = {e["name"]: k for k, e in enumerate(E)}


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


# ══ 1. PORT VALIDATION ═════════════════════════════════════════════════
# E15/E16 flagged that these scorers are Python re-implementations of
# experiment38's Rust ones. corpus_fit.tsv holds the Rust-computed values, so
# the port can be checked rather than assumed.
print("\n" + "═" * 74)
print("1. PORT VALIDATION — Python re-implementation vs experiment38's Rust")
print("═" * 74)
ref = {}
for line in open(os.path.join(PD, "corpus_fit.tsv")):
    f = line.rstrip("\n").split("\t")
    if len(f) < 5 or f[0] == "name":
        continue
    ref[f[0]] = (f[1], float(f[2]), float(f[3]), float(f[4]))
shared = [nm for nm in ref if nm in byname]
print(f"\n  {len(shared)} entries in both corpus_fit.tsv and the embedded corpus")

mem_t = defaultdict(list)
for nm in shared:
    mem_t[ref[nm][0]].append(byname[nm])
bydom_t = defaultdict(list)
for nm in shared:
    bydom_t[ref[nm][0].split("/")[0]].append(byname[nm])
mine_cos, mine_con, mine_anc, r_cos, r_con, r_anc = [], [], [], [], [], []
for nm in shared:
    i = byname[nm]; c = ref[nm][0]
    peers = [j for j in mem_t[c] if j != i]
    if len(peers) < 2:
        continue
    ctr = l2(X[peers].mean(0, keepdims=True))[0]
    mine_cos.append(1.0 - float(X[i] @ ctr)); r_cos.append(ref[nm][1])
    d = c.split("/")[0]
    sa = any(axis[j] == axis[i] and axis[i] for j in bydom_t[d] if j != i)
    sc_ = any(cat[j] == cat[i] and cat[i] for j in bydom_t[d] if j != i)
    mine_con.append((not sa) + (not sc_)); r_con.append(ref[nm][2])
    if ANC[i]:
        ps = [j for j in peers if ANC[j]][:12]
        if ps:
            js = [len(ANC[i] & ANC[j]) / max(len(ANC[i] | ANC[j]), 1) for j in ps]
            mine_anc.append(1.0 - float(np.mean(js))); r_anc.append(ref[nm][3])
for lbl, a, b in [("COSINE", mine_cos, r_cos), ("CONSTRAINT", mine_con, r_con),
                  ("ANCESTRY", mine_anc, r_anc)]:
    if len(a) > 10:
        r = float(np.corrcoef(a, b)[0, 1])
        tag = "  faithful" if r > 0.85 else ("  approximate" if r > 0.5 else "  DIVERGENT")
        print(f"  {lbl:<12} Pearson r = {r:+.3f}   n={len(a)}{tag}")
print("\n  A high r means the Python arm measures what Stage 12 measured. A low")
print("  one means E15/E16's numbers are about a different scorer than the")
print("  refutations they are being compared against.")

# ══ 2. ANCHORVOC CONTROLS ══════════════════════════════════════════════
print("\n" + "═" * 74)
print("2. ANCHORVOC CONTROLS — real vocabulary effect, or a text-length artifact?")
print("═" * 74)
rows = []
for line in open(os.path.join(PD, "refiling_gate0.tsv")):
    if line.startswith("#"):
        continue
    f = line.rstrip("\n").split("\t")
    if len(f) < 7 or f[0] == "n":
        continue
    rows.append((f[2], f[3], f[5] == "MOVED", f[6] == "blind"))
use = [(nm, oc, mv) for nm, oc, mv, bl in rows if bl and nm in byname]
idx = [byname[nm] for nm, _, _ in use]
oldc = [oc for _, oc, _ in use]; yv = [mv for _, _, mv in use]
mem = defaultdict(list)
for i, c in enumerate(oldc):
    mem[c].append(i)
ALLC = sorted(AH.keys())
rng = np.random.default_rng(4)
arms = defaultdict(list)
for i in range(len(use)):
    if len([j for j in mem[oldc[i]] if j != i]) < 2:
        continue
    t = etoks[idx[i]]; y = yv[i]
    own = AH.get(oldc[i], set())
    arms["ANCHORVOC (E16)"].append((-len(t & own), y))
    arms["  normalised by entry length"].append((-len(t & own) / max(len(t), 1), y))
    arms["  CONTROL random cell's anchor"].append((-len(t & AH[ALLC[rng.integers(len(ALLC))]]), y))
    arms["  CONTROL entry length only"].append((-len(t), y))
    arms["  CONTROL anchor size only"].append((-len(own), y))
base_c = []
for i in range(len(use)):
    peers = [j for j in mem[oldc[i]] if j != i]
    if len(peers) < 2:
        continue
    ctr = l2(X[[idx[j] for j in peers]].mean(0, keepdims=True))[0]
    base_c.append((1.0 - float(X[idx[i]] @ ctr), yv[i]))
print(f"\n  {'arm':<34}{'AUC':>8}   vs COSINE (95% CI)")
print(f"  {'COSINE (reference)':<34}{auc(base_c):>8.3f}   --")
for k, v in arms.items():
    d0, lo, hi = boot(v, base_c)
    print(f"  {k:<34}{auc(v):>8.3f}   {d0:+.3f} [{lo:+.3f}, {hi:+.3f}]"
          f"{'  sig' if lo > 0 else ''}")
# The comparison that actually separates the readings: does the LENGTH-NORMALISED
# vocabulary arm beat the length-only control? If not, ANCHORVOC is length.
d0, lo, hi = boot(arms["  normalised by entry length"], arms["  CONTROL entry length only"])
print(f"\n  DECISIVE: normalised ANCHORVOC vs length-only  {d0:+.3f} "
      f"[{lo:+.3f}, {hi:+.3f}]{'  significant' if lo > 0 else '  NOT significant'}")
print("  (entry length alone reaches 0.619 of ANCHORVOC's 0.664, so this is the")
print("   comparison that says whether any vocabulary effect survives at all)")
print("\n  If the random-anchor control also beats cosine, the effect is about")
print("  entry text rather than about the CELL's vocabulary, and 0.664 is an")
print("  artifact. If only the own-anchor arm wins, the effect is real.")

# ══ 3. SECOND GROUND TRUTH ═════════════════════════════════════════════
# cell_adjudication.tsv audits the ORIGINAL shipped filing -- one the rater did
# NOT produce -- so it is more independent than refiling_gate0.
print("\n" + "═" * 74)
print("3. SECOND GROUND TRUTH — the audit of a filing the rater did not produce")
print("═" * 74)
adj = []
for line in open(os.path.join(PD, "cell_adjudication.tsv")):
    if line.startswith("#"):
        continue
    f = line.rstrip("\n").split("\t")
    if len(f) < 4 or f[0] == "n":
        continue
    adj.append((f[2], f[1], f[3]))
ok = [(nm, c, v) for nm, c, v in adj if nm in byname]
print(f"\n  {len(adj)} adjudicated, {len(ok)} matched to an embedded entry")
for posdef, lbl in [({"NO"}, "NO only"), ({"NO", "MARGINAL"}, "NO + MARGINAL")]:
    m2 = defaultdict(list)
    for k, (nm, c, v) in enumerate(ok):
        m2[c].append(k)
    A2, C2 = [], []
    for k, (nm, c, v) in enumerate(ok):
        peers = [j for j in m2[c] if j != k]
        y = v in posdef
        A2.append((-len(etoks[byname[nm]] & AH.get(c, set())), y))
        if len(peers) >= 2:
            ctr = l2(X[[byname[ok[j][0]] for j in peers]].mean(0, keepdims=True))[0]
            C2.append((1.0 - float(X[byname[nm]] @ ctr), y))
    np_ = sum(1 for x in A2 if x[1])
    print(f"\n  positives = {lbl}  ({np_} of {len(A2)})")
    print(f"    ANCHORVOC  AUC {auc(A2):.3f}")
    if len(C2) > 20:
        print(f"    COSINE     AUC {auc(C2):.3f}   (n={len(C2)}, cells are sparse here)")
print("\n  n is small and the cells are thinly populated, so this is a")
print("  consistency check, not a second confirmation.")
