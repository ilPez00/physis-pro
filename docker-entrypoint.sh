#!/bin/sh
# Physis web entrypoint — ensures a real embedding model is present before boot.
# Without it, physis silently falls back to RandomProjection (semantic results = noise).
#
# Set MODEL_BASE_URL to override the source (e.g. a Supabase Storage / S3 bucket).
# Defaults to the public Jina CLIP v2 weights on Hugging Face.
set -e

MODEL_BASE_URL="${MODEL_BASE_URL:-https://huggingface.co/jinaai/jina-clip-v2/resolve/main}"
MODEL_DIR="models/jina-clip-v2"
MODEL_FILE="$MODEL_DIR/onnx/model_q4f16.onnx"
TOK_FILE="$MODEL_DIR/tokenizer.json"

if [ ! -f "$MODEL_FILE" ]; then
    echo "[entrypoint] embedding model missing — downloading from $MODEL_BASE_URL ..."
    mkdir -p "$MODEL_DIR/onnx"
    curl -fSL "$MODEL_BASE_URL/onnx/model_q4f16.onnx" -o "$MODEL_FILE"
    curl -fSL "$MODEL_BASE_URL/tokenizer.json" -o "$TOK_FILE"
    echo "[entrypoint] model ready ($(du -h "$MODEL_FILE" | cut -f1))."
else
    echo "[entrypoint] embedding model present."
fi

exec ./physis-pro-web
