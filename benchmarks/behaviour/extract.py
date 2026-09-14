#!/usr/bin/env python3
"""Extract agent file-touch sequences from Claude Code transcripts.

One record per session: the project it ran in, and the ordered list of files
the agent touched (Read/Edit/Write/NotebookEdit and any tool carrying a
`file_path`). This is the behaviour E67 and E70 are measured on, and it is
written to disk once so both experiments score the same population.

    python3 benchmarks/behaviour/extract.py [--out sessions.json] [--min 10]

Prints its denominators: transcripts seen, sessions kept, touches, and the
candidate-set size per project. A session shorter than --min is dropped and
counted, never silently skipped.
"""
import argparse, collections, glob, json, os, sys

TOOLS_WITH_PATHS = ("file_path", "notebook_path")


def touches(path):
    """Ordered (tool, file) pairs from one transcript. Malformed lines skipped
    and counted by the caller — a transcript is an append-only file that may be
    cut mid-write."""
    out, bad = [], 0
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            try:
                rec = json.loads(line)
            except Exception:
                bad += 1
                continue
            if rec.get("type") != "assistant":
                continue
            content = (rec.get("message") or {}).get("content")
            if not isinstance(content, list):
                continue
            for block in content:
                if block.get("type") != "tool_use":
                    continue
                args = block.get("input") or {}
                for key in TOOLS_WITH_PATHS:
                    if isinstance(args.get(key), str):
                        out.append((block.get("name", "?"), args[key]))
                        break
    return out, bad


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.expanduser("~/.claude/projects"))
    ap.add_argument("--out", default="benchmarks/behaviour/sessions.json")
    ap.add_argument("--min", type=int, default=10)
    args = ap.parse_args()

    files = sorted(glob.glob(os.path.join(args.root, "*", "*.jsonl")))
    if not files:
        sys.exit(f"0 transcripts under {args.root}: nothing was extracted")

    sessions, dropped, bad_lines = [], 0, 0
    for path in files:
        seq, bad = touches(path)
        bad_lines += bad
        if len(seq) < args.min:
            dropped += 1
            continue
        sessions.append(
            {
                "session": os.path.basename(path)[:-6],
                "project": os.path.basename(os.path.dirname(path)),
                "touches": [{"tool": t, "path": p} for t, p in seq],
            }
        )

    by_project = collections.Counter(s["project"] for s in sessions)
    candidates = {
        project: sorted({t["path"] for s in sessions if s["project"] == project for t in s["touches"]})
        for project in by_project
    }
    payload = {
        "transcripts_seen": len(files),
        "sessions_kept": len(sessions),
        "sessions_dropped_below_min": dropped,
        "min_touches": args.min,
        "total_touches": sum(len(s["touches"]) for s in sessions),
        "unparseable_lines": bad_lines,
        "candidate_set_sizes": {p: len(c) for p, c in candidates.items()},
        "sessions": sessions,
    }
    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    json.dump(payload, open(args.out, "w"), indent=1)

    print(f"transcripts seen        {payload['transcripts_seen']}")
    print(f"sessions kept (>= {args.min})  {payload['sessions_kept']}"
          f"  · dropped {dropped}  · unparseable lines {bad_lines}")
    print(f"touches                 {payload['total_touches']}")
    for project, n in by_project.most_common():
        print(f"  {project:42} {n:3} session(s) · {len(candidates[project]):4} distinct files")
    print(f"written to {args.out}")


if __name__ == "__main__":
    main()
