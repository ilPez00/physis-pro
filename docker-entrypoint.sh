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
  export OLLAMA_MODELS="${OLLAMA_MODELS:-/app/.physis/ollama}"   # persist on volume
  mkdir -p "$OLLAMA_MODELS" || true
  if ! command -v ollama >/dev/null 2>&1; then
    echo "[entrypoint] installing ollama..." >&2
    curl -fsSL https://ollama.com/install.sh | sh || echo "[entrypoint] ollama install failed (floor disabled)" >&2
  fi
  if command -v ollama >/dev/null 2>&1; then
    ollama serve > /tmp/ollama.log 2>&1 &
    # Everything below is backgrounded so /health passes while the (multi-GB)
    # model downloads/imports. Two paths:
    #   A) Custom GGUF (e.g. the uncensored Gemma in models/) — set GEMMA_GGUF_URL
    #      (any https) or GEMMA_GGUF_S3_KEY (key in the same bucket as the ONNX
    #      weights). Imported via `ollama create` under tag OLLAMA_FLOOR_MODEL
    #      (default 'gemma-uncensored'). File cached on the volume → import once.
    #   B) Registry pull (default gemma2:2b) when no GGUF source is set.
    if [ -n "${GEMMA_GGUF_URL:-}" ] || [ -n "${GEMMA_GGUF_S3_KEY:-}" ]; then
      export OLLAMA_FLOOR_MODEL="${OLLAMA_FLOOR_MODEL:-gemma-uncensored}"
      echo "[entrypoint] ollama custom GGUF floor: $OLLAMA_FLOOR_MODEL" >&2
      GGUF_DIR="$OLLAMA_MODELS/gguf"; GGUF_PATH="$GGUF_DIR/floor.gguf"
      ( mkdir -p "$GGUF_DIR"
        if [ ! -s "$GGUF_PATH" ]; then
          if [ -n "${GEMMA_GGUF_S3_KEY:-}" ] && [ -n "${AWS_S3_BUCKET:-}" ]; then
            echo "[entrypoint] fetching GGUF from s3://$AWS_S3_BUCKET/$GEMMA_GGUF_S3_KEY" >&2
            aws s3 cp "s3://$AWS_S3_BUCKET/$GEMMA_GGUF_S3_KEY" "$GGUF_PATH" \
              ${AWS_S3_ENDPOINT:+--endpoint-url "$AWS_S3_ENDPOINT"} >> /tmp/ollama.log 2>&1
          else
            echo "[entrypoint] fetching GGUF from $GEMMA_GGUF_URL" >&2
            curl -fL "$GEMMA_GGUF_URL" -o "$GGUF_PATH" >> /tmp/ollama.log 2>&1
          fi
        fi
        # Gemma chat template so Ollama formats turns correctly.
        printf 'FROM %s\nTEMPLATE """<start_of_turn>user\n{{ .Prompt }}<end_of_turn>\n<start_of_turn>model\n"""\nPARAMETER stop "<end_of_turn>"\n' \
          "$GGUF_PATH" > "$GGUF_DIR/Modelfile"
        if [ -s "$GGUF_PATH" ]; then
          ollama create "$OLLAMA_FLOOR_MODEL" -f "$GGUF_DIR/Modelfile" >> /tmp/ollama.log 2>&1 \
            && echo "[entrypoint] ollama floor ready: $OLLAMA_FLOOR_MODEL" >&2 \
            || echo "[entrypoint] ollama create failed" >&2
        else
          echo "[entrypoint] GGUF download failed; floor unavailable" >&2
        fi
      ) &
    else
      export OLLAMA_FLOOR_MODEL="${OLLAMA_FLOOR_MODEL:-gemma2:2b}"
      echo "[entrypoint] ollama registry floor: $OLLAMA_FLOOR_MODEL" >&2
      ( sleep 3
        ollama pull "$OLLAMA_FLOOR_MODEL" >> /tmp/ollama.log 2>&1 \
          && echo "[entrypoint] ollama floor ready: $OLLAMA_FLOOR_MODEL" >&2 \
          || echo "[entrypoint] ollama pull failed: $OLLAMA_FLOOR_MODEL" >&2
      ) &
    fi
  fi
fi

echo "[entrypoint] starting physis-pro-mcp on :9876" >&2
./physis-pro serve > /tmp/physis-mcp.log 2>&1 &
MCP_PID=$!
echo "[entrypoint] mcp pid=$MCP_PID" >&2
echo "[entrypoint] starting physis-pro-web" >&2
exec ./physis-pro-web
