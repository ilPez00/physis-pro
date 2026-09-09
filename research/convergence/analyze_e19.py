#!/usr/bin/env python3
"""E19 scoring. Reads results_e19.json; runs no inference, so it is cheap to re-run.

Reports kappa AND acceptance rates separately, because H0b -- the brief's
hypothesis producing a MORE PERMISSIVE abstraction rather than a better one --
is invisible if you only look at kappa.
"""
import json, math, os, sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
PR = {p["id"]: p for p in json.load(open(os.path.join(HERE, "probes.json")))["probes"]}
ARMS = ("NULL", "A1", "A5", "B5", "S5", "C5", "ORACLE")


def score(v):
    ok = [k for k in v if v[k] is not None]
    pos = [k for k in ok if PR[k]["label"]]
    neg = [k for k in ok if not PR[k]["label"]]
    if not pos or not neg:
        return None
    tp = sum(bool(v[k]) for k in pos)
    fp = sum(bool(v[k]) for k in neg)
    n = len(ok)
    po = (tp + len(neg) - fp) / n
    yes = tp + fp
    pe = (yes / n) * (len(pos) / n) + ((n - yes) / n) * (len(neg) / n)
    kap = (po - pe) / (1 - pe) if pe < 1 else 0.0
    # pair discrimination: pair solved iff positive in and negative out
    pairs = defaultdict(dict)
    for k in ok:
        pairs[PR[k]["pair"]][PR[k]["label"]] = bool(v[k])
    solved = sum(1 for d in pairs.values() if len(d) == 2 and d[1] and not d[0])
    return dict(kappa=kap, acc=po, pos_rate=tp / len(pos), neg_rate=fp / len(neg),
                pairs_solved=solved / len(pairs), n=n)


def mean(x):
    return sum(x) / len(x) if x else float("nan")


def sd(x):
    return math.sqrt(sum((v - mean(x)) ** 2 for v in x) / (len(x) - 1)) if len(x) > 1 else 0.0


def boot_ci(a, b, n=20000, seed=0):
    """Percentile CI on mean(a) - mean(b) by resampling seeds with replacement.

    A failed t-test says only that we could not reject equality. What a null
    result needs is a bound: is the true difference SMALLER than the smallest
    difference this instrument could resolve? That bound is the NULL->ORACLE
    dynamic range. E7 in this track recorded twelve rejections that were
    underpowered rather than refuted; this is the check that tells them apart.
    """
    import random
    rng = random.Random(seed)
    d = []
    for _ in range(n):
        ra = [a[rng.randrange(len(a))] for _ in a]
        rb = [b[rng.randrange(len(b))] for _ in b]
        d.append(mean(ra) - mean(rb))
    d.sort()
    return d[int(0.025 * n)], d[int(0.975 * n)]


def welch(a, b):
    if len(a) < 2 or len(b) < 2:
        return float("nan")
    va, vb = sd(a) ** 2 / len(a), sd(b) ** 2 / len(b)
    return (mean(a) - mean(b)) / math.sqrt(va + vb) if va + vb > 0 else float("nan")


def main():
    R = json.load(open(os.path.join(HERE, "results_e19.json")))["runs"]
    by = defaultdict(list)
    for r in R:
        s = score(r["verdicts"])
        if s:
            by[r["arm"]].append(s)

    print("E19 — cross-domain structural convergence      model:", 
          json.load(open(os.path.join(HERE, "results_e19.json")))["model"])
    print("=" * 78)
    print(f"{'arm':7s} {'n':>2s}  {'kappa':>14s}  {'pairs':>6s}  {'POS acc':>7s}  {'NEG acc':>7s}")
    for a in ARMS:
        if a not in by:
            print(f"{a:7s}  -   {'(not yet run)':>14s}")
            continue
        ks = [s["kappa"] for s in by[a]]
        print(f"{a:7s} {len(ks):2d}  {mean(ks):+.3f} +/- {sd(ks):.3f}  "
              f"{mean([s['pairs_solved'] for s in by[a]]):6.2f}  "
              f"{mean([s['pos_rate'] for s in by[a]]):7.2f}  "
              f"{mean([s['neg_rate'] for s in by[a]]):7.2f}")

    print("\nDECLARED FLOORS (preflight.py, fixed before authoring)")
    print("  length AUC 0.378 words / 0.420 chars (chance) ; tf-idf LOPO forced choice 0.750")
    if "NULL" in by and "ORACLE" in by:
        band = mean([s["kappa"] for s in by["ORACLE"]]) - mean([s["kappa"] for s in by["NULL"]])
        print(f"  instrument dynamic range NULL->ORACLE = {band:+.3f} kappa")
        print("  => no between-arm difference smaller than this band is interpretable")

    print("\nPRE-REGISTERED TESTS (Welch t on seed-level kappa)")
    for lo in ("A1", "A5", "B5", "S5"):
        if "C5" in by and lo in by:
            t = welch([s["kappa"] for s in by["C5"]], [s["kappa"] for s in by[lo]])
            verdict = "C5 higher" if t > 2 else ("C5 lower" if t < -2 else "no difference")
            print(f"  C5 vs {lo:3s}  t = {t:+6.2f}   {verdict}")
    if "C5" in by and "S5" in by:
        print("\n  S5 is THE construction-matched control: same tokens, congruence destroyed.")
        print("  If C5 beats A1 but not S5, the effect is text quantity, not congruence.")

    if "NULL" in by and "ORACLE" in by and "C5" in by:
        band = mean([s["kappa"] for s in by["ORACLE"]]) - mean([s["kappa"] for s in by["NULL"]])
        print("\nEQUIVALENCE — 95% bootstrap CI on the kappa difference")
        print(f"  instrument band (smallest resolvable difference) = {band:.3f}")
        for lo in ("A1", "A5", "S5"):
            if lo in by:
                ka = [s["kappa"] for s in by["C5"]]
                kb = [s["kappa"] for s in by[lo]]
                l, h = boot_ci(ka, kb)
                inside = abs(l) < band and abs(h) < band
                print(f"  C5 - {lo:3s}  {mean(ka)-mean(kb):+.3f}  95% CI [{l:+.3f}, {h:+.3f}]  "
                      + ("ENTIRELY INSIDE the band => equivalent at this resolution"
                         if inside else "CI extends beyond the band => underpowered, not equivalent"))

    print("\nPER-CLAUSE FALSE-ACCEPT RATE (which decoys get through)")
    clauses = sorted({p["violates"] for p in PR.values() if p["violates"]})
    print(f"  {'arm':7s} " + "  ".join(f"{c:>5s}" for c in clauses))
    for a in ARMS:
        if a not in by:
            continue
        runs = [r for r in R if r["arm"] == a]
        row = []
        for c in clauses:
            ids = [k for k in PR if PR[k]["violates"] == c]
            got = [bool(r["verdicts"].get(k)) for r in runs for k in ids
                   if r["verdicts"].get(k) is not None]
            row.append(f"{mean(got):5.2f}" if got else "    -")
        print(f"  {a:7s} " + "  ".join(row))
    print("  (1.00 = every decoy of that kind wrongly accepted)")


if __name__ == "__main__":
    main()
