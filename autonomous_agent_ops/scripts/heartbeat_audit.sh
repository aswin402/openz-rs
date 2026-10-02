#!/bin/bash
# Autonomous Agent Heartbeat Audit Script
# Checks system health, disk usage, OpenZ process, logs metrics

set -euo pipefail

LOG_FILE="${HOME}/.openz/heartbeat.log"
OPENZ_DIR="${HOME}/.openz"
TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')

# Initialize status
STATUS="OK"
ISSUES=()

# 1. System load (1-minute average)
load_1=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $1}' | tr -d ',')
load_5=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $2}' | tr -d ',')
num_cores=$(nproc 2>/dev/null || echo 4)
load_threshold=$(echo "scale=2; $num_cores * 2" | bc)

# Check if load is elevated
if command -v bc &>/dev/null; then
    if (( $(echo "$load_1 > $load_threshold" | bc -l) )); then
        STATUS="DEGRADED"
        ISSUES+=("load_high:${load_1}")
    fi
else
    # Fallback without bc
    load_int=${load_1%.*}
    threshold_int=$((num_cores * 2))
    if [[ ${load_int} -gt ${threshold_int} ]]; then
        STATUS="DEGRADED"
        ISSUES+=("load_high:${load_1}")
    fi
fi

# 2. Disk usage on ~/.openz
openz_disk_usage=$(df -k "$OPENZ_DIR" 2>/dev/null | awk 'NR==2 {print $5}' | tr -d '%' || echo "0")
if [[ "${openz_disk_usage}" -gt 90 ]]; then
    STATUS="CRITICAL"
    ISSUES+=("disk_critical:${openz_disk_usage}%")
elif [[ "${openz_disk_usage}" -gt 80 ]]; then
    if [[ "$STATUS" != "CRITICAL" ]]; then
        STATUS="DEGRADED"
    fi
    ISSUES+=("disk_warn:${openz_disk_usage}%")
fi

# 3. Check if openz process is running
openz_pid=$(pgrep -x openz 2>/dev/null || pgrep -f "openz" 2>/dev/null | head -1 || echo "")
if [[ -z "$openz_pid" ]]; then
    STATUS="CRITICAL"
    ISSUES+=("process_missing")
fi

# 4. Memory check
mem_available=$(free -m 2>/dev/null | awk 'NR==2 {print $7}' || echo "0")
mem_threshold=500
if [[ "${mem_available}" -lt "${mem_threshold}" ]]; then
    if [[ "$STATUS" != "CRITICAL" ]]; then
        STATUS="DEGRADED"
    fi
    ISSUES+=("low_mem:${mem_available}MB")
fi

# 5. OpenZ dir existence
if [[ ! -d "$OPENZ_DIR" ]]; then
    STATUS="CRITICAL"
    ISSUES+=("dir_missing")
fi

# Log timestamped metrics
echo "${TIMESTAMP} | status=${STATUS} | load=${load_1},${load_5} | disk=${openz_disk_usage}% | mem_avail=${mem_available}MB | openz_pid=${openz_pid:-none} | issues:${ISSUES[*]:-none}" >> "$LOG_FILE"

# Print result
if [[ "$STATUS" == "OK" ]]; then
    echo "OK"
else
    echo "STATUS: $STATUS | ${ISSUES[*]}"
fi
