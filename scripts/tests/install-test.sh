#!/usr/bin/env bash
# PH-002 gate — install.sh --dry-run must be non-destructive and must name the
# correct binary (physis-pro-web), never the bogus one.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

TMP_HOME="$(mktemp -d)"
trap 'rm -rf "$TMP_HOME"' EXIT

# 1. --dry-run exits 0 and is a no-op in a fresh HOME.
OUT="$(HOME="$TMP_HOME" "$ROOT/install.sh" --dry-run)"
[ -z "$(ls -A "$TMP_HOME")" ] || { echo "FAIL: dry-run created files in HOME"; exit 1; }

# 2. The plan names the correct web binary.
grep -q 'physis-pro-web' <<<"$OUT" || { echo "FAIL: dry-run plan does not mention physis-pro-web"; exit 1; }

# 3. The plan never references the bogus binary name.
grep -q 'physis-web' <<<"$OUT" && { echo "FAIL: dry-run plan references bogus physis-web"; exit 1; }

# 4. Syntax-check on both the script and its help path.
bash -n "$ROOT/install.sh"
"$ROOT/install.sh" --help >/dev/null

echo "install-test.sh: PASS"