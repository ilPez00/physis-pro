#!/usr/bin/env bash
# fleet-pull-indexes.sh — wait for per-node prescan to finish, scp the index,
# merge into pansa's fleet index via nav import --prefix.
# Run after launching fleet prescan with: scripts/fleet-pull-indexes.sh &
set -euo pipefail

PANSA_DATA="${PHYSIS_DATA_DIR:-$HOME/physis_pro/.physis}"
HUD="$HOME/physis_pro/target/release/physis-hud"
FLEET_DATA="$HOME/.physis-fleet"
LOG="/tmp/fleet-pull.log"

log() { echo "[fleet-pull $(date +%H:%M:%S)] $*" | tee -a "$LOG"; }

declare -A NODES=(
  ["100.94.69.0"]="feather-1"
  ["100.66.51.47"]="air"
  ["100.116.39.57"]="omo"
)

log "Fleet index pull started. Watching for prescan completion..."

for ip in "${!NODES[@]}"; do
  name="${NODES[$ip]}"
  (
    log "$name: waiting for prescan to finish..."
    # Poll until physis-hud is no longer running on the node
    while ssh -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=5 gio@"$ip" \
        "pgrep -x physis-hud > /dev/null 2>&1"; do
      sleep 15
    done
    log "$name: prescan complete, pulling index..."

    # Pull the index file
    NODE_DATA="$HOME/.physis-fleet/$name"
    mkdir -p "$NODE_DATA"
    INDEX_FILE="$NODE_DATA/nav-index.json"

    # Find the index on the remote
    REMOTE_INDEX=$(ssh -o StrictHostKeyChecking=no -o BatchMode=yes gio@"$ip" \
        "ls ~/.physis-fleet/$name/nav-index*.json 2>/dev/null | head -1" 2>/dev/null || echo "")

    if [[ -z "$REMOTE_INDEX" ]]; then
      log "$name: no index found at ~/.physis-fleet/$name/ — skipping"
      exit 0
    fi

    scp -o StrictHostKeyChecking=no gio@"$ip":"$REMOTE_INDEX" "$INDEX_FILE"
    SIZE=$(ls -lh "$INDEX_FILE" | awk '{print $5}')
    log "$name: index pulled ($SIZE)"

    # Check stamp
    STAMP=$(python3 -c "import json; d=json.load(open('$INDEX_FILE')); print(d.get('embedder_stamp','none'))")
    OBJECTS=$(python3 -c "import json; d=json.load(open('$INDEX_FILE')); print(len(d.get('objects',{})))")
    log "$name: $OBJECTS objects, stamp=$STAMP"
    log "$name: ready for 'physis-pro nav import --prefix $name $INDEX_FILE'"
    echo "DONE: $name $OBJECTS objects stamp=$STAMP" >> "$LOG"
  ) &
done

wait
log "All fleet pulls complete. See $LOG for import commands."
