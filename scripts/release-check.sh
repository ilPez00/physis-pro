#!/usr/bin/env bash
# PH-010: release-check.sh — the release ritual as a runnable gate.
# Asserts: (a) clippy + fast-tier tests green, (b) CARGO_PKG_VERSION is NEWER
# than the last v* tag, (c) CHANGELOG.md has an entry for this version,
# (d) licence-consistency passes, (e) bench-pack smoke passes.
# Every failure prints its reason (silent gates make models hallucinate).
# Usage: bash scripts/release-check.sh [--self-test]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

fail() { echo "RELEASE-CHECK FAIL: $1" >&2; exit 1; }

if [ "${1:-}" = "--self-test" ]; then
    T=$(mktemp -d)
    trap 'rm -rf "$T"' EXIT
    # Forged stale version: CHANGELOG entry older than crate version.
    mkdir -p "$T/a"
    printf 'version = "9.9.9"\n' > "$T/a/Cargo.toml"
    printf '## [0.0.1] - 2020-01-01\n' > "$T/a/CHANGELOG.md"
    (cd "$T/a" && bash "$ROOT/scripts/release-check.sh" --only-version-changelog) \
        2>"$T/err" && fail "self-test: stale version passed" || true
    grep -q "no CHANGELOG entry for 9.9.9" "$T/err" \
        || fail "self-test: stale version wrong reason: $(cat "$T/err")"
    echo "self-test stale-version: OK (right reason)"
    # Missing changelog entry, current version.
    VER=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
    mkdir -p "$T/b"
    printf 'version = "%s"\n' "$VER" > "$T/b/Cargo.toml"
    printf '# Changelog\n' > "$T/b/CHANGELOG.md"
    (cd "$T/b" && bash "$ROOT/scripts/release-check.sh" --only-version-changelog) \
        2>"$T/err2" && fail "self-test: missing entry passed" || true
    grep -q "no CHANGELOG entry for $VER" "$T/err2" \
        || fail "self-test: missing entry wrong reason: $(cat "$T/err2")"
    echo "self-test missing-entry: OK (right reason)"
    echo "RELEASE-CHECK SELF-TEST OK"
    exit 0
fi

ONLY_VC=0
[ "${1:-}" = "--only-version-changelog" ] && ONLY_VC=1

# --- (b) version newer than last tag + (c) changelog entry ---
VER=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
[ -n "$VER" ] || fail "cannot read crate version from Cargo.toml"
LAST_TAG=$(git tag --list 'v*' --sort=-v:refname | head -n 1 || true)
ver_gt() { [ "$(printf '%s\n%s' "$2" "$1" | sort -V | head -n 1)" != "$1" ] || [ "$1" = "$2" ]; }
if [ -n "$LAST_TAG" ]; then
    LAST_VER="${LAST_TAG#v}"
    ver_gt "$VER" "$LAST_VER" || fail "crate version $VER is not newer than last tag $LAST_TAG"
    echo "version: $VER > $LAST_TAG OK"
else
    echo "version: $VER (no v* tag yet — first release) OK"
fi
[ -f CHANGELOG.md ] || fail "CHANGELOG.md missing"
grep -Eq "^## \[$VER\]" CHANGELOG.md \
    || fail "no CHANGELOG entry for $VER (want a '## [$VER]' section)"
echo "changelog: entry for $VER OK"

[ "$ONLY_VC" = "1" ] && exit 0

cd "$ROOT"

# --- (d) licence consistency (PH-003) ---
bash scripts/tests/licence-consistency.sh \
    || fail "licence-consistency.sh failed (see above)"
echo "licence: consistent OK"

# --- (a) clippy + full test suite + tier floor ---
cargo clippy --all-targets -- -D warnings \
    || fail "clippy --all-targets -- -D warnings failed"
echo "clippy: clean OK"
cargo test \
    || fail "cargo test failed (see above)"
echo "tests: green OK"
bash scripts/check-test-tiers.sh \
    || fail "check-test-tiers.sh failed (see above)"
echo "tiers: OK"

# --- (e) bench-pack smoke (N=10, generous thresholds) ---
if [ -x ./target/debug/physis-pro ]; then
    bash scripts/bench-pack.sh --nodes 10 \
        --assert-warm-under 30s --assert-cold-under 600s \
        --results /tmp/release-bench-pack.results \
        || fail "bench-pack smoke failed (see above)"
    echo "bench-pack smoke: OK"
else
    echo "bench-pack smoke: SKIPPED (no ./target/debug/physis-pro; run cargo build --bin physis-pro)"
fi

echo "RELEASE-CHECK OK (version $VER)"
