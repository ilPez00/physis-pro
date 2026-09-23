#!/usr/bin/env bash
# Build-and-swap updater for Physis on pansa-ts.
#
# The important ordering is: fetch → build detached commit → smoke-test the
# candidate → swap/restart → health-check → stamp. A failed build, missing
# licence, or unhealthy restart leaves the currently running binary and
# checkout untouched, so one bad commit cannot poison the next update tick.
# The stamp ($RUNTIME_DIR/deployed-rev) is the trigger: a tick rebuilds when
# origin/main differs from the *deployed* revision, not merely from the local
# checkout — otherwise a push made from this checkout skips forever.

set -Eeuo pipefail

REPO_DIR="${PHYSIS_REPO_DIR:-/home/gio/physis_pro}"
RUNTIME_DIR="${PHYSIS_RUNTIME_DIR:-/home/gio/physis-runtime}"
LOG_FILE="${PHYSIS_UPDATE_LOG:-$REPO_DIR/logs/auto_update.log}"
LOCK_FILE="${PHYSIS_UPDATE_LOCK:-/tmp/physis-auto-update.lock}"
DEPLOY_BRANCH="${DEPLOY_BRANCH:-main}"
HEALTH_URL="${PHYSIS_HEALTH_URL:-http://127.0.0.1:19876/health}"
BUILD_IMAGE="${PHYSIS_BUILD_IMAGE:-rust:1-trixie}"
FEATURES="${PHYSIS_BUILD_FEATURES:-web,cli,embed-onnx}"
# The service unit exports this; the smoke test must reproduce it. Without
# MODEL_DIR the candidate cannot find the ONNX weights, the embedder fails its
# self-test, and the server deliberately refuses to serve a degraded
# random-projection embedder — so every candidate "failed to start" while the
# engine it was replacing was perfectly healthy.
MODEL_DIR="${PHYSIS_MODEL_DIR:-/home/gio/physis-models}"
# Same reasoning for the licence: the service unit exports it, but the updater
# must not depend on being launched by systemd to find it.
LICENSE_FILE="${PHYSIS_LICENSE_FILE:-/home/gio/.physis/license.key}"
# Service unit name varies per host (pansa: physis-engine; omo: physis-omo).
ENGINE_SERVICE="${PHYSIS_ENGINE_SERVICE:-physis-engine.service}"

# Where the throwaway worktree and the throwaway CARGO_HOME live. Both used to
# be `mktemp -d /tmp/...`, and /tmp on pansa is a 7.8 GB tmpfs — the registry
# alone exhausted it (`No space left on device` unpacking aws-lc-sys), so every
# tick failed before compiling a line. Prefer the data disk, fall back to /var/tmp
# (disk-backed, unlike /tmp) when it is not mounted.
if [[ -z "${PHYSIS_BUILD_ROOT:-}" ]]; then
  if mountpoint -q /mnt/bigstore; then
    PHYSIS_BUILD_ROOT=/mnt/bigstore/physis-build
  else
    PHYSIS_BUILD_ROOT=/var/tmp/physis-build
  fi
fi
BUILD_ROOT="$PHYSIS_BUILD_ROOT"

# Revision of the binary the engine is actually running. Comparing the local
# checkout with origin is not enough: when the push happens *from* this checkout
# the two are equal while the running binary is still months old, and the
# updater then skips forever. The stamp is written only after a healthy restart.
STAMP_FILE="${PHYSIS_DEPLOY_STAMP:-$RUNTIME_DIR/deployed-rev}"

mkdir -p "$(dirname "$LOG_FILE")" "$RUNTIME_DIR" "$BUILD_ROOT"
log() { echo "[$(date -u +'%Y-%m-%dT%H:%M:%SZ')] $*" | tee -a "$LOG_FILE"; }

exec 9>"$LOCK_FILE"
flock -n 9 || { log "Another update is running; skipping."; exit 0; }

cd "$REPO_DIR"
branch=$(git rev-parse --abbrev-ref HEAD)
[[ "$branch" == "$DEPLOY_BRANCH" ]] || { log "On $branch, not $DEPLOY_BRANCH; skipping."; exit 0; }
git diff-index --quiet HEAD -- || { log "Dirty checkout; refusing automatic update."; git status --short | head -20 | tee -a "$LOG_FILE"; exit 0; }

git fetch origin "$DEPLOY_BRANCH" >>"$LOG_FILE" 2>&1 || { log "Fetch failed; retrying later."; exit 0; }
local_rev=$(git rev-parse HEAD)
remote_rev=$(git rev-parse "origin/$DEPLOY_BRANCH")
deployed_rev=$(cat "$STAMP_FILE" 2>/dev/null || true)
if [[ "$local_rev" == "$remote_rev" && "$deployed_rev" == "$remote_rev" ]]; then
  exit 0
fi
log "Preparing $local_rev -> $remote_rev (deployed: ${deployed_rev:-unknown})"

worktree=$(mktemp -d "$BUILD_ROOT/physis-build.XXXXXX")
cleanup() { git worktree remove --force "$worktree" >>"$LOG_FILE" 2>&1 || true; rmdir "$worktree" 2>/dev/null || true; }
trap cleanup EXIT
git worktree add --detach "$worktree" "$remote_rev" >>"$LOG_FILE" 2>&1

# A linked worktree checks out no submodule contents, so `physis-core/Cargo.toml`
# is absent and the workspace fails to load — every tick since 2026-09-12 died
# here, leaving the running binary untouched. Populate the submodule at the
# revision the superproject pins before handing the tree to docker.
if ! git -C "$worktree" submodule update --init --recursive >>"$LOG_FILE" 2>&1; then
  log "Submodule init failed; running binary remains untouched."
  exit 1
