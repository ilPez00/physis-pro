#!/usr/bin/env python3
"""Close the disclosed gap in paper 4: the permutation null used 5 shuffles.

The project's own notes call five shuffles too weak for a permutation
baseline. This re-runs ONLY the permuted arm at the largest split (frac
0.75, i.e. 436 decisions seen) with 1000 shuffles and reports the null,
so the published 0.136 top-3 figure can be checked against a real null
distribution rather than a five-sample mean.

Imports run_e21.py so the null comes from the same code path as the
geometry arm.

Note: topk() seeds its own RNG per replicate (`random.Random(s)` inside the
seed loop), so the shuffle count must be passed as `seeds=`, NOT by calling
topk repeatedly -- repeated calls would reuse seed 0 and return the same
value every time.

  cd /home/gio/dev/.physis-pro-retired-20260920-014539/research/mapper
  python3 /home/gio/dev/physis-pro/research/papers/e21_perm_null_1000.py
"""
import importlib.util, json, os, sys

MAPPER = "/home/gio/dev/.physis-pro-retired-20260920-014539/research/mapper"
sys.path.insert(0, MAPPER)

spec = importlib.util.spec_from_file_location("run_e21", os.path.join(MAPPER, "run_e21.py"))
e21 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e21)

N = 1000
rows = e21.load()
print(f"rows: {len(rows)}")

null = e21.topk(rows, ks=(1, 3, 5), seeds=N, fracs=(0.75,), mode="permuted")[0.75]
geom = e21.topk(rows, ks=(1, 3, 5), seeds=N, fracs=(0.75,), mode="geom")[0.75]

out = {"n_shuffles": N, "k_seen": 436, "null": null, "geometry": geom}
dest = "/home/gio/dev/physis-pro/research/e21_permutation_null_1000.json"
json.dump(out, open(dest, "w"), indent=2)

for k in (1, 3, 5):
    print(f"  top-{k}: null {null[k]:.3f}   geometry {geom[k]:.3f}   gap {geom[k] - null[k]:+.3f}")
print(f"wrote {dest}")

assert abs(null[3] - 0.136) < 0.04, (
    f"1000-shuffle top-3 null is {null[3]:.3f}, which disagrees with the published 0.136")
assert geom[3] > null[3], "geometry no longer beats the null"
print("published 0.136 top-3 null is consistent with the 1000-shuffle estimate")
