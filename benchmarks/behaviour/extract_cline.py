#!/usr/bin/env python3
"""Extract actions with outcomes from Cline session files.

A third harness. Cline stores Anthropic-shaped `tool_use` / `tool_result`
blocks in `~/.cline/data/sessions/<id>/<id>.messages.json`, so the pairing rule
is the same one `extract_actions.py` uses on Claude Code transcripts; only the
container differs.

    python3 benchmarks/behaviour/extract_cline.py --corpus pansa --out actions-cline.json

`is_error` is present on only a small share of results here (39 of 8342 in the
local store), so the marker label carries most of the signal for this harness —
which is stated rather than hidden, because a 0.005 positive rate would
otherwise look like a very reliable agent.
"""
import argparse, collections, glob, json, os, re, sys

MARKERS = re.compile(
    r"^(traceback \(most recent call last\)|error\[[a-z]\d+\]|error:|fatal:|"
    r"failed|failure|command not found|no such file or directory|"
    r"permission denied|panicked at|exit code: [1-9]|test result: failed|"
    r"\{\"error\")",
    re.I | re.M,
)


def first_command(args):
    """Cline batches shell work: `run_commands` takes a list. The first command
    is the action kind; a batch that fails anywhere is a failed action."""
    commands = args.get("commands")
    if isinstance(commands, list) and commands:
        return str(commands[0])
    for key in ("command", "cmd"):
        if isinstance(args.get(key), str):
            return args[key]
    return ""


def kind_of(name, args):
    if name in ("bash", "execute_command", "run_command", "run_commands"):
        command = first_command(args).strip()
        # `cd x && cargo test` is a cargo test; the shell prelude is not it.
        for token in command.split():
            if token in ("&&", "||", ";", "|") or "=" in token and not token.startswith("-"):
                continue
            if token in ("cd", "sudo", "env", "time", "timeout", "nohup"):
                continue
            return f"bash:{os.path.basename(token)}"
        return "bash:?"
    return name or "?"


def target_of(args):
    for key in ("path", "file_path", "command", "cmd", "pattern"):
        value = args.get(key)
        if isinstance(value, str) and value:
            return value[:120]
    files = args.get("files")
    if isinstance(files, list) and files:
        first = files[0]
        if isinstance(first, dict):
            return str(first.get("path", ""))[:120]
    return ""


def text_of(block):
    """Result text, including Cline's `{"query": …, "result": …}` batches —
    reading only `text` keys returned nothing for shell actions, which is how a
    0.005 failure rate over 8301 actions first looked like a very reliable
    agent."""
    content = block.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for p in content:
            if not isinstance(p, dict):
                continue
            for key in ("text", "result", "output", "error"):
                value = p.get(key)
                if isinstance(value, str):
                    parts.append(value)
        return "\n".join(parts)
    return ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.expanduser("~/.cline/data/sessions"))
    ap.add_argument("--corpus", default=os.uname().nodename)
    ap.add_argument("--out", default="benchmarks/behaviour/actions-cline.json")
    ap.add_argument("--min", type=int, default=20)
    args = ap.parse_args()

    files = sorted(glob.glob(os.path.join(args.root, "*", "*.messages.json")))
    if not files:
        sys.exit(f"0 session files under {args.root}: nothing was extracted")

    sessions, dropped, unmatched = [], 0, 0
    for path in files:
        try:
            data = json.load(open(path, encoding="utf-8", errors="replace"))
        except Exception:
            continue
        pending, actions = {}, []
        for message in data.get("messages", []):
            content = message.get("content")
            if not isinstance(content, list):
                continue
            for block in content:
                kind = block.get("type")
                if kind == "tool_use":
                    a = block.get("input") or {}
                    pending[block.get("id")] = {
                        "tool": block.get("name", "?"),
                        "kind": kind_of(block.get("name", "?"), a),
                        "target": (first_command(a)[:120] if block.get("name") in
                                   ("bash", "execute_command", "run_command", "run_commands")
                                   else target_of(a)),
                    }
                elif kind == "tool_result":
                    action = pending.pop(block.get("tool_use_id"), None)
                    if action is None:
                        unmatched += 1
                        continue
                    body = text_of(block)
                    flagged = bool(block.get("is_error"))
                    action["harness_error"] = flagged
                    action["marker_error"] = flagged or bool(MARKERS.search(body))
                    action["output_bytes"] = len(body)
                    actions.append(action)
        if len(actions) < args.min:
            dropped += 1
            continue
        sessions.append({
            "session": f"cline-{os.path.basename(path).split('.')[0]}",
            "project": data.get("origin", {}).get("cwd", "cline") if isinstance(data.get("origin"), dict) else "cline",
            "corpus": f"cline:{args.corpus}",
            "actions": actions,
        })

    total = sum(len(s["actions"]) for s in sessions)
    if total == 0:
        sys.exit(f"0 actions over {len(files)} session file(s): nothing was extracted")
    harness = sum(a["harness_error"] for s in sessions for a in s["actions"])
    marker = sum(a["marker_error"] for s in sessions for a in s["actions"])
    kinds = collections.Counter(a["kind"] for s in sessions for a in s["actions"])
    payload = {
        "transcripts_seen": len(files), "sessions_kept": len(sessions),
        "sessions_dropped_below_min": dropped, "min_actions": args.min,
        "actions": total, "unmatched_results": unmatched,
        "harness_error_rate": harness / total, "marker_error_rate": marker / total,
        "distinct_action_kinds": len(kinds), "corpus": f"cline:{args.corpus}",
        "sessions": sessions,
    }
    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    json.dump(payload, open(args.out, "w"), indent=1)
    print(f"session files {len(files)} · kept {len(sessions)} · dropped {dropped} · "
          f"unmatched {unmatched}")
    print(f"actions {total} · flagged failures {harness} ({harness/total:.4f}) · "
          f"marker failures {marker} ({marker/total:.3f}) · {len(kinds)} kinds")
    print("most common:", ", ".join(f"{k}={n}" for k, n in kinds.most_common(6)))
    print(f"written to {args.out}")


if __name__ == "__main__":
    main()
