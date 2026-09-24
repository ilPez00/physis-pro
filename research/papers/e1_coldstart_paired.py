#!/usr/bin/env python3
"""P1's cold-start effect, evaluated without leakage.

The original curve (e74_replication.py) built the local prior from target
sessions and scored it on those same sessions, so the own-history and mixed
arms were contaminated and both looked like wins. This re-runs the question.

Protocol:
  * the target harness's sessions are split ONCE, seeded, into a training
    pool and a held-out test set;
  * every arm is scored on the SAME held-out test set;
  * N is swept over the TRAINING pool only, so no held-out session ever
    contributes to a prior;
  * comparison is paired -- one resample of test sessions, both arms scored
    on it, difference recorded, repeated -- so the CI is on the DIFFERENCE.

Arms:
  A  local-only      prior from the target's first N training sessions
  B  borrowed-only   prior from another harness, no target data at all
  C  mixed           local + borrowed counts pooled
  D  pooled-others   prior from ALL other harnesses pooled (global baseline)

Priors are {kind: [total, fail]} -- the shape check_bootstrap_unit.collect()
expects. Reusing its estimator keeps one implementation.

  python3 research/papers/e1_coldstart_paired.py
"""
import importlib.util, json, sys
from collections import defaultdict
import numpy as np

REPO = "/home/gio/dev/physis-pro"
TARGET = "claude-code"
SOURCE = "opencode"
N_TRAIN = 30          # sessions in the training pool
N_TEST = 12           # sessions held out
N_BOOT = 2000
SEED = 42

spec = importlib.util.spec_from_file_location(
    "cbu", f"{REPO}/research/papers/check_bootstrap_unit.py")
if spec is None or spec.loader is None:
    sys.exit("cannot load check_bootstrap_unit.py")
cbu = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cbu)          # module-level corpus load; main() guarded
AUC = cbu.roc_auc_score

rng = np.random.default_rng(SEED)
tgt = list(cbu.by_h[TARGET])
rng.shuffle(tgt)
train_pool, test_set = tgt[:N_TRAIN], tgt[N_TRAIN:N_TRAIN + N_TEST]
print(f"target={TARGET}: {len(tgt)} sessions -> {len(train_pool)} train / "
      f"{len(test_set)} held out   source={SOURCE}")
if len(train_pool) < N_TRAIN or len(test_set) < 2:
    sys.exit("not enough target sessions for this split")


def merge(*priors):
    """Pool {kind: [total, fail]} priors additively."""
    out = defaultdict(lambda: [0, 0])
    for p in priors:
        for k, v in p.items():
            out[k][0] += v[0]
            out[k][1] += v[1]
    return {k: [t, f] for k, (t, f) in out.items()}


SOURCE_STATS = cbu.key_stats(cbu.by_h[SOURCE])
OTHERS = merge(*[cbu.key_stats(cbu.by_h[h]) for h in cbu.harn if h != TARGET])


def paired_ci(sc_a, sc_b, labels, sids, n=N_BOOT):
    """CI on AUC(a) - AUC(b), resampling test sessions."""
    groups = defaultdict(list)
    for i, s in enumerate(sids):
        groups[int(s)].append(i)
    keys = list(groups)
    r = np.random.default_rng(SEED)
    out = []
    for _ in range(n):
        drawn = r.choice(len(keys), len(keys), replace=True)
        idx = np.concatenate([groups[keys[d]] for d in drawn])
        if len(set(labels[idx])) < 2:
            continue
        out.append(AUC(labels[idx], sc_a[idx]) - AUC(labels[idx], sc_b[idx]))
    if not out:
        return None, None, None
    a = np.array(out)
    return float(a.mean()), float(np.percentile(a, 2.5)), float(np.percentile(a, 97.5))


results = []
for n_local in (0, 2, 5, 10, 20, N_TRAIN):
    local = cbu.key_stats(train_pool[:n_local]) if n_local else {}
    # Arm A is UNDEFINED at n_local=0: there is no local history to build a
    # prior from. We record null rather than substituting a placeholder, since
    # a placeholder would be scored as if it were the local arm.
    arms = {
        "A_local":  local if n_local else None,
        "B_borrow": SOURCE_STATS,
        "C_mixed":  merge(local, SOURCE_STATS) if n_local else SOURCE_STATS,
        "D_pool":   merge(local, OTHERS) if n_local else OTHERS,
    }
    sc = {name: (cbu.collect(test_set, p) if p is not None else None)
          for name, p in arms.items()}
    row = {"n_local_train_sessions": n_local, "n_test_sessions": len(test_set),
           "n_test_actions": int(len(sc["B_borrow"][1])),
           "A_local_defined": n_local > 0}
    for name, res in sc.items():
        if res is None:
            row[f"auc_{name}"] = None
            continue
        s, l, _sid = res
        row[f"auc_{name}"] = float(AUC(l, s)) if len(set(l)) >= 2 else None
    for name in ("A_local", "C_mixed", "D_pool"):
        if sc[name] is None:
            row[f"diff_{name}_minus_B"] = {"mean": None, "lo": None, "hi": None,
                                           "excludes_zero": None,
                                           "note": "arm undefined at this N"}
            continue
        s_a, l_a, sid_a = sc[name]
        s_b, l_b, _ = sc["B_borrow"]
        m, lo, hi = paired_ci(s_a, s_b, l_a, sid_a)
        row[f"diff_{name}_minus_B"] = {
            "mean": m, "lo": lo, "hi": hi,
            "excludes_zero": bool(lo is not None and hi is not None and (lo > 0 or hi < 0)),
        }
    results.append(row)
    print(f"\nn_local={n_local:<3d} test_actions={row['n_test_actions']}")
    for name in ("A_local", "B_borrow", "C_mixed", "D_pool"):
        v = row[f"auc_{name}"]
        print(f"    {name:9s} auc={'undefined' if v is None and not row['A_local_defined'] and name == 'A_local' else ('None' if v is None else f'{v:.3f}')}")
    for name in ("A_local", "C_mixed", "D_pool"):
        d = row[f"diff_{name}_minus_B"]
        if d["mean"] is not None:
            print(f"    {name:9s} minus B: {d['mean']:+.3f} "
                  f"[{d['lo']:+.3f}, {d['hi']:+.3f}]  "
                  f"{'EXCLUDES 0' if d['excludes_zero'] else 'spans 0'}")
        else:
            print(f"    {name:9s} minus B: n/a ({d.get('note', 'undefined')})")

dest = f"{REPO}/research/e1_coldstart_paired.json"
json.dump({
    "target": TARGET, "source": SOURCE,
    "split": {"n_train_pool": N_TRAIN, "n_test": N_TEST, "seed": SEED,
              "note": "single seeded split; N swept over the TRAINING pool "
                      "only, so no held-out session contributes to a prior"},
    "inference_unit": "session",
    "n_boot": N_BOOT,
    "arms": {"A_local": "prior from the target's first N training sessions",
             "B_borrow": f"prior from {SOURCE} only, no target data",
             "C_mixed": "local + borrowed counts pooled",
             "D_pool": "prior from all other harnesses pooled"},
    "results": results,
}, open(dest, "w"), indent=2)

n_clean = sum(1 for r in results for a in ("A_local", "C_mixed", "D_pool")
              if r[f"diff_{a}_minus_B"]["excludes_zero"])
print(f"\npaired differences excluding zero: {n_clean} of {len(results) * 3}")
print(f"wrote {dest}")
