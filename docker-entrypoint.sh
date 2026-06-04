#!/bin/sh
# Physis web entrypoint
set -e

# Ensure the persistent data directory exists
mkdir -p "$PHYSIS_DATA_DIR"

# The engine's live embedder is BGE-base-en-v1.5 (768-d), loaded from
# ./models/bge-base-en-v1.5 (Jina is skipped — its ONNX won't run under the
# pinned ort; the self-test rejects it). Weights are ~435MB and CANNOT ship in
# git (models/ is git-ignored), so on a fresh Railway build the image has no
# weights and the engine would fall back to random projection (PREMORTEM.md P0).
#
# Fix: fetch the weights at boot from MODEL_BASE_URL when missing. Point
# MODEL_BASE_URL at a bucket holding the bge layout:
#   <MODEL_BASE_URL>/onnx/model.onnx
#   <MODEL_BASE_URL>/tokenizer.json
MODEL_DIR="${MODEL_DIR:-/app/models/bge-base-en-v1.5}"
MODEL_FILE="$MODEL_DIR/onnx/model.onnx"
TOKENIZER_FILE="$MODEL_DIR/tokenizer.json"

if [ ! -f "$MODEL_FILE" ] || [ ! -f "$TOKENIZER_FILE" ]; then
    if [ -n "$MODEL_BASE_URL" ]; then
        echo "[entrypoint] weights missing — downloading from $MODEL_BASE_URL"
        mkdir -p "$MODEL_DIR/onnx"
        curl -fsSL "$MODEL_BASE_URL/onnx/model.onnx" -o "$MODEL_FILE"
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
