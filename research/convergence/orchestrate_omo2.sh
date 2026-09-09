#!/bin/sh
# Step 4 of the omo queue: qwen3:8b, the clean scale point.
# qwen2.5:7b and ornith-9b are different families/finetunes from feather's
# qwen3:4b, so neither isolates SCALE. qwen3:8b is the same family and
# generation at 2x the parameters, which is the comparison E19's limit 1 asks
# for. Runs last so it never shares omo's 8GB VRAM with another grid.
cd /home/gio/dev/physis-pro/research/convergence || exit 1
while pgrep -f 'orchestrate_omo.sh' >/dev/null 2>&1; do sleep 20; done
while pgrep -f 'supervise_e19_remote' >/dev/null 2>&1; do sleep 20; done
E19_MODEL=qwen3:8b E19_HOST=http://100.116.39.57:11434 E19_SEEDS=5 \
  ./supervise_e19_remote.sh > e19_run_qwen3_8b.log 2>&1
echo "ORCHESTRATOR2: qwen3:8b complete"
