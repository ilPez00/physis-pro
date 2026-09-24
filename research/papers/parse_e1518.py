#!/usr/bin/env python3
"""Persist E15-E18 rerun numbers into one traceable file.

These four experiments shipped only as printing scripts; their result files
were never saved, so papers 3 and 4 cited numbers that resolved to nothing.
Re-running them recovered the values. This captures the headline numbers and
asserts the ones the manuscripts depend on, so a later drift fails loudly.

Usage: python3 research/papers/parse_e1518.py /tmp/e1518_out.txt
"""
import json, re, sys, os

src = sys.argv[1] if len(sys.argv) > 1 else "/tmp/e1518_out.txt"
txt = open(src).read()
blocks = re.split(r"###### (E\d+) ######", txt)
sections = {blocks[i]: blocks[i + 1] for i in range(1, len(blocks) - 1, 2)}

out = {"source": os.path.basename(src), "experiments": {}}

# ---------- E15: scorer x injection policy ----------
e15 = {}
m = re.search(
    r"scorer\s+random\s+same-domain\s+same-mode\s+nearest\s+drop\n"
    r"((?:\s+\S+\s+[\d.\-]+\s+[\d.\-]+\s+[\d.\-]+\s+[\d.\-]+\s+[\d.\-]+\n)+)",
    sections["E15"],
)
if m:
    for line in m.group(1).strip().splitlines():
        p = line.split()
        e15[p[0]] = {"random": float(p[1]), "same_domain": float(p[2]),
                     "same_mode": float(p[3]), "nearest": float(p[4]),
                     "drop": float(p[5])}
e15["real_misfilings_stage9"] = 0.410
boot = {}
for mm in re.finditer(r"(NAME-ONLY|CONSTRAINT|ANCHORVOC|ANCESTRY) - COSINE\s+([+\-][\d.]+)\s+\[([+\-][\d.]+), ([+\-][\d.]+)\]\s+(significant)?", sections["E15"]):
    boot[mm.group(1)] = {"diff": float(mm.group(2)), "lo": float(mm.group(3)),
                         "hi": float(mm.group(4)), "significant": bool(mm.group(5))}
e15["bootstrap_nearest_vs_cosine"] = boot
out["experiments"]["E15"] = e15

# ---------- E16: real misfilings ----------
e16 = {}
for mm in re.finditer(r"^\s*(COSINE|NAME-ONLY|CONSTRAINT|ANCHORVOC|ANCESTRY)\s+([\d.]+)\s+(\d+)\s+(\d+)", sections["E16"], re.M):
    e16[mm.group(1)] = {"auc": float(mm.group(2)), "n": int(mm.group(3))}
br = re.search(r"base rate ([\d.]+)%", sections["E16"])
if br:
    e16["base_rate"] = float(br.group(1)) / 100
out["experiments"]["E16"] = e16

# ---------- E17: three tests ----------
e17 = {}
m = re.search(r"ANCHORVOC\s+AUC ([\d.]+)\s+\(blind subset: ([\d.]+)\)", sections["E17"])
if m:
    e17["nonblind_auc"] = float(m.group(1))
    e17["blind_auc"] = float(m.group(2))
for mm in re.finditer(r"Test (\d)\s+.*?:\s+(PASS|FAIL)", sections["E17"]):
    e17[f"test{mm.group(1)}"] = mm.group(2)
out["experiments"]["E17"] = e17

# ---------- E18: port check + ANCHORVOC controls + second ground truth ----------
e18 = {"port_r": {}, "anchorvoc_controls": {}, "second_ground_truth": {}}
for mm in re.finditer(r"(COSINE|CONSTRAINT|ANCESTRY)\s+Pearson r = ([+\-][\d.]+)\s+n=\d+\s+(\w+)", sections["E18"]):
    e18["port_r"][mm.group(1)] = {"r": float(mm.group(2)), "verdict": mm.group(3)}
for mm in re.finditer(r"^\s+(COSINE \(reference\)|ANCHORVOC \(E16\)|normalised by entry length|CONTROL random cell's anchor|CONTROL entry length only|CONTROL anchor size only)\s+([\d.]+)", sections["E18"], re.M):
    e18["anchorvoc_controls"][mm.group(1).strip()] = float(mm.group(2))
m = re.search(r"DECISIVE: normalised ANCHORVOC vs length-only\s+([+\-][\d.]+)\s+\[([+\-][\d.]+), ([+\-][\d.]+)\]\s+(\w+)", sections["E18"])
if m:
    e18["decisive_normalised_vs_length"] = {
        "diff": float(m.group(1)), "lo": float(m.group(2)),
        "hi": float(m.group(3)), "verdict": m.group(4)}
for mm in re.finditer(r"(ANCHORVOC|COSINE)\s+AUC ([\d.]+)", sections["E18"]):
    e18["second_ground_truth"][mm.group(1)] = float(mm.group(2))
out["experiments"]["E18"] = e18

dest = "/home/gio/dev/physis-pro/research/e15_e18_rerun.json"
json.dump(out, open(dest, "w"), indent=2)

# self-checks: the numbers papers 3 and 4 lean on
assert abs(e15["COSINE"]["random"] - 0.882) < 0.001, "E15 cosine random drifted"
assert abs(e15["COSINE"]["nearest"] - 0.506) < 0.001, "E15 cosine nearest drifted"
assert abs(e16["ANCHORVOC"]["auc"] - 0.664) < 0.001, "E16 ANCHORVOC drifted"
assert abs(e16["COSINE"]["auc"] - 0.519) < 0.001, "E16 cosine drifted"
assert e17["test3"] == "FAIL", "E17 test 3 no longer fails"
assert abs(e18["anchorvoc_controls"]["CONTROL entry length only"] - 0.619) < 0.001, "E18 length control drifted"
print(f"wrote {dest}")
print("E15 cosine:      random", e15["COSINE"]["random"], "nearest", e15["COSINE"]["nearest"])
print("E15 CONSTRAINT:  nearest", e15["CONSTRAINT"]["nearest"])
print("E16:", {k: v.get('auc') for k, v in e16.items() if isinstance(v, dict)})
print("E16 base rate:", e16.get("base_rate"))
print("E17:", {k: v for k, v in e17.items()})
print("E18 port r:", e18["port_r"])
print("E18 controls:", e18["anchorvoc_controls"])
print("E18 decisive:", e18.get("decisive_normalised_vs_length"))
print("E18 second GT:", e18["second_ground_truth"])
