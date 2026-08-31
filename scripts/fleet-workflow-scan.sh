#!/usr/bin/env bash
# fleet-workflow-scan.sh — the physis fleet campaign.
#
# On ONE machine: for every top-level directory and every drive mount, run the
# physis workflow scanner (documentation coverage for AI agents + humans,
# timestamps/metadata, git attribution = "who did what where"), then run the
# cargo test gate, then probe the agentic surfaces (MCP tool list, Claude
# session transcripts). Write one JSON + markdown report per root under
# ~/physis-fleet-reports.
#
# Usage:
#   scripts/fleet-workflow-scan.sh [dir...]
#     If dirs are given, scan those; otherwise scan every top-level dir under
#     the HOME drive plus every /mnt mount.
#
# The scanner is licence-gated; the dev licence must already exist (see
# TESTING.md). The report is appended to a per-machine manifest so the results
# can be merged across the fleet.

set -Eeuo pipefail

BIN="${PHYSIS_BIN:-$(cd "$(dirname "$0")/.." && pwd)/target/debug/physis-pro}"
REPORT_DIR="${PHYSIS_REPORT_DIR:-$HOME/physis-fleet-reports}"
MACHINE="$(hostname -s)"
LICENSE_FILE="${PHYSIS_LICENSE_FILE:-$HOME/.physis/license.key}"
SCAN_TIMEOUT="${PHYSIS_SCAN_TIMEOUT:-150}"
GIT_REPO="${PHYSIS_REPO:-$(cd "$(dirname "$0")/.." && pwd)}"

mkdir -p "$REPORT_DIR"
export PHYSIS_LICENSE_FILE="$LICENSE_FILE"
export PHYSIS_SCAN_ROOT="$HOME"

log() { echo "[$(date -u +%H:%M:%SZ)] $*"; }

manifest="$REPORT_DIR/$MACHINE.jsonl"
[[ -f "$manifest" ]] || : > "$manifest"

scan_one() {
  local root="$1"
  local key
  key="$(printf '%s' "$root" | tr '/' '_' | tr -c 'A-Za-z0-9_.-' '_')"
  local out="$REPORT_DIR/$MACHINE$key.json"
  # A non-git root makes git_attribution return fast; a git root adds author
  # attribution. Run inside the timeout; a slow tree must not stall the fleet.
  if timeout "$SCAN_TIMEOUT" "$BIN" workflow "$root" --json >"$out" 2>/dev/null; then
    local total audience harness
    total="$(python3 -c "import json;print(json.load(open('$out')).get('total',0))" 2>/dev/null || echo 0)"
    audience="$(python3 -c "import json;print([(t['key'],t['count']) for t in json.load(open('$out')).get('by_audience',[])])" 2>/dev/null || echo '[]')"
    harness="$(python3 -c "import json;print([(t['key'],t['count']) for t in json.load(open('$out')).get('by_harness',[])])" 2>/dev/null || echo '[]')"
    log "$root: OK total=$total audience=$audience harness=$harness"
    printf '{"machine":"%s","root":"%s","status":"ok","total":%s,"audience":%s,"harness":%s,"at":"%s"}\n' \
      "$MACHINE" "$root" "$total" "$audience" "$harness" "$(date -u +%FT%TZ)" >> "$manifest"
  else
    log "$root: TIMEOUT/FAIL (exit $?)"
    printf '{"machine":"%s","root":"%s","status":"fail","at":"%s"}\n' \
      "$MACHINE" "$root" "$(date -u +%FT%TZ)" >> "$manifest"
    rm -f "$out"
  fi
}

cargo_gate() {
  log "cargo gate: check --features full"
  if (cd "$GIT_REPO" && cargo check --features full 2>&1 | tail -2); then
    log "cargo check: OK"
  else
    log "cargo check: FAILED"
  fi
  log "cargo gate: tests"
  (cd "$GIT_REPO" && cargo test --features full 2>&1 | grep -E 'test result' | sort | uniq -c | tail -5)
}

harness_flows() {
  # MCP stdio handshake must list physis_workflow.
  log "harness: MCP tools/list"
  if (printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"fleet","version":"1"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n'; sleep 4) \
      | timeout 60 "$BIN" serve --stdio 2>/dev/null \
      | grep -o '"name":"physis_workflow"' | head -1 > "$REPORT_DIR/$MACHINE-mcp.txt"; then
    log "harness: MCP ok -> $(cat "$REPORT_DIR/$MACHINE-mcp.txt")"
  else
    log "harness: MCP no physis_workflow"
  fi

  # Claude Code session transcripts exist under ~/.claude/projects; the
  # decisions-mine adapter must parse them without panicking.
  if [[ -d "$HOME/.claude/projects" ]]; then
    log "harness: decisions-mine over ~/.claude/projects"
    (cd "$GIT_REPO" && timeout 60 "$BIN" decisions-mine --dir "$HOME/.claude/projects" 2>/dev/null | head -3)
  fi
}

main() {
  log "machine=$MACHINE  bin=$BIN  reports=$REPORT_DIR"
  if [[ "${PHYSIS_CAMPAIGN_SKIP_CARGO:-0}" != "1" ]]; then
    cargo_gate
  fi

  # cargo check/test rebuild target/debug/physis-pro with the default dev
  # pubkey, which invalidates a licence minted from the local vendor keypair.
  # Rebuild the scan binary with the vendor pubkey so the scans below run.
  if [[ "${PHYSIS_CAMPAIGN_SKIP_CARGO:-0}" != "1" && -f "$GIT_REPO/.physis-vendor/verifying.key" ]]; then
    log "rebuilding scan binary with vendor pubkey"
    PHYSIS_LICENSE_PUBKEY="$(tr -d '\n' < "$GIT_REPO/.physis-vendor/verifying.key")" \
      cargo build --features full --bin physis-pro --manifest-path "$GIT_REPO/Cargo.toml" 2>&1 | tail -1
  fi

  if (($# > 0)); then
    for root in "$@"; do scan_one "$root"; done
  else
    # Home drive top-level dirs (skip the scanner's own heavy noise).
    for root in "$HOME"/*/; do
      scan_one "${root%/}"
    done
    # Every /mnt mount (drives).
    for root in /mnt/*/; do
      [[ -d "$root" ]] && scan_one "${root%/}"
    done
  fi

  harness_flows

  log "done. reports in $REPORT_DIR ($(wc -l < "$manifest") roots scanned)"
}

main "$@"