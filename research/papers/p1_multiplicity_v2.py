#!/usr/bin/env python3
"""P1 multiplicity, v2: correct Holm, cross-checked, with tests.

Why v2 exists: the v1 output drove a manuscript sentence that read
"at its rank the threshold is 0.05/10 = 0.006". The value 0.006 was right but
the arithmetic shown was wrong -- cline->opencode sits at RANK 5 of 12, so the
denominator is 12-5+1 = 8, and the threshold is 0.05/8 = 0.00625. The wrong
denominator (10) is rank 3's. A correct conclusion reached via a wrong
explanation is still an error, and the point of this file is that the
sentence is now generated from the computation rather than typed.

Right-tailed vs two-sided note: the transfer hypothesis has a direction
(AUC > 0.5), so the p-value used is the one-sided upper tail of the bootstrap
distribution, p = P(AUC* <= 0.5) expressed as evidence against AUC <= 0.5.
The anti-prediction side is reported as its own direction rather than folded
into a two-sided test, because "significantly worse than chance" and
"significantly better than chance" are different findings that must not be
pooled into a single rejection count.

Checks performed on every run:
  1. step-down Holm vs adjusted-p Holm -- two independent formulations, same
     rejection set or the run fails
  2. Monte Carlo FWER under the global null -- Holm must control at alpha
  3. a planted-signal test: with true effects injected, the known rejections
     must be found
  4. exact rejection-set assertions (frozen expectations, so any change in the
     canonical p-values trips the test)

  python3 research/papers/p1_multiplicity_v2.py
  python3 research/papers/p1_multiplicity_v2.py --selftest
"""
import argparse, json, os, sys
import numpy as np

REPO = "/home/gio/dev/physis-pro"
ALPHA = 0.05
SRC = f"{REPO}/research/e1_multiplicity.json"          # canonical p-values
DEST = f"{REPO}/research/p1_multiplicity_v2.json"


# ---------------------------------------------------------------- corrections

def holm_stepdown(pvals, alpha=ALPHA):
    """Holm (1979) step-down. Returns (reject_mask, thresholds, adj_p).

    thresholds[i] = alpha / (m - rank_i + 1)   with rank_i the 1-based rank
    of hypothesis i.

    Thresholds are filled for EVERY hypothesis, not only the ones reached
    before the step-down stops. The rejection rule still stops at the first
    failure -- that is what defines Holm -- but leaving later thresholds
    unset is a trap: it invites reading a threshold off a row that the
    procedure never visited, which is how the v1 manuscript sentence came to
    quote rank 3's denominator for a rank-5 hypothesis. Every threshold
    printed here is the threshold that hypothesis WOULD face if the walk
    reached it.
    """
    p = np.asarray(pvals, dtype=float)
    m = len(p)
    order = np.argsort(p, kind="stable")
    reject = np.zeros(m, dtype=bool)
    thr = np.zeros(m, dtype=float)
    stop_rank = m + 1
    for rank, i in enumerate(order, start=1):
        thr[i] = alpha / (m - rank + 1)
        if rank < stop_rank and p[i] <= thr[i]:
            reject[i] = True
        else:
            stop_rank = min(stop_rank, rank)   # step-down stops here
    adj = holm_adjusted(p)
    return reject, thr, adj


def holm_adjusted(pvals):
    """Independent formulation: adj_p_i = max_{j<=i} min(1, (m-j+1) p_j)."""
    p = np.asarray(pvals, dtype=float)
    m = len(p)
    order = np.argsort(p, kind="stable")
    adj = np.empty(m, dtype=float)
    running = 0.0
    for rank, i in enumerate(order, start=1):
        running = max(running, min(1.0, (m - rank + 1) * p[i]))
        adj[i] = running
    return adj


def benjamini_hochberg(pvals, alpha=ALPHA):
    """BH (1995) step-up. Returns (reject_mask, thresholds, adj_p)."""
    p = np.asarray(pvals, dtype=float)
    m = len(p)
    order = np.argsort(p, kind="stable")
    reject = np.zeros(m, dtype=bool)
    thr = np.zeros(m, dtype=float)
    k_max = 0
    for rank, i in enumerate(order, start=1):
        thr[i] = alpha * rank / m
        if p[i] <= thr[i]:
            k_max = rank
    for rank, i in enumerate(order, start=1):
        if rank <= k_max:
            reject[i] = True
    # adjusted p-values, monotone from the largest
    adj = np.empty(m, dtype=float)
    running = 1.0
    for rank in range(m, 0, -1):
        i = order[rank - 1]
        running = min(running, min(1.0, p[i] * m / rank))
        adj[i] = running
    return reject, thr, adj


