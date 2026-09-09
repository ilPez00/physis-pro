#!/bin/sh
# Same contract as supervise_e19.sh, for a run driven against a remote ollama.
# run_e19.py is resumable: it skips (arm,seed) pairs already in its results file,
# and refuses to write rows for one model into another model's file.
: "${E19_MODEL:?set E19_MODEL}"
: "${E19_HOST:?set E19_HOST}"
for i in $(seq 1 40); do
  if E19_SEEDS="${E19_SEEDS:-5}" python3 run_e19.py; then
    echo "SUPERVISOR($E19_MODEL): clean exit on attempt $i"; exit 0
  fi
  echo "SUPERVISOR($E19_MODEL): attempt $i died, restarting in 30s"
  sleep 30
done
echo "SUPERVISOR($E19_MODEL): gave up"; exit 1
