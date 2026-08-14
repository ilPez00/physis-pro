#!/bin/bash
# Physis Pro Resource Monitor/Throttler
# This script monitors the CPU usage of physis-web and throttles it via cgroups if usage exceeds 90%.

TARGET_PROCESS="physis-web"
CGROUP_NAME="physis_pro_throttle"
CPU_THRESHOLD=90

# Check if script is run as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root to manage cgroups."
  exit 1
fi

# Setup cgroup
if [ ! -d "/sys/fs/cgroup/cpu/$CGROUP_NAME" ]; then
    mkdir -p "/sys/fs/cgroup/cpu/$CGROUP_NAME"
fi

echo "Monitoring $TARGET_PROCESS for resource usage..."

while true; do
    PID=$(pgrep -f "$TARGET_PROCESS" | head -n 1)
    
    if [ -n "$PID" ]; then
        # Add PID to cgroup if not already added
        if ! grep -q "$PID" "/sys/fs/cgroup/cpu/$CGROUP_NAME/tasks" 2>/dev/null; then
            echo "$PID" > "/sys/fs/cgroup/cpu/$CGROUP_NAME/tasks"
        fi

        # Get CPU usage percentage for the process
        CPU_USAGE=$(top -bn1 -p "$PID" | grep "$TARGET_PROCESS" | awk '{print $9}' | cut -d'.' -f1)
        
        if [ -n "$CPU_USAGE" ]; then
            if [ "$CPU_USAGE" -gt "$CPU_THRESHOLD" ]; then
                echo "[$(date)] CPU usage high ($CPU_USAGE%). Throttling $TARGET_PROCESS..."
                # 50% CPU limit
                echo 100000 > "/sys/fs/cgroup/cpu/$CGROUP_NAME/cpu.cfs_period_us"
                echo 50000 > "/sys/fs/cgroup/cpu/$CGROUP_NAME/cpu.cfs_quota_us"
            else
                # Remove limit (set quota to -1)
                echo -1 > "/sys/fs/cgroup/cpu/$CGROUP_NAME/cpu.cfs_quota_us"
            fi
        fi
    fi
    sleep 5
done
