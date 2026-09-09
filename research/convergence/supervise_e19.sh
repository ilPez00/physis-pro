#!/bin/sh
# run_e19.py is resumable and idempotent: it skips (arm,seed) pairs already in
# results_e19.json. The box is swap-thrashing and the OOM killer takes the
# python process, so restart it until it exits clean or we run out of attempts.
for i in $(seq 1 40); do
  if E19_SEEDS=10 python3 run_e19.py; then
    echo "SUPERVISOR: clean exit on attempt $i"; exit 0
  fi
  echo "SUPERVISOR: attempt $i died (exit $?), restarting in 30s"
  sleep 30
done
echo "SUPERVISOR: gave up after 40 attempts"; exit 1
