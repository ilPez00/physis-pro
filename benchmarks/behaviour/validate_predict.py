#!/usr/bin/env python3
"""Does the shipped `physis system predict` reproduce E71's measured prior?

E71 measured the idea in Python: the per-kind failure rate, accumulated across
sessions, separates failures from successes at AUC 0.620 against a 0.500 base
rate. That says nothing about the code that shipped. This replays real recorded
actions into a real workspace log, then asks the actual binary, one subprocess
per prediction, and scores what it answers.

    python3 benchmarks/behaviour/validate_predict.py [--bin …] [--limit 400]

Held-out session's bash actions are the test set; every other session's actions
are written into the store as `system.run.finish` observations in the engine's
own on-disk format. Prints its denominators, and exits 2 if nothing was scored.
"""
import argparse, collections, json, os, random, subprocess, sys, tempfile, datetime

BASE = os.path.dirname(os.path.abspath(__file__))


def observation(seq, argv, failed, workspace):
    """One record in the engine's log format — the same shape `run` appends."""
    payload = {
        "run_id": f"replay-{seq}",
        "argv": argv,
        "intent": "replayed from a transcript for validation",
        "actor": "validate_predict.py",
        "process_success": not failed,
        "exit_code": 1 if failed else 0,
        "workspace": workspace,
    }
    return {
        "seq": seq,
        "at": datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z"),
        "source": "system.run.finish",
        "subject": "replayed",
        "body": json.dumps(payload),
        "by": "validate_predict.py",
    }


def bash_actions(session):
    """Transcript actions that were a shell command, with their outcome."""
    out = []
    for a in session["actions"]:
        if not a["kind"].startswith("bash:"):
            continue
        argv = a["target"].split()
        if argv:
            out.append((argv, bool(a["harness_error"] or a["marker_error"])))
    return out


def auc(scored):
    pos = [p for p, y in scored if y]
    neg = [p for p, y in scored if not y]
    if not pos or not neg:
        return None
    wins = sum((a > b) + 0.5 * (a == b) for a in pos for b in neg)
    return wins / (len(pos) * len(neg))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--actions", default=os.path.join(BASE, "actions.json"))
    ap.add_argument("--bin", default=os.path.expanduser("~/dev/physis-pro/target/debug/physis-core"))
    ap.add_argument("--limit", type=int, default=80, help="predictions per held-out session")
    ap.add_argument("--sessions", type=int, default=8, help="held-out sessions to score")
    args = ap.parse_args()

    data = json.load(open(args.actions))
    sessions = [s for s in data["sessions"] if bash_actions(s)]
    if len(sessions) < 2:
        sys.exit("fewer than 2 sessions with shell actions: nothing to hold out")

    # Leave-one-session-out over every session that has both outcomes, exactly
    # as E71 scored it. An earlier version held out the single most
    # failure-heavy session and read its AUC as the result: one session, chosen
    # for being extreme, is a sample of one.
    usable = [s for s in sessions
              if any(f for _, f in bash_actions(s)) and any(not f for _, f in bash_actions(s))]
    if len(usable) < 2:
        sys.exit(f"{len(usable)} session(s) contain both outcomes: nothing to hold out")

    rows = []
    total_scored = total_positive = 0
    for held in usable[: args.sessions]:
        others = [s for s in sessions if s is not held]
        workspace = tempfile.mkdtemp(prefix="physis-predict-")
        state = os.path.join(workspace, ".physis", "system")
        os.makedirs(state, exist_ok=True)
        seq = written = failures = 0
        with open(os.path.join(state, "observations.jsonl"), "w") as fh:
            for s in others:
                for argv, failed in bash_actions(s):
                    seq += 1
                    written += 1
                    failures += failed
                    fh.write(json.dumps(observation(seq, argv, failed, workspace)) + "\n")

        test = bash_actions(held)
        random.Random(0).shuffle(test)
        test = test[: args.limit]
        scored = []
        for argv, failed in test:
            result = subprocess.run(
                [args.bin, "system", "--root", workspace, "--json", "predict", "--"] + argv,
                capture_output=True, text=True, timeout=120,
            )
            try:
                payload = json.loads(result.stdout)["data"]
            except Exception:
                continue
            if payload.get("status") == "NOT MEASURED":
                continue
            scored.append((payload["failure_probability"], failed))
        if not scored:
            continue
        base = failures / written if written else 0.0
        brier = sum((p - float(y)) ** 2 for p, y in scored) / len(scored)
        base_brier = sum((base - float(y)) ** 2 for _, y in scored) / len(scored)
        rows.append({
            "session": held["session"][:8],
            "scored": len(scored),
            "positives": sum(1 for _, y in scored if y),
            "auc": auc(scored),
            "brier": brier,
            "base_brier": base_brier,
        })
        total_scored += len(scored)
        total_positive += rows[-1]["positives"]

    if not rows:
        sys.exit("0 predictions scored over 0 sessions: nothing was measured")

    aucs = [r["auc"] for r in rows if r["auc"] is not None]
    print(f"{len(rows)} held-out session(s) · {total_scored} prediction(s) "
          f"({total_positive} failures) · one subprocess per prediction")
    print(f"{'session':10}{'scored':>8}{'fails':>7}{'AUC':>8}{'Brier':>9}{'base':>9}")
    for r in rows:
        a = f"{r['auc']:.3f}" if r["auc"] is not None else "  n/a"
        print(f"{r['session']:10}{r['scored']:8}{r['positives']:7}{a:>8}"
              f"{r['brier']:9.4f}{r['base_brier']:9.4f}")
    mean_auc = sum(aucs) / len(aucs) if aucs else 0.0
    wins = sum(1 for r in rows if r["brier"] < r["base_brier"])
    print(f"\nmean AUC {mean_auc:.3f} over {len(aucs)} session(s) "
          f"(0.500 = the base rate carries everything)")
    print(f"Brier better than the base rate in {wins}/{len(rows)} session(s)")
    print("E71 measured the same idea in Python at AUC 0.620; this is the shipped binary.")

if __name__ == "__main__":
    main()
