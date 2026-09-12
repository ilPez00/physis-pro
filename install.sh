#!/usr/bin/env bash
# Physis Pro — install from a local build into ~/.local/bin and ~/.physis.
#
# Mirrors the installer that scripts/package.sh ships, but consumes the
# workspace's own target/release output instead of a staged tarball. Idempotent
# (safe to run twice), never touches system paths, never deletes anything it did
# not create. Use --dry-run to see the plan without changing anything.
#
# Requires: cargo build --release  (run that first — see scripts/package.sh for
# the signed, licence-gated customer package).

set -euo pipefail

# --- configuration -----------------------------------------------------------
PREFIX="${PREFIX:-$HOME/.local}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$ROOT/target/release"
# The product binaries a release build produces (see [[bin]] in Cargo.toml).
BINARIES=(physis-pro physis-pro-web physis-pro-tui physis-pro-wiki)

# --- arguments --------------------------------------------------------------
DRY_RUN=0
case "${1:-}" in
  --dry-run) DRY_RUN=1 ;;
  --help|-h)
    cat <<'EOF'
Physis Pro — install from a local build.
Usage: install.sh [--dry-run]
  --dry-run   print the plan; change nothing; exit 0
Installs the release binaries to $PREFIX/bin (default ~/.local/bin) and config
files to ~/.physis/config. Assumes `cargo build --release` has already run.
EOF
    exit 0
    ;;
esac

echo "Physis Pro install $([ "$DRY_RUN" = 1 ] && echo "(dry run)" )"
echo "  prefix : $PREFIX/bin"
echo "  config : $HOME/.physis/config"

# --- dry run: show the plan, touch nothing ----------------------------------
if [ "$DRY_RUN" = 1 ]; then
  if [ -d "$RELEASE_DIR" ]; then
    echo "  source : $RELEASE_DIR"
  else
    echo "  source : $RELEASE_DIR (MISSING — first run: cargo build --release)"
  fi
  for b in "${BINARIES[@]}"; do
    if [ -f "$RELEASE_DIR/$b" ]; then
      echo "  plan   : install $b -> $PREFIX/bin/$b"
    else
      echo "  plan   : WARN binary not built: $b (skipped until built)"
    fi
  done
  echo "  plan   : copy config/*.json -> $HOME/.physis/config/"
  echo "  (dry run only — nothing was installed)"
  exit 0
fi

# --- real install -----------------------------------------------------------
mkdir -p "$PREFIX/bin" "$HOME/.physis/config"

installed=0
for b in "${BINARIES[@]}"; do
  if [ -f "$RELEASE_DIR/$b" ]; then
    cp -f "$RELEASE_DIR/$b" "$PREFIX/bin/$b"
    installed=$((installed + 1))
  else
    printf '  WARN: %s not found in %s; skipped (build first)\n' "$b" "$RELEASE_DIR" >&2
  fi
done

if [ -d "$ROOT/config" ]; then
  cp -f "$ROOT"/config/*.json "$HOME/.physis/config/" 2>/dev/null || true
fi

echo "Installed $installed binary(ies) to $PREFIX/bin"
echo "Config in $HOME/.physis/config"
echo "Done."