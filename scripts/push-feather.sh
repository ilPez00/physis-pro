#!/usr/bin/env bash
# Push the pansa-built trixie binaries to feather.
#
# Feather is a weak client: no toolchain, ~96% full disk. It never builds — it
# installs and smoke-tests (docs/FEATHER.md). This script is the "push" half.
#
# Two rules, because feather may be *in use* by another agent while you run it:
#   1. Never touch feather's git checkout. Nothing here reads or writes ~/dev.
#   2. Replace binaries by `mv` inside the destination directory, which is an
#      atomic rename on one filesystem. A running `physis-pro` keeps the inode it
#      started with instead of dying with "text file busy" or executing a
#      half-copied file.
#
# Usage:
#   scripts/push-feather.sh                 # push every binary in BINS
#   scripts/push-feather.sh physis-studio   # push a subset
#   PHYSIS_FEATHER_SMOKE=1 scripts/push-feather.sh   # also run the offline smoke
set -Eeuo pipefail

HOST="${PHYSIS_FEATHER_HOST:-feather}"
BINDIR="${PHYSIS_FEATHER_BINDIR:-/home/gio/.local/bin}"
SRC="${PHYSIS_TARGET_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/target-trixie/release}"
ALL_BINS=(physis physis-pro physis-pro-web physis-pro-tui physis-pro-wiki physis-studio physis-hud)

BINS=("$@")
if [[ ${#BINS[@]} -eq 0 ]]; then
  BINS=("${ALL_BINS[@]}")
fi

log() { printf '[push-feather] %s\n' "$*"; }

# 1. Every requested binary must exist locally first: a partial push that
#    silently skips a target leaves feather with a mixed-version set.
missing=0
for b in "${BINS[@]}"; do
  if [[ ! -x "$SRC/$b" ]]; then
    log "MISSING: $SRC/$b (build it: cargo build --release, see docs/FEATHER.md)"
    missing=1
  fi
done
[[ $missing -eq 0 ]] || exit 1

# 2. Stage on the destination filesystem, so the final move is a rename.
stage="$BINDIR/.push-$$"
log "staging in $HOST:$stage"
ssh "$HOST" "mkdir -p '$stage' '$BINDIR'"

for b in "${BINS[@]}"; do
  scp -q "$SRC/$b" "$HOST:$stage/$b"
done

# 3. Atomic swap, then record versions for the report.
remote_swap=$(cat <<EOS
set -Eeuo pipefail
for b in ${BINS[*]}; do
  chmod 755 "$stage/\$b"
  mv -f "$stage/\$b" "$BINDIR/\$b"
done
rmdir "$stage"
for b in ${BINS[*]}; do
  printf '%-18s %s bytes  %s\n' "\$b" "\$(stat -c%s "$BINDIR/\$b")" "\$(stat -c%y "$BINDIR/\$b" | cut -d. -f1)"
done
EOS
)
log "installing into $HOST:$BINDIR"
ssh "$HOST" "$remote_swap"

# 4. Cheap compatibility proof: the binaries must start on feather's glibc.
ssh "$HOST" "ldd --version | head -1; physis-pro --version 2>&1 | head -1; physis-studio --help 2>&1 | head -2"

if [[ "${PHYSIS_FEATHER_SMOKE:-0}" == "1" ]]; then
  log "offline smoke (lexical embedder, no server, /tmp only)"
  ssh "$HOST" bash -s <<'EOS'
set -Eeuo pipefail
export PHYSIS_ALLOW_DEV_LICENCE=1 PHYSIS_EMBEDDER=random-projection
mkdir -p /tmp/physis-smoke
printf '# hello\n\nsmall test file about retrieval and packing\n' > /tmp/physis-smoke/a.md
printf 'fn main() {}\n' > /tmp/physis-smoke/b.rs
physis-hud /tmp/physis-smoke --prescan-only 2>&1 | tail -2
physis-pro pack /tmp/physis-smoke "retrieval packing" --budget 500 --top-k 5 2>&1 | tail -5
EOS
fi

log "done"
