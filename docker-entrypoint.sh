#!/bin/sh
# Physis web entrypoint
set -e

# Ensure the persistent data directory exists
mkdir -p "$PHYSIS_DATA_DIR"

# Use the pre-bundled models, or default to the local models folder in the app dir
export MODEL_DIR="${MODEL_DIR:-/app/models/jina-clip-v2}"

echo "[entrypoint] using models from $MODEL_DIR"

exec ./physis-pro-web
