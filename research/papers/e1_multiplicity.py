#!/usr/bin/env python3
"""Paper 1: do the transfer results survive multiplicity correction?

Twelve directional source->target tests are reported. "One 95% CI excludes 0.5"
is not evidence of a real effect when it is selected from twelve; that is the
selection the interval does not account for.

This computes a bootstrap p-value per pair (session-level resampling, the
correct inferential unit, 10,000 replicates) and applies Holm and
Benjamini-Hochberg across the family, plus Fisher's combined test.

Reuses the construction in check_bootstrap_unit.py so there is one
implementation of the estimator, not two.

  python3 research/papers/e1_multiplicity.py
"""
import importlib.util, json, os, sys
import numpy as np

REPO = "/home/gio/dev/physis-pro"
N_BOOT = 10000

spec = importlib.util.spec_from_file_location(
    "cbu", f"{REPO}/research/papers/check_bootstrap_unit.py")
if spec is None or spec.loader is None:
    sys.exit("cannot load check_bootstrap_unit.py")
cbu = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cbu)          # module-level corpus load only; main() is guarded

stats = {h: cbu.key_stats(cbu.by_h[h]) for h in cbu.harn}

print(f"session-level bootstrap, {N_BOOT} replicates, 12 directional pairs")
rows = []
for src in cbu.harn:
    for tgt in cbu.harn:
        if src == tgt:
            continue
        sc, lb, sid = cbu.collect(cbu.by_h[tgt], stats[src])
        lo, hi, draws = cbu.boot(sc, lb, sid, "session", n=N_BOOT, return_draws=True)
        if draws is None:
            continue
        auc = float(np.mean(draws))          # bootstrap mean of the AUC
        # two-sided bootstrap p against the null AUC = 0.5
        p = 2.0 * min(float(np.mean(draws <= 0.5)), float(np.mean(draws >= 0.5)))
        p = min(1.0, max(p, 1.0 / N_BOOT))   # floor at resolution
        rows.append({"pair": f"{src}->{tgt}", "auc_boot_mean": auc,
                     "lo": lo, "hi": hi, "p_raw": p})
        print(f"  {src:12s} -> {tgt:12s} auc={auc:.3f} [{lo:.3f},{hi:.3f}] p={p:.4f}")

# ---- Holm and Benjamini-Hochberg over the family of 12 ----
order = sorted(range(len(rows)), key=lambda i: rows[i]["p_raw"])
m = len(rows)
for rank, i in enumerate(order, start=1):
    rows[i]["holm_threshold"] = 0.05 / (m - rank + 1)
    rows[i]["bh_threshold"] = 0.05 * rank / m

# Holm: step down, first failure stops rejection
holm_reject = set()
for rank, i in enumerate(order, start=1):
    if rows[i]["p_raw"] <= rows[i]["holm_threshold"]:
        holm_reject.add(i)
    else:
        break
# BH: step up from the largest p
bh_reject = set()
k_max = 0
for rank, i in enumerate(order, start=1):
    if rows[i]["p_raw"] <= rows[i]["bh_threshold"]:
        k_max = rank
bh_reject = set(order[:k_max]) if k_max else set()

for i, r in enumerate(rows):
    r["holm_reject"] = i in holm_reject
    r["bh_reject"] = i in bh_reject

survive_holm = [rows[i]["pair"] for i in sorted(holm_reject)]
survive_bh = [rows[i]["pair"] for i in sorted(bh_reject)]

# Fisher's combined test (pairs are NOT independent -- they share harnesses,
# so this is reported as a rough global check, not a calibrated p-value)
ps = np.array([r["p_raw"] for r in rows])
fisher_stat = float(-2.0 * np.sum(np.log(ps)))
from math import erf, sqrt
def chi2_sf(x, k):
    # survival function of chi-square with even k, via the series
    # P(X > x) = exp(-x/2) * sum_{j=0}^{k/2-1} (x/2)^j / j!
    if k % 2:
        raise ValueError("k must be even")
    t, s = x / 2.0, 0.0
    term = 1.0
    for j in range(k // 2):
        s += term
        term *= t / (j + 1)
    return float(np.exp(-t) * s)
fisher_p = chi2_sf(fisher_stat, 2 * m)

out = {
    "n_boot": N_BOOT,
    "inference_unit": "session",
    "family_size": m,
    "pairs": rows,
    "holm_survivors": survive_holm,
    "bh_survivors": survive_bh,
    "fisher_statistic": fisher_stat,
    "fisher_p_approx": fisher_p,
    "fisher_caveat": ("pairs are not independent: the 12 comparisons share 4 "
                      "harnesses, so this combined p is optimistic and is "
                      "reported only as a rough global check"),
}
dest = f"{REPO}/research/e1_multiplicity.json"
json.dump(out, open(dest, "w"), indent=2)

print()
print(f"raw p<0.05                 : {sum(1 for r in rows if r['p_raw'] < 0.05)} of {m}")
print(f"surviving Holm             : {len(survive_holm)}  {survive_holm}")
print(f"surviving Benjamini-Hochberg: {len(survive_bh)}  {survive_bh}")
print(f"Fisher combined (optimistic): stat={fisher_stat:.1f} p~{fisher_p:.3g}")
print()
print(f"wrote {dest}")
