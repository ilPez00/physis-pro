#!/usr/bin/env python3
"""E14 — why does the machine find INJECTED misfilings at 0.866 and REAL ones at 0.410?

Two numbers in this track have never been reconciled:

  Stage 9   AUC 0.410 detecting the corpus's OWN misfilings — chance inside the
            confidence interval.
  E1        AUC 0.866 detecting INJECTED misfilings on the same corpus with the
            same scorer.

That gap has been carried for the whole track without explanation, and 0.866 is
the number I have been citing as the best scorer this work has. If it only holds
for errors nobody would actually make, every recommendation resting on it needs
a caveat.

THE HYPOTHESIS
--------------
Stage 12's injection moves an entry to a UNIFORMLY RANDOM other cell. A real
misfiling is a near-miss: a careful author confuses adjacent cells, not distant
ones. So the injected task may be easy for a reason that has nothing to do with
the scorer's quality.

Injection policies, from implausible to plausible:

  random        uniform over other cells        — Stage 12's original
  same-domain   a different MODE, same domain   — a mode confusion
  same-mode     a different DOMAIN, same mode   — a domain confusion
  nearest       the entry's closest alternative cell by centroid similarity
                — the most plausible error available, and the closest available
                proxy for what a careful author actually does

If AUC collapses toward 0.41 as the injection becomes plausible, the gap is
explained and the 0.866 belongs only to errors that would never be made.
"""
import json, os, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))


def hdr(t):
    print("\n" + "═" * 78); print(t); print("═" * 78)


def auc(scored):
    v = sorted(scored, key=lambda x: x[0])
    pos = sum(1 for x in v if x[1]); neg = len(v) - pos
    if pos == 0 or neg == 0:
        return float("nan")
    rank_sum, i = 0.0, 0
    while i < len(v):
        j = i
        while j + 1 < len(v) and v[j + 1][0] == v[i][0]:
            j += 1
        avg = (i + 1 + j + 1) / 2.0
        for x in v[i:j + 1]:
            if x[1]:
                rank_sum += avg
        i = j + 1
    return (rank_sum - pos * (pos + 1) / 2.0) / (pos * neg)


E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
n = len(E)
X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
dom = [e["domain"] for e in E]
mode = [e["mode"] for e in E]
cell = [f"{d}/{m}" for d, m in zip(dom, mode)]
cells = sorted(set(cell))
ci = {c: k for k, c in enumerate(cells)}

print("E14 — is 0.866 an artifact of implausible injected errors?\n")
print(f"  {n} entries · {len(cells)} occupied cells · BGE-base, the E1 scorer")

# cell centroids from the TRUE filing, for the "nearest alternative" policy
true_ctr = np.stack([l2(X[[i for i in range(n) if cell[i] == c]].mean(0, keepdims=True))[0]
                     for c in cells])
sim_to_cell = X @ true_ctr.T


def inject(policy, stride, rng):
    """Return (assigned_cell_list, injected_flags)."""
    assigned = list(cell)
    flags = [False] * n
    for i in range(n):
        if i % stride != 0:
            continue
        cur = cell[i]
        if policy == "random":
            alts = [c for c in cells if c != cur]
            pick = alts[rng.integers(len(alts))]
        elif policy == "same-domain":
            alts = [c for c in cells if c != cur and c.split("/")[0] == dom[i]]
            if not alts:
                continue
            pick = alts[rng.integers(len(alts))]
        elif policy == "same-mode":
            alts = [c for c in cells if c != cur and c.split("/")[1] == mode[i]]
            if not alts:
                continue
            pick = alts[rng.integers(len(alts))]
        elif policy == "nearest":
            order = np.argsort(-sim_to_cell[i])
            pick = None
            for k in order:
                if cells[k] != cur:
                    pick = cells[k]
                    break
            if pick is None:
                continue
        else:
            raise ValueError(policy)
        assigned[i] = pick
        flags[i] = True
    return assigned, flags


def score_all(assigned, flags):
    """E1's scorer: distance to the assigned cell's centroid, self excluded."""
    members = defaultdict(list)
    for i, c in enumerate(assigned):
        members[c].append(i)
    out = []
    for i in range(n):
        peers = [j for j in members[assigned[i]] if j != i]
        if len(peers) < 2:
            continue
        ctr = l2(X[peers].mean(0, keepdims=True))[0]
        out.append((1.0 - float(X[i] @ ctr), flags[i]))
    return out


hdr("AUC by how PLAUSIBLE the injected error is")
print("\n  Same scorer, same corpus, same injection rate. Only the choice of")
print("  WHERE a misfiled entry goes changes.\n")
print(f"  {'injection policy':<44}{'AUC':>8}{'n inj':>8}")
results = {}
for policy, label in [("random", "random cell — Stage 12's original"),
                      ("same-mode", "same MODE, different domain"),
                      ("same-domain", "same DOMAIN, different mode"),
                      ("nearest", "the entry's NEAREST alternative cell")]:
    vals, ninj = [], 0
    for seed in range(8):
        rng = np.random.default_rng(100 + seed)
        assigned, flags = inject(policy, 7, rng)
        sc = score_all(assigned, flags)
        a = auc(sc)
        if a == a:
            vals.append(a)
        ninj = sum(flags)
    results[policy] = float(np.mean(vals))
    print(f"  {label:<44}{np.mean(vals):>8.3f}{ninj:>8}")

print(f"\n  Stage 9, on the corpus's OWN misfilings:                    0.410")

hdr("Does the plausibility of the error explain the gap?")
r, nr = results["random"], results["nearest"]
print(f"\n  random cell        {r:.3f}")
print(f"  nearest cell       {nr:.3f}   drop {nr - r:+.3f}")
print(f"  Stage 9, real      0.410       remaining gap {0.410 - nr:+.3f}")
if nr < 0.55:
    print("\n  -> the gap is EXPLAINED. Once the injected error is one a careful")
    print("     author might actually make, the scorer falls to near chance and")
    print("     meets Stage 9. The 0.866 belongs to errors nobody would make.")
elif nr < r - 0.15:
    print("\n  -> plausibility explains MOST of the gap but not all of it.")
else:
    print("\n  -> plausibility does NOT explain the gap. Stage 9's 0.410 needs")
    print("     another explanation and should be re-examined.")

hdr("How far does an entry actually move under each policy?")
print("\n  mean similarity between an entry and the centroid of the cell it is")
print("  misfiled INTO — higher means a more plausible error\n")
for policy in ["random", "same-mode", "same-domain", "nearest"]:
    rng = np.random.default_rng(100)
    assigned, flags = inject(policy, 7, rng)
    sims = [float(sim_to_cell[i][ci[assigned[i]]]) for i in range(n) if flags[i]]
    own = [float(sim_to_cell[i][ci[cell[i]]]) for i in range(n) if flags[i]]
    print(f"  {policy:<16}into {np.mean(sims):.3f}   own cell {np.mean(own):.3f}   "
          f"deficit {np.mean(own) - np.mean(sims):+.3f}")
