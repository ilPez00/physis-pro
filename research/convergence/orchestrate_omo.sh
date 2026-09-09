#!/bin/sh
# Sequential omo queue. omo has 8GB VRAM; two q4 weights would evict each other
# on every call, so never run two of these at once.
cd /home/gio/dev/physis-pro/research/convergence || exit 1
OMO=http://100.116.39.57:11434

# 1. let the in-flight qwen2.5:7b grid finish
while pgrep -f 'supervise_e19_remote' >/dev/null 2>&1; do sleep 20; done

# 2. ornith-1.5-9b: a fresh python, so this grid includes the B5c arm
E19_MODEL=ornith-1.5-9b-q4_k_m:latest E19_HOST=$OMO E19_SEEDS=5 \
  ./supervise_e19_remote.sh > e19_run_ornith_9b.log 2>&1

# 3. back to qwen2.5:7b to pick up B5c, added after its grid started
E19_MODEL=qwen2.5:7b-instruct-q4_K_M E19_HOST=$OMO E19_SEEDS=5 \
  ./supervise_e19_remote.sh >> e19_run_qwen25_7b.log 2>&1

echo "ORCHESTRATOR: omo queue complete"
