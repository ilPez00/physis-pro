#!/usr/bin/env bash
# mvp-demo.sh — reproducible MVP demo transcript.
#
# One real repository, one closed loop, one transcript file:
#   scan + map → natural-language intent → scoped world slice → inspect →
#   dependencies → bounded action + validation → semantic Δ → remap →
#   replayable transition records.
#
# Usage:  scripts/mvp-demo.sh <directory> [output-transcript.md]
#
# No override needed: unset PHYSIS_EMBEDDER uses the semantic cascade
# (local ollama nomic-embed-text, self-test gated — still offline, no cloud).
# Set PHYSIS_EMBEDDER=random-projection only for the deterministic lexical
# fallback; the transcript records whichever embedder actually answered.
set -uo pipefail

ROOT="${1:?usage: scripts/mvp-demo.sh <directory> [output-transcript.md]}"
ROOT="$(cd "$ROOT" && pwd)"
OUT="${2:-$ROOT/.physis/MVP_DEMO_TRANSCRIPT.md}"
# Binary resolution, most specific first: an explicit override, then the two
# build profiles of the checkout we are standing in, then whatever is installed
# on PATH (how a machine that received only the binary runs it).
# NOTE: $PWD/target/* must win over a stale ~/.local/bin install — the script
# is the reproducibility gate, and a stale binary silently voids it.
resolve_bin() {
  if [ -n "${PHYSIS_BIN:-}" ]; then printf '%s' "$PHYSIS_BIN"; return; fi
  for candidate in "$PWD/target/release/physis-pro" "$PWD/target/debug/physis-pro"; do
    [ -x "$candidate" ] && { printf '%s' "$candidate"; return; }
  done
  command -v physis-pro 2>/dev/null || printf '%s' "$PWD/target/debug/physis-pro"
}
BIN="$(resolve_bin)"

if [ ! -x "$BIN" ]; then
  echo "error: no physis-pro binary (looked at \$PHYSIS_BIN, ./target/release, ./target/debug, PATH)" >&2
  echo "build one: cargo build --release --features cli --bin physis-pro" >&2
  exit 2
fi

# NOTE: no `export PHYSIS_EMBEDDER` line here on purpose — the cascade
# default (unset) is the semantic path. Callers pinning the lexical fallback
# export it themselves; the transcript header below records the result.
export PHYSIS_MODEL="${PHYSIS_MODEL:-none}"
export PHYSIS_DATA_DIR="$ROOT/.physis"
mkdir -p "$PHYSIS_DATA_DIR"

# The actions the demo walks, in order. Fixed and minimal on purpose.
# Covers the full loop: navigate → intent → context packet → attention →
# action → Δ → rescan → bench (all model-free; PHYSIS_MODEL=none default).
# The second `improve … --yes` is the O(Δ) proof: a second bounded edit
# whose remap touches only what moved (Phase 32 §32.4 G).
run_demo() {
  cat <<'CMDS'
what does this project do?
where is retrieval implemented?
inspect
dependencies
differences
contradictions
history
context
focus retrieval
what changed recently?
improve retrieval without changing its public API
improve retrieval without changing its public API --yes
rescan
bench
improve retrieval without changing its public API
improve retrieval without changing its public API --yes
bench
quit
CMDS
}

{
  echo "# Physis MVP demo transcript"
  echo
  echo "- repository: \`$ROOT\`"
  echo "- embedder request: \`${PHYSIS_EMBEDDER:-<unset: semantic cascade>}\`"
  echo "- generator: \`${PHYSIS_MODEL:-none}\` (none = deterministic/manual path)"
  echo "- started: \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\`"
  echo
  echo '```text'
} > "$OUT"

start=$(date +%s)
# `stdbuf -oL` so the transcript grows while the loop runs instead of in one
# buffered burst at exit — a reviewer watching the demo sees it happen.
run_demo | stdbuf -oL -eL "$BIN" "$ROOT" 2>&1 | stdbuf -oL tee -a "$OUT"
end=$(date +%s)

{
  echo '```'
  echo
  echo "- finished: \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\` (wall $((end - start))s)"
  echo "- transition log: \`$PHYSIS_DATA_DIR/mvp_transitions.jsonl\`"
  echo "- transitions recorded: \`$(wc -l < "$PHYSIS_DATA_DIR/mvp_transitions.jsonl" 2>/dev/null || echo 0)\`"
} >> "$OUT"

echo
echo "transcript: $OUT"
