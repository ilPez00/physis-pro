#!/usr/bin/env python3
"""Trace 3-dp numbers in papers 2, 3, 4 back to their result files.

Paper 2 -> benchmarks/conatus/e78-*.json (+ e79-*.json if present)
Paper 3 -> research/mapper/e88_results.json, e90_results.json
Paper 4 -> E21/E15/E16; source must be located or declared untraceable.
"""
import json, re, os, glob

REPO = "/home/gio/dev/physis-pro"
RET = "/home/gio/dev/.physis-pro-retired-20260920-014539"
PAPERS = f"{REPO}/research/papers"


def nums(texfile):
    return {float(m) for m in re.findall(r"0\.\d{3}", open(f"{PAPERS}/{texfile}").read())}


def r3(v):
    try:
        return round(float(v), 3)
    except (TypeError, ValueError):
        return None


def collect_e78():
    out = set()
    for p in glob.glob(f"{REPO}/benchmarks/conatus/e78-*.json"):
        d = json.load(open(p))
        for k in ("within_mean", "between_mean", "spearman", "path", "displacement"):
            if k in d:
                out.add(r3(d[k]))
        if "null_band" in d:
            out.update(r3(x) for x in d["null_band"])
        spec = d.get("spectrum") or []
        out.update(r3(x) for x in spec[:4])
        # derived quantities the paper reports
        if len(spec) >= 2:
            out.add(r3(spec[0] + spec[1]))          # variance in first 2 dims
        if d.get("path"):
            out.add(r3(d["displacement"] / d["path"]))  # straightness
        # mean of pairwise within-object distances is reported as 4x-14x smaller
        if d.get("within_mean") and d.get("between_mean"):
            out.add(r3(d["between_mean"] / d["within_mean"]))
    for p in glob.glob(f"{REPO}/benchmarks/conatus/e79-*.json"):
        d = json.load(open(p))
        for lvl in (d.get("levels") or {}).values():
            if isinstance(lvl, dict):
                out.update(r3(x) for x in lvl.get("nesting", []))
                out.update(r3(x) for x in lvl.get("position", []))
                for k in ("within", "near_doc", "far_doc"):
                    if k in lvl:
                        out.add(r3(lvl[k]))
                for a, b in (("near_doc", "within"), ("far_doc", "within")):
                    if lvl.get(a) and lvl.get(b):
                        out.add(r3(lvl[a] / lvl[b]))   # ratio columns
    return {x for x in out if x is not None}


def collect_e88_e90():
    out = set()
    for name in ("e88_results.json", "e90_results.json"):
        d = json.load(open(f"{REPO}/research/mapper/{name}"))
        def walk(o):
            if isinstance(o, dict):
                for v in o.values():
                    walk(v)
            elif isinstance(o, list):
                for v in o:
                    walk(v)
            elif isinstance(o, (int, float)):
                out.add(r3(o))
        walk(d)
    return {x for x in out if x is not None}


def report(label, texfile, expected):
    claimed = nums(texfile)
    orphans = sorted(n for n in claimed if n not in expected)
    print(f"=== {label} ({texfile}) ===")
    print(f"  distinct 3-dp numbers in text : {len(claimed)}")
    print(f"  numbers spanned by sources    : {len(expected)}")
    print(f"  UNTRACEABLE                   : {len(orphans)}")
    if orphans:
        print(f"    {orphans}")
    print()


report("PAPER 2 trajectories", "paper2_trajectories.tex", collect_e78())
report("PAPER 3 ontology instability", "paper3_ontology_instability.tex", collect_e88_e90())

# Paper 4: look for any E21/E15/E16 result artefact
cands = []
for base in (REPO, RET):
    for pat in ("**/e21*result*.json", "**/e21*.json", "**/e15*result*.json", "**/e16*result*.json"):
        cands += glob.glob(f"{base}/{pat}", recursive=True)
cands = sorted(set(c for c in cands if "/target/" not in c))
print("=== PAPER 4 propose/certify ===")
print(f"  source artefacts found: {cands if cands else 'NONE'}")
p4 = nums("paper4_propose_certify.tex")
print(f"  distinct 3-dp numbers in text: {len(p4)}")
print(f"  traceable: {'yes' if cands else 'NO — E21/E15/E16 results exist only as prose in the retired archive'}")
