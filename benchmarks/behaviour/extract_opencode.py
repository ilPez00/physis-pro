#!/usr/bin/env python3
"""Extract actions with outcomes from OpenCode's local database.

A fourth harness. OpenCode stores message parts in SQLite; a part of type
`tool` carries the tool name, its input, and a `state` whose `status` is
`completed` or `error` — an outcome the harness itself assigned, like Claude
Code's `is_error` and unlike Codex's real exit code.

    python3 benchmarks/behaviour/extract_opencode.py --corpus pansa --out actions-opencode.json

Reads the database read-only and copies nothing out of it but the action kind,
a truncated target, and the outcome.
"""
import argparse, collections, json, os, re, sqlite3, sys

MARKERS = re.compile(
    r"^(traceback \(most recent call last\)|error\[[a-z]\d+\]|error:|fatal:|"
    r"failed|failure|command not found|no such file or directory|"
    r"permission denied|panicked at|exit code: [1-9]|test result: failed)",
    re.I | re.M,
)
SHELL_TOOLS = {"bash", "shell", "run", "execute"}


def kind_of(tool, args):
    if tool in SHELL_TOOLS:
        command = str(args.get("command") or args.get("cmd") or "").strip()
        for token in command.split():
            if token in ("&&", "||", ";", "|"):
                continue
            if "=" in token and not token.startswith("-"):
                continue
            if token in ("cd", "sudo", "env", "time", "timeout", "nohup"):
                continue
            return f"bash:{os.path.basename(token)}"
        return "bash:?"
    return tool or "?"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=os.path.expanduser("~/.local/share/opencode/opencode.db"))
    ap.add_argument("--corpus", default=os.uname().nodename)
    ap.add_argument("--out", default="benchmarks/behaviour/actions-opencode.json")
    ap.add_argument("--min", type=int, default=20)
    args = ap.parse_args()

    if not os.path.exists(args.db):
        sys.exit(f"no database at {args.db}: nothing was extracted")
    con = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
    rows = con.execute(
        "select session_id, data from part order by session_id, time_created"
    ).fetchall()
    if not rows:
        sys.exit(f"0 parts in {args.db}: nothing was extracted")

    by_session, unreadable, without_state = collections.defaultdict(list), 0, 0
    for session_id, data in rows:
        try:
            part = json.loads(data)
        except Exception:
            unreadable += 1
            continue
        if part.get("type") != "tool":
            continue
        state = part.get("state") or {}
        status = state.get("status")
        # `pending` and `running` are actions still in flight: no outcome, so no
        # row, rather than a row scored as a success.
        if status not in ("completed", "error"):
            without_state += 1
            continue
        tool = part.get("tool", "?")
        inputs = state.get("input") or {}
        output = state.get("output")
        body = output if isinstance(output, str) else json.dumps(output or "")
        failed = status == "error"
        by_session[session_id].append({
            "tool": tool,
            "kind": kind_of(tool, inputs if isinstance(inputs, dict) else {}),
            "target": str(
                (inputs.get("command") or inputs.get("filePath") or inputs.get("path") or "")
                if isinstance(inputs, dict) else ""
            )[:120],
            "harness_error": failed,
            "marker_error": failed or bool(MARKERS.search(body)),
            "output_bytes": len(body),
        })

    sessions, dropped = [], 0
    for session_id, actions in by_session.items():
        if len(actions) < args.min:
            dropped += 1
            continue
        sessions.append({
            "session": f"opencode-{session_id}",
            "project": "opencode",
            "corpus": f"opencode:{args.corpus}",
            "actions": actions,
        })

    total = sum(len(s["actions"]) for s in sessions)
    if total == 0:
        sys.exit(f"0 actions over {len(by_session)} session(s): nothing was extracted")
    harness = sum(a["harness_error"] for s in sessions for a in s["actions"])
    marker = sum(a["marker_error"] for s in sessions for a in s["actions"])
    kinds = collections.Counter(a["kind"] for s in sessions for a in s["actions"])
    payload = {
        "transcripts_seen": len(by_session), "sessions_kept": len(sessions),
        "sessions_dropped_below_min": dropped, "min_actions": args.min,
        "actions": total, "unmatched_results": without_state,
        "unreadable_parts": unreadable,
        "harness_error_rate": harness / total, "marker_error_rate": marker / total,
        "distinct_action_kinds": len(kinds), "corpus": f"opencode:{args.corpus}",
        "sessions": sessions,
    }
    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    json.dump(payload, open(args.out, "w"), indent=1)
    print(f"sessions seen {len(by_session)} · kept {len(sessions)} · dropped {dropped} · "
          f"in-flight parts skipped {without_state} · unreadable {unreadable}")
    print(f"actions {total} · harness failures {harness} ({harness/total:.3f}) · "
          f"marker failures {marker} ({marker/total:.3f}) · {len(kinds)} kinds")
    print("most common:", ", ".join(f"{k}={n}" for k, n in kinds.most_common(6)))
    print(f"written to {args.out}")


if __name__ == "__main__":
    main()
