#!/usr/bin/env python3
"""Paper 9 (dataset release): label-granularity kill test + corpus statistics.

Kill test from the portfolio: "If a simple baseline achieves comparable
prediction to the full model, the dataset's value is limited."

A constant global-failure-rate predictor has AUC 0.5 by construction, so
that comparison is uninformative. The informative version: does the
fine-grained `kind` label (871 kinds) do better than coarser labels the
dataset could have shipped instead — `tool` (133 tools) and the raw first
token of the kind? If a coarse label matches `kind`, the fine label earns
nothing and the dataset should ship the coarse one.

Writes research/dataset_baselines.json
"""
import gzip, json, os, numpy as np
from collections import defaultdict
from sklearn.metrics import roc_auc_score

REPO = "/home/gio/dev/physis-pro"
os.chdir(REPO)

with gzip.open("benchmarks/behaviour/corpus-redacted.json.gz", "rb") as fh:
    data = json.loads(fh.read())
sessions = data["sessions"]

by_harness = defaultdict(list)
for s in sessions:
    by_harness[s["harness"]].append(s)


def key_of(a, level):
    if level == "kind":
        return a.get("kind", "?")
    if level == "tool":
        return a.get("tool", "?")
    if level == "kind_prefix":          # coarse: bash:git -> bash
        return a.get("kind", "?").split(":")[0]
    raise ValueError(level)


def rates(session_list, level):
    st = defaultdict(lambda: [0, 0])
    for s in session_list:
        for a in s.get("actions", []):
            st[key_of(a, level)][0] += 1
            if a.get("marker_error", False):
                st[key_of(a, level)][1] += 1
    return st


def loo_auc(session_list, level):
    """Leave-one-session-out AUC for a per-key prior at the given granularity."""
    scores, labels, sids = [], [], []
    for i, held in enumerate(session_list):
        st = rates(session_list[:i] + session_list[i + 1:], level)
        for a in held.get("actions", []):
            e = st.get(key_of(a, level))
            p = e[1] / e[0] if e and e[0] > 0 else 0.5
            scores.append(p)
            labels.append(1 if a.get("marker_error", False) else 0)
            sids.append(i)
    if len(set(labels)) < 2:
        return None, len(labels), None
    return float(roc_auc_score(labels, scores)), len(labels), np.array(sids)


out = {"corpus": {}, "levels": {}, "tools": {}, "kinds": {}}

# corpus statistics
out["corpus"] = {
    "sessions": len(sessions),
    "actions": sum(len(s.get("actions", [])) for s in sessions),
    "harnesses": {h: {"sessions": len(v), "actions": sum(len(s.get("actions", [])) for s in v)}
                  for h, v in by_harness.items()},
    "machines": {},
    "distinct_kinds": len({a.get("kind") for s in sessions for a in s.get("actions", [])}),
    "distinct_tools": len({a.get("tool") for s in sessions for a in s.get("actions", [])}),
}
# session size distribution: cited in paper 9 and paper 1 (the clustering argument)
_sizes = sorted(len(s.get("actions", [])) for s in sessions)
_n = len(_sizes)
out["corpus"]["session_sizes"] = {
    "median": float(np.median(_sizes)),
    "max": int(_sizes[-1]),
    "min": int(_sizes[0]),
    "p90": float(np.percentile(_sizes, 90)),
    "n": _n,
}
for s in sessions:
    m = s.get("machine")
    if m:
        out["corpus"]["machines"][m] = out["corpus"]["machines"].get(m, 0) + 1

# label-granularity comparison
for level in ("kind", "tool", "kind_prefix"):
    row = {}
    all_s, all_l = [], []
    for h in sorted(by_harness):
        auc, n, _ = loo_auc(by_harness[h], level)
        row[h] = {"auc": auc, "n_actions": n}
        st = rates(by_harness[h], level)
        scores = []
        labels = []
        for s in by_harness[h]:
            for a in s.get("actions", []):
                e = st.get(key_of(a, level))
                scores.append(e[1] / e[0] if e[0] > 0 else 0.5)
                labels.append(1 if a.get("marker_error", False) else 0)
        if len(set(labels)) >= 2:
            all_s.append(auc if auc is not None else 0.5)
    out["levels"][level] = row
    out["levels"][level]["n_keys"] = len(rates(sessions, level))

# most and least failure-prone kinds (corpus documentation)
st = rates(sessions, "kind")
freq = sorted(((k, v[0], v[1] / v[0] if v[0] else 0.0) for k, v in st.items()),
              key=lambda r: -r[1])
out["kinds"]["top_by_frequency"] = [{"kind": k, "n": n, "rate": round(r, 4)} for k, n, r in freq[:15]]
risky = [r for r in freq if r[1] >= 20]
out["kinds"]["highest_rate_n20"] = [
    {"kind": k, "n": n, "rate": round(r, 4)}
    for k, n, r in sorted(risky, key=lambda r: -r[2])[:15]
]

dest = f"{REPO}/research/dataset_baselines.json"
json.dump(out, open(dest, "w"), indent=2)

print("=== LABEL GRANULARITY (leave-one-session-out AUC) ===")
print(f"{'harness':14s} {'kind(871)':>10s} {'tool(133)':>10s} {'prefix':>10s}")
for h in sorted(by_harness):
    k = out["levels"]["kind"][h]["auc"]
    t = out["levels"]["tool"][h]["auc"]
    p = out["levels"]["kind_prefix"][h]["auc"]
    if k is None:
        continue
    fmt = lambda v: f"{v:.3f}" if v is not None else "n/a"
    print(f"{h:14s} {fmt(k):>10s} {fmt(t):>10s} {fmt(p):>10s}")
print(f"\nkey counts: kind={out['levels']['kind']['n_keys']} "
      f"tool={out['levels']['tool']['n_keys']} prefix={out['levels']['kind_prefix']['n_keys']}")
print(f"corpus: {out['corpus']['sessions']} sessions, {out['corpus']['actions']} actions, "
      f"{out['corpus']['distinct_kinds']} kinds, {out['corpus']['distinct_tools']} tools")
print(f"wrote {dest}")