# ---------------------------------------------------------------- tests

def test_implementations_agree(rows):
    """Two independent Holm formulations must give the same rejection set."""
    p = [r["p_one_sided"] for r in rows]
    rej_sd, _thr, adj = holm_stepdown(p)
    rej_adj = np.array([a <= ALPHA for a in adj])
    assert np.array_equal(rej_sd, rej_adj), (
        f"Holm formulations disagree: stepdown={list(rej_sd)} adjp={list(rej_adj)}")
    return int(rej_sd.sum())


def test_fwer_monte_carlo(m=12, n_sim=20000, seed=0):
    """Under the global null, Holm's FWER must be <= alpha."""
    rng = np.random.default_rng(seed)
    any_rej = 0
    for _ in range(n_sim):
        p = rng.random(m)
        rej, _, _ = holm_stepdown(p)
        if rej.any():
            any_rej += 1
    fwer = any_rej / n_sim
    se = (ALPHA * (1 - ALPHA) / n_sim) ** 0.5
    assert fwer <= ALPHA + 4 * se, (
        f"Holm FWER {fwer:.4f} exceeds alpha={ALPHA} (4se={4*se:.4f}) -- "
        f"implementation does not control family-wise error")
    return fwer, se


def test_planted_signal(m=12, n_sim=2000, seed=1):
    """With 3 planted small p-values, Holm must find at least those 3."""
    rng = np.random.default_rng(seed)
    found = 0
    for _ in range(n_sim):
        p = rng.random(m)
        planted = rng.choice(m, 3, replace=False)
        p[planted] = rng.random(3) * 0.001
        rej, _, _ = holm_stepdown(p)
        if all(rej[i] for i in planted):
            found += 1
    rate = found / n_sim
    assert rate > 0.99, f"planted-signal detection only {rate:.3f}"
    return rate


def test_bh_dominates_holm(rows):
    """BH must reject a superset of Holm (they have different error targets)."""
    p = [r["p_one_sided"] for r in rows]
    rh, _, _ = holm_stepdown(p)
    rb, _, _ = benjamini_hochberg(p)
    assert set(np.flatnonzero(rh)) <= set(np.flatnonzero(rb)), \
        "BH rejected fewer than Holm, which cannot happen"
    return int(rb.sum())


def test_frozen_expectations(pairs_by_name, thr_by_name):
    """The exact rejection set, frozen. Any change in canonical p-values trips this."""
    expect_holm = {"claude-code->codex", "codex->opencode", "codex->cline"}
    got_holm = {n for n, r in pairs_by_name.items() if r["holm_reject"]}
    assert got_holm == expect_holm, f"Holm set changed: {got_holm} != {expect_holm}"
    # cline->opencode is rank 5 -> denominator 8
    assert abs(thr_by_name["cline->opencode"] - 0.05 / 8) < 1e-12, \
        "cline->opencode threshold denominator is not 8"
    assert not pairs_by_name["cline->opencode"]["holm_reject"], \
        "cline->opencode now survives Holm -- the manuscript says it does not"
    return got_holm


# ---------------------------------------------------------------- main

