#!/usr/bin/env python3
"""E22 — what the confusions say the axes should be.

E21 showed the machine carries the domain axis (top-2 0.837 over 5) and not the
mode axis (0.678 over 14). "Refine the modes" is the right response, but a
re-brainstorm from a blank page repeats whatever intuition produced the current
14. This asks the corpus instead:

  1. POPULATION   which classes are starved. E7: n ~ 20 -> F1 0.26-0.49, so a
                  class under ~20 entries is not weak, it is unmeasured.
  2. CONFUSION    which classes the proposer cannot tell apart. Two modes that
                  are systematically swapped are one mode wearing two names, and
                  merging them is a data-driven proposal rather than a taste call.
  3. SYMMETRY     confusion that runs both ways (A->B and B->A) means overlap;
                  one-way confusion means one class is absorbing another.

Same human/machine split as E21, one level up: the machine proposes the grid
revision, the person decides. It does not edit GRID_AXES.md.
"""
import json, os, random, sys
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from run_e21 import load, centroids, rank_cells


def confusion(rows, axis, seeds=8, frac=0.75):
    """Held-out top-1 confusion, aggregated over shuffles."""
    i = 0 if axis == "domain" else 1
    sub = [dict(r, new=r["new"].split("/")[i]) for r in rows]
    conf = Counter()
    tot = Counter()
    for s in range(seeds):
        rng = random.Random(s)
        idx = list(range(len(sub)))
        rng.shuffle(idx)
        n = int(frac * len(sub))
        tr = [sub[j] for j in idx[:n]]
        te = [sub[j] for j in idx[n:]]
        cent, _ = centroids(tr)
        for r in te:
            pred = rank_cells(r["v"], cent)[0]
            conf[(r["new"], pred)] += 1
            tot[r["new"]] += 1
    return conf, tot


def report(rows, axis):
    i = 0 if axis == "domain" else 1
    pop = Counter(r["new"].split("/")[i] for r in rows)
    conf, tot = confusion(rows, axis)
    print(f"\n{'='*74}\n{axis.upper()} — {len(pop)} classes, {sum(pop.values())} entries")

    print(f"\n  POPULATION (E7's usable threshold is about 20)")
    for c, n in pop.most_common():
        acc = conf[(c, c)] / tot[c] if tot[c] else float("nan")
        flag = "  <-- STARVED" if n < 20 else ""
        print(f"    {c:14s} n={n:4d}   held-out top-1 {acc:5.2f}{flag}")

    print(f"\n  CONFUSION PAIRS — where the axis does not separate")
    off = [(a, b, n) for (a, b), n in conf.items() if a != b]
    off.sort(key=lambda t: -t[2])
    seen = set()
    for a, b, n in off[:14]:
        back = conf[(b, a)]
        rate_ab = n / tot[a] if tot[a] else 0
        rate_ba = back / tot[b] if tot[b] else 0
        kind = "MUTUAL — candidates to merge" if rate_ba >= 0.5 * rate_ab and back else \
               f"ONE-WAY — {a} is being absorbed into {b}"
        key = tuple(sorted((a, b)))
        if key in seen:
            continue
        seen.add(key)
        print(f"    {a:14s} -> {b:14s}  {rate_ab:5.2f}  (reverse {rate_ba:5.2f})  {kind}")


def main():
    rows = load()
    print(f"E22 — axis structure from the corpus.  {len(rows)} blind entries")
    print("  Proposals only. GRID_AXES.md is not touched by this script.")
    for axis in ("domain", "mode"):
        report(rows, axis)
    print(f"\n{'='*74}")
    print("""HOW TO READ THIS
  A STARVED class is not evidence the distinction is wrong — E7 showed a
  well-populated mode cut to 3 entries scores F1 0.06. It is evidence the class
  has never been tested. Populate it or drop it; do not judge it.
  A MUTUAL confusion pair is the real merge candidate: the corpus does not
  distinguish them, in either direction.
  A ONE-WAY confusion means the absorbing class is the corpus's default, which
  is usually a sign its definition is too broad rather than that the other is wrong.""")


if __name__ == "__main__":
    main()
