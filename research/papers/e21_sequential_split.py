#!/usr/bin/env python3
"""Paper 4's outstanding check: a SEQUENTIAL (not random) train/test split.

The published E21 result splits the 582 entries at random, which can leak
order effects: a test entry sits between training entries rather than after
them. This runs the same protocol in file order -- first N decisions train,
the remaining decisions test -- so the two can be compared.

Honest limitation, stated in the paper: refiling_gate0.tsv carries a
sequence column `n` but NO timestamps. Whether `n` is the rater's
adjudication order or merely the corpus order is not documented, so this is
a sequential split and we do not claim it is chronological.

Geometry arm and baselines come from run_e21.py's own helpers, so the only
difference from the published run is the split.

  cd /home/gio/dev/.physis-pro-retired-20260920-014539/research/mapper
  python3 /home/gio/dev/physis-pro/research/papers/e21_sequential_split.py
"""
import importlib.util, json, os, sys

MAPPER = "/home/gio/dev/.physis-pro-retired-20260920-014539/research/mapper"
sys.path.insert(0, MAPPER)

spec = importlib.util.spec_from_file_location("run_e21", os.path.join(MAPPER, "run_e21.py"))
e21 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e21)

FRACS = (0.10, 0.25, 0.50, 0.75)
rows = e21.load()
import random

N_SHUFFLES = 1000

out = {}
for frac in FRACS:
    n = max(2, int(frac * len(rows)))
    tr, te = rows[:n], rows[n:]
    if not te:
        continue
    cent, _ = e21.centroids(tr)
    order = [e21.rank_cells(r["v"], cent) for r in te]
    res = {f"top{k}": sum(r["new"] in o[:k] for r, o in zip(te, order)) / len(te)
           for k in (1, 3, 5)}

    # permuted-label null under the SAME sequential split, using the same
    # construction as run_e21.topk(mode="permuted"): shuffle the training
    # labels, keep the features, rebuild centroids, predict the same test set.
    null = {f"top{k}": 0.0 for k in (1, 3, 5)}
    rng = random.Random(0)
    for _ in range(N_SHUFFLES):
        labs = [r["new"] for r in tr]
        rng.shuffle(labs)
        ptr = [dict(r, new=l) for r, l in zip(tr, labs)]
        pcent, _ = e21.centroids(ptr)
        porder = [e21.rank_cells(r["v"], pcent) for r in te]
        for k in (1, 3, 5):
            null[f"top{k}"] += sum(r["new"] in o[:k] for r, o in zip(te, porder)) / len(te)
    for k in (1, 3, 5):
        null[f"top{k}"] /= N_SHUFFLES
    res["null"] = null
    res["n_shuffles"] = N_SHUFFLES

    # in-order baselines, same rows
    from collections import Counter, defaultdict
    ln = defaultdict(list)
    for r in tr:
        ln[r["new"]].append(len(r["name"]))
    mu = {c: sum(v) / len(v) for c, v in ln.items()}
    lenorder = [[c for c, _ in sorted(mu.items(), key=lambda t: abs(t[1] - len(r["name"])))]
                for r in te]
    tab = defaultdict(Counter)
    for r in tr:
        tab[r["old"]][r["new"]] += 1
    glob = Counter(r["new"] for r in tr)
    priororder = [[c for c, _ in (tab[r["old"]] or glob).most_common()]
                  + [c for c, _ in glob.most_common()] for r in te]
    predorder = [[c for c, _ in glob.most_common()] for _ in te]

    res["prior"] = {f"top{k}": sum(r["new"] in o[:k] for r, o in zip(te, priororder)) / len(te)
                    for k in (1, 3, 5)}
    res["majority"] = {f"top{k}": sum(r["new"] in o[:k] for r, o in zip(te, predorder)) / len(te)
                       for k in (1, 3, 5)}
    res["len_only"] = {f"top{k}": sum(r["new"] in o[:k] for r, o in zip(te, lenorder)) / len(te)
                       for k in (1, 3, 5)}
    res["n_train"] = n
    res["n_test"] = len(te)
    out[f"{frac}"] = res

dest = "/home/gio/dev/physis-pro/research/e21_sequential_split.json"
json.dump({"split": "sequential (file order)", "n_rows": len(rows), "results": out},
          open(dest, "w"), indent=2)

print(f"rows {len(rows)}  split=sequential(file order)  null={N_SHUFFLES} shuffles")
print(f"{'frac':>6s} {'n_train':>8s} {'n_test':>7s} {'top1':>7s} {'top3':>7s} {'top5':>7s}"
      f" | {'null1':>7s} {'null3':>7s} {'null5':>7s} | {'p3-null3':>8s}")
for frac in FRACS:
    r = out.get(f"{frac}")
    if not r:
        continue
    print(f"{frac:>6.2f} {r['n_train']:>8d} {r['n_test']:>7d} "
          f"{r['top1']:>7.3f} {r['top3']:>7.3f} {r['top5']:>7.3f} | "
          f"{r['null']['top1']:>7.3f} {r['null']['top3']:>7.3f} {r['null']['top5']:>7.3f} | "
          f"{r['top3'] - r['null']['top3']:>+8.3f}")

print()
print("compare with the published RANDOM split at frac 0.75:")
print("  random : geometry top-1/3/5 = 0.438 / 0.667 / 0.755   (5-seed mean)")
print("  random : geometry top-1/3/5 = 0.419 / 0.668 / 0.769   (1000 splits)")
print(f"  seq    : geometry top-1/3/5 = {out['0.75']['top1']:.3f} / "
      f"{out['0.75']['top3']:.3f} / {out['0.75']['top5']:.3f}")
print(f"wrote {dest}")
assert out["0.75"]["top3"] > out["0.75"]["majority"]["top3"], "geometry no longer beats majority in order"
print("sequential split still beats the majority baseline")
