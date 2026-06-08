#!/bin/sh
set -e

echo "[entrypoint] starting"
echo "[entrypoint] model files:"
ls -la /app/models/bge-base-en-v1.5/onnx/ 2>/dev/null || echo "[entrypoint] no model dir at /app/models/bge-base-en-v1.5/onnx/"
echo "[entrypoint] starting physis-pro-web"
exec ./physis-pro-web
