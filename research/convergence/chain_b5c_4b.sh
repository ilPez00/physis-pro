#!/bin/sh
cd /home/gio/dev/physis-pro/research/convergence || exit 1
# wait for the local 4B supervisor to exit, then re-run: a fresh python picks up
# the B5c arm added to run_e19.py after the current process started.
while pgrep -f 'supervise_e19.sh' >/dev/null 2>&1; do sleep 20; done
E19_SEEDS=5 ./supervise_e19.sh >> e19_run.log 2>&1
