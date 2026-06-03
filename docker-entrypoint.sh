#!/bin/sh
# Physis web entrypoint
set -e

# Ensure the persistent data directory exists
mkdir -p "$PHYSIS_DATA_DIR"

# The binary loads Jina CLIP v2 from ./models/jina-clip-v2 (relative to CWD).
# Weights are ~820MB and CANNOT ship in git (models/ is git-ignored), so on a
# fresh Railway build the image has no weights and the engine would silently
# fall back to the random-projection embedder (see PREMORTEM.md P0).
#
# Fix: fetch the weights at boot from MODEL_BASE_URL when missing. Point
# MODEL_BASE_URL at a bucket holding the jina-clip-v2 layout:
#   <MODEL_BASE_URL>/onnx/model_q4f16.onnx
#   <MODEL_BASE_URL>/tokenizer.json
MODEL_DIR="${MODEL_DIR:-/app/models/jina-clip-v2}"
MODEL_FILE="$MODEL_DIR/onnx/model_q4f16.onnx"
TOKENIZER_FILE="$MODEL_DIR/tokenizer.json"

if [ ! -f "$MODEL_FILE" ] || [ ! -f "$TOKENIZER_FILE" ]; then
    if [ -n "$MODEL_BASE_URL" ]; then
        echo "[entrypoint] weights missing — downloading from $MODEL_BASE_URL"
        mkdir -p "$MODEL_DIR/onnx"
        curl -fsSL "$MODEL_BASE_URL/onnx/model_q4f16.onnx" -o "$MODEL_FILE"
        curl -fsSL "$MODEL_BASE_URL/tokenizer.json" -o "$TOKENIZER_FILE"
        echo "[entrypoint] download complete"
    else
        echo "[entrypoint] WARNING: no weights at $MODEL_DIR and MODEL_BASE_URL unset."
        echo "[entrypoint] WARNING: server will start in DEGRADED random-projection mode."
        echo "[entrypoint] WARNING: classification output will be non-semantic noise."
    fi
else
    echo "[entrypoint] using bundled weights at $MODEL_DIR"
fi

exec ./physis-pro-web
