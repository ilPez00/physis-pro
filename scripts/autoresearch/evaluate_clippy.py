#!/usr/bin/env python3
"""autoresearch evaluator — clippy pedantic warning count on a target file.

Metric: `warnings` = number of clippy::pedantic warnings clippy emits for the
TARGET file. LOWER is better. Compilation is a hard gate: if the crate does not
compile (clippy exits non-zero for reasons other than lint warnings, i.e. a real
`error[...]`), the metric is a large sentinel so the loop never rewards code that
doesn't build.

Also emits `total_warnings` (whole-lib pedantic count) as an informational line
so a run that cuts the target file's warnings by pushing them elsewhere is
visible.

Usage:
    python3 scripts/autoresearch/evaluate_clippy.py [TARGET_FILE]

TARGET_FILE defaults to src/discovery.rs. Output (stdout), grep-friendly:
    warnings: <int>          # the optimized metric (target file only)
    total_warnings: <int>    # informational (whole lib)
    compiles: <0|1>

The autoresearch config greps `^warnings:` with metric_direction=lower.
"""
import os
import re
import subprocess
import sys

SENTINEL = 100000  # "worse than any real warning count" — used when build breaks.


def repo_root() -> str:
    # scripts/autoresearch/<this> -> repo root is two dirs up.
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(here, "..", ".."))


def run_clippy(root: str):
    """Run clippy pedantic on the lib, short format. Return (exit_code, lines)."""
    env = dict(os.environ)
    env["CARGO_NET_OFFLINE"] = "false"
    # Ensure cargo is on PATH even under a minimal CI/agent shell.
    cargo_bin = os.path.expanduser("~/.cargo/bin")
    if cargo_bin not in env.get("PATH", ""):
        env["PATH"] = cargo_bin + os.pathsep + env.get("PATH", "")
    proc = subprocess.run(
        [
            "cargo", "clippy", "--lib", "--message-format=short",
            "--", "-W", "clippy::pedantic",
        ],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
        timeout=900,
    )
    # clippy writes diagnostics to stderr in short format.
    return proc.returncode, proc.stderr.splitlines()


def main() -> int:
    target = sys.argv[1] if len(sys.argv) > 1 else "src/discovery.rs"
    root = repo_root()

    try:
        code, lines = run_clippy(root)
    except subprocess.TimeoutExpired:
        print("compiles: 0")
        print("total_warnings:", SENTINEL)
        print("warnings:", SENTINEL)
        return 0

    # A real compile error (not just lint) means the tree doesn't build.
    has_error = any(re.search(r":\s*error(\[|:)", ln) or ln.startswith("error[")
                    for ln in lines)
    if has_error:
        print("compiles: 0")
        print("total_warnings:", SENTINEL)
        print("warnings:", SENTINEL)
        return 0

    # Count `warning:` lines attributed to the target file (short format:
    #   src/discovery.rs:110:63: warning: ...
    tgt_norm = target.lstrip("./")
    target_warnings = 0
    total_warnings = 0
    for ln in lines:
        if ": warning: " not in ln:
            continue
        # Skip clippy's summary line ("generated N warnings").
        if "generated" in ln and "warning" in ln and ".rs:" not in ln:
            continue
        total_warnings += 1
        if ln.startswith(tgt_norm + ":") or ("/" + tgt_norm + ":") in ln:
            target_warnings += 1

    print("compiles: 1")
    print("total_warnings:", total_warnings)
    print("warnings:", target_warnings)
    return 0


if __name__ == "__main__":
    sys.exit(main())
