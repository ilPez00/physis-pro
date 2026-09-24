#!/usr/bin/env python3
"""E74 replication from the committed redacted corpus.

Regenerates research/e74_replication_results.json containing the four
quantities the manuscript cites: cross-harness transfer AUCs with bootstrap
CIs, in-corpus leave-one-session-out baselines, the cold-start curve, and
coverage-vs-transfer.

Fixed 2026-09-24: previous version persisted in_corpus as
roc_auc_score(labels, [0.5]*N), which is identically 0.5 by construction.
It also printed the cold-start curve and coverage without saving them.
Both are persisted here.

Run from the repo root: python3 research/papers/e74_replication.py
"""
import gzip, json, os, numpy as np
from collections import defaultdict
from sklearn.metrics import roc_auc_score

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
os.chdir(REPO)

with gzip.open("benchmarks/behaviour/corpus-redacted.json.gz", "rb") as fh:
    data = json.loads(fh.read())
sessions = data["sessions"]

harness_sessions = defaultdict(list)
for s in sessions:
    harness_sessions[s["harness"]].append(s)
harnesses = sorted(harness_sessions)


def build_kind_stats(session_list):
    stats = defaultdict(lambda: {"total": 0, "fail": 0})
    for s in session_list:
        for a in s.get("actions", []):
            k = a.get("kind", "unknown")
            stats[k]["total"] += 1
            if a.get("marker_error", False):
                stats[k]["fail"] += 1
    return stats


harness_kind_stats = {h: build_kind_stats(harness_sessions[h]) for h in harnesses}


def predict(target_sessions, stats):
    """Return (scores, labels, session_ids) with session_ids aligned per action."""
    scores, labels, sids = [], [], []
    for si, s in enumerate(target_sessions):
        for a in s.get("actions", []):
            k = a.get("kind", "unknown")
            e = stats.get(k)
            p = e["fail"] / e["total"] if e and e["total"] > 0 else 0.5
            scores.append(p)
            labels.append(1 if a.get("marker_error", False) else 0)
            sids.append(si)
    return np.array(scores), np.array(labels), np.array(sids)


def boot_ci(labels, scores, sids, n=500, seed=42):
    """Bootstrap over SESSIONS, not actions.

    Actions are clustered within sessions, so resampling actions would treat
    correlated observations as independent and understate the interval.
    Each resample draws sessions with replacement and pools their actions.
    """
    rng = np.random.default_rng(seed)
    by_session = {}
    for i, s in enumerate(sids):
        by_session.setdefault(int(s), []).append(i)
    session_ids = list(by_session)
    out = []
    for _ in range(n):
        drawn = rng.choice(len(session_ids), len(session_ids), replace=True)
        idx = np.concatenate([by_session[session_ids[d]] for d in drawn])
        if len(set(labels[idx])) < 2:
            continue
        out.append(roc_auc_score(labels[idx], scores[idx]))
    if not out:
        return float("nan"), float("nan")
    return float(np.percentile(out, 2.5)), float(np.percentile(out, 97.5))


# --- cross-harness transfer, bootstrap over target SESSIONS ---
transfer = {}
for src in harnesses:
    for tgt in harnesses:
        if src == tgt:
            continue
        s, l, sid = predict(harness_sessions[tgt], harness_kind_stats[src])
        if len(set(l)) < 2:
            transfer[f"{src}->{tgt}"] = {"auc": None, "lo": None, "hi": None}
            continue
        lo, hi = boot_ci(l, s, sid)
        transfer[f"{src}->{tgt}"] = {"auc": float(roc_auc_score(l, s)), "lo": lo, "hi": hi}

# --- in-corpus leave-one-session-out (the quantity that was saved as 0.5) ---
in_corpus = {}
for h in harnesses:
    sl = harness_sessions[h]
    if len(sl) < 2:
        continue
    scores, labels, sids = [], [], []
    for i, held in enumerate(sl):
        train_stats = build_kind_stats(sl[:i] + sl[i + 1:])
        s, l, _ = predict([held], train_stats)
        scores.extend(s)
        labels.extend(l)
        sids.extend([i] * len(l))
    if len(set(labels)) < 2:
        in_corpus[h] = None
        continue
    lo, hi = boot_ci(np.array(labels), np.array(scores), np.array(sids))
    in_corpus[h] = {
        "auc": float(roc_auc_score(labels, scores)),
        "lo": lo,
        "hi": hi,
        "n_sessions": len(sl),
        "n_actions": len(labels),
    }

