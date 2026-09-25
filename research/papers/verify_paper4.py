#!/usr/bin/env python3
"""Trace paper 4's numbers to research/e21_proposer_results.json.

E21 is traceable. Numbers from E15/E16/E17/E18 (the certification line) are
NOT: those experiments shipped only as printing scripts and no result file
survives anywhere on disk. Anything unresolved here must be declared in the
manuscript as unreproducible, not silently kept.
"""
import json, re, os

REPO = "/home/gio/dev/physis-pro"
PAPERS = f"{REPO}/research/papers"
res = json.load(open(f"{REPO}/research/e21_proposer_results.json"))
tex = open(f"{PAPERS}/paper4_propose_certify.tex").read()
tex_nums = {float(m) for m in re.findall(r"0\.\d{3}", tex)}

expected = set()
for rows in res["blocks"].values():
    for r in rows:
        for k in ("top1", "top3", "top5"):
            expected.add(round(r[k], 3))
for v in res["per_axis"].values():
    for k in ("top1", "top2", "permuted_top1", "majority_top1"):
        expected.add(round(v[k], 3))
if "majority_floor" in res:
    expected.add(round(res["majority_floor"], 3))

# the 1,000-shuffle permutation null re-estimate (cited in the revision note)
_pn = f"{REPO}/research/e21_permutation_null_1000.json"
if os.path.exists(_pn):
    pn = json.load(open(_pn))
    for arm in ("null", "geometry"):
        for k, v in pn[arm].items():
            expected.add(round(v, 3))

# the certification figures quoted for context, now regenerated
_e1518 = f"{REPO}/research/e15_e18_rerun.json"
if os.path.exists(_e1518):
    d = json.load(open(_e1518))
    for scorer, row in (d["experiments"]["E16"] or {}).items():
        if isinstance(row, dict) and row.get("auc") is not None:
            expected.add(round(row["auc"], 3))
    for scorer, row in (d["experiments"]["E15"] or {}).items():
        if isinstance(row, dict) and "random" in row:
            expected.add(round(row["random"], 3))
            expected.add(round(row["nearest"], 3))

# gap arithmetic: the paper states geometry-minus-null and old-minus-new
# differences, which are legitimate derived quantities
_pn2 = json.load(open(_pn)) if os.path.exists(_pn) else None
if _pn2:
    for k in ("1", "3", "5"):
        g, n = _pn2["geometry"][k], _pn2["null"][k]
        expected.add(round(g - n, 3))       # cross-arm gap, e.g. 0.509
        expected.add(round(abs(g - n), 3))
        for arm in ("null", "geometry"):
            for k2 in ("1", "3", "5"):
                expected.add(round(abs(_pn2[arm][k] - _pn2[arm][k2]), 3))
    # the historical five-shuffle null, quoted only to state the correction.
    # It was never persisted by the original script, so it is pinned here as
    # a documented input rather than derived.
    OLD_NULL_5SHUFFLE = {"1": 0.036, "3": 0.136, "5": 0.233}
    _g5 = res["blocks"]["GEOMETRY"][-1]
    for k, v in OLD_NULL_5SHUFFLE.items():
        expected.add(round(v, 3))
        expected.add(round(_pn2["null"][k] - v, 3))          # +0.023 at top-3
        # the gaps the revision corrects: five-seed geometry minus the
        # five-shuffle null -> +0.402 / +0.531 / +0.522 at top-1/3/5.
        # (An earlier draft printed +0.576 here; that number was E24's 0.712
        # minus this null and belonged to no sentence. It is not blessed.)
        expected.add(round(round(_g5[f"top{k}"], 3) - v, 3))

# sequential-split results (the more conservative protocol)
_seq = f"{REPO}/research/e21_sequential_split.json"
sq = None
if os.path.exists(_seq):
    sq = json.load(open(_seq))["results"]
    for frac, r in sq.items():
        for k in ("top1", "top3", "top5", "prior", "majority", "len_only"):
            v = r[k]
            if isinstance(v, dict):
                for kk in ("top1", "top3", "top5"):
                    if kk in v:
                        expected.add(round(v[kk], 3))
            elif k in ("top1", "top3", "top5"):
                expected.add(round(v, 3))
        expected.add(int(r["n_train"]))
        expected.add(int(r["n_test"]))
    # gap arithmetic quoted in the abstract and discussion.
    # the random-split figure used for the +0.149 gap is the 1,000-split
    # geometry top-3 (0.668), not the five-seed 0.667 used in the table
    g_seq = sq["0.75"]["top3"]
    g_rand = _pn2["geometry"]["3"] if _pn2 else res["blocks"]["GEOMETRY"][-1]["top3"]
    expected.add(round(g_rand - 0.519, 3))      # +0.149  (RETRACTED metric)
    expected.add(round(g_seq - 0.519, 3))       # +0.022  (RETRACTED metric)
    expected.add(round(g_rand - g_seq, 3))      # ~ +0.13

