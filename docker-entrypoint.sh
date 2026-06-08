#!/bin/sh
set -e

# stderr to bypass pipe buffering (Railway captures stderr)
echo "[entrypoint] starting" >&2
echo "[entrypoint] model files:" >&2
ls -la /app/models/bge-base-en-v1.5/onnx/ 2>/dev/null || echo "[entrypoint] no model dir at /app/models/bge-base-en-v1.5/onnx/" >&2
echo "[entrypoint] starting physis-pro-mcp on :9876" >&2
./physis-pro serve > /tmp/physis-mcp.log 2>&1 &
MCP_PID=$!
echo "[entrypoint] mcp pid=$MCP_PID" >&2
echo "[entrypoint] starting physis-pro-web" >&2
exec ./physis-pro-web
