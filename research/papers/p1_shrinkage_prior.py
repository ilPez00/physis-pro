#!/usr/bin/env python3
"""P1 1D: can cross-agent information be USED, even though naive transfer fails?

"Naive cross-agent transfer fails" and "cross-agent information contains no
usable signal" are different claims. 1B/1C test the first. This tests the
second, and it is the one worth trying to kill.

A borrowed prior may fail because it substitutes source rates for target
rates. As shrinkage it is used differently: it moves the target estimate by
an amount proportional to how little local evidence there is, and is
overruled as local evidence accumulates.

    p_target(k) = (fail_local(k) + lambda * p_source(k)) / (n_local(k) + lambda)

lambda is tuned on TRAINING data only (inner fit/tune split), never on test.

Estimators compared on the same held-out sessions:
  local        empirical local rate, global fallback
  laplace      (f+1)/(n+2), the smoothing we would use anyway
  global       the training base rate for every kind
  borrowed     the source rate, used directly (naive transfer)
  pooled       local and source raw counts added (what arm C did in 1B)
  shrinkage    the lambda-tuned estimator above

  python3 research/papers/p1_shrinkage_prior.py [--partitions 300]
"""
import argparse, importlib.util, json, sys
from collections import defaultdict
import numpy as np

REPO = "/home/gio/dev/physis-pro"
TARGETS = ["claude-code", "cline", "opencode"]
N_TRAIN, N_TEST = 30, 12
N_LIST = (1, 2, 3, 5, 10, 20, 30)
LAMBDAS = (0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0)
DEST = f"{REPO}/research/p1_shrinkage_prior.json"

spec = importlib.util.spec_from_file_location(
    "cbu", f"{REPO}/research/papers/check_bootstrap_unit.py")
if spec is None or spec.loader is None:
    sys.exit("cannot load check_bootstrap_unit.py")
cbu = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cbu)
AUC = cbu.roc_auc_score


def score_actions(test, scoredict, default):
    sc, lb, sid = [], [], []
    for i, s in enumerate(test):
        for a in s.get("actions", []):
            sc.append(scoredict.get(a.get("kind", "?"), default))
            lb.append(1 if a.get("marker_error") else 0)
            sid.append(i)
    return np.array(sc), np.array(lb), np.array(sid)


def counts(sessions):
    c = defaultdict(lambda: [0, 0])
    for s in sessions:
        for a in s.get("actions", []):
            c[a.get("kind", "?")][0] += 1
            if a.get("marker_error"):
                c[a.get("kind", "?")][1] += 1
    return {k: (t, f) for k, (t, f) in c.items()}


def base_rate(sessions):
    t = f = 0
    for s in sessions:
        for a in s.get("actions", []):
            t += 1
            if a.get("marker_error"):
                f += 1
    return (f / t) if t else 0.0


def logloss(scores, labels):
    s = np.clip(scores, 1e-6, 1 - 1e-6)
    return float(-np.mean(labels * np.log(s) + (1 - labels) * np.log(1 - s)))


def src_rates(src_counts, fallback):
    return {k: (f / t if t else fallback) for k, (t, f) in src_counts.items()}


def est_scores(kind, lc, sr, g, lam=None):
    """Return {kind: score} for a named estimator."""
    out = {}
    kinds = set(lc) | set(sr)
    for k in kinds:
        t, f = lc.get(k, (0, 0))
        ps = sr.get(k, g)
        if kind == "local":
            out[k] = (f / t) if t else g
        elif kind == "laplace":
            out[k] = (f + 1) / (t + 2) if t else g
        elif kind == "global":
            out[k] = g
        elif kind == "borrowed":
            out[k] = ps
        elif kind == "shrinkage":
            out[k] = (f + lam * ps) / (t + lam)
        elif kind == "shrinkage_global":
            # the decisive control: shrink toward the TARGET's own global rate
            # instead of toward the source. Same form, same lambda-tuning
            # protocol -- so any difference is attributable to the source
            # prior and not to shrinkage as such.
            out[k] = (f + lam * g) / (t + lam)
        elif kind == "pooled":
            ts, fs = SRC_COUNTS.get(k, (0, 0))
            out[k] = ((f + fs) / (t + ts)) if (t + ts) else g
    return out


