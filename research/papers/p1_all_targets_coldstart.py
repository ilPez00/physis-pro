#!/usr/bin/env python3
"""P1 1C: does borrowing ever help, for ANY source/target pair?

1B generalised one pair (opencode -> claude-code) across partitions. This
generalises across pairs: every target that is large enough to support a
learning curve, against every source.

Targets: claude-code (42 sessions), cline (69), opencode (288).
Codex (3 sessions) cannot support a 30/12 split and is excluded as a TARGET;
it still serves as a SOURCE.

Per (target, source, partition, N): local-only vs borrowed-only vs mixed,
all scored on the same held-out test sessions, no test session in any prior.

  python3 research/papers/p1_all_targets_coldstart.py [--partitions 200]
"""
import argparse, importlib.util, json, sys
from collections import defaultdict
import numpy as np

REPO = "/home/gio/dev/physis-pro"
N_TRAIN, N_TEST = 30, 12
N_LIST = (1, 2, 3, 5, 10, 15, 20, 25, 30)
MIN_SESSIONS = N_TRAIN + N_TEST
DEST = f"{REPO}/research/p1_all_targets_coldstart.json"

spec = importlib.util.spec_from_file_location(
    "cbu", f"{REPO}/research/papers/check_bootstrap_unit.py")
if spec is None or spec.loader is None:
    sys.exit("cannot load check_bootstrap_unit.py")
cbu = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cbu)
AUC = cbu.roc_auc_score


def merge(*priors):
    out = defaultdict(lambda: [0, 0])
    for p in priors:
        for k, v in p.items():
            out[k][0] += v[0]
            out[k][1] += v[1]
    return {k: [t, f] for k, (t, f) in out.items()}


