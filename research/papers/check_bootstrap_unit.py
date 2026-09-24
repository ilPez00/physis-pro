#!/usr/bin/env python3
"""Check paper 1's claim: how many transfer pairs exclude chance under
action-level vs session-level bootstrap?

The manuscript asserts (abstract and Section 4) that the action-level
bootstrap yields four of twelve established pairs while the session-level
yields one. That is a load-bearing methodological claim, so it must be
recomputed rather than asserted.
"""
import gzip, json
import numpy as np
from collections import defaultdict
from sklearn.metrics import roc_auc_score

REPO = "/home/gio/dev/physis-pro"
with gzip.open(f"{REPO}/benchmarks/behaviour/corpus-redacted.json.gz", "rb") as fh:
    sessions = json.loads(fh.read())["sessions"]

by_h = defaultdict(list)
for s in sessions:
    by_h[s["harness"]].append(s)
harn = sorted(by_h)


def key_stats(sl):
    st = defaultdict(lambda: [0, 0])
    for s in sl:
        for a in s.get("actions", []):
            st[a.get("kind", "?")][0] += 1
            if a.get("marker_error"):
                st[a.get("kind", "?")][1] += 1
    return st


def collect(tgt, stats):
    sc, lb, sid = [], [], []
    for i, s in enumerate(tgt):
        for a in s.get("actions", []):
            e = stats.get(a.get("kind", "?"))
            sc.append(e[1] / e[0] if e and e[0] else 0.5)
            lb.append(1 if a.get("marker_error") else 0)
            sid.append(i)
    return np.array(sc), np.array(lb), np.array(sid)


def boot(sc, lb, sid, unit, n=500, seed=42):
    rng = np.random.default_rng(seed)
    groups = {}
    for i, s in enumerate(sid):
        groups.setdefault(int(s), []).append(i)
    keys = list(groups)
    out = []
    for _ in range(n):
        if unit == "session":
            drawn = rng.choice(len(keys), len(keys), replace=True)
            idx = np.concatenate([groups[keys[d]] for d in drawn])
        else:
            idx = rng.choice(len(lb), len(lb), replace=True)
        if len(set(lb[idx])) < 2:
            continue
        out.append(roc_auc_score(lb[idx], sc[idx]))
    if not out:
        return None, None
    return float(np.percentile(out, 2.5)), float(np.percentile(out, 97.5))


stats = {h: key_stats(by_h[h]) for h in harn}
res = {"pairs": {}, "counts": {}}
for unit in ("action", "session"):
    excl = 0
    for src in harn:
        for tgt in harn:
            if src == tgt:
                continue
            sc, lb, sid = collect(by_h[tgt], stats[src])
            lo, hi = boot(sc, lb, sid, unit)
            name = f"{src}->{tgt}"
            if unit == "action":
                res["pairs"].setdefault(name, {})["action"] = {"lo": lo, "hi": hi}
            else:
                res["pairs"].setdefault(name, {})["session"] = {"lo": lo, "hi": hi}
            if lo is not None and lo > 0.5:
                excl += 1
    res["counts"][unit] = excl

json.dump(res, open(f"{REPO}/research/bootstrap_unit_check.json", "w"), indent=2)

print(f"pairs whose 95% CI excludes chance (lo > 0.5), of 12:")
print(f"  action-level bootstrap : {res['counts']['action']}")
print(f"  session-level bootstrap: {res['counts']['session']}")
print()
for name, d in sorted(res["pairs"].items()):
    a, s = d["action"], d["session"]
    fa = "excl" if a["lo"] and a["lo"] > 0.5 else "incl"
    fs = "excl" if s["lo"] and s["lo"] > 0.5 else "incl"
    print(f"  {name:26s} action[{a['lo']:.3f},{a['hi']:.3f}] {fa}   "
          f"session[{s['lo']:.3f},{s['hi']:.3f}] {fs}")
