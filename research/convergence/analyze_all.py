#!/usr/bin/env python3
"""Cross-model E19 scoring. Reads every results_e19*.json; runs no inference.

The point of a multi-model read is NOT a leaderboard. E19's limit 1 was "one 4B
model, CPU-only", and its limit 3 was "ORACLE at +0.583 is a low ceiling" —
i.e. the whole result might be a small-model artifact. What settles that is
whether the CEILING and the ARM ORDERING move with scale, so both are printed
per model and nothing is pooled across models.
"""
import glob, json, os, sys
from collections import defaultdict
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyze_e19 import score, mean, sd, welch, boot_ci

ARMS = ("NULL", "A1", "A5", "B5", "B5b", "B5c", "S5", "C5", "ORACLE")
VALID = {"A1", "A5", "C5"}          # arms whose comparison to C5 is not withdrawn
HERE = os.path.dirname(os.path.abspath(__file__))


def load(path):
    d = json.load(open(path))
    by = defaultdict(list)
    for r in d["runs"]:
        s = score(r["verdicts"])
        if s:
            by[r["arm"]].append(s)
    return d["model"], by


def main():
    files = sorted(glob.glob(os.path.join(HERE, "results_e19*.json")))
    files = [f for f in files if "discarded" not in f]
    models = [load(f) for f in files]

    for model, by in models:
        print("=" * 78)
        print(f"MODEL {model}")
        print(f"{'arm':7s} {'n':>2s}  {'kappa':>15s}  {'pairs':>6s}  {'POS':>5s}  {'NEG':>5s}")
        for a in ARMS:
            if a not in by:
                continue
            ks = [s["kappa"] for s in by[a]]
            print(f"{a:7s} {len(ks):2d}  {mean(ks):+.3f} +/- {sd(ks):.3f}  "
                  f"{mean([s['pairs_solved'] for s in by[a]]):6.2f}  "
                  f"{mean([s['pos_rate'] for s in by[a]]):5.2f}  "
                  f"{mean([s['neg_rate'] for s in by[a]]):5.2f}")
        if "NULL" in by and "ORACLE" in by:
            band = mean([s["kappa"] for s in by["ORACLE"]]) - mean([s["kappa"] for s in by["NULL"]])
            print(f"  instrument band NULL->ORACLE = {band:+.3f}"
                  f"   (nothing smaller than this is interpretable)")
        for lo in ("A1", "A5", "B5c"):
            if "C5" in by and lo in by:
                ka = [s["kappa"] for s in by["C5"]]
                kb = [s["kappa"] for s in by[lo]]
                t = welch(ka, kb)
                l, h = boot_ci(ka, kb)
                print(f"  C5 vs {lo:3s}  diff {mean(ka)-mean(kb):+.3f}  t={t:+5.2f}  "
                      f"95% CI [{l:+.3f}, {h:+.3f}]")
        for withdrawn in ("S5", "B5", "B5b"):
            if withdrawn in by:
                print(f"  ({withdrawn} present but NOT a valid control — see E19b; no test printed)")

    print("=" * 78)
    print("CEILING ACROSS MODELS — the H0c / limit-3 question")
    print(f"  {'model':34s} {'NULL':>7s} {'ORACLE':>7s} {'band':>7s} {'C5':>7s}")
    for model, by in models:
        def m(a):
            return mean([s["kappa"] for s in by[a]]) if a in by else float("nan")
        print(f"  {model:34s} {m('NULL'):+7.3f} {m('ORACLE'):+7.3f} "
              f"{m('ORACLE')-m('NULL'):+7.3f} {m('C5'):+7.3f}")
    print("\n  If the ceiling does not rise with scale, E19's conclusions are not")
    print("  an artifact of the 4B model, which was its own first-listed limit.")


if __name__ == "__main__":
    main()
