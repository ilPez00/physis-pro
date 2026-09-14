#!/usr/bin/env python3
"""Extract actions with outcomes from Codex rollout logs.

A second harness, on the same machines. Codex records `custom_tool_call` /
`custom_tool_call_output` pairs, and the output carries a real `exit_code`
rather than a harness flag — a cleaner label than the Claude Code transcripts
give, which is the point of reading more than one corpus.

    python3 benchmarks/behaviour/extract_codex.py --corpus omo --out actions-codex-omo.json

Emits the schema `extract_actions.py` emits, so the same experiments score both
without knowing which harness they came from.
"""
import argparse, collections, glob, json, os, re, sys

MARKERS = re.compile(
    r"^(traceback \(most recent call last\)|error\[[a-z]\d+\]|error:|fatal:|"
    r"failed|failure|command not found|no such file or directory|"
    r"permission denied|panicked at|exit code: [1-9]|test result: failed)",
    re.I | re.M,
)
CMD = re.compile(r'cmd\s*:\s*"((?:[^"\\]|\\.)*)"')
EXIT = re.compile(r'"exit_code"\s*:\s*(-?\d+)')


def kind_of(command):
    words = command.strip().split()
    if not words:
        return "bash:?"
    # `cd x && cargo test` is a cargo test; the shell prelude is not the action.
    for token in words:
        if token in ("&&", "||", ";", "|"):
            continue
        if "=" in token and not token.startswith("-"):
            continue
        if token in ("cd", "sudo", "env", "time", "timeout", "nohup"):
            continue
        return f"bash:{os.path.basename(token)}"
    return f"bash:{os.path.basename(words[0])}"


def output_text(payload):
    out = payload.get("output")
    if isinstance(out, str):
        return out
    if isinstance(out, list):
        return "\n".join(p.get("text", "") for p in out if isinstance(p, dict))
    return ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.expanduser("~/.codex/sessions"))
    ap.add_argument("--corpus", default=os.uname().nodename)
    ap.add_argument("--out", default="benchmarks/behaviour/actions-codex.json")
    ap.add_argument("--min", type=int, default=10)
    args = ap.parse_args()

    files = sorted(glob.glob(os.path.join(args.root, "**", "*.jsonl"), recursive=True))
    if not files:
        sys.exit(f"0 rollouts under {args.root}: nothing was extracted")

    sessions, dropped, unmatched = [], 0, 0
    for path in files:
        calls, actions = {}, []
        for line in open(path, encoding="utf-8", errors="replace"):
            try:
                rec = json.loads(line)
            except Exception:
                continue
            if rec.get("type") != "response_item":
                continue
            payload = rec.get("payload") or {}
            kind = payload.get("type")
            if kind == "custom_tool_call":
                command = CMD.search(payload.get("input") or "")
                calls[payload.get("call_id")] = {
                    "tool": payload.get("name", "?"),
                    "kind": kind_of(command.group(1)) if command else payload.get("name", "?"),
                    "target": (command.group(1)[:120] if command else ""),
                }
            elif kind == "custom_tool_call_output":
                action = calls.pop(payload.get("call_id"), None)
                if action is None:
                    unmatched += 1
                    continue
                body = output_text(payload)
                exit_code = EXIT.search(body)
                # A real exit status when the harness reported one; otherwise
                # fall back to markers and say so by leaving harness_error False.
                failed = bool(exit_code and int(exit_code.group(1)) != 0)
                action["harness_error"] = failed
                action["marker_error"] = failed or bool(MARKERS.search(body))
                action["output_bytes"] = len(body)
                actions.append(action)
        if len(actions) < args.min:
            dropped += 1
            continue
        sessions.append({
            "session": os.path.basename(path)[:-6],
            "project": "codex",
            "corpus": f"codex:{args.corpus}",
            "actions": actions,
        })

    total = sum(len(s["actions"]) for s in sessions)
    if total == 0:
        sys.exit(f"0 actions over {len(files)} rollout(s): nothing was extracted")
    harness = sum(a["harness_error"] for s in sessions for a in s["actions"])
    marker = sum(a["marker_error"] for s in sessions for a in s["actions"])
    kinds = collections.Counter(a["kind"] for s in sessions for a in s["actions"])
    payload = {
        "transcripts_seen": len(files), "sessions_kept": len(sessions),
        "sessions_dropped_below_min": dropped, "min_actions": args.min,
        "actions": total, "unmatched_results": unmatched,
        "harness_error_rate": harness / total, "marker_error_rate": marker / total,
        "distinct_action_kinds": len(kinds), "corpus": f"codex:{args.corpus}",
        "sessions": sessions,
    }
    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    json.dump(payload, open(args.out, "w"), indent=1)
    print(f"rollouts {len(files)} · sessions kept {len(sessions)} · dropped {dropped} · "
          f"unmatched {unmatched}")
    print(f"actions {total} · exit-code failures {harness} ({harness/total:.3f}) · "
          f"marker failures {marker} ({marker/total:.3f}) · {len(kinds)} kinds")
    print("most common:", ", ".join(f"{k}={n}" for k, n in kinds.most_common(6)))
    print(f"written to {args.out}")


if __name__ == "__main__":
    main()