fi

# Build as the deployment user. This avoids root-owned target artifacts and
# keeps the source checkout usable by the updater after a failed build.
cargo_home=$(mktemp -d "$BUILD_ROOT/physis-cargo.XXXXXX")
trap 'rm -rf "$cargo_home"; cleanup' EXIT
candidate="$worktree/target/release/physis-pro-web"
log "Building candidate with $BUILD_IMAGE ($FEATURES)"
if ! docker run --rm --user "$(id -u):$(id -g)" \
    -v "$worktree":/app -w /app -v "$cargo_home":/cargo \
    -e CARGO_HOME=/cargo -e PHYSIS_LICENSE_PUBKEY="$(cat /home/gio/.physis-vendor/verifying.key)" "$BUILD_IMAGE" \
    cargo build --release --no-default-features --features "$FEATURES" \
    --bin physis-pro-web >>"$LOG_FILE" 2>&1; then
  log "Build failed; running binary remains untouched."
  exit 1
fi
# docker is unavailable on some hosts (omo-ts: desktop.sock absent, no dockerd).
# Fall back to a native cargo build so the engine still updates without a
# container runtime. Mirrors praxis auto_update.sh, which dropped docker for the
# same failure mode.
if [[ ! -x "$candidate" ]]; then
  if docker info >/dev/null 2>&1; then
    log "Build produced no executable (docker path)."; exit 1
  fi
  log "docker unavailable — building natively with cargo instead."
  export PHYSIS_BUILD_FEATURES="${PHYSIS_BUILD_FEATURES:-$FEATURES}"
  ( cd "$worktree" && PHYSIS_LICENSE_PUBKEY="$(cat /home/gio/.physis-vendor/verifying.key 2>/dev/null || true)" \
      cargo build --release --no-default-features --features "$FEATURES" --bin physis-pro-web ) >>"$LOG_FILE" 2>&1 \
    || { log "Native build failed; running binary remains untouched."; exit 1; }
fi
[[ -x "$candidate" ]] || { log "Build produced no executable."; exit 1; }

# A web binary is licence-gated before it binds. Run it on an ephemeral port;
# timeout 124 means startup succeeded and was intentionally stopped.
log "Smoke-testing candidate startup"
set +e
# Run from $RUNTIME_DIR, exactly as the unit does. The engine resolves weights
# at the relative path `models/bge-base-en-v1.5`, so a smoke test started from
# the repo (where `models` is a different, incomplete tree) loads no weights,
# falls back to random projection, and the server refuses to serve degraded —
# reporting a healthy candidate as broken.
#
# The throwaway token is not the service's: the server refuses a loopback bind
# with no tokens and no PHYSIS_DEV, and the smoke test only needs the process to
# reach "listening" on an ephemeral port before `timeout` kills it.
( cd "$RUNTIME_DIR" && timeout 25s env PHYSIS_HOST=127.0.0.1 PHYSIS_PORT=0 \
    MODEL_DIR="$MODEL_DIR" PHYSIS_LICENSE_FILE="$LICENSE_FILE" \
    PHYSIS_API_TOKEN=smoke-test-only \
    "$candidate" ) >>"$LOG_FILE" 2>&1
smoke=$?
set -e
if [[ "$smoke" != 124 ]]; then
  log "Candidate startup failed (exit $smoke); see the lines above."
  exit 1
fi

live="$RUNTIME_DIR/physis-pro-web"
backup="$RUNTIME_DIR/physis-pro-web.previous"
if [[ ! -e "$live" ]]; then
  service_exec=$(systemctl --user show $ENGINE_SERVICE -p ExecStart --value | sed -n 's/.*path=\([^ ;]*\).*/\1/p')
  [[ -n "$service_exec" && -x "$service_exec" ]] || { log "No existing binary to seed runtime."; exit 1; }
  cp "$service_exec" "$live"
fi
cp -p "$live" "$backup"
install -m 755 "$candidate" "$live.next"
mv -f "$live.next" "$live"

log "Restarting physis-engine"
if ! systemctl --user restart $ENGINE_SERVICE; then
  log "Restart failed; restoring previous binary."
  mv -f "$backup" "$live"
  systemctl --user restart $ENGINE_SERVICE || true
  exit 1
fi

healthy=false
for _ in $(seq 1 90); do
  if curl -sf -m 3 "$HEALTH_URL" >/dev/null 2>&1; then healthy=true; break; fi
  sleep 4
done
if [[ "$healthy" != true ]]; then
  log "Health check failed; restoring previous binary."
  mv -f "$backup" "$live"
  systemctl --user restart $ENGINE_SERVICE || true
  exit 1
fi

# Stamp only after the new binary is observed healthy. The stamp is what stops
# the next tick from rebuilding an unchanged revision, and what makes a tick
# rebuild when the checkout already matched origin but the running binary did
# not (the push-from-this-checkout case).
printf '%s' "$remote_rev" > "$STAMP_FILE"

# The checkout is a convenience, not the artifact: the engine runs from
# $RUNTIME_DIR. A failed fast-forward (the push came from this very checkout, so
# HEAD is ahead of, or diverged from, origin) must not undo a healthy deploy.
if git merge --ff-only "origin/$DEPLOY_BRANCH" >>"$LOG_FILE" 2>&1; then
  log "Updated successfully to $(git rev-parse --short HEAD)."
else
  log "Engine healthy and stamped at ${remote_rev:0:7}; checkout is not a fast-forward of origin/$DEPLOY_BRANCH (HEAD $(git rev-parse --short HEAD)) — left as-is."
fi
