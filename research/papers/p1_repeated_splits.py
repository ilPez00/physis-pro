#!/usr/bin/env python3
"""P1 1B: does the cold-start result survive partition choice?

The corrected cold-start experiment used ONE seeded 30/12 split of the 42
claude-code sessions. That measures uncertainty conditional on one partition.
This repeats the whole partition many times and reports how often each claim
holds.

Per partition:
  * shuffle the 42 target sessions, take 30 train / 12 test
  * no test session ever enters a prior
  * every arm scored on the SAME 12 test sessions
  * N swept over the TRAINING pool only

Arms:
  A  local-only      prior from the target's first N training sessions
  B  borrowed-only   prior from opencode, no target data
  C  mixed           local + borrowed counts pooled
  D  pooled-others   prior from all non-target harnesses pooled
  E  constant        the training global failure rate (one score for all kinds)

Note on E: a constant predictor has AUC exactly 0.5 (all ties), so the
"global base-rate" and "chance" arms requested are the same quantity. We
report it once and say so rather than dressing one number as two arms.

  python3 research/papers/p1_repeated_splits.py [--partitions 500]
"""
import argparse, importlib.util, json, sys
from collections import defaultdict
import numpy as np

REPO = "/home/gio/dev/physis-pro"
TARGET = "claude-code"
SOURCE = "opencode"
N_TRAIN, N_TEST = 30, 12
N_LIST = (1, 2, 3, 5, 10, 15, 20, 25, 30)
DEST = f"{REPO}/research/p1_repeated_splits.json"

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


SOURCE_STATS = cbu.key_stats(cbu.by_h[SOURCE])
OTHERS = merge(*[cbu.key_stats(cbu.by_h[h]) for h in cbu.harn if h != TARGET])
TGT = list(cbu.by_h[TARGET])


