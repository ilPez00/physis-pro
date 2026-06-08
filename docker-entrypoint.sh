#!/bin/sh
set -e

MODEL_DIR="${MODEL_DIR:-/app/models/bge-base-en-v1.5}"
MODEL_FILE="$MODEL_DIR/onnx/model.onnx"
TOKENIZER_FILE="$MODEL_DIR/tokenizer.json"

echo "[entrypoint] start MODEL_DIR=$MODEL_DIR"

if [ ! -f "$MODEL_FILE" ] || [ ! -f "$TOKENIZER_FILE" ]; then
  echo "[entrypoint] model files not found, downloading from S3..."
  mkdir -p "$MODEL_DIR/onnx"
  if aws s3 cp "s3://$AWS_S3_BUCKET/bge-base-en-v1.5/onnx/model.onnx" "$MODEL_FILE" --endpoint-url "$AWS_S3_ENDPOINT"; then
    aws s3 cp "s3://$AWS_S3_BUCKET/bge-base-en-v1.5/tokenizer.json" "$TOKENIZER_FILE" --endpoint-url "$AWS_S3_ENDPOINT"
    echo "[entrypoint] download complete"
  else
    echo "[entrypoint] download failed, will be DEGRADED"
  fi
else
  echo "[entrypoint] model files exist"
fi

ls -la "$MODEL_DIR/onnx/" 2>/dev/null || echo "[entrypoint] no model dir"

echo "[entrypoint] starting physis-pro-web"
exec ./physis-pro-web
