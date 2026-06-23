#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/welfare-service"
CONFIG="$SCRIPT_DIR/config.toml"
DATA_DIR="$SCRIPT_DIR/data"
FRONTEND_DIR="$SCRIPT_DIR/frontend"
STATIC_INDEX="$SCRIPT_DIR/static/index.html"

# ---- 模式解析 ----
MODE="prod"
case "${1:-}" in
    --dev)  MODE="dev"  ;;
    --prod) MODE="prod" ;;
    "")     MODE="prod" ;;
    *)      echo "用法: $0 [--dev|--prod]"; exit 1 ;;
esac

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

# ---- 清理旧进程（按精确进程名 + 工作目录匹配） ----
cleanup_old() {
    # 匹配 release 二进制
    local old_pids
    old_pids=$(pgrep -x "welfare-service" 2>/dev/null || true)
    # 也匹配 cargo run 产生的进程（进程名可能带有路径前缀）
    local cargo_pids
    cargo_pids=$(pgrep -f "welfare-service.*serve" 2>/dev/null || true)
    # 合并去重
    local all_pids
    all_pids=$(echo "$old_pids $cargo_pids" | tr ' ' '\n' | sort -u | grep -v '^$' || true)

    if [[ -n "$all_pids" ]]; then
        warn "发现旧进程，正在停止..."
        for pid in $all_pids; do
            # 跳过自身
            [[ "$pid" -eq "$$" ]] && continue
            kill "$pid" 2>/dev/null || true
        done
        sleep 2
        for pid in $all_pids; do
            [[ "$pid" -eq "$$" ]] && continue
            if kill -0 "$pid" 2>/dev/null; then
                warn "进程 $pid 未响应，强制终止..."
                kill -9 "$pid" 2>/dev/null || true
            fi
        done
        sleep 0.5
        info "旧进程已停止"
    fi
}

# ---- 检查配置文件 ----
check_config() {
    if [[ ! -f "$CONFIG" ]]; then
        error "配置文件不存在: $CONFIG"
        error "请先复制并编辑 config.toml"
        exit 1
    fi

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
}

# ============================================================
#  开发模式：Vite HMR + cargo run
# ============================================================
run_dev() {
    info "启动开发模式..."

    # 检查依赖
    if ! command_exists npm; then
        error "开发模式需要 npm"
        exit 1
    fi

    # 检查配置（与生产模式一致，避免后端启动失败）
    check_config

    # 确保前端依赖已安装
    if [[ ! -d "$FRONTEND_DIR/node_modules" ]]; then
        info "安装前端依赖..."
        (cd "$FRONTEND_DIR" && npm install)
    fi

    # 清理旧进程
    cleanup_old

    # 确保数据目录存在
    mkdir -p "$DATA_DIR"

    # 后台启动后端
    info "启动后端 (cargo run)..."
    cargo run --manifest-path "$SCRIPT_DIR/Cargo.toml" -- -c "$CONFIG" serve &
    BACKEND_PID=$!

    # 后台启动 Vite dev server（用完整路径确保可靠）
    info "启动前端 (Vite HMR)..."
    (cd "$FRONTEND_DIR" && npx vite --clearScreen false) &
    FRONTEND_PID=$!

    # 进程清理函数
    dev_cleanup() {
        echo ""
        info "正在停止开发服务..."
        kill "$FRONTEND_PID" 2>/dev/null || true
        kill "$BACKEND_PID" 2>/dev/null || true
        # 也清理 cargo 可能产生的子进程
        pkill -P "$BACKEND_PID" 2>/dev/null || true
        wait "$FRONTEND_PID" 2>/dev/null || true
        wait "$BACKEND_PID" 2>/dev/null || true
        info "开发服务已停止"
        exit 0
    }

    trap dev_cleanup SIGINT SIGTERM

    echo ""
    info "=========================================="
    info "  开发模式已启动"
    info "  前端: ${CYAN}http://localhost:5173${NC} (Vite HMR)"
    info "  后端: ${CYAN}http://localhost:8080${NC} (API)"
    info "  按 Ctrl+C 停止所有服务"
    info "=========================================="
    echo ""

    # 轮询等待，任一子进程退出则清理
    while kill -0 "$BACKEND_PID" 2>/dev/null && kill -0 "$FRONTEND_PID" 2>/dev/null; do
        sleep 1
    done

    # 检查哪个进程退出了
    if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
        error "后端进程已退出，请检查上方日志"
    fi
    if ! kill -0 "$FRONTEND_PID" 2>/dev/null; then
        error "前端进程已退出，请检查上方日志"
    fi

    dev_cleanup
}

# ============================================================
#  生产模式：构建 + release 启动（原有逻辑）
# ============================================================
run_prod() {
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

    # ---- 检查配置 ----
    check_config

    # ---- 确保数据目录存在 ----
    mkdir -p "$DATA_DIR"

    # ---- 清理旧进程 ----
    cleanup_old

    # ---- 启动 ----
    info "启动 Welfare Service..."
    info "配置文件: $CONFIG"
    info "数据目录: $DATA_DIR"
    echo ""

    exec "$BINARY" -c "$CONFIG" serve
}

# ---- 主入口 ----
case "$MODE" in
    dev)  run_dev  ;;
    prod) run_prod ;;
esac
