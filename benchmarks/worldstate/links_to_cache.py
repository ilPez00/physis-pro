#!/usr/bin/env python3
"""Convert an interpreter link file (position-indexed) into the seq-indexed
cache `physis-world` reads.

The scorer works in corpus positions; the shell works in observation sequence
numbers, because a log grows and positions shift while a seq never does. This is
the one-line bridge, kept as a file so the conversion is reproducible rather
than retyped.

Usage:
  python3 benchmarks/worldstate/links_to_cache.py CORPUS.jsonl LINKS.json OUT.json
"""
import json
import sys

corpus, links, out = sys.argv[1], sys.argv[2], sys.argv[3]
seq_of = {}
for line in open(corpus):
    r = json.loads(line)
    if r.get("seq") is not None:
        seq_of[r["pos"]] = r["seq"]
rows = []
for r in json.load(open(links)):
    seq = seq_of.get(r["pos"])
    if seq is not None and r.get("links"):
        rows.append({"seq": seq, "links": r["links"]})
json.dump(rows, open(out, "w"), indent=1)
print(f"{len(rows)} seq-keyed link rows -> {out}")
