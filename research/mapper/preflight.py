#!/usr/bin/env python3
"""Pre-flight feasibility checks — refute an experiment before running it.

WHAT A VERIFIER CAN AND CANNOT DO HERE
--------------------------------------
A proof assistant verifies DEDUCTIVE claims. "Will this signal predict the
outcome?" is empirical and no verifier answers it. But a specific, common and
expensive class of failure IS deductive, and this track produced several:

  SPAN        E1's divergence arms could not beat a model containing s1 and s2,
              because dz = 8.40*s1 - 20.41*s2 lies in their span. That is a
              theorem about linear algebra, decidable before any run.
  WELL-POSED  AUC is undefined with no positives; kappa degenerates when one
              class dominates; a merge that collapses to one group makes every
              metric meaningless. Decidable from the label vector alone.
  POWER       Whether the design can detect an effect of the claimed size is a
              computation, not an experiment.
  LEAKAGE     Whether a feature is a deterministic function of the label is
              decidable by fitting it.

Lean would express the SPAN lemmas in a few lines each, and a rank computation
gives the same answer with less ceremony -- so these are implemented directly
rather than in a proof assistant. The honest claim is not "we formalised the
research"; it is "the deductively-decidable failures are now checked first."

Every check returns PASS / WARN / REFUTED with the number that decides it.
"""
import numpy as np

PASS, WARN, REFUTED = "PASS", "WARN", "REFUTED"


def _fmt(status, name, msg):
    return {"status": status, "check": name, "detail": msg}


def check_span(new, baseline, tol=0.98, basis="linear"):
    """Is the proposed feature already in the span of the baseline features?

    Regress `new` on `baseline`; R^2 near 1 means any model containing the
    baseline can reproduce `new`, so the new arm cannot add information to it.
    This is the check that would have refuted E1's standardised-divergence arm
    without running the sweep.
    """
    baseline = np.asarray(baseline)
    if baseline.ndim == 1:
        baseline = baseline[:, None]
    cols = [np.ones(len(new)), *baseline.T]
    if basis == "quadratic":
        # smooth-function redundancy, not just linear: catches an arm that is a
        # well-behaved transform of the baseline rather than a linear one
        for i in range(baseline.shape[1]):
            cols.append(baseline[:, i] ** 2)
            for j in range(i + 1, baseline.shape[1]):
                cols.append(baseline[:, i] * baseline[:, j])
    B = np.column_stack(cols)
    coef, *_ = np.linalg.lstsq(B, new, rcond=None)
    resid = new - B @ coef
    ss_res = float(resid @ resid)
    ss_tot = float(((new - new.mean()) ** 2).sum())
    r2 = 1.0 - ss_res / max(ss_tot, 1e-12)
    tag = "" if basis == "linear" else " (quadratic basis)"
    if r2 >= tol:
        return _fmt(REFUTED, "span" + tag,
                    f"R^2 = {r2:.4f} on the baseline features: the arm is a linear "
                    f"function of what the baseline already has and cannot add to it")
    if r2 >= 0.90:
        return _fmt(WARN, "span" + tag,
                    f"R^2 = {r2:.4f}: mostly redundant with the baseline; expect a "
                    f"small effect at best")
    return _fmt(PASS, "span" + tag, f"R^2 = {r2:.4f}: carries variance the baseline lacks")


def check_metric_wellposed(labels, metric="auc"):
    """Is the metric even defined on this label vector?"""
    labels = list(labels)
    n = len(labels)
    if metric == "auc":
        pos = sum(bool(x) for x in labels)
        if pos == 0 or pos == n:
            return _fmt(REFUTED, "well-posed",
                        f"AUC undefined: {pos} positives of {n}")
        if min(pos, n - pos) < 10:
            return _fmt(WARN, "well-posed",
                        f"only {min(pos, n-pos)} in the minority class; AUC will be "
                        f"very noisy")
        return _fmt(PASS, "well-posed", f"{pos} positives of {n}")
    if metric == "kappa":
        from collections import Counter
        c = Counter(labels)
        share = max(c.values()) / n
        if share > 0.95:
            return _fmt(REFUTED, "well-posed",
                        f"largest class holds {share:.1%}: kappa is degenerate and "
                        f"accuracy will track the floor")
        if share > 0.60:
            return _fmt(WARN, "well-posed",
                        f"largest class holds {share:.1%}: report kappa, never accuracy")
        return _fmt(PASS, "well-posed", f"largest class holds {share:.1%}")
    return _fmt(WARN, "well-posed", f"no rule for metric '{metric}'")


