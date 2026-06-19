# 实时活跃密钥展示模块设计

## 概述

系统多密钥池自动调度分发请求，页面实时展示当前正在使用的 key、所属号池、请求模型。采用 SSE 主动推送机制，每次调度切换密钥后立即同步状态至前端，无延迟，无需轮询。

## 需求

- 展示每个请求正在使用哪个 key（非汇总统计，而是实时路由信息）
- 展示所有并发活跃 key（多个并发请求同时显示）
- 展示内容：key 标识 + 号池（platform）+ 请求模型
- 只展示当前瞬态，不保留历史
- 空闲时显示"当前无活跃请求"
- 推送机制：SSE（Server-Sent Events）

## 方案选型

**方案 A：SSE + 内存活跃表（已选定）**

在 `AppState` 中维护 `ActiveKeysMap`（DashMap），请求开始时写入，结束时移除。新增 SSE endpoint 推送变更事件。前端用 `EventSource` 接收。

选择理由：单实例部署（SQLite + 嵌入式前端），不需要跨进程广播；DashMap 提供快照查询，SSE 重连时补查即可；实现最简单，改动最少。

## 设计

### 1. 后端 — 活跃密钥状态管理

**新增类型**（`src/db/models.rs`）：

```rust
#[derive(Serialize, Clone)]
pub struct ActiveKeyEntry {
    pub request_id: u64,
    pub key_id: i64,
    pub key_name: String,
    pub key_prefix: String,
    pub platform: String,
    pub model: String,
    pub started_at: i64,
}
```

**新增 `ActiveKeysNotifier`**（`src/state.rs`）：

```rust
pub type ActiveKeysMap = DashMap<u64, ActiveKeyEntry>;  // key: request_id

pub struct ActiveKeysNotifier {
    pub active_keys: Arc<ActiveKeysMap>,
    pub version: watch::Sender<u64>,
    pub version_rx: watch::Receiver<u64>,
}
```

- `activate(request_id, entry)`：插入 `active_keys`，递增 version 并 send
- `deactivate(request_id)`：移除 `active_keys`，递增 version 并 send
- `snapshot()`：返回当前所有活跃条目的 Vec

**集成到 `AppState`**：新增 `pub active_keys_notifier: Arc<ActiveKeysNotifier>` 字段。

**请求 ID 生成**：使用 `AtomicU64` 全局递增计数器，在 orchestrator 中生成。

### 2. 后端 — SSE 推送端点

**新增端点 `GET /admin/keys/active-stream`**

SSE 事件格式：

| 事件类型 | 触发时机 | data 格式 |
|---------|---------|----------|
| `snapshot` | 连接建立/重连 | `ActiveKeyEntry[]`（完整快照） |
| `activate` | 新请求开始使用 key | `ActiveKeyEntry`（单条） |
| `deactivate` | 请求完成，key 释放 | `{"request_id": 12345}` |

实现逻辑：

1. 连接建立时，立即推送 `snapshot` 事件（从 `ActiveKeysMap` 读取）
2. 之后监听 `version_rx.changed()`，每次变更时对比上次快照，推送 `activate`/`deactivate` 差异事件
3. 使用 `axum::response::Sse` + `tokio_stream` 生成 SSE 流
4. 设置 `KeepAlive` 防止连接超时

### 3. 后端 — Orchestrator 集成

在 `src/proxy/orchestrator.rs` 的 `handle_proxy_request` 中插入追踪逻辑：

**插入点**：

- 在进入候选遍历循环前，生成唯一 `request_id`
- 每次尝试候选 key、调用 `forwarder::forward_request` 前：`activate(request_id, entry)`
- `forwarder::forward_request` 返回后（无论成功失败）：`deactivate(request_id)`

关键约束：当一个 key 失败后尝试下一个候选 key 时，先 deactivate 旧的，再 activate 新的。每次迭代是完整的 activate → forward → deactivate 周期。

### 4. 前端 — ActiveKeysBar 组件

**新增组件 `ActiveKeysBar.vue`**，放置位置：StatsOverview 下方、PoolKeysTable 上方。

**布局**：

- 横向条状区域，背景色 `--ws-channel`
- 左侧标题："活跃密钥" + 实时脉冲指示灯（绿色小圆点，有请求时闪烁）
- 右侧内容区：
  - **有活跃请求**：横向排列活跃 key 卡片，每个卡片显示：
    - 号池标签（带颜色圆点，复用现有 platform 颜色逻辑）
    - key 名称或前缀
    - 模型名称（小字）
    - 持续时间（从 started_at 计算，每秒更新）
  - **空闲**：显示"当前无活跃请求"灰色文字

**SSE 连接管理**：

- `onMounted` 时创建 `EventSource('/admin/keys/active-stream')`
- 收到 `snapshot`：替换整个活跃列表
- 收到 `activate`：添加到活跃列表
- 收到 `deactivate`：按 request_id 移除
- `onUnmounted` 时关闭 `EventSource`
- `EventSource` 自带断线重连，重连后后端自动推送 `snapshot`

**持续时间更新**：`setInterval` 每秒更新持续时间显示，组件销毁时清除。

**样式**：复用 `variables.css` 设计系统，卡片用 `--ws-pool`（teal）左边框标识活跃状态。

### 5. 前后端对接

**前端新增类型**（`frontend/src/types/index.ts`）：

```typescript
export interface ActiveKeyEntry {
  request_id: number
  key_id: number
  key_name: string
  key_prefix: string
  platform: string
  model: string
  started_at: number
}
```

**路由注册**（`src/server/routes.rs`）：新增 `GET /admin/keys/active-stream` → `admin::active_keys_stream`

**前端不需要新增 REST API 函数**，SSE 连接在组件中用 `EventSource` 直接创建。

### 6. 数据流

```
请求进入 → orchestrator 选中 key
         → activate(request_id, entry)
         → ActiveKeysMap.insert + version.notify
         → SSE handler 检测到变更 → 推送 activate 事件
         → 前端 EventSource 收到 → 添加到活跃列表 → UI 更新

请求完成 → orchestrator 请求结束
         → deactivate(request_id)
         → ActiveKeysMap.remove + version.notify
         → SSE handler 检测到变更 → 推送 deactivate 事件
         → 前端 EventSource 收到 → 从活跃列表移除 → UI 更新

SSE 重连 → 后端推送 snapshot
         → 前端替换整个活跃列表
```

## 涉及文件

| 文件 | 变更类型 | 说明 |
|-----|---------|-----|
| `src/db/models.rs` | 修改 | 新增 `ActiveKeyEntry` |
| `src/state.rs` | 修改 | 新增 `ActiveKeysNotifier`，集成到 `AppState` |
| `src/proxy/orchestrator.rs` | 修改 | 插入 activate/deactivate 调用 |
| `src/server/handlers/admin.rs` | 修改 | 新增 `active_keys_stream` handler |
| `src/server/handlers/mod.rs` | 修改 | 导出新增 handler |
| `src/server/routes.rs` | 修改 | 注册 SSE 路由 |
| `src/main.rs` | 修改 | 初始化 `ActiveKeysNotifier` |
| `frontend/src/types/index.ts` | 修改 | 新增 `ActiveKeyEntry` 接口 |
| `frontend/src/components/ActiveKeysBar.vue` | 新增 | 活跃密钥展示组件 |
| `frontend/src/App.vue` | 修改 | 引入 ActiveKeysBar 组件 |
| `Cargo.toml` | 修改 | 新增 dashmap 依赖（如未引入） |
