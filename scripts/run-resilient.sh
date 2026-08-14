#!/usr/bin/env bash
# run-resilient.sh — run physis-pro on a flaky / removable mount.
#
# Two problems this solves when the checkout lives on a drive that can unmount
# mid-run (USB / network / encrypted volume):
#
#   1. CORRUPTION / SIGBUS. cargo mmaps build artifacts in target/; the engine
#      mmaps its sled state. If those live on the volatile mount, an unmount mid-
#      access faults the process (SIGBUS / "Input/output error"). So we move both
#      onto STABLE storage ($HOME by default) — the source stays on the mount,
#      but nothing mutable-and-mmapped does.
#
#   2. RESUME. We supervise the command: when it exits, if the mount has gone we
#      wait for it to come back and restart. The engine reloads its durable sled
#      state from the (stable) data dir, so it resumes where it left off; cargo
#      resumes incrementally from the (stable) target dir.
#
# Usage:
#   scripts/run-resilient.sh                       # run the web server (default)
#   scripts/run-resilient.sh cargo test            # supervise any command
#   scripts/run-resilient.sh ./target/release/physis-pro machine chronos -i ev.json
#
# Env:
#   PHYSIS_STABLE     stable dir for cache+state  (default: $HOME/.physis-stable)
#   PHYSIS_REPO       the (flaky) checkout         (default: this script's repo)
#   PHYSIS_MAX_RETRIES  retries on a real error    (default: 5)

set -uo pipefail

REPO="${PHYSIS_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
STABLE="${PHYSIS_STABLE:-$HOME/.physis-stable}"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$STABLE/target}"
export PHYSIS_DATA_DIR="${PHYSIS_DATA_DIR:-$STABLE/data}"
mkdir -p "$CARGO_TARGET_DIR" "$PHYSIS_DATA_DIR"

echo "[resilient] repo:   $REPO"
echo "[resilient] target: $CARGO_TARGET_DIR"
echo "[resilient] state:  $PHYSIS_DATA_DIR"

mounted() { [ -d "$REPO/src" ]; }

wait_mount() {
  echo "[resilient] mount gone — waiting for $REPO to return…"
  until mounted; do sleep 5; done
  echo "[resilient] mount back."
}

CMD=( "$@" )
if [ ${#CMD[@]} -eq 0 ]; then
  CMD=( cargo run --release --bin physis-pro-web --features web )
fi

attempt=0
while :; do
  attempt=$((attempt + 1))
  mounted || wait_mount
  echo "[resilient] start (attempt $attempt): ${CMD[*]}"
  ( cd "$REPO" && "${CMD[@]}" )
  code=$?
  echo "[resilient] exited with code=$code"

  [ "$code" -eq 0 ] && { echo "[resilient] clean exit."; break; }

  if ! mounted; then
    # Disk dropped under us — not a real failure. Wait and resume.
    wait_mount
    continue
  fi

  # Nonzero with the disk present = a genuine error. Bounded retry.
  if [ "$attempt" -ge "${PHYSIS_MAX_RETRIES:-5}" ]; then
    echo "[resilient] giving up after $attempt attempts (last code $code)"
    exit "$code"
  fi
  echo "[resilient] retrying in 3s…"
  sleep 3
done
