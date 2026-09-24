#!/usr/bin/env python3
"""Trace paper 9 and paper 10 numbers to their result files."""
import json, re, os

REPO = "/home/gio/dev/physis-pro"
PAPERS = f"{REPO}/research/papers"


def nums(f):
    txt = open(f"{PAPERS}/{f}").read()
    # strip bibliography lines: DOIs and arXiv ids contain digit runs that
    # look like results (e.g. 10.1137/070710111 -> "0.113")
    txt = "\n".join(l for l in txt.splitlines()
                    if "DOI" not in l and "arXiv" not in l and "\\bibitem" not in l)
    return {float(m) for m in re.findall(r"\d\.\d{3}", txt)}


def r3(v):
    return round(float(v), 3)


# ---- paper 9: dataset ----
ds = json.load(open(f"{REPO}/research/dataset_baselines.json"))
exp9 = set()
for h, v in ds["levels"].items():
    for k, row in v.items():
        if isinstance(row, dict) and row.get("auc") is not None:
            exp9.add(r3(row["auc"]))
for group in ("top_by_frequency", "highest_rate_n20"):
    for k in ds["kinds"][group]:
        exp9.add(r3(k["rate"]))
# label rates live in the E74 result file, not in dataset_baselines.json
e74 = json.load(open(f"{REPO}/research/e74_replication_results.json"))
for h, v in e74["corpus"]["label_rates"].items():
    exp9.add(r3(v["marker_error"]))
exp9.add(r3(e74["corpus"]["merged_marker_error"]))

# ---- paper 10: empedocles ----
ft = json.load(open(f"{REPO}/research/empedocles_fits.json"))
exp10 = set()
for k, v in ft["settings"].items():
    exp10.add(r3(float(k)))
    f = v.get("fit")
    if f:
        for key in ("alpha", "ks", "lognormal_shape"):
            if f.get(key) is not None:
                exp10.add(r3(f[key]))
        for key in ("vuong_R", "vuong_p"):
            if f.get(key) is not None:
                exp10.add(r3(f[key]))
    exp10.add(r3(v["mean_largest"]))

for label, f, exp in (("PAPER 9 dataset", "paper9_dataset.tex", exp9),
                      ("PAPER 10 empedocles", "paper10_empedocles.tex", exp10)):
    claimed = nums(f)
    orphan = sorted(n for n in claimed if n not in exp and n not in {0.0, 1.0})
    print(f"=== {label} ===")
    print(f"  3-dp numbers in text : {len(claimed)}")
    print(f"  spanned by results   : {len(exp)}")
    print(f"  untraceable          : {len(orphan)} -> {orphan}")
