#!/usr/bin/env bash
# install-physis.sh — one-shot on-prem deploy for Physis (Ontology Studio server).
#
# What it does (idempotent, no root required — uses systemd --user):
#   1. Builds the physis web server (release, with the `web` + ontology features).
#   2. Downloads ONNX embedding models if missing.
#   3. Writes a systemd --user unit that binds the server on the LAN
#      (0.0.0.0) behind a generated role-aware bearer token table.
#   4. (Optional) if `caddy` is installed, writes a Caddyfile that terminates
#      TLS on a local mDNS name (e.g. physis.local) — LAN-only, no public DNS.
#
# The server refuses to bind non-loopback without tokens (PREMORTEM P6), so the
# install always provisions at least one editor + one viewer token and prints them.
#
# Usage:
#   ./install-physis.sh [--port 19876] [--host 0.0.0.0]
#
set -euo pipefail

PORT="${PORT:-19876}"
HOST="${HOST:-0.0.0.0}"
CARGO="${CARGO:-cargo}"
FEATURES="${FEATURES:-web,pdf,email}"
MODELS_DIR="${MODELS_DIR:-$(pwd)/models}"

echo "== Physis on-prem installer =="

# 0. Ensure ONNX embedding models are present
echo "[0/4] Checking ONNX embedding models..."
if [ -f "$MODELS_DIR/bge-base-en-v1.5/onnx/model.onnx" ] && [ -f "$MODELS_DIR/bge-base-en-v1.5/tokenizer.json" ]; then
  echo "  ✅ Models present at $MODELS_DIR/bge-base-en-v1.5/"
else
  echo "  ⬇️  Downloading BGE-base-en-v1.5 ONNX model..."
  mkdir -p "$MODELS_DIR/bge-base-en-v1.5/onnx"
  HF_URL="https://huggingface.co/BAAI/bge-base-en-v1.5/resolve/main/onnx/model.onnx"
  TOK_URL="https://huggingface.co/BAAI/bge-base-en-v1.5/resolve/main/tokenizer.json"
  curl -fL --retry 5 --retry-delay 5 "$HF_URL" -o "$MODELS_DIR/bge-base-en-v1.5/onnx/model.onnx" || {
    echo "  ⚠️  HF download failed; set MODEL_BASE_URL or download manually"
  }
  curl -fL --retry 5 --retry-delay 5 "$TOK_URL" -o "$MODELS_DIR/bge-base-en-v1.5/tokenizer.json" || true
  if [ -f "$MODELS_DIR/bge-base-en-v1.5/onnx/model.onnx" ]; then
    echo "  ✅ Model downloaded"
  else
    echo "  ⚠️  Model not downloaded — semantic embedder will fall back to random projection"
  fi
fi

# 1. Build
echo "[1/4] Building physis web server (features: $FEATURES) ..."
"$CARGO" build --release --features "$FEATURES" --bin web_server
BIN="$(pwd)/target/release/web_server"
test -x "$BIN"

# 2. Tokens (role:secret). Generated, printed once. Store them now.
ADMIN_SECRET="$(openssl rand -hex 16)"
EDITOR_SECRET="$(openssl rand -hex 16)"
VIEWER_SECRET="$(openssl rand -hex 16)"
PHYSIS_TOKENS="admin:${ADMIN_SECRET} editor:${EDITOR_SECRET} viewer:${VIEWER_SECRET}"

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/physis"
mkdir -p "$DATA_DIR"

UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$UNIT_DIR"
UNIT="$UNIT_DIR/physis.service"

echo "[2/4] Writing systemd --user unit: $UNIT"
cat > "$UNIT" <<EOF
[Unit]
Description=Physis Ontology Studio server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$DATA_DIR
Environment=PHYSIS_HOST=$HOST
Environment=PHYSIS_PORT=$PORT
Environment=PHYSIS_TOKENS=$PHYSIS_TOKENS
Environment=PHYSIS_SCAN_ROOT=$DATA_DIR/corpus
ExecStart=$BIN
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
EOF

# 3. Caddy (optional, LAN TLS via mDNS — no public exposure)
if command -v caddy >/dev/null 2>&1; then
  CADDYFILE="$DATA_DIR/Caddyfile"
  echo "[3/4] caddy found — writing $CADDYFILE (LAN-only physis.local)"
  cat > "$CADDYFILE" <<EOF
# LAN-only reverse proxy. Resolve physis.local via mDNS (avahi) on the client.
physis.local {
    reverse_proxy 127.0.0.1:$PORT
    # Basic LAN hardening: bind to private interfaces only.
    bind 192.168.0.0/16 10.0.0.0/8 172.16.0.0/12
}
EOF
  echo "      Start with: caddy run --config $CADDYFILE"
else
  echo "[3/4] caddy not installed — skipping TLS reverse proxy (server still runs on $HOST:$PORT)."
fi

echo "[4/4] Enabling systemd --user service..."
systemctl --user daemon-reload
systemctl --user enable --now physis.service || {
  echo "systemd --user unavailable in this environment. Start manually:"
  echo "  PHYSIS_HOST=$HOST PHYSIS_PORT=$PORT PHYSIS_TOKENS='$PHYSIS_TOKENS' $BIN"
}

echo
echo "== Install complete =="
echo "Health:   curl -fsS http://$HOST:$PORT/health"
echo "Viewer:   PHYSIS_VIEWER=$VIEWER_SECRET"
echo "Editor:   PHYSIS_EDITOR=$EDITOR_SECRET"
echo "Admin:    PHYSIS_ADMIN=$ADMIN_SECRET"
echo
echo "Example authenticated call:"
echo "  curl -fsS -H 'Authorization: Bearer $VIEWER_SECRET' http://$HOST:$PORT/api/v1/ontology/list"
echo "  # viewer can GET, but POST /api/v1/goals -> 403; only editor/admin may write."
echo
echo "NOTE: tokens are shown ONCE. Persist them in a secret manager."
