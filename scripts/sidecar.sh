#!/usr/bin/env bash
# Manage the physis sidecar (physis-pro-web) reliably.
#
# Cold starts stalled because the default embedder cascade loads ONNX models off
# slow storage. This launches with the fast random-projection embedder by default
# (comparable vectors — all the coherence/machine recall needs), fully detaches
# the process with setsid so it survives the parent shell, and blocks until the
# health endpoint is actually serving.
#
#   scripts/sidecar.sh start     # start + wait until READY (default)
#   scripts/sidecar.sh stop
#   scripts/sidecar.sh restart
#   scripts/sidecar.sh status
#
# Override the embedder for a real-semantics run:
#   PHYSIS_EMBEDDER=bge-base scripts/sidecar.sh start   # (slow cold start)
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"          # physis_pro root
BIN="$HERE/target/release/physis-pro-web"
PORT="${PHYSIS_PORT:-19876}"
URL="http://127.0.0.1:${PORT}"
PIDFILE="${PHYSIS_PIDFILE:-/tmp/physis-sidecar.${PORT}.pid}"
LOG="${PHYSIS_LOG:-/tmp/physis-sidecar.${PORT}.log}"
EMB="${PHYSIS_EMBEDDER:-random-projection}"       # fast default; set to onnx kind to override
READY_TIMEOUT="${PHYSIS_READY_TIMEOUT:-90}"

is_up() { [ "$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 "${URL}/health" 2>/dev/null)" = "200" ]; }

start() {
  if is_up; then echo "sidecar already up at ${URL}"; return 0; fi
  [ -x "$BIN" ] || { echo "binary missing: $BIN — build it: cargo build --release --bin physis-pro-web"; exit 1; }
  echo "starting sidecar (embedder=${EMB}, port=${PORT})..."
  PHYSIS_EMBEDDER="$EMB" PHYSIS_PORT="$PORT" RUST_LOG="${RUST_LOG:-warn}" \
    setsid "$BIN" >"$LOG" 2>&1 </dev/null &
  local pid=$!
  echo "$pid" > "$PIDFILE"
  for i in $(seq 1 "$READY_TIMEOUT"); do
    if is_up; then echo "READY at ${URL} (${i}s, pid ${pid}, embedder=${EMB})"; return 0; fi
    kill -0 "$pid" 2>/dev/null || { echo "FATAL: sidecar exited during startup. Log tail:"; tail -20 "$LOG"; exit 1; }
    sleep 1
  done
  echo "FATAL: not healthy after ${READY_TIMEOUT}s. Log tail:"; tail -20 "$LOG"; exit 1
}

stop() {
  if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    kill "$(cat "$PIDFILE")" 2>/dev/null || true
    sleep 1; kill -9 "$(cat "$PIDFILE")" 2>/dev/null || true
    rm -f "$PIDFILE"; echo "stopped."
  else
    pkill -9 -f "$BIN" 2>/dev/null && echo "stopped (by name)." || echo "not running."
  fi
}

status() {
  if is_up; then echo "up at ${URL} ($(curl -s "${URL}/api/v1/embedder" 2>/dev/null))"; else echo "down"; fi
}

case "${1:-start}" in
  start)   start ;;
  stop)    stop ;;
  restart) stop; start ;;
  status)  status ;;
  *) echo "usage: $0 {start|stop|restart|status}"; exit 2 ;;
esac
