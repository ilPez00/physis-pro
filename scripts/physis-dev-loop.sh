#!/usr/bin/env bash
# physis-dev-loop.sh — dogfood the engine while developing it.
#
# Recall past outcome-memory for a task, run the fast gate, feed the result
# back as quality feedback + a verdict-stamped note. Offline: uses the CLI
# against the shared sled store, no server needed.
#
# Usage:
#   ./scripts/physis-dev-loop.sh "<task description>" [-- gate args...]
#   ./scripts/physis-dev-loop.sh "fix clippy E0599 on coherence edit"
#   ./scripts/physis-dev-loop.sh "refactor ledger" -- cargo clippy --all-targets -- -D warnings
#
# Exit code is the gate's exit code. Failure always prints expected vs actual
# (the tail of the gate log), never fails silent.
set -u
set -o pipefail

TASK="${1:-}"
if [[ -z "$TASK" ]]; then
  echo "usage: $0 \"<task description>\" [-- gate command...]" >&2
  exit 2
fi
shift
if [[ "${1:-}" == "--" ]]; then shift; fi
if [[ "$#" -eq 0 ]]; then
  set -- cargo check --all-targets
fi

BIN="${PHYSIS_BIN:-cargo run --quiet --bin physis-pro --}"

echo "=== physis recall: '$TASK' ==="
# shellcheck disable=SC2086
$BIN node-search "$TASK" --top 8 || echo "(recall unavailable — empty store?)"

echo "=== gate: $* ==="
LOG="$(mktemp)"
if "$@" 2>&1 | tee "$LOG"; then
  echo "=== gate GREEN ==="
  # Outcome memory only — the quality tracker's penalties/boosts are about
  # classification output, not dev gates, so the loop writes notes, never
  # fail/pass. Keeps the two signals from polluting each other.
  # shellcheck disable=SC2086
  $BIN note "dev-loop GREEN: $TASK" --verdict 1.0 || echo "(note unavailable)"
  rm -f "$LOG"
  exit 0
else
  CODE="${PIPESTATUS[0]}"
  echo "=== gate RED (exit $CODE) ==="
  echo "--- tail: what failed ---"
  tail -30 "$LOG"
  echo "--- end tail ---"
  SUMMARY="$(grep -E "^(error|warning: unused|failures:|test .* FAILED)" "$LOG" | head -5 | tr '\n' '; ')"
  # shellcheck disable=SC2086
  $BIN note "dev-loop RED: $TASK — ${SUMMARY:-gate failed}" --verdict -1.0 || echo "(note unavailable)"
  rm -f "$LOG"
  exit "$CODE"
fi
