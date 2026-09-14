#!/usr/bin/env python3
"""Merge action corpora from several machines and harnesses into one file.

Transcripts are partly synced between machines: of 42 distinct Claude Code
sessions across pansa, omo and feather, 18 are byte-identical copies of the same
session. Pooling the files naively would count those three times and report a
corpus 1.6x larger than it is. Sessions are keyed by id; the first file that
carries one wins, and every session keeps the list of machines it was found on.

    python3 benchmarks/behaviour/merge.py benchmarks/behaviour/actions-*.json \
        --out benchmarks/behaviour/actions-merged.json
"""
import argparse, collections, json, os, sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--out", default="benchmarks/behaviour/actions-merged.json")
    args = ap.parse_args()

    merged, seen_on, duplicates = {}, collections.defaultdict(set), 0
    for path in args.files:
        if not os.path.exists(path):
            print(f"skipped (absent): {path}", file=sys.stderr)
            continue
        data = json.load(open(path))
        for session in data["sessions"]:
            sid = session["session"]
            seen_on[sid].add(session.get("corpus", data.get("corpus", "?")))
            if sid in merged:
                duplicates += 1
                continue
            merged[sid] = session
    if not merged:
        sys.exit("0 sessions merged: nothing was written")

    for sid, session in merged.items():
        session["seen_on"] = sorted(seen_on[sid])
        session["harness"] = "codex" if session.get("project") == "codex" else "claude-code"

    sessions = list(merged.values())
    total = sum(len(s["actions"]) for s in sessions)
    harness_fail = sum(a["harness_error"] for s in sessions for a in s["actions"])
    marker_fail = sum(a["marker_error"] for s in sessions for a in s["actions"])
    by_harness = collections.Counter(s["harness"] for s in sessions)
    kinds = collections.Counter(a["kind"] for s in sessions for a in s["actions"])

    payload = {
        "sessions_kept": len(sessions),
        "duplicate_sessions_dropped": duplicates,
        "actions": total,
        "harness_error_rate": harness_fail / total,
        "marker_error_rate": marker_fail / total,
        "distinct_action_kinds": len(kinds),
        "sessions_by_harness": dict(by_harness),
        "sessions": sessions,
    }
    json.dump(payload, open(args.out, "w"), indent=1)
    print(f"merged {len(args.files)} file(s) → {len(sessions)} session(s), "
          f"{duplicates} duplicate session(s) dropped")
    print(f"actions {total} · harness failures {harness_fail} ({harness_fail/total:.3f}) · "
          f"marker failures {marker_fail} ({marker_fail/total:.3f}) · kinds {len(kinds)}")
    print("by harness:", dict(by_harness))
    print("machines:", dict(collections.Counter(m for s in sessions for m in s["seen_on"])))
    print(f"written to {args.out}")


if __name__ == "__main__":
    main()
