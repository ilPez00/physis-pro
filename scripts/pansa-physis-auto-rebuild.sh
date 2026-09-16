#!/usr/bin/env bash
# Build-and-swap updater for Physis on pansa-ts.
#
# The important ordering is: fetch → build detached commit → smoke-test the
# candidate → swap/restart → health-check → fast-forward git. A failed build,
# missing licence, or unhealthy restart leaves the currently running binary
# and checkout untouched, so one bad commit cannot poison the next update tick.

set -Eeuo pipefail

REPO_DIR="${PHYSIS_REPO_DIR:-/home/gio/physis_pro}"
RUNTIME_DIR="${PHYSIS_RUNTIME_DIR:-/home/gio/physis-runtime}"
LOG_FILE="${PHYSIS_UPDATE_LOG:-$REPO_DIR/logs/auto_update.log}"
LOCK_FILE="${PHYSIS_UPDATE_LOCK:-/tmp/physis-auto-update.lock}"
DEPLOY_BRANCH="${DEPLOY_BRANCH:-main}"
HEALTH_URL="${PHYSIS_HEALTH_URL:-http://127.0.0.1:19876/health}"
BUILD_IMAGE="${PHYSIS_BUILD_IMAGE:-rust:1-trixie}"
FEATURES="${PHYSIS_BUILD_FEATURES:-web,cli,embed-onnx}"

mkdir -p "$(dirname "$LOG_FILE")" "$RUNTIME_DIR"
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
[[ "$local_rev" != "$remote_rev" ]] || exit 0
log "Preparing $local_rev -> $remote_rev"

worktree=$(mktemp -d /tmp/physis-build.XXXXXX)
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
cargo_home=$(mktemp -d /tmp/physis-cargo.XXXXXX)
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
[[ -x "$candidate" ]] || { log "Build produced no executable."; exit 1; }

# A web binary is licence-gated before it binds. Run it on an ephemeral port;
# timeout 124 means startup succeeded and was intentionally stopped.
log "Smoke-testing candidate startup"
set +e
timeout 25s env PHYSIS_HOST=127.0.0.1 PHYSIS_PORT=0 \
  "$candidate" >>"$LOG_FILE" 2>&1
smoke=$?
set -e
if [[ "$smoke" != 124 ]]; then
  log "Candidate startup failed (exit $smoke); likely missing/invalid licence."
  exit 1
fi

live="$RUNTIME_DIR/physis-pro-web"
backup="$RUNTIME_DIR/physis-pro-web.previous"
if [[ ! -e "$live" ]]; then
  service_exec=$(systemctl --user show physis-engine.service -p ExecStart --value | sed -n 's/.*path=\([^ ;]*\).*/\1/p')
  [[ -n "$service_exec" && -x "$service_exec" ]] || { log "No existing binary to seed runtime."; exit 1; }
  cp "$service_exec" "$live"
fi
cp -p "$live" "$backup"
install -m 755 "$candidate" "$live.next"
mv -f "$live.next" "$live"

log "Restarting physis-engine"
if ! systemctl --user restart physis-engine.service; then
  log "Restart failed; restoring previous binary."
  mv -f "$backup" "$live"
  systemctl --user restart physis-engine.service || true
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
  systemctl --user restart physis-engine.service || true
  exit 1
fi

git merge --ff-only "origin/$DEPLOY_BRANCH" >>"$LOG_FILE" 2>&1 || {
  log "Engine is healthy but checkout fast-forward failed; resolve manually."
  exit 1
}
log "Updated successfully to $(git rev-parse --short HEAD)."
