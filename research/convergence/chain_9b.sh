#!/bin/sh
# Wait for the qwen2.5:7b supervisor to exit, then run the 9B grid.
# Sequential, not concurrent: omo has 8GB VRAM and the two q4 weights (4.7GB +
# 5.8GB) would evict each other on every call.
cd /home/gio/dev/physis-pro/research/convergence || exit 1
while pgrep -f 'E19_MODEL=qwen2.5' >/dev/null 2>&1 || pgrep -f 'supervise_e19_remote.sh' >/dev/null 2>&1; do
  sleep 30
done
E19_MODEL=ornith-1.5-9b-q4_k_m:latest E19_HOST=http://100.116.39.57:11434 E19_SEEDS=5 \
  ./supervise_e19_remote.sh > e19_run_ornith_9b.log 2>&1
