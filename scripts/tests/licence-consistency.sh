#!/usr/bin/env bash
# PH-003 gate — the four customer pages must carry the SAME licence claim and
# that claim must equal scripts/licence-claim.txt, and physis-core/Cargo.toml
# must declare a compatible `license`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

CLAIM="$(cat scripts/licence-claim.txt)"
PAGES=("index.html" "src/bin/ui/about.html" "src/bin/ui/pricing.html" "src/bin/ui/customer_dashboard.html")

for p in "${PAGES[@]}"; do
  [ -f "$p" ] || { echo "FAIL: missing $p"; exit 1; }
  if ! grep -Fq "$CLAIM" "$p"; then
    echo "FAIL: $p does not contain the canonical licence claim:"
    echo "  $CLAIM"
    exit 1
  fi
done

CORE_LIC="$(grep -m1 '^license' physis-core/Cargo.toml | cut -d'"' -f2)"
[ "$CORE_LIC" = "Apache-2.0" ] || { echo "FAIL: physis-core license=$CORE_LIC (want Apache-2.0)"; exit 1; }
grep -Fq 'physis-core is Apache-2.0' <<<"$CLAIM" || { echo "FAIL: claim does not mention Apache-2.0 core"; exit 1; }

# The proprietary EULA must never be accompanied by a MIT licence line for physis_pro.
ROOT_LIC_LINES="$(grep -E '^license(\s*)=|license-file' Cargo.toml | head -5 || true)"
if grep -vE '^[[:space:]]*#' Cargo.toml | grep -qE '^[[:space:]]*license[[:space:]]*=[[:space:]]*"MIT"'; then
  echo "FAIL: root Cargo.toml declares MIT for the proprietary crate:"; echo "$ROOT_LIC_LINES"; exit 1
fi

# A name-match is not enough: assert all four pages contain byte-identical
# canonical claim text (via a fixed-string occurrence count).
for p in "${PAGES[@]}"; do
  count="$(grep -oF "$CLAIM" "$p" | wc -l)"
  [ "$count" -ge 1 ] || { echo "FAIL: $p claim mismatch"; exit 1; }
done

echo "licence-consistency.sh: PASS (4 pages + physis-core Cargo.toml agree on: $CLAIM)"