#!/bin/sh
set -e

# stderr to bypass pipe buffering (Railway captures stderr)
echo "[entrypoint] starting" >&2
echo "[entrypoint] model files:" >&2
ls -la /app/models/bge-base-en-v1.5/onnx/ 2>/dev/null || echo "[entrypoint] no model dir at /app/models/bge-base-en-v1.5/onnx/" >&2
# ── Self-hosted Ollama floor (opt-in) ────────────────────────────────────────
# Guaranteed last-resort LLM for the provider cascade (priority "ollama_floor",
# http://localhost:11434). OFF by default — running a 2B model needs ~3GB RAM, so
# size the service before enabling. Model cached on the mounted volume (pulled
# once). The pull is backgrounded so /health passes while it downloads.
#   ENABLE_OLLAMA=1                 turn it on
#   OLLAMA_FLOOR_MODEL=gemma2:2b    tag to pull AND request (kept in sync below)
# Sidecar alternative (recommended for prod): run the official ollama/ollama
# image as its own service and point OLLAMA_FLOOR_URL at it instead of enabling
# this in-container path. See MANUAL_CONFIG.md.
if [ "${ENABLE_OLLAMA:-0}" = "1" ]; then
  export OLLAMA_FLOOR_MODEL="${OLLAMA_FLOOR_MODEL:-gemma2:2b}"   # cascade reads this too
  export OLLAMA_MODELS="${OLLAMA_MODELS:-/app/.physis/ollama}"   # persist on volume
  mkdir -p "$OLLAMA_MODELS" || true
  if ! command -v ollama >/dev/null 2>&1; then
    echo "[entrypoint] installing ollama..." >&2
    curl -fsSL https://ollama.com/install.sh | sh || echo "[entrypoint] ollama install failed (floor disabled)" >&2
  fi
  if command -v ollama >/dev/null 2>&1; then
    echo "[entrypoint] starting ollama serve (floor model: $OLLAMA_FLOOR_MODEL)" >&2
    ollama serve > /tmp/ollama.log 2>&1 &
    ( sleep 3
      ollama pull "$OLLAMA_FLOOR_MODEL" >> /tmp/ollama.log 2>&1 \
        && echo "[entrypoint] ollama floor ready: $OLLAMA_FLOOR_MODEL" >&2 \
        || echo "[entrypoint] ollama pull failed: $OLLAMA_FLOOR_MODEL" >&2
    ) &
  fi
fi

echo "[entrypoint] starting physis-pro-mcp on :9876" >&2
./physis-pro serve > /tmp/physis-mcp.log 2>&1 &
MCP_PID=$!
echo "[entrypoint] mcp pid=$MCP_PID" >&2
echo "[entrypoint] starting physis-pro-web" >&2
exec ./physis-pro-web
