#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

PIDS=$(pgrep -x "welfare-service" 2>/dev/null || true)

if [[ -z "$PIDS" ]]; then
    echo -e "${GREEN}[INFO]${NC}  服务未运行"
    exit 0
fi

echo -e "${GREEN}[INFO]${NC}  停止 Welfare Service (PID: $PIDS)..."
for pid in $PIDS; do
    kill "$pid" 2>/dev/null || true
done

# 等待进程退出（最多 10 秒）
for i in $(seq 1 20); do
    REMAINING=""
    for pid in $PIDS; do
        if kill -0 "$pid" 2>/dev/null; then
            REMAINING="$pid"
        fi
    done
    if [[ -z "$REMAINING" ]]; then
        echo -e "${GREEN}[INFO]${NC}  已停止"
        exit 0
    fi
    sleep 0.5
done

echo -e "${RED}[WARN]${NC}  进程未响应，强制终止..."
for pid in $PIDS; do
    kill -9 "$pid" 2>/dev/null || true
done
echo -e "${GREEN}[INFO]${NC}  已强制停止"
