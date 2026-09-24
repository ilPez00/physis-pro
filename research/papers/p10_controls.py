#!/usr/bin/env python3
"""P10: can this pipeline detect criticality where criticality is KNOWN?

Paper 10 says the Empedoclean node field is not power-law distributed. That is
only informative if the same pipeline detects a power law when one is really
there. A test that cannot fire is not a falsification, it is silence.

This runs known-critical and known-non-critical systems through
`fit_empedocles.fit_sample` -- the *same* code that produced the Empedocles
verdict, imported rather than reimplemented. A control on a different pipeline
would prove nothing about this instrument.

Controls
--------
POSITIVE (a power law is the truth, and the exponent is known analytically):
  * critical branching process, Poisson(1) offspring. Progeny distribution has
    exponent alpha = 3/2 in the mean-field limit.
  * 2D site percolation at p_c ~ 0.5927. Cluster-size exponent tau = 187/91
    ~= 2.055 (with known finite-size corrections).

NEGATIVE (no power law):
  * subcritical branching, mean offspring 0.7 -- exponential tail.
  * supercritical branching, mean offspring 1.3 -- giant cluster plus a small
    finite-component tail.
  * lognormal sizes -- lognormal is the literal truth, so a pipeline that
    prefers the power law here is over-eager.

Sensitivity is reported as the FRACTION OF SEEDS on which the pipeline returns
"POWER LAW beats lognormal", not as one seed's verdict: the Empedocles analysis
already found this pipeline to be seed-unstable, so a single seed would be
measuring luck.

REPRODUCTION: python3 research/papers/p10_controls.py --seeds 12
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time

import numpy as np

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(REPO, "research", "papers"))
from fit_empedocles import fit_sample          # noqa: E402  the real instrument

OUT = os.path.join(REPO, "research", "p10_controls.json")


# ------------------------------------------------------------- generators

def branching_sizes(rng, mean_offspring, n_clusters=6000, cap=200_000):
    """Galton-Watson total progeny per cluster.

    Critically, mean 1 is the case with a power-law progeny distribution. The
    cap bounds the rare enormous clusters; without it a critical run can wander
    for a long time before going extinct.
    """
    sizes = np.empty(n_clusters, dtype=np.int64)
    for i in range(n_clusters):
        total, living = 1, 1
        while living > 0 and total < cap:
            kids = rng.poisson(mean_offspring, size=living).sum()
            living = int(kids)
            total += living
        sizes[i] = total
    return sizes.astype(float)


def percolation_sizes(rng, L=160, p=0.5927):
    """2D site percolation cluster sizes at (near) p_c."""
    from scipy import ndimage
    occ = rng.random((L, L)) < p
    lab, n = ndimage.label(occ)                    # 4-neighbour, standard for site
    if n == 0:
        return np.array([1.0])
    return np.bincount(lab.ravel())[1:].astype(float)


def lognormal_sizes(rng, n=6000, mu=1.0, sigma=1.2):
    return np.round(rng.lognormal(mu, sigma, size=n)).astype(float) + 1.0


def run_control(name, gen, seeds, **kw):
    rows = []
    for s in range(seeds):
        rng = np.random.default_rng(10_000 + s)
        sizes = gen(rng, **kw)
        sizes = sizes[sizes > 0]
        t0 = time.time()
        entry, verdict = fit_sample(sizes)
        fit = entry.get("fit") or {}
        rows.append({
            "seed": s, "n_clusters": int(len(sizes)), "verdict": verdict,
            "xmin": fit.get("xmin"), "alpha": fit.get("alpha"),
            "ks": fit.get("ks"), "vuong_R": fit.get("vuong_R"),
            "vuong_p": fit.get("vuong_p"), "seconds": round(time.time() - t0, 1),
        })
    n_pl = sum(1 for r in rows if r["verdict"] == "POWER LAW beats lognormal")
    alphas = [r["alpha"] for r in rows if r["alpha"] is not None]
    xmins = [r["xmin"] for r in rows if r["xmin"] is not None]
    return {
        "control": name, "n_seeds": seeds,
        "frac_power_law_wins": n_pl / seeds,
        "n_power_law_wins": n_pl,
        "frac_lognormal_wins": sum(1 for r in rows
                                   if r["verdict"] == "lognormal beats power law") / seeds,
        "median_alpha": float(np.median(alphas)) if alphas else None,
        "alpha_iqr": ([float(np.percentile(alphas, 25)),
                       float(np.percentile(alphas, 75))] if alphas else None),
        "median_xmin": float(np.median(xmins)) if xmins else None,
        "n_clusters_median": int(np.median([r["n_clusters"] for r in rows])),
        "seeds": rows,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", type=int, default=12)
    ap.add_argument("--fast", action="store_true",
                    help="fewer clusters per sample; for a timing smoke test")
    a = ap.parse_args()
    n_cl = 1500 if a.fast else 6000

    specs = [
        ("positive_branching_critical", branching_sizes,
         dict(mean_offspring=1.0, n_clusters=n_cl), 1.5,
         "mean-field progeny exponent"),
        ("positive_percolation_2d_pc", percolation_sizes,
         dict(L=160, p=0.5927), 2.055, "2D site percolation tau = 187/91"),
        ("negative_branching_subcritical", branching_sizes,
         dict(mean_offspring=0.7, n_clusters=n_cl), None, "exponential tail"),
        # Supercritical branching makes EVERY cluster run to the size cap, so
        # 6000 clusters x 200k steps is a pseudo-hang, not a computation. A few
        # hundred cap-sized clusters carry the same distributional information.
        ("negative_branching_supercritical", branching_sizes,
         dict(mean_offspring=1.3, n_clusters=300, cap=20_000), None,
         "giant cluster"),
        # Lognormal at three widths: if the false positive only appears at one
        # sigma it is an artifact of that choice; if it appears across the
        # range it is a property of the instrument.
        ("negative_lognormal_s0.6", lognormal_sizes,
         dict(n=n_cl, sigma=0.6), None, "lognormal is the literal truth"),
        ("negative_lognormal_s1.2", lognormal_sizes,
         dict(n=n_cl, sigma=1.2), None, "lognormal is the literal truth"),
        ("negative_lognormal_s2.0", lognormal_sizes,
         dict(n=n_cl, sigma=2.0), None, "lognormal is the literal truth"),
    ]

    results, t0 = [], time.time()
    for name, gen, kw, expect_alpha, why in specs:
        print(f"=== {name} ({a.seeds} seeds, "
              f"{kw.get('n_clusters', n_cl)} clusters each) ===", flush=True)
        r = run_control(name, gen, a.seeds, **kw)
        r["expected_alpha"] = expect_alpha
        r["expectation"] = why
        results.append(r)
        print(f"    power law wins {r['n_power_law_wins']}/{a.seeds} "
              f"({r['frac_power_law_wins']:.2f})  median alpha "
              f"{r['median_alpha'] if r['median_alpha'] is None else round(r['median_alpha'],3)}"
              f"  expected {expect_alpha}", flush=True)

    pos = [r for r in results if r["control"].startswith("positive")]
    neg = [r for r in results if r["control"].startswith("negative")]
    # Sensitivity: does it fire where a power law exists? If not, the
    # Empedocles negative result is uninformative, not a finding.
    sensitive = [r for r in pos if r["frac_power_law_wins"] >= 0.5]
    # Specificity: does it stay quiet where no power law exists?
    quiet = [r for r in neg if r["frac_power_law_wins"] < 0.5]

    verdict = {
        "positive_controls_detected": [r["control"] for r in sensitive],
        "positive_controls_missed": [r["control"] for r in pos
                                     if r["control"] not in
                                     [s["control"] for s in sensitive]],
        "negative_controls_stayed_quiet": [r["control"] for r in quiet],
        "negative_controls_false_positive": [r["control"] for r in neg
                                             if r["control"] not in
                                             [q["control"] for q in quiet]],
        "instrument_sensitive": len(sensitive) == len(pos),
        "instrument_specific": len(quiet) == len(neg),
        "total_seconds": round(time.time() - t0, 1),
        "interpretation": ("If instrument_sensitive is false, Paper 10's "
                           "negative result must be WEAKENED: the pipeline "
                           "cannot detect a power law even where one is known "
                           "to exist, so its silence on the Empedoclean field "
                           "is uninformative. If instrument_sensitive and "
                           "instrument_specific are both true, the pipeline is "
                           "a valid falsification instrument and the Empedocles "
                           "result stands."),
    }
    out = {"control_results": results, "verdict": verdict,
           "n_seeds": a.seeds, "clusters_per_sample": n_cl,
           "instrument": "research/papers/fit_empedocles.py::fit_sample "
                         "(imported, not reimplemented)",
           "reproduce": f"python3 research/papers/p10_controls.py --seeds {a.seeds}"}
    with open(OUT, "w") as fh:
        json.dump(out, fh, indent=2)

    print("\n=== instrument verdict ===")
    for k in ("instrument_sensitive", "instrument_specific"):
        print(f"  {k}: {verdict[k]}")
    print(f"  positive detected: {verdict['positive_controls_detected']}")
    print(f"  positive missed:   {verdict['positive_controls_missed']}")
    print(f"  negative quiet:    {verdict['negative_controls_stayed_quiet']}")
    print(f"  negative false+:   {verdict['negative_controls_false_positive']}")
    print(f"\nwrote {os.path.relpath(OUT, REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
