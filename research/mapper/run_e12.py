#!/usr/bin/env python3
"""E12 — audit all 70 anchors, and test whether the written definitions beat them.

E11's miner pulled `crane`, `hoist` and `freight elevator` for CONSTRUCT cells,
although `docs/GRID_AXES.md` defines CONSTRUCT as "the durable arrangement other
work happens inside or on top of". The authored anchor and the written
definition disagree — and the anchor is what the system actually uses.

Stage 11 is the one intervention in this whole track that demonstrably worked:
it found the domain axis had no written definition, wrote one, and regenerated
against it (misfiling proxy 31.8% -> 16.2%). This applies the same treatment at
the anchor level, but measures before prescribing.

  A. PER-ANCHOR QUALITY. Can each cell's anchor retrieve its own entries out of
     all 658? Ranked worst-first, that is the rewrite queue.
  B. IS THE FAILURE SYSTEMATIC BY DOMAIN? The CONSTRUCT hypothesis, tested
     across all five domains rather than asserted from one example.
  C. ANCHORS vs DEFINITIONS. A/B the hand-authored anchor text against the two
     written axis definitions composed. If the definitions retrieve better, the
     anchors should be regenerated from them, which is Stage 11's move.
"""
import json, os, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "═" * 78); print(t); print("═" * 78)


E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
cell = [f"{e['domain']}/{e['mode']}" for e in E]
dom = [e["domain"] for e in E]
mode = [e["mode"] for e in E]
A = json.load(open(os.path.join(HERE, "anchors.json")))["anchors"]
AV = l2(np.array([a["vec"] for a in A], dtype=np.float32))
akey = [f"{a['domain']}/{a['mode']}" for a in A]
aname = {f"{a['domain']}/{a['mode']}": a["name"] for a in A}
D = json.load(open(os.path.join(HERE, "definitions.json")))["cells"]
DV = l2(np.array([d["vec"] for d in D], dtype=np.float32))
dkey = [f"{d['domain']}/{d['mode']}" for d in D]

sizes = Counter(cell)
print("E12 — anchor audit\n")
print(f"  {n} entries · {len(A)} authored anchors · {len(D)} composed definitions")


def retrieval(vecs, keys, label, min_n=3):
    """For each cell, rank all entries by similarity to that cell's probe and
    measure how many of the cell's own entries land in the top-n."""
    rows = []
    for i, k in enumerate(keys):
        want = [j for j in range(n) if cell[j] == k]
        if len(want) < min_n:
            continue
        sims = X @ vecs[i]
        order = np.argsort(-sims)
        top = set(order[: len(want)].tolist())
        prec = len(top & set(want)) / len(want)
        # rank of the cell's median own-entry
        pos = {int(j): r for r, j in enumerate(order)}
        med = float(np.median([pos[j] for j in want]))
        rows.append((k, len(want), prec, med))
    return rows


hdr("A. Per-anchor quality — can an anchor find its own cell's entries?")
rows = retrieval(AV, akey, "anchor")
rows.sort(key=lambda r: (r[2], -r[3]))
print(f"\n  precision@n = of a cell's n entries, how many are in the anchor's top n")
print(f"  median rank = where the cell's typical entry sits among all {n}\n")
print(f"  {'cell':<24}{'n':>4}{'prec@n':>9}{'median rank':>13}  anchor name")
for k, cnt, prec, med in rows[:14]:
    print(f"  {k:<24}{cnt:>4}{prec:>9.2f}{med:>13.0f}  {aname.get(k,'')[:28]}")
print(f"\n  ... best-performing anchors, for contrast:")
for k, cnt, prec, med in rows[-6:]:
    print(f"  {k:<24}{cnt:>4}{prec:>9.2f}{med:>13.0f}  {aname.get(k,'')[:28]}")
mean_prec = float(np.mean([r[2] for r in rows]))
print(f"\n  mean precision@n across {len(rows)} measurable cells: {mean_prec:.3f}")
print(f"  chance for a cell of size s is s/{n}, i.e. under 0.11 for every cell")

hdr("B. Is the failure systematic by domain?")
bydom = defaultdict(list)
for k, cnt, prec, med in rows:
    bydom[k.split("/")[0]].append(prec)
print(f"\n  {'domain':<12}{'cells':>7}{'mean prec@n':>14}")
for d in sorted(bydom, key=lambda x: np.mean(bydom[x])):
    print(f"  {d:<12}{len(bydom[d]):>7}{np.mean(bydom[d]):>14.3f}")
print("\n  If CONSTRUCT sits at the bottom, E11's observation generalises from")
print("  one example to the whole domain. If not, it was a local artifact.")

hdr("C. Authored anchors vs written definitions — the Stage 11 move, measured")
ra = {k: (p, m) for k, c, p, m in retrieval(AV, akey, "a")}
rd = {k: (p, m) for k, c, p, m in retrieval(DV, dkey, "d")}
both = sorted(set(ra) & set(rd))
pa = float(np.mean([ra[k][0] for k in both]))
pd = float(np.mean([rd[k][0] for k in both]))
ma = float(np.mean([ra[k][1] for k in both]))
md = float(np.mean([rd[k][1] for k in both]))
print(f"\n  over {len(both)} cells with enough entries to measure\n")
print(f"  {'probe':<34}{'mean prec@n':>14}{'mean median rank':>19}")
print(f"  {'authored anchor (name + hints)':<34}{pa:>14.3f}{ma:>19.0f}")
print(f"  {'written axis definitions':<34}{pd:>14.3f}{md:>19.0f}")
wins = sum(1 for k in both if rd[k][0] > ra[k][0])
ties = sum(1 for k in both if rd[k][0] == ra[k][0])
print(f"\n  definitions beat the anchor in {wins} of {len(both)} cells "
      f"({ties} ties, {len(both)-wins-ties} losses)")

print(f"\n  the cells where the written definition helps MOST — rewrite these first:")
gain = sorted(both, key=lambda k: rd[k][0] - ra[k][0], reverse=True)
print(f"  {'cell':<24}{'anchor':>9}{'definition':>13}{'gain':>8}")
for k in gain[:8]:
    print(f"  {k:<24}{ra[k][0]:>9.2f}{rd[k][0]:>13.2f}{rd[k][0]-ra[k][0]:>+8.2f}")
print(f"\n  and where the authored anchor is genuinely better — keep these:")
for k in gain[-5:]:
    print(f"  {k:<24}{ra[k][0]:>9.2f}{rd[k][0]:>13.2f}{rd[k][0]-ra[k][0]:>+8.2f}")

out = {"mean_precision_anchor": pa, "mean_precision_definition": pd,
       "rewrite_queue": [{"cell": k, "anchor_prec": ra[k][0],
                          "definition_prec": rd[k][0],
                          "gain": rd[k][0] - ra[k][0]} for k in gain]}
json.dump(out, open(os.path.join(HERE, "anchor_audit.json"), "w"), indent=1)
print(f"\nwrote {os.path.join(HERE,'anchor_audit.json')}")
