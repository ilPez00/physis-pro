#!/usr/bin/env bash
# Stable MCP entry point: select this checkout's Pro binary and shared workspace log.
set -euo pipefail
PHYSIS_CHECKOUT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PHYSIS_WORKSPACE="${PHYSIS_WORKSPACE:-$PHYSIS_CHECKOUT}"
PHYSIS_SYSTEM_STATE="${PHYSIS_SYSTEM_STATE:-$PHYSIS_WORKSPACE/.physis/system}"
# `system`/`system_cli`/`system_mcp` moved from physis-core to physis-pro in the
# 2026-09-14 Core/Product split (src/ported/system*.rs); physis-core no longer
# has a `system` subcommand. PHYSIS_CORE_BIN is kept as the override name for
# compatibility with existing configs — it now names the resolved executable,
# not specifically a Core one.
if [[ -n "${PHYSIS_CORE_BIN:-}" ]]; then
  PHYSIS_EXECUTABLE="$PHYSIS_CORE_BIN"
else
  PHYSIS_EXECUTABLE="$PHYSIS_CHECKOUT/target/debug/physis-pro"
fi
if [[ ! -x "$PHYSIS_EXECUTABLE" ]]; then
  echo "Physis Pro missing: run cargo build --bin physis-pro in $PHYSIS_CHECKOUT" >&2
  exit 2
fi
exec "$PHYSIS_EXECUTABLE" system --root "$PHYSIS_WORKSPACE" --state "$PHYSIS_SYSTEM_STATE" serve
