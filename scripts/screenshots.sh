#!/usr/bin/env bash
# screenshots.sh — capture the Physis UI for release posts and the marketing
# track (Phase 17 U6). Boots physis-pro-web, captures each page with a headless
# browser, and writes a `shots/` contact sheet you can review before posting.
#
#   ./scripts/screenshots.sh              # captures to ./shots/
#   PHYSIS_PORT=19877 ./scripts/screenshots.sh
#
# Requires the ONNX model (marketing shots need real semantics, same rule as
# demo.sh) and a headless browser (chromium / google-chrome). Air-gap honest:
# a missing model or browser fails loudly with the fix, never a half-shot.
set -euo pipefail
cd "$(dirname "$0")/.."

export PHYSIS_HOST="${PHYSIS_HOST:-127.0.0.1}"
export PHYSIS_PORT="${PHYSIS_PORT:-19876}"
export PHYSIS_RATE_LIMIT="${PHYSIS_RATE_LIMIT:-600/60}"
# Local capture server: loopback bind without a token is an explicit dev opt-in.
export PHYSIS_DEV="${PHYSIS_DEV:-1}"

if [ ! -f models/bge-base-en-v1.5/onnx/model.onnx ] && [ ! -f models/model.onnx ]; then
  echo "ERROR: no ONNX model found under ./models — run ./scripts/download_models.sh first." >&2
  echo "  (marketing shots need real semantic embeddings for classify + the semiotic grid)" >&2
  exit 1
fi

BROWSER=""
for c in chromium chromium-browser google-chrome google-chrome-stable; do
  if command -v "$c" >/dev/null 2>&1; then BROWSER="$c"; break; fi
done
if [ -z "$BROWSER" ]; then
  echo "ERROR: no headless-capable browser (chromium/google-chrome) on PATH." >&2
  echo "  install one (e.g. 'apt install chromium') or capture the URLs below by hand." >&2
  exit 1
fi

BIN=target/debug/physis-pro-web
if [ ! -x "$BIN" ]; then
  echo "Building physis-pro-web (first run, ~1 min)…"
  cargo build --features web --bin physis-pro-web
fi

OUT=shots
mkdir -p "$OUT"

BASE="http://$PHYSIS_HOST:$PHYSIS_PORT"
"$BIN" &
SERVER=$!
trap 'kill $SERVER 2>/dev/null || true' EXIT

echo "waiting for $BASE …"
for _ in $(seq 1 120); do
  if curl -sf "$BASE/health" >/dev/null 2>&1; then break; fi
  sleep 0.5
done
if ! curl -sf "$BASE/health" >/dev/null 2>&1; then
  echo "ERROR: server did not come up on $BASE — check the build/logs." >&2
  exit 1
fi

# name|path — every page worth a release visual. Product shells render better
# with a seeded graph (open /demo and click [Seed demo data] first, or ingest
# your own corpus); the marketing pages stand alone.
PAGES=(
  "landing|/"
  "pricing|/pricing"
  "about|/about"
  "deploy|/deploy"
  "settings|/settings"
  "pulse|/pulse"
  "console|/console"
  "search|/search"
  "studio|/studio"
  "demo|/demo"
)

index="<!DOCTYPE html><html><head><meta charset='utf-8'><title>Physis — capture sheet</title>"
index="$index<style>body{font-family:ui-monospace,monospace;background:#070b14;color:#dfe7ff;margin:2rem}"
index="$index img{width:720px;max-width:100%;border:1px solid #223;border-radius:8px}td{padding:8px 12px;vertical-align:top}</style></head><body>"
index="$index<h1>Physis — release capture sheet</h1><table>"

for entry in "${PAGES[@]}"; do
  name="${entry%%|*}"
  path="${entry##*|}"
  file="$OUT/$name.png"
  if "$BROWSER" --headless --disable-gpu --hide-scrollbars --screenshot="$file" \
      --window-size=1440,900 --virtual-time-budget=8000 "$BASE$path" >/dev/null 2>&1; then
    echo "  captured $file"
    index="$index<tr><td><img src='$name.png'></td><td><a href='$path' style='color:#7ef7dd'>$path</a></td></tr>"
  else
    echo "  warn: capture failed for $path (retry once)…"
    if "$BROWSER" --headless --disable-gpu --hide-scrollbars --screenshot="$file" \
        --window-size=1440,900 "$BASE$path" >/dev/null 2>&1; then
      echo "  captured (retry) $file"
      index="$index<tr><td><img src='$name.png'></td><td><a href='$path' style='color:#7ef7dd'>$path</a></td></tr>"
    fi
  fi
done
index="$index</table></body></html>"
printf '%s\n' "$index" > "$OUT/index.html"

echo
echo "  shots → $OUT/  (open $OUT/index.html to review)"
echo "  tip: seed the graph via /demo before re-capturing the product pages."
