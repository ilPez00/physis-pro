#!/usr/bin/env bash
# Stable MCP entry point: select this checkout's Core and shared workspace log.
set -euo pipefail
PHYSIS_CHECKOUT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PHYSIS_WORKSPACE="${PHYSIS_WORKSPACE:-$PHYSIS_CHECKOUT}"
PHYSIS_SYSTEM_STATE="${PHYSIS_SYSTEM_STATE:-$PHYSIS_WORKSPACE/.physis/system}"
if [[ -n "${PHYSIS_CORE_BIN:-}" ]]; then
  PHYSIS_EXECUTABLE="$PHYSIS_CORE_BIN"
else
  PHYSIS_EXECUTABLE="$PHYSIS_CHECKOUT/target/debug/physis-core"
fi
if [[ ! -x "$PHYSIS_EXECUTABLE" ]]; then
  echo "Physis Core missing: run cargo build --offline -p physis-core --no-default-features --features cli --bin physis-core in $PHYSIS_CHECKOUT" >&2
  exit 2
fi
exec "$PHYSIS_EXECUTABLE" system --root "$PHYSIS_WORKSPACE" --state "$PHYSIS_SYSTEM_STATE" serve
