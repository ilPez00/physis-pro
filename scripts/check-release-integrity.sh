#!/usr/bin/env bash
# scripts/check-release-integrity.sh — refuse to publish from a tree that does
# not match what would be recorded (PH-014 Item B).
#
# Three incidents on 2026-09-04/05 shared one root cause: something was
# committed or published from a working tree that did not match what was
# recorded. This script is the gate that makes the next release reproducible:
# it exits non-zero when
#   1. the working tree is dirty (git status --porcelain is non-empty);
#   2. HEAD is not tagged v<version>, where <version> is the crate's
#      `version =` field;
#   3. that version already exists on crates.io.
#
# Usage:
#   scripts/check-release-integrity.sh [--crate-dir DIR] [--crate NAME]
#   scripts/check-release-integrity.sh --self-test
set -euo pipefail

CRATE_DIR="."
CRATE_NAME=""
SELF_TEST=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --crate-dir) CRATE_DIR="$2"; shift 2 ;;
    --crate) CRATE_NAME="$2"; shift 2 ;;
    --self-test) SELF_TEST=1; shift ;;
    *) echo "check-release-integrity: unknown argument: $1" >&2; exit 2 ;;
  esac
done

# Run the three checks against the git repo rooted in the crate dir $1.
# Prints the reason and returns non-zero on the first failing check.
check_release_integrity() {
  local dir="$1"
  local crate_toml="$dir/Cargo.toml"

  if [[ ! -f "$crate_toml" ]]; then
    echo "check-release-integrity: no Cargo.toml in $dir" >&2
    return 2
  fi

  if [[ -n "$(git -C "$dir" status --porcelain)" ]]; then
    echo "check-release-integrity: working tree is dirty — commit or stash before publishing" >&2
    return 1
  fi

  local version
  version=$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$crate_toml" | head -1)
  if [[ -z "$version" ]]; then
    echo "check-release-integrity: could not read version from $crate_toml" >&2
    return 2
  fi

  local tag="v${version}"
  if ! git -C "$dir" tag --points-at HEAD | grep -qx "$tag"; then
    echo "check-release-integrity: HEAD is not tagged $tag — tag the release commit before publishing" >&2
    return 1
  fi

  local name
  name="${CRATE_NAME:-$(sed -n 's/^name *= *"\([^"]*\)".*/\1/p' "$crate_toml" | head -1)}"
  if command -v curl >/dev/null 2>&1; then
    local http_status
    http_status=$(curl -s -o /dev/null -w '%{http_code}' \
      "https://crates.io/api/v1/crates/${name}/${version}" || echo "000")
    if [[ "$http_status" == "200" ]]; then
      echo "check-release-integrity: ${name} ${version} is already published on crates.io — bump the version" >&2
      return 1
    fi
  else
    echo "check-release-integrity: curl not found, skipping the crates.io duplicate-version check" >&2
  fi

  echo "check-release-integrity: OK — ${name} ${version} at tag ${tag}, tree clean, not yet published"
  return 0
}

if [[ "$SELF_TEST" -eq 1 ]]; then
  # Prove the guard actually rejects the conditions it exists for. Runs
  # against disposable repos, not the live one — this repo is normally dirty
  # mid-development, which would make a live check a permanent no-op.
  fail=0
  tmp_dirty=$(mktemp -d)
  tmp_clean=$(mktemp -d)
  trap 'rm -rf "$tmp_dirty" "$tmp_clean"' EXIT

  git -C "$tmp_dirty" init -q
  printf 'name = "selftest"\nversion = "0.0.1"\n' > "$tmp_dirty/Cargo.toml"
  git -C "$tmp_dirty" add Cargo.toml
  git -C "$tmp_dirty" -c user.email=t@t -c user.name=t commit -q -m init
  git -C "$tmp_dirty" tag v0.0.1
  echo "# dirty" >> "$tmp_dirty/Cargo.toml"
  if check_release_integrity "$tmp_dirty" >/dev/null 2>&1; then
    echo "SELF-TEST FAIL: a dirty tree was not rejected" >&2
    fail=1
  else
    echo "SELF-TEST PASS: dirty tree rejected"
  fi

  git -C "$tmp_clean" init -q
  printf 'name = "selftest"\nversion = "0.0.1"\n' > "$tmp_clean/Cargo.toml"
  git -C "$tmp_clean" add Cargo.toml
  git -C "$tmp_clean" -c user.email=t@t -c user.name=t commit -q -m init
  # Deliberately left untagged.
  if check_release_integrity "$tmp_clean" >/dev/null 2>&1; then
    echo "SELF-TEST FAIL: an untagged HEAD was not rejected" >&2
    fail=1
  else
    echo "SELF-TEST PASS: untagged HEAD rejected"
  fi

  exit "$fail"
fi

check_release_integrity "$CRATE_DIR"