def check_power_auc(n_pos, n_neg, effect=0.05, alpha=0.05):
    """Can this design detect an AUC difference of `effect`?

    Hanley-McNeil standard error for AUC, two-sided at `alpha`. Returns the
    minimum detectable difference at 80% power alongside the requested one.
    """
    if n_pos < 2 or n_neg < 2:
        return _fmt(REFUTED, "power", f"{n_pos} positives, {n_neg} negatives")
    a = 0.75
    q1 = a / (2 - a)
    q2 = 2 * a * a / (1 + a)
    se = np.sqrt((a * (1 - a) + (n_pos - 1) * (q1 - a * a)
                  + (n_neg - 1) * (q2 - a * a)) / (n_pos * n_neg))
    mde = (1.96 + 0.84) * se * np.sqrt(2)
    if effect < mde:
        return _fmt(REFUTED, "power",
                    f"minimum detectable AUC difference is {mde:.3f} at 80% power; "
                    f"the claimed effect of {effect:.3f} cannot be resolved")
    return _fmt(PASS, "power",
                f"MDE {mde:.3f} at 80% power, below the claimed {effect:.3f}")


def check_class_sparsity(labels, train_frac=0.70, floor_n=6):
    """How many classes are too small to learn, and what will they score?

    The forecast comes from E7's measured F1(n) curve, which was read off modes
    that DO have data: n=3 -> F1 ~0.06, n=6 -> ~0.10, n=10 -> ~0.21, n=20 ->
    ~0.38, n=40 -> ~0.46.
    """
    from collections import Counter
    c = Counter(labels)
    curve = [(3, 0.06), (6, 0.10), (10, 0.21), (20, 0.38), (40, 0.46), (10**9, 0.52)]
    starved = {k: v for k, v in c.items() if v * train_frac < floor_n}
    def forecast(n_tr):
        for lim, f in curve:
            if n_tr <= lim:
                return f
        return 0.52
    exp = {k: forecast(v * train_frac) for k, v in starved.items()}
    if len(starved) > len(c) / 3:
        return _fmt(REFUTED, "sparsity",
                    f"{len(starved)} of {len(c)} classes fall below {floor_n} training "
                    f"entries; forecast F1 for those is "
                    f"{np.mean(list(exp.values())):.2f} — the design measures sparsity, "
                    f"not the hypothesis")
    if starved:
        return _fmt(WARN, "sparsity",
                    f"{len(starved)} of {len(c)} classes below {floor_n} training "
                    f"entries (forecast F1 ~{np.mean(list(exp.values())):.2f}): "
                    f"{', '.join(sorted(starved, key=starved.get)[:6])}")
    return _fmt(PASS, "sparsity", f"all {len(c)} classes have >= {floor_n} training entries")


def check_leakage(feature, labels):
    """Is the feature a near-deterministic function of the label?"""
    from collections import defaultdict
    g = defaultdict(list)
    for f, y in zip(feature, labels):
        g[y].append(f)
    if len(g) < 2:
        return _fmt(WARN, "leakage", "only one class present")
    within = np.mean([np.var(v) for v in g.values() if len(v) > 1] or [0.0])
    total = float(np.var(list(feature)))
    ratio = within / max(total, 1e-12)
    if ratio < 0.02:
        return _fmt(REFUTED, "leakage",
                    f"within-class variance is {ratio:.1%} of total: the feature is "
                    f"essentially the label")
    if ratio < 0.15:
        return _fmt(WARN, "leakage", f"within-class variance {ratio:.1%} of total")
    return _fmt(PASS, "leakage", f"within-class variance {ratio:.1%} of total")


def report(checks, title):
    print(f"\n  PRE-FLIGHT: {title}")
    worst = PASS
    for c in checks:
        mark = {"PASS": "  ok ", "WARN": " warn", "REFUTED": " STOP"}[c["status"]]
        print(f"   {mark}  {c['check']:<12} {c['detail']}")
        if c["status"] == REFUTED:
            worst = REFUTED
        elif c["status"] == WARN and worst == PASS:
            worst = WARN
    verdict = {"PASS": "RUN IT", "WARN": "RUN IT, with the caveat above",
               "REFUTED": "DO NOT RUN — the result is decided before the experiment"}[worst]
    print(f"   -> {verdict}")
    return worst
