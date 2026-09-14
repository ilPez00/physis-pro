#!/usr/bin/env python3
"""Token cost of `physis system` against the shell commands an agent would
otherwise run, measured with cl100k_base over captured stdout+stderr.

Two arms per task: the interface operation, and the conventional command that
answers the same question. Retrieval tasks also carry a ground-truth string, so
cost is reported next to hit@5 rather than on its own -- a cheaper answer that
misses is not a saving.

    pip install tiktoken
    python3 benchmarks/system-interface/token_cost.py [--bin path/to/physis]

Every arm prints its denominator (documents scanned, tokens produced). A task
whose workspace is absent is reported as SKIPPED, not silently dropped.
"""
import argparse, json, os, statistics, subprocess, sys

try:
    import tiktoken
except ImportError:
    sys.exit("tiktoken required: pip install tiktoken")

ENC = tiktoken.get_encoding("cl100k_base")
DEV = os.path.expanduser("~/dev")

# (label, workspace, interface arm, conventional arm, ground truth or None)
TASKS = [
    ("orient randomesh", f"{DEV}/randomesh",
     "{B} system inspect",
     "ls -la; find . -type f -not -path './.git/*' | wc -l", None),
    ("orient praxis", f"{DEV}/praxis",
     "{B} system inspect",
     "ls -la; find . -type f -not -path './.git/*' -not -path '*/node_modules/*' | wc -l", None),
    ("list 100 files", f"{DEV}/physis-global",
     "{B} system list --limit 100",
     "find . -type f -not -path './.git/*' -not -path '*/vendor/*' -not -path '*/target/*' | head -100", None),
    ("find rag bridge", f"{DEV}/randomesh",
     "{B} system find 'rag bridge embedding' --limit 5",
     "grep -rni -e 'rag bridge' -e embedding . --exclude-dir=.git | head -20",
     "physis_rag_bridge.py"),
    ("find mcp gateway", f"{DEV}/physis-global",
     "{B} system find 'mcp gateway tool registration' --limit 5",
     "grep -rni -e 'mcp gateway' -e 'register.*tool' . --exclude-dir=.git --exclude-dir=vendor --exclude-dir=target | head -20",
     "physis_mcp_gateway.py"),
    ("find auth", f"{DEV}/praxis",
     "{B} system find 'authentication login session token' --limit 5",
     "grep -rni -e 'login' -e 'session token' . --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target | head -20",
     "auth/"),
    ("find sidecar", f"{DEV}/physis-global",
     "{B} system find 'sidecar startup script' --limit 5",
     "grep -rni -e sidecar . --exclude-dir=.git --exclude-dir=vendor --exclude-dir=target | head -20",
     "physis-sidecar"),
    ("find systemd timer", f"{DEV}/randomesh",
     "{B} system find 'systemd timer sync service' --limit 5",
     "grep -rni -e systemd -e timer . --exclude-dir=.git | head -20",
     "randomesh-physis-sync"),
    ("find android login", f"{DEV}/praxis",
     "{B} system find 'android login activity kotlin' --limit 5",
     "grep -rni -e LoginActivity -e 'android login' . --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target | head -20",
     "LoginActivity.kt"),
]


def capture(cmd, cwd):
    p = subprocess.run(cmd, shell=True, cwd=cwd, capture_output=True,
                       text=True, timeout=600)
    return p.stdout + p.stderr


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=f"{DEV}/physis-pro/target/debug/physis")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    rows, skipped = [], []
    for label, cwd, a_tmpl, b_cmd, truth in TASKS:
        if not os.path.isdir(cwd):
            skipped.append(label)
            continue
        a = capture(a_tmpl.format(B=args.bin), cwd)
        b = capture(b_cmd, cwd)
        rows.append(dict(task=label, workspace=cwd,
                         interface_tokens=len(ENC.encode(a)),
                         shell_tokens=len(ENC.encode(b)),
                         truth=truth,
                         interface_hit=(truth in a) if truth else None,
                         shell_hit=(truth in b) if truth else None))

    if not rows:
        sys.exit("0 tasks over 0 workspaces: nothing was measured")

    print(f"{'task':22}{'physis':>8}{'shell':>8}{'delta':>8}  hit@5")
    for r in rows:
        d = r["shell_tokens"] - r["interface_tokens"]
        hit = "" if r["truth"] is None else \
            f"  physis {'HIT' if r['interface_hit'] else 'miss'} / shell {'HIT' if r['shell_hit'] else 'miss'}"
        print(f"{r['task']:22}{r['interface_tokens']:>8}{r['shell_tokens']:>8}{d:>8}{hit}")

    ti = sum(r["interface_tokens"] for r in rows)
    ts = sum(r["shell_tokens"] for r in rows)
    print(f"{'TOTAL':22}{ti:>8}{ts:>8}{ts - ti:>8}   over {len(rows)} tasks")

    ret = [r for r in rows if r["truth"]]
    if ret:
        pi = [r["interface_tokens"] for r in ret]
        ps = [r["shell_tokens"] for r in ret]
        print(f"retrieval hit@5: physis {sum(r['interface_hit'] for r in ret)}/{len(ret)}"
              f" · shell {sum(r['shell_hit'] for r in ret)}/{len(ret)}")
        print(f"retrieval cost: physis mean {statistics.mean(pi):.0f} sd {statistics.pstdev(pi):.0f}"
              f" · shell mean {statistics.mean(ps):.0f} sd {statistics.pstdev(ps):.0f}")
    if skipped:
        print("SKIPPED (workspace absent):", ", ".join(skipped))
    if args.out:
        json.dump(rows, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