ALL_STATS = {h: cbu.key_stats(cbu.by_h[h]) for h in cbu.harn}
TARGETS = [h for h in cbu.harn if len(cbu.by_h[h]) >= MIN_SESSIONS]
EXCLUDED = [h for h in cbu.harn if h not in TARGETS]
print(f"targets: {[(t, len(cbu.by_h[t])) for t in TARGETS]}")
print(f"excluded as targets (too few sessions): "
      f"{[(h, len(cbu.by_h[h])) for h in EXCLUDED]}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--partitions", type=int, default=200)
    args = ap.parse_args()
    R = args.partitions

    cells = []
    for target in TARGETS:
        sessions = list(cbu.by_h[target])
        for source in cbu.harn:
            if source == target:
                continue
            src_stats = ALL_STATS[source]
            for r in range(R):
                rng = np.random.default_rng(20_000 + r)
                perm = rng.permutation(len(sessions))
                train = [sessions[i] for i in perm[:N_TRAIN]]
                test = [sessions[i] for i in perm[N_TRAIN:N_TRAIN + N_TEST]]
                b_sc, lb, _ = cbu.collect(test, src_stats)
                auc_b = float(AUC(lb, b_sc)) if len(set(lb)) >= 2 else None
                for N in N_LIST:
                    local = cbu.key_stats(train[:N])
                    a_sc, l2, _ = cbu.collect(test, local)
                    c_sc, _, _ = cbu.collect(test, merge(local, src_stats))
                    auc_a = float(AUC(l2, a_sc)) if len(set(l2)) >= 2 else None
                    auc_c = float(AUC(l2, c_sc)) if len(set(l2)) >= 2 else None
                    if auc_a is None or auc_b is None:
                        continue
                    cells.append({
                        "target": target, "source": source, "partition": r, "N": N,
                        "auc_local": auc_a, "auc_borrow": auc_b, "auc_mixed": auc_c,
                        "diff_local_minus_borrow": auc_a - auc_b,
                        "diff_mixed_minus_local": auc_c - auc_a,
                    })
        print(f"  {target}: {len(cells)} cells so far", flush=True)

    def cell_arrays(target, source, N):
        sub = [c for c in cells
               if c["target"] == target and c["source"] == source and c["N"] == N]
        if not sub:
            return None
        return (np.array([c["auc_local"] for c in sub]),
                np.array([c["auc_borrow"] for c in sub]),
                np.array([c["auc_mixed"] for c in sub]),
                np.array([c["diff_local_minus_borrow"] for c in sub]),
                np.array([c["diff_mixed_minus_local"] for c in sub]))

    tensor = {}
    pair_summary = {}
    for target in TARGETS:
        for source in cbu.harn:
            if source == target:
                continue
            key = f"{target}<-{source}"
            per_n = {}
            for N in N_LIST:
                a = cell_arrays(target, source, N)
                if a is None:
                    continue
                la, lb, lc, dab, dca = a
                per_n[str(N)] = {
                    "n_partitions": len(la),
                    "auc_local_median": float(np.median(la)),
                    "auc_borrow_median": float(np.median(lb)),
                    "auc_mixed_median": float(np.median(lc)),
                    "frac_borrow_above_chance": float(np.mean(lb > 0.5)),
                    "frac_local_beats_borrow": float(np.mean(dab > 0)),
                    "frac_mixed_beats_local": float(np.mean(dca > 0)),
                    "median_diff_local_minus_borrow": float(np.median(dab)),
                }
            if not per_n:
                continue
            tensor[key] = per_n
            pair_summary[key] = {
                "target": target, "source": source,
                "max_frac_borrow_above_chance": max(
                    v["frac_borrow_above_chance"] for v in per_n.values()),
                "min_frac_local_beats_borrow": min(
                    v["frac_local_beats_borrow"] for v in per_n.values()),
                "max_frac_mixed_beats_local": max(
                    v["frac_mixed_beats_local"] for v in per_n.values()),
                "borrow_ever_helps": bool(max(
                    v["frac_borrow_above_chance"] for v in per_n.values()) > 0.5),
                "mixed_ever_consistently_helps": bool(max(
                    v["frac_mixed_beats_local"] for v in per_n.values()) > 0.95),
            }

    any_help = [k for k, v in pair_summary.items() if v["borrow_ever_helps"]]
    any_mix = [k for k, v in pair_summary.items() if v["mixed_ever_consistently_helps"]]
    verdict = {
        "pairs_tested": len(pair_summary),
        "pairs_where_borrowing_helps": any_help,
        "pairs_where_mixing_consistently_helps": any_mix,
        "no_pair_benefits": len(any_help) == 0,
        "resolution_note": ("'helps' means the borrowed prior exceeds chance in "
                            ">50% of partitions at some N; 'mixing helps' means "
                            "mixed beats local-only in >95% of partitions"),
    }

    json.dump({
        "targets": TARGETS, "excluded_targets": EXCLUDED,
        "sources": list(cbu.harn),
        "n_partitions": R, "n_train": N_TRAIN, "n_test": N_TEST,
        "N_list": list(N_LIST), "inference_unit": "session",
        "tensor": tensor, "pair_summary": pair_summary, "verdict": verdict,
        "cells": cells,
    }, open(DEST, "w"), indent=2)

    print(f"\n=== per source/target pair (AUC medians at N=30) ===")
    print(f"{'pair':26s} {'local':>7s} {'borrow':>7s} {'mixed':>7s} "
          f"{'A>B frac':>9s} {'B>0.5 max':>10s} {'C>A max':>8s}")
    for k, v in sorted(pair_summary.items()):
        last = tensor[k]["30"]
        print(f"{k:26s} {last['auc_local_median']:>7.3f} {last['auc_borrow_median']:>7.3f} "
              f"{last['auc_mixed_median']:>7.3f} {v['min_frac_local_beats_borrow']:>9.3f} "
              f"{v['max_frac_borrow_above_chance']:>10.3f} {v['max_frac_mixed_beats_local']:>8.3f}")
    print()
    for k, v in verdict.items():
        print(f"  {k}: {v}")
    print(f"\nwrote {DEST}")


if __name__ == "__main__":
    main()
