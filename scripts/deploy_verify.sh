#!/usr/bin/env bash
# Verify the live physis management API through the public praxisweb proxy.
#
#   praxisweb.xyz/api/physis/v1/<x>  →  praxis-backend  →  physis-engine /api/v1/<x>
#
# Runs the OEE (P0) and predictive-maintenance (P1) flows end-to-end against
# production. No auth needed while the proxy is mounted public+rate-limited.
#
# Usage: scripts/deploy_verify.sh [base_url]
#   base_url defaults to https://praxisweb.xyz/api/physis
set -euo pipefail

BASE="${1:-https://praxisweb.xyz/api/physis}"
V1="$BASE/v1"
JQ() { command -v jq >/dev/null && jq "$@" || cat; }

echo "== 1. wait for engine via proxy =="
for i in $(seq 1 60); do
  if curl -sf -m5 "$BASE/health" | grep -q '"alive":true'; then
    echo "   engine alive after ${i} checks"; break
  fi
  [ "$i" = 60 ] && { echo "   ERROR: engine not alive after 60 checks"; curl -s -m5 "$BASE/health"; exit 1; }
  sleep 3
done

echo "== 2. OEE — register machine + textbook shift =="
curl -sf -X POST "$V1/oee/machine" -H 'Content-Type: application/json' \
  -d '{"machine_id":"press-01","shift_length_secs":28800,"ideal_cycle_time_secs":1.0}' >/dev/null
curl -sf -X POST "$V1/oee/event" -H 'Content-Type: application/json' \
  -d '{"kind":"stop","machine_id":"press-01","reason":"planned_stop","duration_secs":1800,"note":"lunch"}' >/dev/null
curl -sf -X POST "$V1/oee/event" -H 'Content-Type: application/json' \
  -d '{"kind":"stop","machine_id":"press-01","reason":"breakdown","duration_secs":2820,"note":"die jam"}' >/dev/null
curl -sf -X POST "$V1/oee/event" -H 'Content-Type: application/json' \
  -d '{"kind":"production","machine_id":"press-01","good":18848,"scrap":423}' >/dev/null
OEE=$(curl -sf "$V1/oee/press-01")
echo "$OEE" | JQ '{availability,performance,quality,oee,world_class}'
echo "$OEE" | grep -q '"oee":0.69' && echo "   ✓ OEE ≈ 0.698 (textbook)" || echo "   ✗ unexpected OEE"

echo "== 3. OEE plant rollup =="
curl -sf "$V1/oee/plant" | JQ '{machine_count,oee,losses:[.losses[].label]}'

echo "== 4. maintenance — train baseline (30 healthy readings) =="
for i in $(seq 0 29); do
  vib=$(awk "BEGIN{print 2.0 + ($i%4)*0.05}")
  curl -sf -X POST "$V1/maintenance/observe" -H 'Content-Type: application/json' \
    -d "{\"machine_id\":\"press-01\",\"temperature\":50.0,\"vibration\":$vib}" >/dev/null
done

echo "== 5. maintenance — normal reading (expect no anomaly) =="
curl -sf -X POST "$V1/maintenance/check" -H 'Content-Type: application/json' \
  -d '{"machine_id":"press-01","temperature":50.1,"vibration":2.05}' | JQ '.score|{is_anomaly,max_abs_z,worst_feature}'

echo "== 6. maintenance — anomalous vibration (expect anomaly + coherence node) =="
curl -sf -X POST "$V1/maintenance/check" -H 'Content-Type: application/json' \
  -d '{"machine_id":"press-01","temperature":50.0,"vibration":4.5,"rul_metric":"vibration","rul_limit":6.0}' \
  | JQ '{is_anomaly:.score.is_anomaly,worst:.score.worst_feature,sigma:.score.max_abs_z,node:.coherence_node_id,rul:.rul}'

echo "== 7. blocked endpoint (chat must 403 via proxy) =="
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$V1/chat" -H 'Content-Type: application/json' -d '{"messages":[]}')
[ "$code" = "403" ] && echo "   ✓ /v1/chat blocked (403)" || echo "   ✗ /v1/chat returned $code (expected 403)"

echo "== DONE =="
