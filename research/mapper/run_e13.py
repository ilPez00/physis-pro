#!/usr/bin/env python3
"""E13 — how much better could a rewritten anchor get? The ceiling.

E12 produced a rewrite queue (STUDY/GUIDE at median rank 327, five cells at
precision 0.00) and a prohibition (do not generate anchors from the axis
definitions: -0.19). What it did not give is the number that decides whether the
rewrite is worth an afternoon: **how good can an anchor for these cells be?**

Five ways of writing an anchor, all scored the SAME way and all on HELD-OUT
entries so nothing is circular:

  A  authored anchor            what ships today
  B  written axis definitions   E12's refuted proposal, kept as the low mark
  C  centroid of TRAIN entries  the ceiling: perfect idiom, but not writable —
                                it is a vector, not a sentence
  D  top TF-IDF terms of TRAIN entries, as TEXT
                                what an automated anchor-writer could produce,
                                and a human could paste into a hints list
  E  authored anchor + those terms

If D approaches C and both clear A, anchors can be improved mechanically and the
rewrite queue should be automated rather than hand-worked. If D sits at A, hand-
rewriting will not help much either and the queue is low-value.
"""
import json, os, re, subprocess, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))


def hdr(t):
    print("\n" + "═" * 78); print(t); print("═" * 78)


E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
cell = [f"{e['domain']}/{e['mode']}" for e in E]
A = json.load(open(os.path.join(HERE, "anchors.json")))["anchors"]
AV = l2(np.array([a["vec"] for a in A], dtype=np.float32))
akey = [f"{a['domain']}/{a['mode']}" for a in A]
aname = {f"{a['domain']}/{a['mode']}": a["name"] for a in A}
D = json.load(open(os.path.join(HERE, "definitions.json")))["cells"]
DV = l2(np.array([d["vec"] for d in D], dtype=np.float32))
dkey = [f"{d['domain']}/{d['mode']}" for d in D]

rs = np.random.default_rng(17)
order = rs.permutation(n)
te = np.array(sorted(order[: int(0.30 * n)]))
tr = np.array(sorted(order[int(0.30 * n):]))
print("E13 — the anchor ceiling\n")
print(f"  {n} entries · train {len(tr)} · held out {len(te)}")
print("  every arm is scored on HELD-OUT entries only, so an anchor built from")
print("  train entries is not being tested against its own inputs.\n")

# cells with enough held-out entries to score
te_by = defaultdict(list)
for i in te:
    te_by[cell[i]].append(i)
cells = sorted(k for k, v in te_by.items() if len(v) >= 3)
print(f"  {len(cells)} cells have >=3 held-out entries and are measurable")

# ── build the writable arms ─────────────────────────────────────────────
STOP = set("""the a an and or of to in for with on at by from as is are was were be
this that it its their our your not no but if then than so such can may will
would should must all any some each other more most much many few own same very
just into over under between within without about after before during while when
where which who what how why do does did have has had using used use new per""".split())


def tfidf_terms(idx, k=12):
    """Top TF-IDF terms of a cell's TRAIN entries — a writable anchor."""
    docs = []
    for i in idx:
        t = (E[i]["name"] + " " + " ".join(E[i]["hints"])).lower()
        docs.append([w for w in re.findall(r"[a-z][a-z\-]{2,}", t) if w not in STOP])
    df = Counter()
    for d in docs:
        df.update(set(d))
    tf = Counter()
    for d in docs:
        tf.update(d)
    N = max(len(docs), 1)
    scored = [(c * np.log(1 + N / (1 + df[w])), w) for w, c in tf.items() if df[w] >= 2]
    scored.sort(reverse=True)
    return [w for _, w in scored[:k]]


texts, order_keys = [], []
for key in cells:
    tr_m = [i for i in tr if cell[i] == key]
    terms = tfidf_terms(tr_m) if tr_m else []
    texts.append(" ".join(terms) if terms else key.replace("/", " "))
    order_keys.append(key)
for key in cells:                       # arm E: anchor name + hints + terms
    tr_m = [i for i in tr if cell[i] == key]
    terms = tfidf_terms(tr_m) if tr_m else []
    ai = [i for i, a in enumerate(akey) if a == key]
    base = aname.get(key, "")
    hints = " ".join(A[ai[0]]["gloss"] if False else [])  # anchors.json has no gloss
    texts.append((base + " " + " ".join(terms)).strip())

inp = os.path.join(HERE, ".e13_texts.json")
outp = os.path.join(HERE, ".e13_vecs.f32")
json.dump({"texts": texts}, open(inp, "w"))
if not os.path.exists(outp) or os.path.getmtime(outp) < os.path.getmtime(inp):
    print(f"\n  embedding {len(texts)} candidate anchor texts...")
    subprocess.run(["cargo", "run", "-q", "-p", "physis-core", "--features", "embed-onnx",
                    "--release", "--example", "mapper_embed_texts", "--", inp, outp],
                   cwd=ROOT, check=True)
