#!/usr/bin/env bash
# Autonomous ROCm enablement + physis GPU test.
# Waits for the air-clone apt lock to clear, installs ROCm userspace,
# builds the rocm physis binary, boots it, and verifies the GPU EP is used.
set -uo pipefail
export PATH="/home/gio/.cargo/bin:$PATH"
export HOME=/home/gio
PROJ=/media/gio/c459b156-2cc1-4b0b-a695-3db1923219bf/dev/physis_pro
LOG=/tmp/rocm_physis.log
exec >"$LOG" 2>&1

echo "[$(date)] waiting for dpkg lock to clear (air-clone apt)..."
for i in $(seq 1 600); do
  if ! pgrep -x apt-get >/dev/null && ! pgrep -x dpkg >/dev/null; then
    echo "[$(date)] apt clear after $((i*5))s"; break
  fi
  sleep 5
done
pgrep -x apt-get >/dev/null && { echo "apt STILL locked; aborting"; exit 1; }

echo "[$(date)] installing ROCm userspace (rocminfo hipcc libamdhip64-dev)..."
sudo apt-get install -y -qq rocminfo hipcc libamdhip64-dev librocm-smi64-1
echo "rocm-install-exit=$?"

echo "[$(date)] rocminfo GPU detect:"
rocminfo 2>&1 | grep -iE "Agent |Name:|gfx|Device Type" | head -20

echo "[$(date)] building physis-pro-web-rocm (embed-onnx,rocm)..."
cd "$PROJ"
cargo build --bin physis-pro-web-rocm --features embed-onnx,rocm 2>&1 | tail -25
echo "cargo-build-exit=${PIPESTATUS[0]}"

# stop existing CPU web server on 19876 to free the port for the GPU build
curl -sS -m 3 http://127.0.0.1:19876/api/v1/embedder >/dev/null 2>&1 && {
  echo "[$(date)] stopping CPU web server (pid 340019) to free :19876"
  kill 340019 2>/dev/null || true
  sleep 2
}

echo "[$(date)] booting ROCm web server on :19876..."
PHYSIS_PORT=19876 nohup ./target/debug/physis-pro-web-rocm >/tmp/physis_web_rocm.log 2>&1 &
ROCM_PID=$!
sleep 8

echo "[$(date)] embedder health (expect kind=bge-base, should now run on ROCm GPU):"
curl -sS -m 10 http://127.0.0.1:19876/api/v1/embedder | head -c 300
echo
echo "[$(date)] classify smoke:"
curl -sS -m 15 -X POST http://127.0.0.1:19876/api/v1/classify -H "Content-Type: application/json" -d '{"text":"optimize the industrial pump maintenance schedule to reduce downtime"}' | head -c 300
echo
echo "[$(date)] ROCm web server log tail:"
tail -8 /tmp/physis_web_rocm.log
echo "[$(date)] DONE rocm_pid=$ROCM_PID"