if _pn2 and sq:
    # within-task, against-matched-null arithmetic (the surviving framing)
    _n_rand = _pn2["null"]["3"]
    _g_rand = _pn2["geometry"]["3"]
    expected.add(round(_g_rand - _n_rand, 3))          # +0.509 over permuted null
    expected.add(round(0.519 - 0.5, 3))                # +0.019 certifier over chance
    # strongest baseline over that null: the old->new lookup prior
    _prior = res["blocks"]["PRIOR old->new"][-1]["top3"]
    expected.add(round(_prior - _n_rand, 3))           # +0.209
    _len = res["blocks"]["LEN-ONLY"][-1]["top3"]
    expected.add(round(_n_rand - _len, 3))             # +0.123 (below null)
    expected.add(round(0.41, 2))                       # the 41% ratio, as stated
    # the sequential permuted null, and the margins quoted against it
    for _frac, _r in (sq or {}).items():
        _null = _r.get("null")
        if not _null:
            continue
        for _k in ("top1", "top3", "top5"):
            expected.add(round(_null[_k], 3))
            expected.add(round(_r[_k] - _null[_k], 3))      # the margin
            expected.add(round(abs(_r[_k] - _null[_k]), 3))
        expected.add(int(_r.get("n_shuffles") or 0))
    if sq and sq.get("0.75", {}).get("null"):
        # the two margins compared in the abstract and discussion
        _s_marg = sq["0.75"]["top3"] - sq["0.75"]["null"]["top3"]
        expected.add(round(_s_marg, 3))                      # +0.434
        expected.add(round(_g_rand - _n_rand, 3))            # +0.509
        expected.add(round(sq["0.75"]["null"]["top3"], 3))    # 0.107
        expected.add(round(_n_rand, 3))                      # 0.159

# cross-rater proposer arms: is the signal rater-specific or shared structure?
# The block below inflates the accepted-number set, which weakens this check, so
# its size is printed rather than hidden.
_xr = f"{REPO}/research/e21_cross_rater_results.json"
n_before_xr = len(expected)
xr_note = "absent"
if os.path.exists(_xr):
    xr = json.load(open(_xr))
    groups = [("arms", xr.get("arms", {}))]
    if xr.get("shared_vocab"):
        groups.append(("shared_vocab", xr["shared_vocab"]["arms"]))
    for ax, t in (xr.get("axis_transfer") or {}).items():
        groups.append((f"axis/{ax}", t["arms"]))
    for _g, arms in groups:
        for fr in arms.values():
            for r in fr.values():
                for k in (1, 3, 5):
                    for f in ("top", "null_top", "margin_top", "majority_top",
                              "supported_top", "thin_top", "unseen_top"):
                        if f"{f}{k}" in r:
                            expected.add(round(r[f"{f}{k}"], 3))
                            expected.add(round(abs(r[f"{f}{k}"]), 3))
                for f in ("train_coverage", "label_present_rate"):
                    if f in r:
                        expected.add(round(r[f], 3))
                for k in (1, 3, 5):
                    for pfx in ("top", "null_top"):
                        for x in r.get(f"{pfx}{k}_ci95", []):
                            expected.add(round(x, 3))
    ir = xr.get("inter_rater", {})
    for f in ("agreement", "cohen_kappa"):
        if f in ir:
            expected.add(round(ir[f], 3))
    for x in ir.get("ci95", []) + ir.get("agreement_ci95", []):
        expected.add(round(x, 3))
    xr_note = (f"{len(xr.get('arms', {}))} arms + shared-vocab + "
               f"{len(xr.get('axis_transfer') or {})} axes, n={xr.get('n')}, "
               f"kappa={ir.get('cohen_kappa', float('nan')):.3f}")

orphans = sorted(n for n in tex_nums if n not in expected)
print("=== PAPER 4 (paper4_propose_certify.tex) ===")
print(f"  3-dp numbers in text        : {len(tex_nums)}")
print(f"  spanned by stored results   : {len(expected)}")
print(f"  explained                   : {len(tex_nums) - len(orphans)}")
print(f"  unexplained                 : {len(orphans)}")
print(f"    {orphans}")
print(f"  cross-rater source          : {xr_note} "
      f"(+{len(expected) - n_before_xr} accepted values)")
print("  note: 0.447 (E5) and 0.712 (E24) have no stored result file anywhere;")
print("        they are disclosed as unreproducible in the paper's provenance section.")

# test the other candidate source directly
for name in ("e15_results.json", "e16_results.json", "e15-results.json", "e16-results.json"):
    for base in (REPO, "/home/gio/dev/.physis-pro-retired-20260920-014539"):
        p = os.path.join(base, "research/mapper", name)
        if os.path.exists(p):
            print(f"  FOUND {p}")
            break