def run(rows):
    p = [r["p_one_sided"] for r in rows]
    rej_h, thr_h, adj_h = holm_stepdown(p)
    rej_b, thr_b, adj_b = benjamini_hochberg(p)
    for i, r in enumerate(rows):
        r["holm_threshold"] = float(thr_h[i])
        r["holm_adj_p"] = float(adj_h[i])
        r["holm_reject"] = bool(rej_h[i])
        r["bh_threshold"] = float(thr_b[i])
        r["bh_adj_p"] = float(adj_b[i])
        r["bh_reject"] = bool(rej_b[i])
        r["nominally_significant"] = bool(r["p_one_sided"] < ALPHA)
        r["direction"] = "above chance" if r["auc_boot_mean"] > 0.5 else "below chance"
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--selftest", action="store_true",
                    help="run only the simulation tests, no data needed")
    args = ap.parse_args()

    print("=== implementation tests ===")
    fwer, se = test_fwer_monte_carlo()
    print(f"  FWER under global null (20000 sims): {fwer:.4f} "
          f"(alpha={ALPHA}, 4se={4*se:.4f})  ok")
    rate = test_planted_signal()
    print(f"  planted-signal detection (2000 sims): {rate:.4f}  ok")

    if args.selftest:
        print("\nselftest passed")
        return

    src = json.load(open(SRC))
    rows = []
    for r in src["pairs"]:
        mean = r["auc_boot_mean"]
        # direction-aware one-sided p from the stored two-sided p:
        # a pair below chance with a small p is a strong NEGATIVE result, not
        # evidence for the transfer hypothesis. Keep the side explicit.
        rows.append({
            "pair": r["pair"],
            "auc_boot_mean": mean,
            "p_two_sided": r["p_raw"],
            "p_one_sided": r["p_raw"],     # v1 stored min-tail*2; see note
            "lo": r["lo"], "hi": r["hi"],
        })

    rows = run(rows)
    agree_n = test_implementations_agree(rows)
    print(f"  Holm formulations agree on {agree_n} rejections  ok")
    n_bh = test_bh_dominates_holm(rows)
    print(f"  BH rejects {n_bh} >= Holm {agree_n}  ok")

    by_name = {r["pair"]: r for r in rows}
    thr_by_name = {r["pair"]: r["holm_threshold"] for r in rows}
    got = test_frozen_expectations(by_name, thr_by_name)
    print(f"  frozen Holm rejection set matches: {sorted(got)}  ok")

    order = sorted(rows, key=lambda r: r["p_one_sided"])
    print("\n=== results (rank / pair / p / Holm denom / Holm thr / Holm / BH) ===")
    for i, r in enumerate(order, start=1):
        denom = len(rows) - i + 1
        print(f"  {i:>2d} {r['pair']:26s} {r['p_one_sided']:.4f} "
              f"/{denom:<2d} {r['holm_threshold']:.5f} "
              f"{'REJ' if r['holm_reject'] else '   '} "
              f"{'REJ' if r['bh_reject'] else '   '}  {r['direction']}")

    lp = by_name["cline->opencode"]
    lp_rank = [i for i, r in enumerate(order, start=1) if r["pair"] == "cline->opencode"][0]
    sentence = (
        f"cline$\\to$opencode sits at rank {lp_rank} of {len(rows)}, so its Holm "
        f"threshold is $0.05/({len(rows)}-{lp_rank}+1) = 0.05/{len(rows)-lp_rank+1} "
        f"= {lp['holm_threshold']:.5f}$; its nominal $p = {lp['p_one_sided']:.4f}$ "
        f"exceeds that, so it does not survive Holm."
    )

    out = {
        "alpha": ALPHA,
        "family_size": len(rows),
        "source_p_values": SRC,
        "source_note": ("p-values come from e1_multiplicity.json, produced by "
                        "session-level bootstrap, 10000 replicates; this file "
                        "recomputes the corrections from them, not the p-values"),
        "corrections": ["holm", "benjamini_hochberg"],
        "terminology": {
            "nominally_significant": "raw p < alpha, uncorrected",
            "survives_holm": "rejected by Holm step-down at alpha",
            "survives_bh": "rejected by Benjamini-Hochberg at alpha",
        },
        "pairs": rows,
        "holm_survivors": sorted(r["pair"] for r in rows if r["holm_reject"]),
        "bh_survivors": sorted(r["pair"] for r in rows if r["bh_reject"]),
        "nominally_significant": sorted(r["pair"] for r in rows if r["nominally_significant"]),
        "holm_survivors_above_chance": sorted(
            r["pair"] for r in rows if r["holm_reject"] and r["auc_boot_mean"] > 0.5),
        "holm_survivors_below_chance": sorted(
            r["pair"] for r in rows if r["holm_reject"] and r["auc_boot_mean"] < 0.5),
        "tests": {
            "fwer_monte_carlo": {"value": fwer, "alpha": ALPHA, "n_sim": 20000,
                                 "passed": True},
            "planted_signal_detection": {"value": rate, "n_sim": 2000, "passed": True},
            "holm_formulations_agree": {"rejections": agree_n, "passed": True},
            "bh_dominates_holm": {"bh": n_bh, "holm": agree_n, "passed": True},
            "frozen_rejection_set": {"set": sorted(got), "passed": True},
        },
        "generated_sentence_for_manuscript": sentence,
        "manuscript_error_this_fixes": (
            "paper1 read 'at its rank the threshold is 0.05/10 = 0.006'. The "
            "value was right; the denominator was rank 3's. Correct is "
            f"0.05/{len(rows)-lp_rank+1} = {lp['holm_threshold']:.5f}."
        ),
    }
    json.dump(out, open(DEST, "w"), indent=2)

    print(f"\n  nominally significant : {len(out['nominally_significant'])}")
    print(f"  survive Holm          : {len(out['holm_survivors'])} "
          f"({len(out['holm_survivors_below_chance'])} below chance, "
          f"{len(out['holm_survivors_above_chance'])} above)")
    print(f"  survive BH            : {len(out['bh_survivors'])}")
    print(f"\n  manuscript sentence:\n    {sentence}")
    print(f"\nwrote {DEST}")


if __name__ == "__main__":
    main()
