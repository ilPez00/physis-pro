#!/usr/bin/env python3
"""E24 — one representation per axis, instead of one for both.

E22 measured that the corpus's hints help `domain` (+0.022, t=+3.13) and hurt
`mode` (-0.034, t=-5.39). The shipped `vec` bakes hints in for BOTH axes, so it
is simultaneously too domain-loaded for mode and, if hints help domain, not
domain-loaded enough for domain.

Test: v(alpha) = normalise( vec_name + alpha * (vec - vec_name) )

  alpha = 0   name only, hints filtered out entirely
  alpha = 1   the shipped representation
  alpha > 1   hints propagated FURTHER

The prediction being tested is that the optimum differs by axis: near 0 for mode,
above 1 for domain. If one alpha is best for both, the split is not worth the
complexity and this should say so.
"""
import json, os, random, statistics, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import run_e21 as E

HERE = os.path.dirname(os.path.abspath(__file__))


def norm(v):
    n = sum(x * x for x in v) ** 0.5 or 1.0
    return [x / n for x in v]


def main():
    corpus = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    by = {e["name"]: e for e in corpus}
    rows = E.load()
    alphas = (0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0)

    print("E24 — per-axis hint weighting.  v(a) = norm(name + a*(hints_delta))")
    print(f"  {len(rows)} blind entries;  a=1.0 is the shipped vector\n")
    print(f"  {'alpha':>6s} " + "".join(f"{('domain' if ax=='domain' else 'mode'):>18s}"
                                        for ax in ("domain", "mode")))
    best = {}
    for a in alphas:
        cells = []
        for axis in ("domain", "mode"):
            i = 0 if axis == "domain" else 1
            sub = []
            for r in rows:
                e = by[r["name"]]
                v = norm([n_ + a * (h - n_) for n_, h in zip(e["vec_name"], e["vec"])])
                sub.append(dict(r, new=r["new"].split("/")[i], v=v))
            accs = []
            for s in range(24):
                rng = random.Random(4000 + s)
                idx = list(range(len(sub)))
                rng.shuffle(idx)
                n = int(0.75 * len(sub))
                tr = [sub[j] for j in idx[:n]]
                te = [sub[j] for j in idx[n:]]
                cent, _ = E.centroids(tr)
                accs.append(sum(E.rank_cells(r["v"], cent)[0] == r["new"] for r in te) / len(te))
            m, sd = statistics.mean(accs), statistics.stdev(accs)
            cells.append(f"{m:.3f} +/- {sd:.3f}")
            best.setdefault(axis, []).append((m, a))
        print(f"  {a:6.2f} " + "".join(f"{c:>18s}" for c in cells))

    print()
    for axis in ("domain", "mode"):
        m, a = max(best[axis])
        base = dict((aa, mm) for mm, aa in best[axis])[1.0]
        print(f"  {axis:7s} best alpha = {a:<4} top-1 {m:.3f}"
              f"   vs shipped(a=1) {base:.3f}   gain {m-base:+.3f}")


if __name__ == "__main__":
    main()
