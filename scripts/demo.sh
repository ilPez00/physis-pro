#!/usr/bin/env bash
# demo.sh — launch Physis as a live product demo.
#
# Opens the demo dashboard at http://<host>:<port>/demo, seeded live from the
# in-process engine (no external services). Run from the repo root so the
# bundled ONNX model resolves (config default `./models`).
#
#   ./scripts/demo.sh                 # 127.0.0.1:19876
#   PHYSIS_HOST=0.0.0.0 ./scripts/demo.sh   # reachable on the LAN (second screen)
#
# Config:
#   PHYSIS_PORT       port (default 19876)
#   PHYSIS_HOST       bind host (default 127.0.0.1)
#   PHYSIS_RATE_LIMIT the demo's seed+refresh burst (~70 req in a minute) exceeds
#                     the production 60 req/min gate; raise it for demo headroom.
#                     Default 600/60. Keep the prod default for real deployments.
set -euo pipefail
cd "$(dirname "$0")/.."

export PHYSIS_HOST="${PHYSIS_HOST:-127.0.0.1}"
export PHYSIS_PORT="${PHYSIS_PORT:-19876}"
export PHYSIS_RATE_LIMIT="${PHYSIS_RATE_LIMIT:-600/60}"

# The embedder degrades to random projection when the model is missing — the
# classify/semiotic panels would be noise, which is exactly what a demo can't
# afford. Fail loudly instead of showing a broken dashboard.
if [ ! -f models/bge-base-en-v1.5/onnx/model.onnx ] && [ ! -f models/model.onnx ]; then
  echo "ERROR: no ONNX model found under ./models — run ./scripts/download_models.sh first." >&2
  echo "  (the dashboard needs real semantic embeddings for classify + semiotic grid)" >&2
  exit 1
fi

BIN=target/debug/physis-pro-web
if [ ! -x "$BIN" ]; then
  echo "Building physis-pro-web (first run, ~1 min)…"
  cargo build --features web --bin physis-pro-web
fi

echo
echo "  Physis demo → http://$PHYSIS_HOST:$PHYSIS_PORT/demo"
echo "  click [Seed demo data] to bring the shop floor alive."
echo "  rate-limit ${PHYSIS_RATE_LIMIT} (prod default is 60/60 — demo needs the headroom)"
echo
exec "$BIN"
