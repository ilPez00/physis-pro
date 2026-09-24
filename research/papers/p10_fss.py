#!/usr/bin/env python3
"""P10 4C: finite-size scaling of the Empedoclean field.

The criticality refutation rests on nine nemesis settings at a single node
count (N=300). That cannot distinguish three quite different situations:

  * an ordinary sharp transition that simply exists;
  * finite-size critical scaling, with a transition that sharpens as N grows;
  * no consistent scaling at all -- the "transition" is a finite-size artifact.

Those have different implications for the paper, so the node count is swept.

Note on naming: even if critical scaling were found, that is NOT
self-organized criticality. SOC requires the system to tune itself to the
critical point without an external knob. Here `nemesis` IS the knob, so at best
this could be a tuned transition.

REPRODUCTION: python3 research/papers/p10_fss.py --seeds-big 8
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time

import numpy as np

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BIN = os.path.join(REPO, "target", "release", "examples", "empedocles_multiseed")
OUT = os.path.join(REPO, "research", "p10_fss.json")
# Bumped whenever the classification logic changes. A long-running process
# holds the code it started with, so a late-finishing run writes diagnostics
# from an OLD classifier. This happened four times in one session: in-flight
# runs launched before a fix overwrote corrected artifacts with the wrong
# verdict (once literally printing the opposite conclusion). The guard below
# refuses to let an older classifier replace a newer one.
CLASSIFIER_VERSION = 2

# Dense across the region where the published nine points change character
# (few giant clusters below ~0.006, many small ones above ~0.02).
NEMESIS = [0.0, 0.002, 0.004, 0.006, 0.008, 0.010, 0.012, 0.015, 0.02, 0.03,
           0.05, 0.08]
N_LIST = [100, 300, 1000, 3000]
# Seeds per node count: cost is ~N^2, so the largest N gets fewer.
SEEDS = {100: 16, 300: 16, 1000: 16, 3000: 8}


def run_binary(nodes, nemesis, seeds, timeout=5400):
    env = dict(os.environ)
    env["PHYSIS_EMPEDOCLES_NODES"] = str(nodes)
    env["PHYSIS_EMPEDOCLES_NEMESIS"] = ",".join(str(x) for x in nemesis)
    p = subprocess.run([BIN, str(seeds)], capture_output=True, text=True,
                       env=env, timeout=timeout)
    if p.returncode != 0:
        raise RuntimeError(f"N={nodes} failed: {p.stderr[:300]}")
    return json.loads(p.stdout)


def stats_for(rows):
    """Solids per (node count, nemesis) from that setting's seeds.

    Two order parameters, because only one of them actually locates a
    transition:

    * `largest_mean` -- the spanning-cluster fraction, the order parameter
      itself (0 = fragmented, 1 = one cluster).
    * `chi_excl_mean` -- percolation susceptibility SUM_{s != largest} s^2 / N.
      The largest cluster is EXCLUDED. Including it makes chi trivially maximal
      (chi = N) whenever the field fuses into one cluster, which is the
      fragmented/fused end point rather than the transition. The standard
      percolation quantity drops the spanning cluster for exactly this reason,
      and it is the quantity that peaks at the transition.
    """
    largest = np.array([r["largest"] for r in rows], dtype=float)
    n_cl = np.array([len(r["sizes"]) for r in rows], dtype=float)
    N = rows[0]["nodes"]
    chi_excl, second = [], []
    for r in rows:
        s = np.sort(np.asarray(r["sizes"], dtype=float))[::-1]
        rest = s[1:] if len(s) > 1 else np.array([0.0])
        chi_excl.append(float((rest ** 2).sum() / N))
        second.append(float(s[1] / N) if len(s) > 1 else 0.0)
    chi_excl = np.array(chi_excl)
    regimes = {}
    for r in rows:
        regimes[r["regime"]] = regimes.get(r["regime"], 0) + 1
    return {
        "nemesis": rows[0]["nemesis"], "nodes": N, "n_seeds": len(rows),
        "largest_mean": float(largest.mean()),
        "largest_std": float(largest.std(ddof=1)) if len(largest) > 1 else 0.0,
        "n_clusters_mean": float(n_cl.mean()),
        "chi_excl_mean": float(chi_excl.mean()),
        "chi_excl_std": float(chi_excl.std(ddof=1)) if len(chi_excl) > 1 else 0.0,
        "second_largest_mean": float(np.mean(second)),
        "regime_counts": regimes,
    }


def write_artifact(payload):
    """Write the artifact, refusing to be reverted by a stale classifier.

    A process that started before a classifier fix keeps the old logic in
    memory and will happily overwrite good diagnostics with bad ones when it
    finally finishes. Progress-durability fixes protect against kills; this
    protects against late finishes, which is the failure that actually bit.
    """
    prev_v = None
    try:
        with open(OUT) as fh:
            prev = json.load(fh)
        prev_v = (prev.get("diagnostics") or {}).get("classifier_version")
    except (OSError, ValueError):
        prev = None
    mine = (payload.get("diagnostics") or {}).get("classifier_version")
    if prev_v is not None and mine is not None and prev_v > mine:
        side = OUT.replace(".json", f"_cv{mine}.json")
        with open(side, "w") as fh:
            json.dump(payload, fh, indent=2)
        print(f"REFUSED to overwrite {OUT} (classifier v{prev_v}) with v{mine}; "
              f"wrote {side}. Recompute with --recompute instead.")
        return
    with open(OUT, "w") as fh:
        json.dump(payload, fh, indent=2)


def compute_diagnostics(curves):
    """Scaling diagnostics from the completed node-count curves."""
    # ---- scaling diagnostics -------------------------------------------
    diag = {}
    peaks, widths, halfs = [], [], []
    for c in curves:
        pts = c["points"]
        pk = max(pts, key=lambda p: p["chi_excl_mean"])
        peaks.append({"nodes": c["nodes"], "nemesis_at_peak": pk["nemesis"],
                      "chi_peak": pk["chi_excl_mean"], "largest_at_peak": pk["largest_mean"]})
        # crossing of largest_mean through 0.5, by linear interpolation
        xs = [p["nemesis"] for p in pts]
        ys = [p["largest_mean"] for p in pts]
        cross = None
        for i in range(len(xs) - 1):
            if (ys[i] - 0.5) * (ys[i + 1] - 0.5) <= 0 and ys[i] != ys[i + 1]:
                cross = xs[i] + (0.5 - ys[i]) * (xs[i + 1] - xs[i]) / (ys[i + 1] - ys[i])
                break
        halfs.append({"nodes": c["nodes"], "nemesis_at_half": cross})
        # width of the transition: span where 0.1 < largest_mean < 0.9
        band = [p["nemesis"] for p in pts if 0.1 <= p["largest_mean"] <= 0.9]
        widths.append({"nodes": c["nodes"],
                       "width": (max(band) - min(band)) if len(band) > 1 else None})

    diag["chi_peak_by_N"] = peaks
    diag["largest_half_crossing_by_N"] = halfs
    diag["transition_width_by_N"] = widths

    # Is the chi peak even INSIDE the sweep? A peak at the first or last grid
    # point means no transition was located -- chi merely rises monotonically to
    # the edge, and its "scaling" is then uninterpretable.
    edge = []
    for c in curves:
        pts = c["points"]
        pk = max(pts, key=lambda p: p["chi_excl_mean"])
        edge.append(pk["nemesis"] in (pts[0]["nemesis"], pts[-1]["nemesis"]))
    diag["chi_peak_at_sweep_boundary"] = all(edge)

    # chi ~ N is the TRIVIAL signature of a fixed small number of O(N) clusters:
    # with k clusters of size ~N/k, chi_excl ~ N(k-1)/k^2, so chi/N is constant.
    # Report chi/N explicitly, because "chi grows with N" alone invites exactly
    # the wrong reading.
    diag["chi_over_N_by_N"] = [
        {"nodes": c["nodes"],
         "chi_over_N": max(p["chi_excl_mean"] for p in c["points"]) / c["nodes"]}
        for c in curves]

    # Does the peak grow with N (critical scaling) or saturate (ordinary
    # crossover)? Log-log slope of chi_peak vs N.
    if not peaks:
        diag["classification"] = "no curves: nothing to diagnose"
        return diag
    ns = np.array([p["nodes"] for p in peaks], dtype=float)
    ch = np.array([p["chi_peak"] for p in peaks], dtype=float)
    ok = (ch > 0) & np.isfinite(ch)
    slope = None
    if ok.sum() >= 3:
        slope = float(np.polyfit(np.log(ns[ok]), np.log(ch[ok]), 1)[0])
    diag["chi_peak_loglog_slope_vs_N"] = slope

    ws = [(w["nodes"], w["width"]) for w in widths if w["width"]]
    shrink = None
    if len(ws) >= 3:
        wn = np.array([x[0] for x in ws], dtype=float)
        wv = np.array([x[1] for x in ws], dtype=float)
        if (wv > 0).all():
            shrink = float(np.polyfit(np.log(wn), np.log(wv), 1)[0])

    chi_over_N = [x["chi_over_N"] for x in diag["chi_over_N_by_N"]]
    flat_chi_over_N = (bool(chi_over_N) and
                       (max(chi_over_N) - min(chi_over_N)) / max(chi_over_N) < 0.30)
    if slope is None:
        kind = "indeterminate (too few points)"
    elif diag["chi_peak_at_sweep_boundary"]:
        kind = ("NO TRANSITION LOCATED: chi peaks at the edge of the nemesis "
                "grid at every node count, so no critical point was found "
                "inside the sweep and no scaling can be read from it")
    elif flat_chi_over_N:
        kind = ("chi/N is approximately constant across N, which is the TRIVIAL "
                "signature of a fixed small number of O(N) clusters, not "
                "critical scaling. No scaling claim is supportable.")
    elif slope > 0.25 and (shrink is not None and shrink < -0.25):
        kind = ("chi peak grows with N while the transition NARROWS "
                "(width ~ N^negative): consistent with finite-size critical "
                "scaling -- but nemesis is an external knob, so this would be a "
                "TUNED transition, never self-organized criticality")
    elif shrink is not None and shrink > 0.25:
        kind = ("the transition WIDENS with N (width ~ N^positive). A critical "
                "transition sharpens with N, so this is an ordinary crossover, "
                "not critical scaling")
    else:
        kind = "no consistent scaling"

    diag["classification"] = kind
    diag["transition_width_loglog_slope_vs_N"] = shrink
    diag["note"] = ("A tuned transition is not self-organized criticality: SOC "
                    "requires the system to reach the critical point without an "
                    "external control parameter, and nemesis_strength is one.")
    diag["classifier_version"] = CLASSIFIER_VERSION
    return diag


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds-big", type=int, default=None,
                    help="override the seed count for every node count")
    ap.add_argument("--nodes", type=str, default=None,
                    help="comma list to override the node sweep")
    ap.add_argument("--recompute", action="store_true",
                    help="recompute diagnostics from the curves already in the "
                         "artifact; does not re-run the simulation")
    a = ap.parse_args()

    if not os.path.exists(BIN):
        print(f"missing binary: {BIN}\n"
              f"build: cargo build --release --features empedocles "
              f"--example empedocles_multiseed")
        return 1

    if a.recompute:
        d = json.load(open(OUT))
        d["diagnostics"] = compute_diagnostics(d["curves"])
        d["partial"] = None
        write_artifact(d)
        g = d["diagnostics"]
        print("=== recomputed diagnostics (no simulation) ===")
        print("  chi_peak_at_sweep_boundary:", g.get("chi_peak_at_sweep_boundary"))
        print("  chi_over_N_by_N:", [(x["nodes"], round(x["chi_over_N"], 4))
                                     for x in g.get("chi_over_N_by_N", [])])
        print("  width loglog slope:", g.get("transition_width_loglog_slope_vs_N"))
        print("  classification:", g.get("classification"))
        return 0

    nodes_list = ([int(x) for x in a.nodes.split(",")] if a.nodes else N_LIST)
    curves, t0 = [], time.time()
    for N in nodes_list:
        seeds = a.seeds_big or SEEDS.get(N, 8)
        t1 = time.time()
        print(f"=== N={N} ({seeds} seeds x {len(NEMESIS)} nemesis settings) ===",
              flush=True)
        data = run_binary(N, NEMESIS, seeds)
        grouped = {}
        for r in data:
            grouped.setdefault(r["nemesis"], []).append(r)
        curve = [stats_for(grouped[k]) for k in sorted(grouped)]
        curves.append({"nodes": N, "seeds": seeds, "points": curve,
                       "seconds": round(time.time() - t1, 1)})
        peak = max(curve, key=lambda c: c["chi_excl_mean"])
        print(f"    done in {round(time.time()-t1,1)}s  "
              f"chi peaks at nemesis={peak['nemesis']} (chi={peak['chi_excl_mean']:.1f}), "
              f"largest_mean there {peak['largest_mean']:.3f}", flush=True)
        # Write partial progress after EVERY node count. The first version
        # wrote only at the very end, so each kill (and this host kills long
        # runs) discarded the entire sweep. Diagnostics come later, once all
        # points exist; a partial artifact still carries the raw curves.
        write_artifact({"nemesis_grid": NEMESIS, "curves": curves,
                       "diagnostics": {"classifier_version": CLASSIFIER_VERSION},
                       "partial": [c["nodes"] for c in curves],
                       "binary": os.path.relpath(BIN, REPO),
                       "reproduce": "python3 research/papers/p10_fss.py"})


    diag = compute_diagnostics(curves)
    diag["total_seconds"] = round(time.time() - t0, 1)

    out = {"nemesis_grid": NEMESIS, "curves": curves, "diagnostics": diag,
           "binary": os.path.relpath(BIN, REPO),
           "reproduce": "python3 research/papers/p10_fss.py"}
    write_artifact(out)

    print("\n=== scaling diagnostics ===")
    for p in peaks:
        print(f"  N={p['nodes']:>4d}  chi_peak={p['chi_peak']:>9.2f}  "
              f"at nemesis={p['nemesis_at_peak']}  largest={p['largest_at_peak']:.3f}")
    print(f"  chi_peak log-log slope vs N: {slope}")
    print(f"  transition-width slope vs N: {shrink}")
    print(f"  classification: {kind}")
    print(f"\nwrote {os.path.relpath(OUT, REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
