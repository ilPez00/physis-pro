#!/usr/bin/env python3
"""Check that numbers in paper1 trace to research/e74_replication_results.json."""
import json, re, os

REPO = "/home/gio/dev/physis-pro"
res = json.load(open(f"{REPO}/research/e74_replication_results.json"))
tex = open(f"{REPO}/research/papers/paper1_agent_priors.tex").read()

# numbers appearing in the tex, 3-dp
tex_nums = {float(m) for m in re.findall(r"0\.\d{3}", tex)}

expected = set()
for p, d in res["transfer_matrix"].items():
    if d["auc"] is not None:
        expected.add(round(d["auc"], 3))
        expected.add(round(d["lo"], 3))
        expected.add(round(d["hi"], 3))
for h, d in res["in_corpus"].items():
    if d:
        expected.add(round(d["auc"], 3))
        expected.add(round(d["lo"], 3))
        expected.add(round(d["hi"], 3))
for c in res["coldstart"]:
    if c["auc"] is not None:
        expected.add(round(c["auc"], 3))
        if c.get("lo") is not None:
            expected.add(round(c["lo"], 3))
        if c.get("hi") is not None:
            expected.add(round(c["hi"], 3))
for c in res["coverage"]:
    expected.add(round(c["coverage"], 3))

# difference arithmetic quoted in the coverage discussion: the coverage and
# AUC gaps between the two pairs it names. (Not the smallest coverage gap in
# the matrix -- that is 0.004 between claude-code->opencode and
# opencode->claude-code; the paper does not claim otherwise.)
_by_pair = {c["pair"]: c for c in res["coverage"]}
_a, _b = _by_pair["codex->cline"], _by_pair["cline->opencode"]
expected.add(round(_b["coverage"] - _a["coverage"], 3))         # 0.022
expected.add(round(abs(_b["transfer_auc"] - _a["transfer_auc"]), 3))  # 0.135
for h, v in res["corpus"]["label_rates"].items():
    expected.add(round(v["marker_error"], 3))
    expected.add(round(v["harness_error"], 3))
expected.add(round(res["corpus"]["merged_marker_error"], 3))

# action-level vs session-level bootstrap comparison (cited in the inferential-unit section)
_bc = f"{REPO}/research/bootstrap_unit_check.json"
if os.path.exists(_bc):
    bc = json.load(open(_bc))
    for pair, d in bc["pairs"].items():
        for unit in ("action", "session"):
            v = d.get(unit)
            if v and v.get("lo") is not None:
                expected.add(round(v["lo"], 3))
                expected.add(round(v["hi"], 3))

print("=== TRANSFER MATRIX vs paper ===")
for p, d in sorted(res["transfer_matrix"].items()):
    if d["auc"] is None:
        continue
    v = round(d["auc"], 3)
    print(f"  {p:26s} {v:.3f}  in_tex={'yes' if v in tex_nums else 'NO'}")

print("\n=== IN-CORPUS vs paper ===")
for h, d in res["in_corpus"].items():
    if not d:
        continue
    v = round(d["auc"], 3)
    print(f"  {h:14s} {v:.3f}  in_tex={'yes' if v in tex_nums else 'NO'}  n_sessions={d['n_sessions']} n_actions={d['n_actions']}")

print("\n=== COLDSTART vs paper ===")
for c in res["coldstart"]:
    v = round(c["auc"], 3) if c["auc"] is not None else None
    print(f"  {c['label']:16s} n={c['n_target_sessions']:3d} auc={v}  in_tex={'yes' if v in tex_nums else 'NO'}")

print("\n=== coverage range ===")
covs = [round(c['coverage'], 3) for c in res["coverage"]]
print(f"  min={min(covs):.3f} max={max(covs):.3f}")

# orphans: 3-dp numbers in tex not explained by any result file
orphans = sorted(n for n in tex_nums if n not in expected)
print("\n=== NUMBERS IN PAPER1 NOT TRACEABLE TO e74_replication_results.json ===")
print(" ", orphans if orphans else "none")