def metrics(scores, labels):
    """AUC, Brier, log loss."""
    if len(set(labels)) < 2:
        return None, None, None
    s = np.clip(scores, 1e-6, 1 - 1e-6)
    brier = float(np.mean((s - labels) ** 2))
    ll = float(-np.mean(labels * np.log(s) + (1 - labels) * np.log(1 - s)))
    return float(AUC(labels, s)), brier, ll


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--partitions", type=int, default=500)
    args = ap.parse_args()
    R = args.partitions

    print(f"target={TARGET} ({len(TGT)} sessions)  source={SOURCE}")
    print(f"partitions={R}  train/test={N_TRAIN}/{N_TEST}  N={list(N_LIST)}")

    rows = []
    for r in range(R):
        rng = np.random.default_rng(10_000 + r)
        perm = rng.permutation(len(TGT))
        train = [TGT[i] for i in perm[:N_TRAIN]]
        test = [TGT[i] for i in perm[N_TRAIN:N_TRAIN + N_TEST]]
        borrow_sc, lb, sid = cbu.collect(test, SOURCE_STATS)
        pool_sc, _, _ = cbu.collect(test, OTHERS)
        base_rate = float(np.mean(lb)) if len(lb) else 0.0
        const_sc = np.full_like(borrow_sc, base_rate)
        test_kinds = {a.get("kind", "?") for s in test for a in s.get("actions", [])}

        for N in N_LIST:
            local = cbu.key_stats(train[:N])
            a_sc, l2, _ = cbu.collect(test, local)
            c_sc, _, _ = cbu.collect(test, merge(local, SOURCE_STATS))
            auc_b, bri_b, ll_b = metrics(borrow_sc, lb)
            auc_a, bri_a, ll_a = metrics(a_sc, lb)
            auc_c, bri_c, ll_c = metrics(c_sc, lb)
            auc_d, bri_d, ll_d = metrics(pool_sc, lb)
            auc_e, bri_e, ll_e = metrics(const_sc, lb)
            covered = len(test_kinds & set(local))
            rows.append({
                "partition": r, "N": N,
                "auc_A_local": auc_a, "auc_B_borrow": auc_b,
                "auc_C_mixed": auc_c, "auc_D_pool": auc_d, "auc_E_const": auc_e,
                "brier_A": bri_a, "brier_B": bri_b, "brier_C": bri_c,
                "logloss_A": ll_a, "logloss_B": ll_b, "logloss_C": ll_c,
                "n_test_actions": int(len(lb)),
                "n_test_failures": int(lb.sum()),
                "n_test_sessions": len(test),
                "distinct_test_kinds": len(test_kinds),
                "kinds_covered_by_local": covered,
                "coverage_frac": (covered / len(test_kinds)) if test_kinds else None,
                "diff_A_minus_B": (auc_a - auc_b) if (auc_a and auc_b) else None,
                "diff_C_minus_A": (auc_c - auc_a) if (auc_c and auc_a) else None,
                "diff_D_minus_A": (auc_d - auc_a) if (auc_d and auc_a) else None,
            })
        if (r + 1) % 50 == 0:
            print(f"  {r+1}/{R} partitions", flush=True)

    def arr(key, N=None):
        return np.array([row[key] for row in rows
                         if row[key] is not None and (N is None or row["N"] == N)],
                        dtype=float)

    summary = {}
    for N in N_LIST:
        sub = [row for row in rows if row["N"] == N]
        a = np.array([r["auc_A_local"] for r in sub], dtype=float)
        b = np.array([r["auc_B_borrow"] for r in sub], dtype=float)
        c = np.array([r["auc_C_mixed"] for r in sub], dtype=float)
        d = np.array([r["auc_D_pool"] for r in sub], dtype=float)
        dab = np.array([r["diff_A_minus_B"] for r in sub], dtype=float)
        dca = np.array([r["diff_C_minus_A"] for r in sub], dtype=float)
        summary[str(N)] = {
            "n_partitions": len(sub),
            "auc_local": {"median": float(np.median(a)), "mean": float(a.mean()),
                          "lo": float(np.percentile(a, 2.5)),
                          "hi": float(np.percentile(a, 97.5))},
            "auc_borrow": {"median": float(np.median(b)), "mean": float(b.mean()),
                           "lo": float(np.percentile(b, 2.5)),
                           "hi": float(np.percentile(b, 97.5))},
            "auc_mixed": {"median": float(np.median(c)), "mean": float(c.mean())},
            "auc_pool": {"median": float(np.median(d)), "mean": float(d.mean())},
            "frac_borrow_above_chance": float(np.mean(b > 0.5)),
            "frac_local_beats_borrow": float(np.mean(dab > 0)),
            "frac_mixed_beats_local": float(np.mean(dca > 0)),
            "frac_pool_beats_local": float(np.mean(
                np.array([r["diff_D_minus_A"] for r in sub], dtype=float) > 0)),
            "diff_local_minus_borrow": {
                "median": float(np.median(dab)), "mean": float(dab.mean()),
                "lo": float(np.percentile(dab, 2.5)),
                "hi": float(np.percentile(dab, 97.5))},
            "median_coverage_frac": float(np.median(
                [r["coverage_frac"] for r in sub if r["coverage_frac"] is not None])),
        }

    # earliest N where local beats borrow in >=95% of partitions
    earliest = None
    for N in N_LIST:
        if summary[str(N)]["frac_local_beats_borrow"] >= 0.95:
            earliest = N
            break

    verdict = {
        "borrow_never_above_chance": all(
            summary[str(N)]["frac_borrow_above_chance"] < 0.5 for N in N_LIST),
        "max_frac_borrow_above_chance": max(
            summary[str(N)]["frac_borrow_above_chance"] for N in N_LIST),
        "earliest_N_local_reliably_beats_borrow": earliest,
        "local_beats_borrow_at_every_N": all(
            summary[str(N)]["frac_local_beats_borrow"] > 0.5 for N in N_LIST),
        "mixed_beats_local_consistently": all(
            summary[str(N)]["frac_mixed_beats_local"] > 0.95 for N in N_LIST),
    }

    json.dump({
        "target": TARGET, "source": SOURCE, "n_partitions": R,
        "n_train": N_TRAIN, "n_test": N_TEST, "N_list": list(N_LIST),
        "inference_unit": "session",
        "arms": {"A": "local-only", "B": "borrowed-only (opencode)",
                 "C": "mixed local+borrowed", "D": "pooled non-target harnesses",
                 "E": "constant training base rate (== chance in AUC; all ties)"},
        "summary": summary,
        "verdict": verdict,
        "per_partition": rows,
    }, open(DEST, "w"), indent=2)

    print("\n=== across partitions ===")
    print(f"{'N':>3s} {'local_med':>9s} {'borrow_med':>10s} {'mixed_med':>9s} "
          f"{'A>B frac':>9s} {'B>0.5':>6s} {'C>A frac':>9s} {'cov':>5s}")
    for N in N_LIST:
        s = summary[str(N)]
        print(f"{N:>3d} {s['auc_local']['median']:>9.3f} {s['auc_borrow']['median']:>10.3f} "
              f"{s['auc_mixed']['median']:>9.3f} {s['frac_local_beats_borrow']:>9.3f} "
              f"{s['frac_borrow_above_chance']:>6.3f} {s['frac_mixed_beats_local']:>9.3f} "
              f"{s['median_coverage_frac']:>5.2f}")
    print()
    for k, v in verdict.items():
        print(f"  {k}: {v}")
    print(f"\nwrote {DEST}")


if __name__ == "__main__":
    main()
