#!/usr/bin/env python3
"""Does packing the context beat the shell sequence that assembles it by hand?

`find` returns pointers; the caller then reads files, and the read is where the
tokens go. This measures the whole errand instead of the first call: each task
is scored on whether the *answer text* is in the returned context, and on what
that context cost in cl100k_base tokens.

Arms
  pack       physis system pack '<query>' --budget N
  grep+cat   grep -rni … | head -10, then cat the highest-ranked file (an agent's
             usual move, and the one that blows the budget on a long file)
  grep -C5   grep -rn -C5 … | head -80 (the cheap, strong shell baseline)

A task hits when the ground-truth string appears in that arm's output; a cheaper
arm that misses is not a saving.

    python3 benchmarks/system-interface/pack_vs_shell.py [--bin …] [--budget 1200]
"""
import argparse, json, os, statistics, subprocess, sys

try:
    import tiktoken
except ImportError:
    sys.exit("tiktoken required: pip install tiktoken")

ENC = tiktoken.get_encoding("cl100k_base")
DEV = os.path.expanduser("~/dev")

# (label, workspace, query, grep terms, file an agent would open, ground truth)
TASKS = [
    ("rag bridge", f"{DEV}/randomesh", "rag bridge offload test physis",
     ["rag bridge", "offload"], "physis_rag_bridge.py",
     "def test_physis_offload"),
    ("mcp gateway", f"{DEV}/physis-global", "mcp gateway sidecar start subprocess",
     ["mcp gateway", "sidecar"], "gateway/physis_mcp_gateway.py",
     "def start_sidecar"),
    ("android login", f"{DEV}/praxis", "android login activity biometric prompt",
     ["LoginActivity", "BiometricPrompt"],
     "praxis-android/app/src/main/java/com/praxis/app/auth/LoginActivity.kt",
     "onAuthenticationSucceeded"),
    ("systemd timer", f"{DEV}/randomesh", "systemd timer nightly convergence persistent",
     ["OnCalendar", "Persistent"], "services/randomesh-physis-sync.timer",
     "Persistent=true"),
    ("token counter", f"{DEV}/physis-pro", "heuristic token counter budget packing",
     ["count_tokens", "token budget"], "physis-core/src/rag.rs",
     "pub fn count_tokens"),
]

EXCLUDES = ("--exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target "
            "--exclude-dir=vendor --exclude-dir=.physis")


def capture(cmd, cwd):
    p = subprocess.run(cmd, shell=True, cwd=cwd, capture_output=True,
                       text=True, timeout=900)
    return p.stdout + p.stderr


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=f"{DEV}/physis-pro/target/debug/physis")
    ap.add_argument("--budget", type=int, default=1200)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    rows, skipped = [], []
    for label, cwd, query, terms, open_file, truth in TASKS:
        if not os.path.isdir(cwd):
            skipped.append(label)
            continue
        pattern = " ".join(f"-e '{t}'" for t in terms)
        arms = {
            "pack": f"{args.bin} system pack '{query}' --budget {args.budget}",
            "grep+cat": f"grep -rni {pattern} . {EXCLUDES} | head -10; cat '{open_file}'",
            "grep -C5": f"grep -rni -C5 {pattern} . {EXCLUDES} | head -80",
        }
        row = {"task": label, "workspace": cwd, "truth": truth}
        for arm, cmd in arms.items():
            out = capture(cmd, cwd)
            row[arm] = {"tokens": len(ENC.encode(out)), "hit": truth in out}
        rows.append(row)

    if not rows:
        sys.exit("0 tasks over 0 workspaces: nothing was measured")

    arms = ["pack", "grep+cat", "grep -C5"]
    print(f"{'task':16}" + "".join(f"{a:>18}" for a in arms))
    for r in rows:
        cells = "".join(f"{r[a]['tokens']:>12}{' HIT' if r[a]['hit'] else ' mis':>6}"
                        for a in arms)
        print(f"{r['task']:16}{cells}")
    print(f"{'TOTAL':16}" + "".join(
        f"{sum(r[a]['tokens'] for r in rows):>12}"
        + f"{sum(r[a]['hit'] for r in rows)}/{len(rows)}".rjust(6) for a in arms))
    for a in arms:
        v = [r[a]["tokens"] for r in rows]
        print(f"{a:10} mean {statistics.mean(v):7.0f}  sd {statistics.pstdev(v):7.0f}"
              f"  hit {sum(r[a]['hit'] for r in rows)}/{len(rows)}")
    if skipped:
        print("SKIPPED (workspace absent):", ", ".join(skipped))
    if args.out:
        json.dump(rows, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
