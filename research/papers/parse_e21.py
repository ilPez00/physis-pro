#!/usr/bin/env python3
"""Parse the E21 re-run output into a traceable result file.

E21 (geometry as proposer) shipped only as a printing script; its numbers
were cited in the manuscript with no persisted artefact. This captures the
run into research/e21_proposer_results.json so paper 4's central numbers
resolve to a file.

Usage: python3 research/papers/parse_e21.py /tmp/e21_out.txt
"""
import json, re, sys, os

src = sys.argv[1] if len(sys.argv) > 1 else "/tmp/e21_out.txt"
text = open(src).read()

out = {"source": os.path.basename(src), "blocks": {}, "per_axis": {}}

# "  GEOMETRY\n      k seen     top-1    top-3    top-5\n          58     0.211 ..."
for m in re.finditer(
    r"\n  ([A-Z][A-Za-z0-9 >\-]+)\n\s+k seen\s+top-1\s+top-3\s+top-5\n((?:\s+\d+\s+[\d.]+\s+[\d.]+\s+[\d.]+\n)+)",
    text,
):
    label = m.group(1).strip()
    rows = []
    for line in m.group(2).strip().splitlines():
        p = line.split()
        rows.append({"k_seen": int(p[0]), "top1": float(p[1]), "top3": float(p[2]), "top5": float(p[3])})
    out["blocks"][label] = rows

# "  domain         5   0.626   0.837      0.174     0.205"
for m in re.finditer(r"\n  (\w+)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)", text):
    out["per_axis"][m.group(1)] = {
        "classes": int(m.group(2)),
        "top1": float(m.group(3)),
        "top2": float(m.group(4)),
        "permuted_top1": float(m.group(5)),
        "majority_top1": float(m.group(6)),
    }

# header figures
h = re.search(r"(\d+) blind entries, (\d+) distinct target cells", text)
if h:
    out["n_entries"], out["n_cells"] = int(h.group(1)), int(h.group(2))
f = re.search(r"largest cell (\d+)/(\d+) = ([\d.]+)", text)
if f:
    out["majority_floor"] = float(f.group(3))
mv = re.search(r"moved: (\d+)\s+kept: (\d+)", text)
if mv:
    out["moved"], out["kept"] = int(mv.group(1)), int(mv.group(2))
emb = re.search(r"embedder: ([\w.\-]+) (\d+)d", text)
if emb:
    out["embedder"], out["dim"] = emb.group(1), int(emb.group(2))

dest = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "e21_proposer_results.json")
dest = os.path.normpath(dest)
json.dump(out, open(dest, "w"), indent=2)

# self-check: the two numbers the manuscript leads with
g = out["blocks"]["GEOMETRY"][-1]
assert g["top3"] == 0.667, f"top-3 at 436 decisions drifted: {g['top3']}"
assert out["blocks"]["PERMUTED"][-1]["top3"] == 0.136, "permuted null drifted"
print(f"wrote {dest}")
print("GEOMETRY @436:", g)
print("PERMUTED @436:", out["blocks"]["PERMUTED"][-1])
print("per-axis:", json.dumps(out["per_axis"]))
