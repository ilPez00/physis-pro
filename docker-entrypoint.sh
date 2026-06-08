#!/bin/sh
# Physis web entrypoint
set -e

# Ensure the persistent data directory exists
mkdir -p "$PHYSIS_DATA_DIR"

MODEL_DIR="${MODEL_DIR:-/app/models/bge-base-en-v1.5}"
MODEL_FILE="$MODEL_DIR/onnx/model.onnx"
TOKENIZER_FILE="$MODEL_DIR/tokenizer.json"

>&2 echo "[entrypoint] starting — MODEL_DIR=$MODEL_DIR"
>&2 echo "[entrypoint] AWS_S3_BUCKET=${AWS_S3_BUCKET:-<unset>} AWS_S3_ENDPOINT=${AWS_S3_ENDPOINT:-<unset>}"

download_s3() {
  >&2 echo "[entrypoint] downloading weights from S3 ($AWS_S3_BUCKET)"
  mkdir -p "$MODEL_DIR/onnx"
  aws_s3_rc=0
  aws s3 cp "s3://$AWS_S3_BUCKET/bge-base-en-v1.5/onnx/model.onnx" "$MODEL_FILE" \
    --endpoint-url "$AWS_S3_ENDPOINT" || aws_s3_rc=$?
  aws_s3_rc2=0
  aws s3 cp "s3://$AWS_S3_BUCKET/bge-base-en-v1.5/tokenizer.json" "$TOKENIZER_FILE" \
    --endpoint-url "$AWS_S3_ENDPOINT" || aws_s3_rc2=$?
  if [ $aws_s3_rc -ne 0 ] || [ $aws_s3_rc2 -ne 0 ]; then
    >&2 echo "[entrypoint] S3 download FAILED (rc=$aws_s3_rc $aws_s3_rc2)"
    return 1
  fi
  >&2 echo "[entrypoint] S3 download complete"
}

download_http() {
  >&2 echo "[entrypoint] downloading weights from $MODEL_BASE_URL"
  mkdir -p "$MODEL_DIR/onnx"
  curl -fsSL "$MODEL_BASE_URL/onnx/model.onnx" -o "$MODEL_FILE" 2>&1
  curl -fsSL "$MODEL_BASE_URL/tokenizer.json" -o "$TOKENIZER_FILE" 2>&1
  >&2 echo "[entrypoint] HTTP download complete"
}

if [ ! -f "$MODEL_FILE" ] || [ ! -f "$TOKENIZER_FILE" ]; then
  >&2 echo "[entrypoint] model files NOT found, attempting download"
  if [ -n "$AWS_S3_BUCKET" ] && [ -n "$AWS_S3_ENDPOINT" ]; then
    download_s3
  elif [ -n "$MODEL_BASE_URL" ]; then
    download_http
  else
    >&2 echo "[entrypoint] WARNING: no weights at $MODEL_DIR"
    >&2 echo "[entrypoint] WARNING: set MODEL_BASE_URL (HTTP) or AWS_S3_BUCKET (S3)."
    >&2 echo "[entrypoint] WARNING: server will start in DEGRADED random-projection mode."
  fi
else
  >&2 echo "[entrypoint] using bundled weights at $MODEL_DIR"
fi

>&2 echo "[entrypoint] checking model: test -f $MODEL_FILE"
if [ -f "$MODEL_FILE" ]; then
  >&2 echo "[entrypoint] MODEL_FILE FOUND ($(stat -c%s "$MODEL_FILE") bytes)"
else
  >&2 echo "[entrypoint] MODEL_FILE NOT FOUND — will be DEGRADED"
fi

exec ./physis-pro-web