SRC_COUNTS: dict = {}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--partitions", type=int, default=300)
    args = ap.parse_args()
    R = args.partitions

    global SRC_COUNTS
    all_counts = {h: counts(cbu.by_h[h]) for h in cbu.harn}

    rows = []
    lam_choices = defaultdict(int)
    for target in TARGETS:
        sessions = list(cbu.by_h[target])
        srcs = [h for h in cbu.harn if h != target]
        for r in range(R):
            rng = np.random.default_rng(30_000 + r)
            perm = rng.permutation(len(sessions))
            train = [sessions[i] for i in perm[:N_TRAIN]]
            test = [sessions[i] for i in perm[N_TRAIN:N_TRAIN + N_TEST]]
            src = srcs[r % len(srcs)]                    # cycle sources
            SRC_COUNTS = all_counts[src]
            for N in N_LIST:
                loc_sessions = train[:N]
                lc = counts(loc_sessions)
                g_tune = base_rate(train)                # training-only fallback
                sr = src_rates(SRC_COUNTS, g_tune)

                # --- tune lambda on an INNER split of the training pool only
                fit, tune = train[:max(1, N_TRAIN - 10)], train[max(1, N_TRAIN - 10):]
                ft_counts = counts(fit)
                g_fit = base_rate(fit)
                best, best_ll = LAMBDAS[0], float("inf")
                for lam in LAMBDAS:
                    sc_t, lb_t, _ = score_actions(
                        tune, est_scores("shrinkage", ft_counts, sr, g_fit, lam), g_fit)
                    if len(set(lb_t)) < 2:
                        continue
                    ll = logloss(sc_t, lb_t)
                    if ll < best_ll:
                        best, best_ll = lam, ll
                # the control gets its OWN tuned lambda, so the comparison is
                # between two equally-tuned estimators and not between a tuned
                # and an untuned one
                best_g, best_gll = LAMBDAS[0], float("inf")
                for lam in LAMBDAS:
                    sc_t, lb_t, _ = score_actions(
                        tune, est_scores("shrinkage_global", ft_counts, sr, g_fit, lam),
                        g_fit)
                    if len(set(lb_t)) < 2:
                        continue
                    ll = logloss(sc_t, lb_t)
                    if ll < best_gll:
                        best_g, best_gll = lam, ll
                lam_choices[(target, str(best))] += 1

                row = {"target": target, "source": src, "partition": r, "N": N,
                       "lambda_selected": best, "lambda_selected_global": best_g}
                for kind, lam in (("local", None), ("laplace", None), ("global", None),
                                  ("borrowed", None), ("pooled", None),
                                  ("shrinkage", best),
                                  ("shrinkage_global", best_g)):
                    sd = est_scores(kind, lc, sr, g_tune, lam)
                    # global is constant -> AUC undefined; keep its log loss
                    sc, lb, _ = score_actions(test, sd, g_tune)
                    row[f"auc_{kind}"] = (float(AUC(lb, sc))
                                          if len(set(lb)) >= 2 and kind != "global" else None)
                    row[f"ll_{kind}"] = logloss(sc, lb)
                row["diff_shrink_minus_local"] = (
                    row["auc_shrinkage"] - row["auc_local"]
                    if row["auc_shrinkage"] is not None and row["auc_local"] is not None
                    else None)
                row["diff_shrink_minus_laplace"] = (
                    row["auc_shrinkage"] - row["auc_laplace"]
                    if row["auc_shrinkage"] is not None and row["auc_laplace"] is not None
                    else None)
                # THE decisive contrast: source-shrinkage vs global-shrinkage
                row["diff_shrink_minus_global"] = (
                    row["auc_shrinkage"] - row["auc_shrinkage_global"]
                    if row["auc_shrinkage"] is not None
                    and row["auc_shrinkage_global"] is not None else None)
                rows.append(row)

    def col(kind, N=None):
        return np.array([r[kind] for r in rows
                         if r[kind] is not None and (N is None or r["N"] == N)],
                        dtype=float)

    summary = {}
    for N in N_LIST:
        sub = [r for r in rows if r["N"] == N]
        e = {}
        for kind in ("local", "laplace", "borrowed", "shrinkage", "shrinkage_global"):
            a = np.array([r[f"auc_{kind}"] for r in sub if r[f"auc_{kind}"] is not None])
            e[f"auc_{kind}_median"] = float(np.median(a)) if len(a) else None
        for key in ("diff_shrink_minus_local", "diff_shrink_minus_laplace",
                    "diff_shrink_minus_global"):
            d = np.array([r[key] for r in sub if r[key] is not None])
            e[key] = {
                "median": float(np.median(d)) if len(d) else None,
                "lo": float(np.percentile(d, 2.5)) if len(d) else None,
                "hi": float(np.percentile(d, 97.5)) if len(d) else None,
                "frac_positive": float(np.mean(d > 0)) if len(d) else None,
            }
        for kind in ("local", "laplace", "borrowed", "shrinkage"):
            l = np.array([r[f"ll_{kind}"] for r in sub])
            e[f"logloss_{kind}_median"] = float(np.median(l)) if len(l) else None
        summary[str(N)] = e

    shrink_wins = all(
        summary[str(N)]["diff_shrink_minus_local"]["frac_positive"] > 0.5
        for N in N_LIST)
    lam_top = sorted(lam_choices.items(), key=lambda t: -t[1])[:6]

    verdict = {
        "shrinkage_beats_local_in_majority_of_splits": bool(shrink_wins),
        "shrinkage_beats_local_at_every_N": bool(all(
            summary[str(N)]["diff_shrink_minus_local"]["median"] > 0
            for N in N_LIST)),
        "shrinkage_beats_laplace_at_every_N": bool(all(
            summary[str(N)]["diff_shrink_minus_laplace"]["median"] > 0
            for N in N_LIST)),
        # DECISIVE: source-shrinkage vs global-shrinkage, equally tuned. If the
        # source version does not beat the target's own global rate, the source
        # prior carries no signal that ordinary target-level shrinkage lacks.
        "source_shrinkage_beats_global_shrinkage_at_every_N": bool(all(
            summary[str(N)]["diff_shrink_minus_global"]["median"] > 0
            for N in N_LIST)),
        "source_shrinkage_beats_global_shrinkage_in_majority_of_splits": bool(all(
            summary[str(N)]["diff_shrink_minus_global"]["frac_positive"] > 0.5
            for N in N_LIST)),
        "source_minus_global_median_by_N": {
            str(N): summary[str(N)]["diff_shrink_minus_global"]["median"]
            for N in N_LIST},
        "source_minus_global_frac_positive_by_N": {
            str(N): summary[str(N)]["diff_shrink_minus_global"]["frac_positive"]
            for N in N_LIST},
        "lambda_selected_most_often": lam_top,
        "interpretation_rule": (
            "source-vs-global is the test that matters. Beating LAPLACE only "
            "shows the source prior beats shrinking toward 0.5, which the "
            "target's own base rate also does. Beating GLOBAL-SHRINKAGE, with "
            "its own tuned lambda, is what shows the source carries signal the "
            "target cannot supply about itself."),
    }

    json.dump({
        "targets": TARGETS, "n_partitions": R, "n_train": N_TRAIN, "n_test": N_TEST,
        "N_list": list(N_LIST), "lambda_grid": list(LAMBDAS),
        "inference_unit": "session",
        "lambda_tuning": "inner fit/tune split of the TRAINING pool only; "
                          "lambda minimising tune-set log loss is applied to test",
        "summary": summary, "verdict": verdict, "per_partition": rows,
    }, open(DEST, "w"), indent=2)

    print(f"=== AUC medians (targets={TARGETS}, {R} partitions each) ===")
    print(f"{'N':>3s} {'local':>7s} {'laplace':>8s} {'borrow':>7s} {'shrink':>7s} "
          f"{'sh_glob':>8s} {'s-local':>8s} {'s-lapl':>7s} {'s-glob':>7s} {'frac>0':>7s}")
    for N in N_LIST:
        s = summary[str(N)]
        dl = s["diff_shrink_minus_local"]
        dp = s["diff_shrink_minus_laplace"]
        dg = s["diff_shrink_minus_global"]
        print(f"{N:>3d} {s['auc_local_median']:>7.3f} {s['auc_laplace_median']:>8.3f} "
              f"{s['auc_borrowed_median']:>7.3f} {s['auc_shrinkage_median']:>7.3f} "
              f"{s['auc_shrinkage_global_median']:>8.3f} "
              f"{dl['median']:>+8.3f} {dp['median']:>+7.3f} {dg['median']:>+7.3f} "
              f"{dg['frac_positive']:>7.3f}")
    print()
    for k, v in verdict.items():
        print(f"  {k}: {v}")
    print(f"\nwrote {DEST}")


if __name__ == "__main__":
    main()
