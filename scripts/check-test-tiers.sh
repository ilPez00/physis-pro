#!/usr/bin/env bash
# PH-009: check-test-tiers.sh — the meta-test as a runnable gate.
# Asserts: (1) fast tier keeps >= MIN_TESTS tests (no silent coverage loss),
# (2) every #[ignore] carries a reason (#[ignore = "..."], never bare),
# (3) prints the buried heavy tier enumerated by --ignored --list.
# Exits non-zero with expected-vs-actual on breach.
set -euo pipefail

MIN_TESTS="${MIN_TESTS:-1300}"

total=$(cargo test -- --list 2>/dev/null | grep -c ": test$")
echo "fast tier: $total tests (floor $MIN_TESTS)"
[ "$total" -ge "$MIN_TESTS" ] || { echo "FAIL: fast tier shrank to $total (< $MIN_TESTS) — coverage was buried"; exit 1; }

# Attribute lines only (leading whitespace allowed); doc prose mentioning
# #[ignore] (e.g. `//! ... is #[ignore]d because`) must not match.
IGNORE_ATTR='^[[:space:]]*#\[ignore'
bare=$(grep -rn "$IGNORE_ATTR" src/ tests/ --include="*.rs" | grep -v '#\[ignore = ' | wc -l || true)
echo "bare #[ignore] without reason: $bare"
[ "$bare" -eq 0 ] || { echo "FAIL: $bare ignored test(s) lack a reason:"; grep -rn "$IGNORE_ATTR" src/ tests/ --include="*.rs" | grep -v '#\[ignore = '; exit 2; }

echo "--- heavy tier (ignored, enumerated) ---"
cargo test -- --ignored --list 2>/dev/null | grep ": test$" || true
echo "--- end heavy tier ---"
echo "TIERS_OK total=$total"