# --- cold-start curve: opencode prior -> claude-code target ---
rng = np.random.default_rng(42)
tgt = list(harness_sessions["claude-code"])
rng.shuffle(tgt)
src_stats = harness_kind_stats["opencode"]
coldstart = []
for n in [0, 2, 5, 10, 20, 30, len(tgt)]:
    if n == 0:
        s, l, sid = predict(tgt, src_stats)
        label = "transferred_only"
    elif n == len(tgt):
        s, l, sid = predict(tgt, build_kind_stats(tgt))
        label = "own_only"
    else:
        own = build_kind_stats(tgt[:n])
        combined = {}
        for k in set(own) | set(src_stats):
            ot, of = own.get(k, {"total": 0, "fail": 0})["total"], own.get(k, {"total": 0, "fail": 0})["fail"]
            st, sf = src_stats.get(k, {"total": 0, "fail": 0})["total"], src_stats.get(k, {"total": 0, "fail": 0})["fail"]
            combined[k] = {"total": ot + st, "fail": of + sf} if ot + st > 0 else {"total": 1, "fail": 0}
        s, l, sid = predict(tgt, combined)
        label = f"mixed_n{n}"
    lo, hi = boot_ci(l, s, sid)
    coldstart.append({
        "label": label,
        "n_target_sessions": min(n, len(tgt)),
        "auc": float(roc_auc_score(l, s)) if len(set(l)) >= 2 else None,
        "lo": lo,
        "hi": hi,
        "n_actions": int(len(l)),
        "n_sessions": len(tgt),
    })

# --- coverage vs transfer ---
coverage = []
for src in harnesses:
    for tgt_h in harnesses:
        if src == tgt_h:
            continue
        src_kinds = set(harness_kind_stats[src])
        total = sum(len(s.get("actions", [])) for s in harness_sessions[tgt_h])
        covered = sum(
            1 for s in harness_sessions[tgt_h] for a in s.get("actions", [])
            if a.get("kind", "unknown") in src_kinds
        )
        coverage.append({
            "pair": f"{src}->{tgt_h}",
            "coverage": float(covered / total) if total else 0.0,
            "transfer_auc": transfer[f"{src}->{tgt_h}"]["auc"],
        })

out = {
    "corpus": {
        "sessions": len(sessions),
        "actions": sum(len(s.get("actions", [])) for s in sessions),
        "harnesses": {h: {"sessions": len(harness_sessions[h]),
                          "actions": sum(len(s.get("actions", [])) for s in harness_sessions[h])}
                      for h in harnesses},
        "label_rates": {
            h: {
                "marker_error": float(np.mean([1 if a.get("marker_error", False) else 0
                                               for s in harness_sessions[h] for a in s.get("actions", [])])),
                "harness_error": float(np.mean([1 if a.get("harness_error", False) else 0
                                                for s in harness_sessions[h] for a in s.get("actions", [])])),
            } for h in harnesses
        },
        "merged_marker_error": float(np.mean([1 if a.get("marker_error", False) else 0
                                              for s in sessions for a in s.get("actions", [])])),
        "source": "benchmarks/behaviour/corpus-redacted.json.gz",
    },
    "transfer_matrix": transfer,
    "in_corpus": in_corpus,
    "coldstart": coldstart,
    "coverage": coverage,
}

dest = "research/e74_replication_results.json"
with open(dest, "w") as fh:
    json.dump(out, fh, indent=2)

# self-check: the bug being fixed must not reappear
assert all(v != 0.5 for v in (x["auc"] for x in in_corpus.values() if x)), "in_corpus collapsed to 0.5 again"
print(f"wrote {dest}")
print("harnesses:", {h: out['corpus']['harnesses'][h]['sessions'] for h in harnesses})
print("in_corpus:", {h: (round(v['auc'], 3) if v else None) for h, v in in_corpus.items()})
print("coldstart:", [(c['label'], round(c['auc'], 3) if c['auc'] else None) for c in coldstart])
