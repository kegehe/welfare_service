#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/welfare-service"
CONFIG="$SCRIPT_DIR/config.toml"
DATA_DIR="$SCRIPT_DIR/data"
FRONTEND_DIR="$SCRIPT_DIR/frontend"
STATIC_INDEX="$SCRIPT_DIR/static/index.html"

# ---- 颜色 ----
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

has_newer_file() {
    local reference="$1"
    shift

    [[ ! -e "$reference" ]] && return 0

    local found
    found=$(find "$@" -type f -newer "$reference" -print -quit 2>/dev/null || true)
    [[ -n "$found" ]]
}

frontend_needs_build() {
    [[ -d "$FRONTEND_DIR" ]] || return 1
    has_newer_file \
        "$STATIC_INDEX" \
        "$FRONTEND_DIR/src" \
        "$FRONTEND_DIR/package.json" \
        "$FRONTEND_DIR/package-lock.json" \
        "$FRONTEND_DIR/vite.config.ts"
}

binary_needs_build() {
    [[ ! -x "$BINARY" ]] && return 0
    has_newer_file \
        "$BINARY" \
        "$SCRIPT_DIR/src" \
        "$SCRIPT_DIR/Cargo.toml" \
        "$SCRIPT_DIR/Cargo.lock" \
        "$STATIC_INDEX"
}

# ---- 构建最新前端静态页面 ----
if frontend_needs_build; then
    if ! command_exists npm; then
        error "检测到前端代码有更新，但未找到 npm，无法构建管理页面"
        exit 1
    fi
    warn "前端页面不是最新，开始构建..."
    (cd "$FRONTEND_DIR" && npm run build)
    info "前端页面构建完成"
fi

# ---- 编译最新二进制 ----
if binary_needs_build; then
    warn "服务二进制不是最新，开始编译..."
    cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
    info "服务编译完成"
fi

# ---- 检查配置文件 ----
if [[ ! -f "$CONFIG" ]]; then
    error "配置文件不存在: $CONFIG"
    error "请先复制并编辑 config.toml"
    exit 1
fi

# ---- 检查必填配置 ----
check_config_field() {
    local field="$1" label="$2"
    local value
    value=$(grep -E "^${field}[[:space:]]*=" "$CONFIG" | head -1 | sed 's/^[^=]*=[[:space:]]*"\{0,1\}\([^"]*\)"\{0,1\}[[:space:]]*$/\1/' | xargs)
    if [[ -z "$value" ]]; then
        error "$label 未配置，请编辑 config.toml 中的 $field"
        exit 1
    fi
}

check_config_field "master_key"   "加密主密钥 (encryption.master_key)"

# ---- 确保数据目录存在 ----
mkdir -p "$DATA_DIR"

# ---- 清理旧进程 ----
# 用 -x 精确匹配可执行文件名，避免匹配到 start.sh 自身或 pgrep 进程
OLD_PIDS=$(pgrep -x "welfare-service" 2>/dev/null || true)
if [[ -n "$OLD_PIDS" ]]; then
    warn "发现旧进程，正在停止..."
    for pid in $OLD_PIDS; do
        kill "$pid" 2>/dev/null || true
    done
    # 等待进程退出
    sleep 2
    # 检查是否还在运行，强制终止
    for pid in $OLD_PIDS; do
        if kill -0 "$pid" 2>/dev/null; then
            warn "进程 $pid 未响应，强制终止..."
            kill -9 "$pid" 2>/dev/null || true
        fi
    done
    sleep 0.5
    info "旧进程已停止"
fi

# ---- 启动 ----
info "启动 Welfare Service..."
info "配置文件: $CONFIG"
info "数据目录: $DATA_DIR"
echo ""

exec "$BINARY" -c "$CONFIG" serve
