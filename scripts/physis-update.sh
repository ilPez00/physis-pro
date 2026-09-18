#!/usr/bin/env bash
# physis-update — local binary updater for the physis-pro workspace.
#
# Rebuilds the release binaries you actually use and swaps them into
# ~/.local/bin with a backup + smoke test + rollback. No full reinstall, no
# docker, no systemd involvement: the symlinks in ~/.local/bin already point
# at the swap directory, so an update is a file replace + one mv.
#
# Design (mirrors scripts/pansa-physis-auto-rebuild.sh, minus the service):
#   build → smoke-test the candidate → swap → verify → stamp
# A failed build or a failed smoke leaves the installed binaries untouched.
#
# Usage:
#   scripts/physis-update.sh              # rebuild + swap all default bins
#   scripts/physis-update.sh --bin physis-pro --bin physis-hud
#   scripts/physis-update.sh --check      # only report what would rebuild
#   scripts/physis-update.sh --rollback   # restore the previous binaries
#
# Bins are built with `--features tui,cli` by default (the nav/HUD set) and
# PHYSIS_ALLOW_DEV_LICENCE must be exported by your shell env as usual.
set -Eeuo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

# Bins updated by default. physis-hud needs the tui feature; physis-pro and
# physis need cli. Add more via --bin.
DEFAULT_BINS=(physis-pro physis physis-hud)
BINS=()
CHECK_ONLY=0
ROLLBACK=0
FEATURES="${PHYSIS_UPDATE_FEATURES:-tui,cli}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) BINS+=("$2"); shift 2 ;;
    --check) CHECK_ONLY=1; shift ;;
    --rollback) ROLLBACK=1; shift ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done
[[ ${#BINS[@]} -eq 0 ]] && BINS=("${DEFAULT_BINS[@]}")

INSTALL_DIR="${PHYSIS_BIN_DIR:-$HOME/.local/bin}"
BACKUP_DIR="$REPO_DIR/target/update-backup"
STAMP="$REPO_DIR/target/installed-rev"
FEATURES="${PHYSIS_UPDATE_FEATURES:-tui,cli}"
# shellcheck disable=SC2034  # FEATURES read by the build below
REV="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

log() { echo "[update] $*"; }
die() { echo "[update] FAIL: $*" >&2; exit 1; }

# ── Rollback ────────────────────────────────────────────────────────────────
if [[ "$ROLLBACK" == 1 ]]; then
  for bin in "${BINS[@]}"; do
    if [[ -f "$BACKUP_DIR/$bin" ]]; then
      install -m 755 "$BACKUP_DIR/$bin" "$INSTALL_DIR/$bin"
      log "rolled back $bin"
    else
      log "no backup for $bin; skipped"
    fi
  done
  exit 0
fi

# ── What would change? ──────────────────────────────────────────────────────
installed_rev="$(cat "$STAMP" 2>/dev/null || echo none)"
dirty=""
git diff-index --quiet HEAD -- || dirty=" (dirty tree)"
if [[ "$CHECK_ONLY" == 1 ]]; then
  log "installed: $installed_rev | checkout: $REV$dirty"
  for bin in "${BINS[@]}"; do
    if [[ -L "$INSTALL_DIR/$bin" ]]; then
      log "  $bin -> $(readlink "$INSTALL_DIR/$bin")"
    elif [[ -f "$INSTALL_DIR/$bin" ]]; then
      log "  $bin: regular file (not managed by this updater)"
    else
      log "  $bin: not installed"
    fi
  done
  exit 0
fi

# ── Build ───────────────────────────────────────────────────────────────────
# One build invocation covers every requested bin; cargo reuses the plan.
log "building ${BINS[*]} (features: $FEATURES) at $REV$dirty"
cargo build --release --features "$FEATURES" $(printf -- "--bin %s " "${BINS[@]}") \
  || die "build failed; installed binaries untouched"

for bin in "${BINS[@]}"; do
  [[ -x "target/release/$bin" ]] || die "build produced no target/release/$bin"
done

# ── Smoke: each candidate answers --help without dying ──────────────────────
# Licence errors surface here, not after the swap. A dev build needs
# PHYSIS_ALLOW_DEV_LICENCE=1; the script exports it for the probe only if the
# caller did not already set a real licence.
probe_env=()
if [[ -z "${PHYSIS_LICENSE_FILE:-}" && -z "${PHYSIS_LICENSE:-}" ]]; then
  probe_env=(PHYSIS_ALLOW_DEV_LICENCE=1 PHYSIS_DATA_DIR=/tmp/physis-update-probe)
fi
for bin in "${BINS[@]}"; do
  # TUI binaries exit non-zero without a TTY but only *after* the licence
  # gate; a licence refusal or a panic is the failure, exit code alone is not.
  out="$(env "${probe_env[@]}" timeout 20 "target/release/$bin" --help 2>&1 || true)"
  if [[ "$out" == *"licence check failed"* || "$out" == *"refusing to enforce"* \
     || "$out" == *"panicked"* ]]; then
    echo "$out" | tail -3 >&2
    die "smoke failed for $bin; installed binaries untouched"
  fi
done
log "smoke passed for ${BINS[*]}"

# ── Swap ────────────────────────────────────────────────────────────────────
# The install dir holds *hard copies* (not symlinks into target/): a cargo
# clean or a branch switch must never break the installed tool.
mkdir -p "$BACKUP_DIR" "$INSTALL_DIR"
for bin in "${BINS[@]}"; do
  if [[ -f "$INSTALL_DIR/$bin" ]]; then
    cp -p "$INSTALL_DIR/$bin" "$BACKUP_DIR/$bin"
  fi
  # Replaces a stale symlink too.
  install -m 755 "target/release/$bin" "$INSTALL_DIR/$bin.new"
  mv -f "$INSTALL_DIR/$bin.new" "$INSTALL_DIR/$bin"
  log "installed $INSTALL_DIR/$bin"
done
printf '%s' "$REV" > "$STAMP"
log "done — installed at $REV$dirty. Rollback: $0 --rollback"
