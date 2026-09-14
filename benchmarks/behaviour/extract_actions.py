#!/usr/bin/env python3
"""Extract actions *with outcomes* from Claude Code transcripts.

E67/E70 scored "which file next", and found that the directory tree explains it.
An adaptive world builder has to answer something a tree cannot: **what will
happen if I do this**. Every tool call in a transcript is paired with its
result, and the result carries whether it failed — which is the one place in
this data where reality, not the model, has the last word.

    python3 benchmarks/behaviour/extract_actions.py [--out actions.json]

Two labels, both reported, because one is clean and narrow and the other is
broad and noisy:

  harness   the transcript's own `is_error` flag on the tool result
  markers   harness OR a failure marker at the start of a line of output
            (Traceback, `error[E0433]`, FAILED, command not found, fatal:,
            panicked at, No such file). Conservative by design, still noisy:
            a command that *prints* the word error is counted.

Prints its denominators: transcripts, sessions, actions, unmatched results, and
the positive rate under each label.
"""
import argparse, collections, glob, json, os, re, sys

MARKERS = re.compile(
    r"^(traceback \(most recent call last\)|error\[[a-z]\d+\]|error:|fatal:|"
    r"failed|failure|command not found|no such file or directory|"
    r"permission denied|panicked at|error: could not|thread '.*' panicked|"
    r"exit code: [1-9]|test result: failed)",
    re.I | re.M,
)


def text_of(block):
    content = block.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(p.get("text", "") for p in content if isinstance(p, dict))
    return ""


def head_token(tool, args):
    """A coarse action type: the program for Bash, the tool name otherwise."""
    if tool == "Bash":
        command = (args.get("command") or "").strip()
        first = command.split()[0] if command.split() else "?"
        return f"bash:{os.path.basename(first)}"
    return tool


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.expanduser("~/.claude/projects"))
    ap.add_argument("--out", default="benchmarks/behaviour/actions.json")
    ap.add_argument("--min", type=int, default=20)
    args = ap.parse_args()

    files = sorted(glob.glob(os.path.join(args.root, "*", "*.jsonl")))
    if not files:
        sys.exit(f"0 transcripts under {args.root}: nothing was extracted")

    sessions, dropped, unmatched = [], 0, 0
    for path in files:
        pending, actions = {}, []
        for line in open(path, encoding="utf-8", errors="replace"):
            try:
                rec = json.loads(line)
            except Exception:
                continue
            content = (rec.get("message") or {}).get("content")
            if not isinstance(content, list):
                continue
            for block in content:
                kind = block.get("type")
                if kind == "tool_use":
                    a = block.get("input") or {}
                    pending[block.get("id")] = {
                        "tool": block.get("name", "?"),
                        "kind": head_token(block.get("name", "?"), a),
                        "target": a.get("file_path") or a.get("path") or
                                  (a.get("command") or "")[:120] or "",
                    }
                elif kind == "tool_result":
                    action = pending.pop(block.get("tool_use_id"), None)
                    if action is None:
                        unmatched += 1
                        continue
                    body = text_of(block)
                    harness = bool(block.get("is_error"))
                    action["harness_error"] = harness
                    action["marker_error"] = harness or bool(MARKERS.search(body))
                    action["output_bytes"] = len(body)
                    actions.append(action)
        if len(actions) < args.min:
            dropped += 1
            continue
        sessions.append(
            {
                "session": os.path.basename(path)[:-6],
                "project": os.path.basename(os.path.dirname(path)),
                "actions": actions,
            }
        )

    total = sum(len(s["actions"]) for s in sessions)
    if total == 0:
        sys.exit("0 actions over 0 sessions: nothing was extracted")
    harness = sum(a["harness_error"] for s in sessions for a in s["actions"])
    marker = sum(a["marker_error"] for s in sessions for a in s["actions"])
    kinds = collections.Counter(a["kind"] for s in sessions for a in s["actions"])

    payload = {
        "transcripts_seen": len(files),
        "sessions_kept": len(sessions),
        "sessions_dropped_below_min": dropped,
        "min_actions": args.min,
        "actions": total,
        "unmatched_results": unmatched,
        "harness_error_rate": harness / total,
        "marker_error_rate": marker / total,
        "distinct_action_kinds": len(kinds),
        "sessions": sessions,
    }
    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    json.dump(payload, open(args.out, "w"), indent=1)
    print(f"transcripts {payload['transcripts_seen']} · sessions kept {len(sessions)} "
          f"(>= {args.min} actions) · dropped {dropped} · unmatched results {unmatched}")
    print(f"actions {total} · harness failures {harness} ({harness/total:.3f}) · "
          f"marker failures {marker} ({marker/total:.3f}) · {len(kinds)} action kinds")
    print("most common kinds:", ", ".join(f"{k}={n}" for k, n in kinds.most_common(8)))
    print(f"written to {args.out}")


if __name__ == "__main__":
    main()
