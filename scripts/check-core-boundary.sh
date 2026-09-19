#!/usr/bin/env bash
# scripts/check-core-boundary.sh — the Core/Pro dependency boundary, enforced.
#
# Day 2 of docs/plans/2026-09-15-core-pro-development.md asks for a boundary
# check with four parts: the dependency direction, the exported modules, the
# command surface, and a Core build that needs no Pro checkout. This script is
# the first three; the fourth (an isolated build) is a CI step in
# .github/workflows/physis-core.yml, because it needs a fresh target directory.
#
# The rule the split exists to make true: `Pro -> Core`, never the reverse. A
# check that only runs when someone remembers is not a boundary, so this is a
# gate: it exits non-zero and says which assertion failed and what it saw.
#
# Usage:
#   scripts/check-core-boundary.sh [--core-dir DIR] [--binary PATH]
#
#   --core-dir  the physis-core checkout (default: physis-core)
#   --binary    a built physis-core CLI; if omitted, `cargo build -p physis-core`
#               is run and target/debug/physis-core is used.
set -euo pipefail

CORE_DIR="physis-core"
BINARY=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --core-dir) CORE_DIR="$2"; shift 2 ;;
    --binary) BINARY="$2"; shift 2 ;;
    *) echo "check-core-boundary: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ ! -f "$CORE_DIR/Cargo.toml" ]]; then
  echo "check-core-boundary: no Cargo.toml in $CORE_DIR" >&2
  exit 2
fi

fail() { echo "check-core-boundary: FAIL — $1" >&2; exit 1; }

# ── 1. Dependency direction ────────────────────────────────────────────────
# Core must not name Pro in code or in its manifest. Comments may reference the
# product (they record where a thing went); `use`/`::`/a manifest dependency may
# not, because that is what would make Core unbuildable without Pro.
code_hits=$(grep -rnE 'use +physis_pro|extern +crate +physis_pro|physis_pro::' \
  "$CORE_DIR/src" 2>/dev/null || true)
if [[ -n "$code_hits" ]]; then
  echo "$code_hits" >&2
  fail "Core source references the product crate"
fi
manifest_hits=$(grep -nE '^\s*physis-pro\s*=' "$CORE_DIR/Cargo.toml" 2>/dev/null || true)
if [[ -n "$manifest_hits" ]]; then
  echo "$manifest_hits" >&2
  fail "Core's Cargo.toml depends on physis-pro"
fi
echo "  [ok] dependency direction: no product reference in Core code or manifest"

# ── 2. No product module exported ──────────────────────────────────────────
# Modules that MOVED_TO_PRODUCT.md records as belonging to the product. The
# anchors are exact so `act` does not match `act_recall` and `system` does not
# match `system_cli`. `worldstate`, `oracle` and `config_run` are deliberately
# NOT here: Core keeps the structural primitives those names refer to.
moved_mods=(studio studio_communities studio_lab system system_cli
  system_delegation system_mcp act act_recall ground notebook nlq quality
  vault history praxis edition tui front_door)
for m in "${moved_mods[@]}"; do
  if grep -qE "^pub mod ${m};" "$CORE_DIR/src/lib.rs"; then
    fail "Core still exports the moved product module \`${m}\`"
  fi
done
echo "  [ok] module surface: none of the ${#moved_mods[@]} moved product modules is exported"

# ── 3. Command surface ─────────────────────────────────────────────────────
# The CLI is the boundary a user actually sees. Product verbs must be gone;
# mechanism verbs must remain (removing those would break Core's own promise).
if [[ -z "$BINARY" ]]; then
  cargo build -q -p physis-core
  BINARY="target/debug/physis-core"
fi
if [[ ! -x "$BINARY" ]]; then
  fail "no executable at $BINARY"
fi

help_text=$("$BINARY" --help 2>&1)
commands=$(printf '%s\n' "$help_text" | awk '/^Commands:/{f=1;next} /^[A-Z][a-z]/{f=0} f' | awk '{print $1}')
if [[ -z "$commands" ]]; then
  printf '%s\n' "$help_text" >&2
  fail "could not read the Commands list from --help"
fi

product_verbs=(studio studio-server notebook vault history praxis quality act
  act-recall act_recall system system-tui ground nlq world watch task tasks
  remember serve)
for v in "${product_verbs[@]}"; do
  if printf '%s\n' "$commands" | grep -qx -- "$v"; then
    fail "product verb \`$v\` is still exposed by physis-core"
  fi
done
mechanism_verbs=(classify scan search claim observed run benchmark replay
  contradiction hypothesis)
for v in "${mechanism_verbs[@]}"; do
  if ! printf '%s\n' "$commands" | grep -qx -- "$v"; then
    fail "mechanism verb \`$v\` is missing — Core's own surface regressed"
  fi
done
echo "  [ok] command surface: no product verb, ${#mechanism_verbs[@]} mechanism verbs present"

# A removed verb must fail loudly, not silently do something else.
set +e
"$BINARY" studio >/dev/null 2>removed_verb.err
status=$?
set -e
if [[ $status -eq 0 ]]; then
  fail "'physis-core studio' exited 0 — a removed product verb must fail"
fi
if ! grep -qi "unrecognized subcommand" removed_verb.err; then
  echo "--- stderr ---" >&2; cat removed_verb.err >&2
  rm -f removed_verb.err
  fail "`physis-core studio` did not report an unrecognized subcommand"
fi
rm -f removed_verb.err
echo "  [ok] removed verb: \`studio\` exits $status with an unrecognized-subcommand error"

echo "check-core-boundary: PASS"
