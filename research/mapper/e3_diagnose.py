#!/usr/bin/env python3
"""E3 diagnostics — two checks before attributing the density collapse.

CHECK 1  Leg B's Qwen rows printed identically with and without extra nodes.
         Either the codebase vertices are genuinely disconnected from the
         labelled block, or they were never wired in. Those are different
         findings and one of them is a bug.

CHECK 2  The union arm changed TWO things at once: relations are denser, and
         the vocabulary is different (2,357 ontology-drawn tokens -> the 8,000
         most frequent over a corpus of philosophy and Rust). The collapse
         could be either. Holding the vocabulary FIXED at the original 2,357
         and estimating PPMI on the union separates them.
"""
import json, os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import (QwenDictionary, build_counts, ppmi, svd_reduce, l2,
                    grounding_curve, build_graph)

HERE = os.path.dirname(os.path.abspath(__file__))
tokpath = sys.argv[1]

lab = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(lab)
labels = [f"{e['domain']}/{e['mode']}" for e in lab]
texts = [" ".join([e["name"]] + e["hints"]) for e in lab]
BGE = l2(np.array([e["vec"] for e in lab], dtype=np.float32))

sc = json.load(open(os.path.join(HERE, "selfcorpus.json")))["chunks"]
stexts = [c["text"] for c in sc]
SV = l2(np.fromfile(os.path.join(HERE, "selfcorpus_vecs.f32"),
                    dtype=np.float32).reshape(len(sc), -1))
draw = [json.loads(l) for l in open(os.path.join(HERE, ".docs_chunks.jsonl"))]
dmeta = json.load(open(os.path.join(HERE, "docs_corpus.json")))["chunks"]
if len(draw) != len(dmeta):
    step = len(draw) // len(dmeta) + 1
    draw = draw[::step][: len(dmeta)]
dtexts = [r["text"] for r in draw]

from tokenizers import Tokenizer
D = QwenDictionary(tokpath)
tk = Tokenizer.from_file(tokpath)
q_lab = [tk.encode(t).ids for t in texts]
bg = stexts + dtexts
q_bg = [tk.encode(t).ids for t in bg]

print("CHECK 1 — are the extra graph vertices actually connected?\n")
EXTRA = 4000
for name, rep_lab, extra in [("BGE", BGE, SV[:: max(1, len(SV)//EXTRA + 1)][:EXTRA])]:
    rep_all = np.vstack([rep_lab, extra]).astype(np.float32)
    W = build_graph(rep_all, knn=10)
    cross = (W[:n, n:] > 0).sum()
    within = (W[:n, :n] > 0).sum()
    print(f"  {name}: labelled->extra edges {cross:,}   labelled->labelled {within:,}"
          f"   cross share {cross/max(cross+within,1):.1%}")

# the Qwen union representation, same construction as run_e3
M_all, keep_b, idx_b = build_counts(q_lab + q_bg, min_df=2, max_features=8000)
wb = np.array([D.rank_weight(t) for t in keep_b], dtype=np.float32)
P1b = ppmi(M_all)
qlab_rep = l2(svd_reduce(l2((M_all[:n]*wb) @ P1b), 256, 17))
M_chunks = M_all[n:]
step = len(M_chunks)//EXTRA + 1
M_chunks = M_chunks[::step][:EXTRA]
qext = l2(svd_reduce(l2((M_chunks*wb) @ P1b), 256, 17))
rep_all = np.vstack([qlab_rep, qext]).astype(np.float32)
W = build_graph(rep_all, knn=10)
cross = (W[:n, n:] > 0).sum(); within = (W[:n, :n] > 0).sum()
print(f"  Qwen L1 union: labelled->extra edges {cross:,}   labelled->labelled {within:,}"
      f"   cross share {cross/max(cross+within,1):.1%}")
print("\n  -> if the cross share is ~0 the extra vertices are isolated and Leg B's")
print("     null for that arm is a property of the representation, not a bug.\n")

print("CHECK 2 — density vs vocabulary displacement, separated\n")
rs = np.random.default_rng(17)
order = rs.permutation(n)
test_idx = np.array(sorted(order[:int(0.30*n)]))
train_pool = np.array(sorted(order[int(0.30*n):]))
ks = [20, 80, 320, len(train_pool)]

# (a) original vocabulary, PPMI from 658 entries      -- E2's arm
M_s, keep_s, idx_s = build_counts(q_lab, min_df=2)
ws = np.array([D.rank_weight(t) for t in keep_s], dtype=np.float32)
P1s = ppmi(M_s)

# (b) original vocabulary, PPMI estimated on the UNION -- density only
cols = [idx_b[t] for t in keep_s if t in idx_b]
rows_keep = [t for t in keep_s if t in idx_b]
sub = np.array([idx_s[t] for t in rows_keep])
M_s_sub = M_s[:, sub]
ws_sub = np.array([D.rank_weight(t) for t in rows_keep], dtype=np.float32)
Mall_sub = M_all[:, cols]
P1_union_sub = ppmi(Mall_sub)
print(f"  vocabulary held fixed at {len(rows_keep):,} of the original {len(keep_s):,} tokens")
print(f"  (the rest fall outside the union's 8,000-token cap)\n")

arms = {
  "(a) orig vocab, PPMI from 658  [E2]": l2(svd_reduce(l2((M_s_sub*ws_sub) @ ppmi(M_s_sub)), 256, 17)),
  "(b) orig vocab, PPMI from union": l2(svd_reduce(l2((M_s_sub*ws_sub) @ P1_union_sub), 256, 17)),
  "(c) union vocab, PPMI from union": qlab_rep,
  "    BGE reference":               BGE,
}
print(f"  {'arm':<38}" + "".join(f"{k:>8}" for k in ks))
for nm, rep in arms.items():
    r = grounding_curve(rep, labels, train_pool, test_idx, ks, reps=10, knn=5, seed=17)
    print(f"  {nm:<38}" + "".join(f"{x[1]:>8.3f}" for x in r))
print("\n  (a)->(b) isolates DENSITY: same features, better-estimated relations.")
print("  (b)->(c) isolates VOCABULARY: the features themselves change.")
