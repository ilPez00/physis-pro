#!/usr/bin/env python3
"""E11 — candidate mining from WordNet, validated before it is trusted.

The expansion spec recorded a negative result: mining free prose for ontology
entries fails, because an entry is a short domain concept and prose contains
none in extractable form. The stated fix was a source that ALREADY holds
concept-shaped items. WordNet is that, and its noun synsets have exactly the
shape of a physis entry -- a lemma plus a gloss, which is name plus hints.

THE VALIDATION, WHICH COMES FIRST
---------------------------------
The previous three miners were shipped straight to output and all three were
useless. This one is tested against ground truth before its output is looked at.

  Hold out 30% of the EXISTING entries. Drop them into a pool with ~20,000
  WordNet synsets as distractors. Build each cell's target from its TRAIN
  members and its authored anchor only. Then rank the whole pool per cell and
  ask: do a cell's own held-out entries surface near the top?

That is a retrieval test with real ground truth and a large distractor set. If
the miner cannot find entries it is KNOWN to want, it will not find ones nobody
has written yet, and the worklist is not worth reading.

Controls: a random-ranking arm, and a shuffled-target arm that keeps the pool
but destroys the cell/target correspondence.
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
mode = [e["mode"] for e in E]
names = [e["name"] for e in E]
A = json.load(open(os.path.join(HERE, "anchors.json")))["anchors"]
AV = l2(np.array([a["vec"] for a in A], dtype=np.float32))
akey = [f"{a['domain']}/{a['mode']}" for a in A]

wp = os.path.join(HERE, "wordnet_candidates.json")
if not os.path.exists(wp):
    sys.exit("wordnet_candidates.json missing — run mapper_wordnet_export first")
W = json.load(open(wp))["synsets"]
WV = l2(np.fromfile(os.path.join(HERE, "wordnet_vecs.f32"),
                    dtype=np.float32).reshape(len(W), -1))

print("E11 — WordNet candidate mining, validated first\n")
print(f"  {n} existing entries · {len(W)} WordNet synsets as candidates/distractors")

rs = np.random.default_rng(17)
order = rs.permutation(n)
te = np.array(sorted(order[: int(0.30 * n)]))
tr = np.array(sorted(order[int(0.30 * n):]))
print(f"  train {len(tr)} · held out {len(te)}")


def target_for(key, train_idx):
    """A cell's target: its TRAIN members plus its authored anchor. Held-out
    entries never contribute, or the retrieval test would be circular."""
    ai = [i for i, a in enumerate(akey) if a == key]
    mem = [i for i in train_idx if cell[i] == key]
    parts = []
    if ai:
        parts.append(AV[ai].mean(0))
    if mem:
        parts.append(X[mem].mean(0))
    if not parts:
        return None
    return l2(np.mean(parts, axis=0)[None, :])[0]


hdr("VALIDATION — can the miner find entries it is known to want?")
print("\n  Pool = held-out entries + WordNet synsets. A cell's target is built")
print("  from TRAIN members and its anchor only. Ranking the whole pool, where")
print("  do that cell's own held-out entries land?\n")

POOL = np.vstack([X[te], WV]).astype(np.float32)
pool_is_entry = np.array([True] * len(te) + [False] * len(W))
pool_cell = [cell[i] for i in te] + [None] * len(W)
cells_with_test = sorted({cell[i] for i in te})

def evaluate(rank_fn, label):
    hits = {1: 0, 5: 0, 25: 0, 100: 0}
    total = 0
    for key in cells_with_test:
        want = [j for j in range(len(te)) if cell[te[j]] == key]
        if not want:
            continue
        ordr = rank_fn(key)
        pos = {j: r for r, j in enumerate(ordr)}
        for j in want:
            total += 1
            r = pos[j]
            for k in hits:
                if r < k:
                    hits[k] += 1
    return label, total, {k: v / max(total, 1) for k, v in hits.items()}


def rank_real(key):
    t = target_for(key, tr)
    if t is None:
        return list(range(len(POOL)))
    return list(np.argsort(-(POOL @ t)))


shuffled = {k: v for k, v in zip(cells_with_test,
                                 rs.permutation(cells_with_test))}
def rank_shuffled(key):
    return rank_real(shuffled[key])


def rank_random(key):
    return list(rs.permutation(len(POOL)))


print(f"  {'arm':<34}{'n held-out':>12}{'r@1':>8}{'r@5':>8}{'r@25':>8}{'r@100':>8}")
for fn, lbl in [(rank_real, "target from members + anchor"),
                (rank_shuffled, "CONTROL shuffled cell targets"),
                (rank_random, "CONTROL random ranking")]:
    label, total, h = evaluate(fn, lbl)
    print(f"  {label:<34}{total:>12}{h[1]:>8.3f}{h[5]:>8.3f}{h[25]:>8.3f}{h[100]:>8.3f}")
print(f"\n  chance recall@100 with a pool of {len(POOL)} is "
      f"{100/len(POOL):.4f} — anything near that is noise.")

hdr("WHAT THE MINER PROPOSES for the starved modes")
existing_names = {nm.lower() for nm in names}
sizes = Counter(cell)
msizes = Counter(mode)
starved = [m for m in msizes if msizes[m] < 30]
worklist = {}
for m in sorted(starved, key=lambda k: msizes[k]):
    keys = [k for k in sorted(set(akey)) if k.endswith("/" + m)]
    print(f"\n  {m}  (has {msizes[m]} entries across {len(keys)} cells)")
    for key in keys[:2]:
        t = target_for(key, np.arange(n))
        if t is None:
            continue
        sims = WV @ t
        # Dedupe by lemma: WordNet carries several synsets per word, and the
        # first run listed "communication" and "ruin" twice in the same cell.
        seen, picks = set(), []
        for j in np.argsort(-sims):
            lem = W[j]["lemma"].lower()
            if lem in seen or lem in existing_names:
                continue
            seen.add(lem)
            picks.append(j)
            if len(picks) >= 40:
                break
        worklist[key] = [{"lemma": W[j]["lemma"], "gloss": W[j]["gloss"],
                          "sim": round(float(sims[j]), 4)} for j in picks]
        print(f"    {key:<22} " + ", ".join(
            f'{W[j]["lemma"]} ({sims[j]:.2f})' for j in picks[:6]))

out = os.path.join(HERE, "wordnet_worklist.json")
json.dump({"note": "candidate ontology entries mined from WordNet; NOT verified",
           "target_per_cell": 20, "worklist": worklist}, open(out, "w"), indent=1)
print(f"\n  full worklist (40 per cell) -> {out}")
