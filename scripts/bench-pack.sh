#!/usr/bin/env bash
# PH-007: bench-pack.sh — measure physis-pro pack() cold vs warm + integrity.
# Usage: bash scripts/bench-pack.sh [--nodes N] [--budget B]
#        [--assert-warm-under SECONDS] [--assert-cold-under SECONDS]
#        [--results PATH]
# Appends PACK_BENCH line to $RESULTS, prints human table, exits non-zero
# if an assert-under threshold is breached. Exits 0 on success.
set -euo pipefail

NODES=1000
BUDGET=4000
WARM_UNDER=""
COLD_UNDER=""
RESULTS="bench/bench-pack.results"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --nodes) NODES="$2"; shift 2;;
    --budget) BUDGET="$2"; shift 2;;
    --assert-warm-under) WARM_UNDER="$2"; shift 2;;
    --assert-cold-under) COLD_UNDER="$2"; shift 2;;
    --results) RESULTS="$2"; shift 2;;
    *) echo "unknown arg: $1" >&2; exit 2;;
  esac
done

BIN="${BIN:-./target/debug/physis-pro}"
[ -x "$BIN" ] || { echo "build missing: $BIN (run cargo build --bin physis-pro)" >&2; exit 3; }
export PHYSIS_ALLOW_DEV_LICENCE=1
# Dev licence lives at the default data dir; point load() at it explicitly so
# we can isolate the embed cache (PHYSIS_DATA_DIR) per run for cold vs warm.
[ -z "${PHYSIS_LICENSE:-}" ] && export PHYSIS_LICENSE_FILE="${PHYSIS_LICENSE_FILE:-$HOME/.physis/license.key}"

CACHE="$(mktemp -d)"
STORE="$(mktemp -d)"
trap 'rm -rf "$CACHE" "$STORE"' EXIT
export PHYSIS_DATA_DIR="$CACHE"

mkdir -p "$STORE/src"
for i in $(seq 1 "$NODES"); do
  printf 'document %d content line alpha beta gamma %d\n' "$i" "$((i*7))" > "$STORE/src/doc_$i.txt"
done

query="gamma content alpha"

cold_secs="$( { start=$(date +%s.%N); "$BIN" pack "$STORE/src" "$query" --budget "$BUDGET" >/dev/null 2>&1; end=$(date +%s.%N); printf '%s' "$(awk "BEGIN{printf \"%.2f\", $end-$start}")"; } )"

warm_secs="$( { start=$(date +%s.%N); "$BIN" pack "$STORE/src" "$query" --budget "$BUDGET" >/dev/null 2>&1; end=$(date +%s.%N); printf '%s' "$(awk "BEGIN{printf \"%.2f\", $end-$start}")"; } )"

# Integrity: stable sorted node list across two runs.
list_a="$( "$BIN" pack "$STORE/src" "$query" --budget "$BUDGET" 2>/dev/null \
  | grep -oE '[0-9.]+ +[A-Za-z0-9_]+\.txt:[0-9-]+ + \([0-9]+ tokens\)' | sort )"
list_b="$( "$BIN" pack "$STORE/src" "$query" --budget "$BUDGET" 2>/dev/null \
  | grep -oE '[0-9.]+ +[A-Za-z0-9_]+\.txt:[0-9-]+ + \([0-9]+ tokens\)' | sort )"
[ "$list_a" = "$list_b" ] || { echo "integrity FAIL: node list not stable" >&2; exit 5; }
node_count="$(printf '%s\n' "$list_a" | grep -c . || true)"

mkdir -p "$(dirname "$RESULTS")"
echo "PACK_BENCH cold=${cold_secs}s warm=${warm_secs}s nodes=$node_count" >> "$RESULTS"

echo "┌─ pack benchmark ───────────────────┐"
echo "│ cold (fresh cache):  ${cold_secs}s              │"
echo "│ warm (cache reuse):  ${warm_secs}s              │"
echo "│ chunks packed:       $node_count               │"
echo "└────────────────────────────────────┘"

rc=0
if [ -n "$WARM_UNDER" ]; then
  if awk "BEGIN{exit !($warm_secs < $WARM_UNDER)}"; then echo "warm under ${WARM_UNDER}s: PASS";
  else echo "warm under ${WARM_UNDER}s: FAIL (got ${warm_secs}s)"; rc=1; fi
fi
if [ -n "$COLD_UNDER" ]; then
  if awk "BEGIN{exit !($cold_secs < $COLD_UNDER)}"; then echo "cold under ${COLD_UNDER}s: PASS";
  else echo "cold under ${COLD_UNDER}s: FAIL (got ${cold_secs}s)"; rc=1; fi
fi
echo "integrity: stable node list ($node_count chunks) PASS"
exit "$rc"