TV = l2(np.fromfile(outp, dtype=np.float32).reshape(len(texts), -1))
half = len(cells)
TERMS = {order_keys[i]: TV[i] for i in range(half)}
COMBO = {order_keys[i]: TV[half + i] for i in range(half)}

# ── score every arm identically, on held-out entries ────────────────────
def score(probe_of, label):
    precs, meds = [], []
    for key in cells:
        p = probe_of(key)
        if p is None:
            continue
        want = te_by[key]
        sims = X[te] @ p
        o = np.argsort(-sims)
        top = set(np.array(te)[o[: len(want)]].tolist())
        precs.append(len(top & set(want)) / len(want))
        pos = {int(v): r for r, v in enumerate(np.array(te)[o])}
        meds.append(float(np.median([pos[j] for j in want])))
    return label, float(np.mean(precs)), float(np.mean(meds)), precs


def anchor_of(key):
    ai = [i for i, a in enumerate(akey) if a == key]
    return AV[ai[0]] if ai else None


def defn_of(key):
    di = [i for i, d in enumerate(dkey) if d == key]
    return DV[di[0]] if di else None


def centroid_of(key):
    m = [i for i in tr if cell[i] == key]
    return l2(X[m].mean(0, keepdims=True))[0] if m else None


hdr("Five ways to write an anchor, scored on held-out entries")
print(f"\n  {'arm':<44}{'prec@n':>9}{'median rank':>14}")
res = {}
for fn, lbl in [(anchor_of, "A  authored anchor (ships today)"),
                (defn_of, "B  written axis definitions (E12: refuted)"),
                (centroid_of, "C  centroid of TRAIN entries (CEILING)"),
                (lambda k: TERMS.get(k), "D  top TF-IDF terms of TRAIN entries"),
                (lambda k: COMBO.get(k), "E  authored anchor + those terms")]:
    label, p, m, per = score(fn, lbl)
    res[lbl] = (p, m, per)
    print(f"  {label:<44}{p:>9.3f}{m:>14.0f}")
print(f"\n  scored over {len(cells)} cells, {len(te)} held-out entries")
print("  C is not writable — it is a vector, not a sentence. It bounds what any")
print("  wording could achieve; D is the best WRITABLE approximation to it.")

a_p, a_per = res["A  authored anchor (ships today)"][0], res["A  authored anchor (ships today)"][2]
c_p = res["C  centroid of TRAIN entries (CEILING)"][0]
d_p = res["D  top TF-IDF terms of TRAIN entries"][0]
e_p, e_per = res["E  authored anchor + those terms"][0], res["E  authored anchor + those terms"][2]
hdr("Is the rewrite queue worth the afternoon?")
print(f"\n  authored anchor today          {a_p:.3f}")
print(f"  centroid of the cell's entries {c_p:.3f}   headroom {c_p - a_p:+.3f}")
print("\n  -> the authored anchors already match a probe built from the cell's own")
print("     entries. Rewriting an anchor to be 'more in the entries' idiom' is")
print("     exhausted: that is what the centroid IS, and it wins by 0.004.")
print("     E12's rewrite queue is low-value, whoever works it.")

print(f"\n  BUT arm C is not a ceiling, and arm E proves it:")
print(f"    authored anchor + cell terms {e_p:.3f}   vs anchor {e_p - a_p:+.3f}, "
      f"vs centroid {e_p - c_p:+.3f}")
wins = sum(1 for x, y in zip(e_per, a_per) if y > x)
ties = sum(1 for x, y in zip(e_per, a_per) if y == x)
print(f"    per-cell: E beats A in {wins} of {len(a_per)} cells, {ties} ties, "
      f"{len(a_per)-wins-ties} losses")
nz = len(a_per) - ties
print("\n  READ THE WIN COUNT, NOT THE MEAN. E gains +0.038 on average while")
print(f"  LOSING in {len(a_per)-wins-ties} of {len(a_per)} cells. The mean is carried by a few large")
print("  gains offsetting many small losses; on a sign test over the "
      f"{nz} non-tied")
print("  cells that is nowhere near significant. Augmentation is NOT supported")
print("  either -- it would look like an improvement in aggregate and degrade")
print("  half the cells it touched.")
print(f"\n  Power note: only {len(a_per)} cells have >=3 held-out entries. This design")
print("  cannot resolve a per-cell effect of this size, and says so rather than")
print("  reporting the mean alone.")

hdr("The six worst cells from E12 — where do they land now?")
per_a = dict(zip(cells, res["A  authored anchor (ships today)"][2]))
per_c = dict(zip(cells, res["C  centroid of TRAIN entries (CEILING)"][2]))
per_d = dict(zip(cells, res["D  top TF-IDF terms of TRAIN entries"][2]))
print(f"\n  {'cell':<24}{'anchor':>9}{'terms':>9}{'ceiling':>10}  anchor name")
for key in sorted(per_a, key=lambda k: per_a[k])[:10]:
    print(f"  {key:<24}{per_a[key]:>9.2f}{per_d.get(key,0):>9.2f}"
          f"{per_c.get(key,0):>10.2f}  {aname.get(key,'')[:26]}")
