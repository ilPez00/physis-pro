#!/usr/bin/env python3
"""Regenerate paper-1 figures from research/e74_replication_results.json.

All three figures read the same result file the manuscript cites, so a
number cannot drift between text and plot.
"""
import json, os
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

REPO = "/home/gio/dev/physis-pro"
OUT = f"{REPO}/research/papers"
res = json.load(open(f"{REPO}/research/e74_replication_results.json"))

harn = sorted(res["corpus"]["harnesses"])
tm = res["transfer_matrix"]
ic = res["in_corpus"]

# --- Fig 1: transfer matrix ---
M = np.full((len(harn), len(harn)), np.nan)
for i, s in enumerate(harn):
    for j, t in enumerate(harn):
        if s == t:
            M[i, j] = ic[t]["auc"] if ic.get(t) else np.nan
        else:
            v = tm.get(f"{s}->{t}")
            M[i, j] = v["auc"] if v and v["auc"] is not None else np.nan

fig, ax = plt.subplots(figsize=(6, 5))
im = ax.imshow(M, cmap="RdYlGn", vmin=0.35, vmax=0.8)
ax.set_xticks(range(len(harn)), harn, rotation=45, ha="right")
ax.set_yticks(range(len(harn)), harn)
ax.set_xlabel("target harness")
ax.set_ylabel("source harness (prior)")
ax.set_title("Transfer AUC: row = source prior, column = target failures\n(inference unit: session; values UNCORRECTED, see Holm results in text)")
for i in range(len(harn)):
    for j in range(len(harn)):
        if not np.isnan(M[i, j]):
            ax.text(j, i, f"{M[i, j]:.3f}", ha="center", va="center", fontsize=8)
fig.colorbar(im, ax=ax, label="AUC")
fig.tight_layout()
fig.savefig(f"{OUT}/fig1_transfer_matrix.pdf")
plt.close(fig)

# --- Fig 2: cold-start curve ---
cs = [c for c in res["coldstart"] if c["auc"] is not None]
xs = [c["n_target_sessions"] for c in cs]
ys = [c["auc"] for c in cs]
fig, ax = plt.subplots(figsize=(6, 4))
ax.plot(xs, ys, "o-", color="#1f77b4")
for c in cs:
    ax.annotate(f"{c['auc']:.3f}", (c["n_target_sessions"], c["auc"]),
                textcoords="offset points", xytext=(0, 7), ha="center", fontsize=8)
ax.axhline(0.5, ls="--", c="gray", lw=1, label="chance")
ax.axhline(ys[0], ls=":", c="red", lw=1, label="transferred prior only")
ax.set_xlabel("target sessions used for local prior")
ax.set_ylabel("AUC")
ax.set_title("WITHDRAWN (train/test leakage) — cold-start, opencode prior -> claude-code")
# The rising curve below was produced by leakage: test sessions entered the
# prior. It is kept only as an error record. Stamped so it cannot be lifted
# out of context and used as a result figure.
ax.text(0.5, 0.55, "WITHDRAWN", transform=ax.transAxes, fontsize=34,
        color="red", alpha=0.30, ha="center", va="center", rotation=22,
        fontweight="bold")
ax.text(0.5, 0.42, "leaked: test sessions in the prior", transform=ax.transAxes,
        fontsize=9, color="red", alpha=0.55, ha="center", va="center", rotation=22)
ax.legend(fontsize=8)
ax.grid(alpha=0.3)
fig.tight_layout()
fig.savefig(f"{OUT}/fig2_cold_start_curve.pdf")
plt.close(fig)

# --- Fig 3: coverage vs AUC ---
cov = res["coverage"]
x = [c["coverage"] for c in cov]
y = [c["transfer_auc"] for c in cov]
fig, ax = plt.subplots(figsize=(6, 4))
ax.scatter(x, y, s=45, color="#d62728")
for c in cov:
    ax.annotate(c["pair"], (c["coverage"], c["transfer_auc"]),
                textcoords="offset points", xytext=(5, 4), fontsize=6)
if len(x) > 1:
    m, b = np.polyfit(x, y, 1)
    xs_ = np.linspace(min(x), max(x), 50)
    ax.plot(xs_, m * xs_ + b, ls="--", c="gray", lw=1,
            label=f"exploratory fit (r={np.corrcoef(x, y)[0,1]:.2f}, n={len(x)})")
ax.axhline(0.5, ls=":", c="black", lw=1, label="chance")
ax.set_xlabel("coverage (fraction of target kinds seen in source)")
ax.set_ylabel("transfer AUC")
ax.set_title("Coverage vs transfer success — EXPLORATORY\n(n=12 directional pairs; correlation, not causation)")
ax.text(0.02, 0.02, f"exploratory: n={len(x)}, no causal claim",
        transform=ax.transAxes, fontsize=7, color="gray")
ax.legend(fontsize=8)
ax.grid(alpha=0.3)
fig.tight_layout()
fig.savefig(f"{OUT}/fig3_coverage_vs_auc.pdf")
plt.close(fig)

print("wrote fig1_transfer_matrix.pdf, fig2_cold_start_curve.pdf, fig3_coverage_vs_auc.pdf")
print("fig2 points:", list(zip(xs, [round(v, 3) for v in ys])))
r = np.corrcoef(x, y)[0, 1]
assert abs(r - 0.433) < 0.02, f"coverage correlation drifted: {r}"
print(f"fig3 pearson r = {r:.3f} (manuscript claims 0.43)")
