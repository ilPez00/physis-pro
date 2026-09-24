#!/usr/bin/env python3
"""Paper 10 kill test: is the critical-band cluster-size distribution
actually power-law, or merely heavy-tailed?

Portfolio's stated kill condition: "If the power-law fit is not
significantly better than lognormal, the 'criticality' claim is
unfounded."

Method (Clauset, Shalizi & Newman 2009):
  - choose xmin by minimising the KS distance between data and the fitted
    discrete power law
  - fit the discrete power-law exponent by MLE
  - fit a lognormal to the same tail
  - compare with the Vuong log-likelihood-ratio test

Input : research/empedocles_multiseed.json (from examples/empedocles_multiseed.rs)
Output: research/empedocles_fits.json
"""
import json, os, sys
import numpy as np
from scipy.optimize import minimize_scalar
from scipy.special import zeta
from scipy.stats import lognorm, norm

REPO = "/home/gio/dev/physis-pro"


def pl_loglik(data, alpha, xmin):
    """Discrete power-law log-likelihood (normalised by zeta(alpha, xmin))."""
    return -alpha * np.sum(np.log(data)) - len(data) * np.log(zeta(alpha, xmin))


def fit_pl(data, xmin):
    """MLE of the discrete power-law exponent above xmin."""
    tail = data[data >= xmin]
    if len(tail) < 10:
        return None, tail
    r = minimize_scalar(
        lambda a: -pl_loglik(tail, a, xmin), bounds=(1.01, 5.0), method="bounded"
    )
    return float(r.x), tail


def ks_distance(data, xmin, alpha):
    tail = np.sort(data[data >= xmin])
    if len(tail) == 0:
        return np.inf
    n = len(tail)
    cdf_emp = np.arange(1, n + 1) / n
    # discrete CDF of the power law above xmin
    xs = tail
    cdf_theo = 1 - zeta(alpha, xs) / zeta(alpha, xmin)
    return float(np.max(np.abs(cdf_emp - cdf_theo)))


def fit_lognormal(tail):
    shape, loc, scale = lognorm.fit(tail, floc=0)
    return shape, loc, scale


def vuong(data, alpha, xmin, ln_params):
    """Return (R, p). R>0 favours power law."""
    tail = data[data >= xmin]
    shape, loc, scale = ln_params
    ll_pl = -alpha * np.log(tail) - np.log(zeta(alpha, xmin))
    ll_ln = lognorm.logpdf(tail, shape, loc=loc, scale=scale)
    d = ll_pl - ll_ln
    n = len(d)
    if n < 10 or np.std(d) == 0:
        return None, None
    R = float(np.sum(d))
    p = float(2 * norm.sf(abs(R) / (np.sqrt(n) * np.std(d))))
    return R, p


def fit_sample(sizes):
    """Run the full pipeline on one sample of cluster sizes.

    Extracted so that control systems can be pushed through byte-identical
    code rather than a reimplementation. A control that runs a *different*
    pipeline proves nothing about the instrument that produced the Empedocles
    result.
    """
    entry: dict = {"n_clusters": int(len(sizes))}
    if len(sizes) < 50:
        entry["fit"] = None
        entry["note"] = "too few clusters to fit"
        return entry, "too few clusters"

    best = None
    for xmin in sorted(set(sizes)):
        if xmin < 2:
            continue
        alpha, tail = fit_pl(sizes, xmin)
        if alpha is None:
            continue
        d = ks_distance(sizes, xmin, alpha)
        if best is None or d < best[2]:
            best = (xmin, alpha, d)
    if best is None:
        entry["fit"] = None
        entry["note"] = "no xmin with a fittable tail"
        return entry, "unfittable"

    xmin, alpha, ks = best
    tail = sizes[sizes >= xmin]
    try:
        ln_params = fit_lognormal(tail)
        R, p = vuong(sizes, alpha, xmin, ln_params)
    except Exception as e:            # noqa: BLE001
        ln_params, R, p = None, None, None
        entry["fit_error"] = str(e)

    if R is None:
        verdict = "undetermined"
    elif R > 0 and p is not None and p < 0.05:
        verdict = "POWER LAW beats lognormal"
    elif R < 0 and p is not None and p < 0.05:
        verdict = "lognormal beats power law"
    else:
        verdict = "no difference (indistinguishable)"

    entry["fit"] = {"xmin": int(xmin), "alpha": float(alpha), "ks": float(ks),
                    "n_tail": int(len(tail)), "vuong_R": R, "vuong_p": p,
                    "verdict": verdict}
    if ln_params is not None:
        entry["fit"]["lognormal_shape"] = float(ln_params[0])
    return entry, verdict


def main():
    src = (sys.argv[1] if len(sys.argv) > 1
           else f"{REPO}/research/empedocles_multiseed.json")
    runs = json.load(open(src))
    results = {"source": os.path.basename(src), "settings": {}}

    by_nem = {}
    for r in runs:
        by_nem.setdefault(r["nemesis"], []).append(r)

    print(f"{'nemesis':>8s} {'runs':>5s} {'n_clusters':>11s} {'xmin':>5s} "
          f"{'alpha':>7s} {'KS':>6s} {'R':>9s} {'p':>8s}  verdict")
    print("-" * 78)

    for nem in sorted(by_nem):
        runs_n = by_nem[nem]
        sizes = np.array([s for r in runs_n for s in r["sizes"] if s > 0],
                         dtype=float)
        largest = np.mean([r["largest"] for r in runs_n])
        entry, verdict = fit_sample(sizes)
        entry["n_runs"] = len(runs_n)
        entry["mean_largest"] = float(largest)
        entry["regimes"] = sorted({r["regime"] for r in runs_n})
        results["settings"][f"{nem}"] = entry

        f = entry.get("fit")
        if not f:
            print(f"{nem:>8.3f} {len(runs_n):>5d} {len(sizes):>11d}  — {verdict}")
            continue
        r_txt = "—" if f["vuong_R"] is None else f"{round(f['vuong_R'], 1)}"
        p_txt = "—" if f["vuong_p"] is None else f"{round(f['vuong_p'], 4)}"
        print(f"{nem:>8.3f} {len(runs_n):>5d} {len(sizes):>11d} {f['xmin']:>5d} "
              f"{f['alpha']:>7.3f} {f['ks']:>6.3f} {r_txt:>9} {p_txt:>8}  {verdict}")

    dest = f"{REPO}/research/empedocles_fits.json"
    json.dump(results, open(dest, "w"), indent=2)
    print(f"\nwrote {dest}")


if __name__ == "__main__":
    main()
