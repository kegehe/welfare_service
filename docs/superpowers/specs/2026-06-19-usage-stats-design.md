# 用量统计功能设计

> 日期: 2026-06-19
> 状态: 待审核

## 概述

为 Welfare Service 添加双维度（号池 Key + 访问 Key）的用量统计功能，追踪每个 Key 的 token 消耗、调用次数等数据，并在管理前端展示统计图表。

## 需求确认

- **统计范围**: 号池 Key 维度 + 访问 Key 维度（双维度）
- **Token 计量**: 从上游响应中提取 `usage` 字段（精确计量）
- **展示方式**: 前端页面（新增统计区域 + 趋势图表）
- **配额限制**: 仅统计展示，不做配额限制
- **时间粒度**: 小时级聚合（`usage_hourly` 表）
- **实现方案**: 方案 B — 内存缓存 + 批量写入
- **累计统计**: 访问 Key 需要每个 key 的历史总计 + 全部 key 的汇总总计，持久化到数据库
- **数据清理**: 号池 Key 的统计数据定期清理（7天），访问 Key 的统计数据长期保留（90天）

## 架构设计

### 数据流

```
用户请求 → verify_access_key → 返回 access_key_id
    → KeySelector 选 key → forward_request
    → 上游响应 → 提取 usage 字段
    → 写 request_logs (含 access_key_id, prompt_tokens, completion_tokens)
    → 更新 UsageCache 内存缓存
    → 返回响应给用户

后台定时任务 (每60秒):
    → UsageCache → UPSERT usage_hourly 表
    → 清空已刷盘的缓存条目

优雅关闭:
    → 触发最后一次刷盘
```

## 第一层：数据采集

### Token 提取策略

**非流式响应**: 上游返回完整 JSON，解析 `usage.prompt_tokens` 和 `usage.completion_tokens`。

**流式响应**: SSE 流中 `usage` 数据出现在最后一个 `data:` 事件中：
- OpenAI 格式: 最后的 `chat.completion.chunk` 包含 `usage`
- Claude 格式: `message_delta` 事件中的 `usage` 字段

在 SSE 流转发过程中拦截包含 `usage` 的帧，提取 token 数据后原样转发给客户端。

### request_logs 表扩展

新增 3 列（通过 ALTER TABLE 迁移）：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `access_key_id` | INTEGER | NULL | 发起请求的访问 Key ID |
| `prompt_tokens` | INTEGER | 0 | 输入 token 数 |
| `completion_tokens` | INTEGER | 0 | 输出 token 数 |

### access_keys 表扩展

新增 3 列用于持久化累计统计（通过 ALTER TABLE 迁移）：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `total_requests` | INTEGER | 0 | 累计请求次数 |
| `total_prompt_tokens` | INTEGER | 0 | 累计输入 token 数 |
| `total_completion_tokens` | INTEGER | 0 | 累计输出 token 数 |

这些字段在每次请求完成时由 UsageCache 刷盘时一并更新，保证访问 Key 的历史总计不会因 `usage_hourly` 数据清理而丢失。

### RequestLogInput 扩展

```rust
pub struct RequestLogInput<'a> {
    pub key_id: Option<i64>,
    pub access_key_id: Option<i64>,  // 新增
    pub model: &'a str,
    pub status_code: Option<i32>,
    pub latency_ms: Option<i64>,
    pub is_success: bool,
    pub affects_key_health: bool,
    pub error_msg: Option<&'a str>,
    pub prompt_tokens: i64,          // 新增
    pub completion_tokens: i64,      // 新增
}
```

## 第二层：内存缓存 + 批量写入

### UsageCache 结构

```rust
struct UsageCacheEntry {
    request_count: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
}

pub struct UsageCache {
    /// 号池 key 维度: (key_id, model, hour_bucket) -> UsageCacheEntry
    pool_usage: RwLock<HashMap<(i64, String, i64), UsageCacheEntry>>,
    /// 访问 key 维度: (access_key_id, model, hour_bucket) -> UsageCacheEntry
    access_usage: RwLock<HashMap<(i64, String, i64), UsageCacheEntry>>,
    /// 累计统计: key_id -> (total_requests, total_prompt_tokens, total_completion_tokens)
    pool_totals: RwLock<HashMap<i64, (u64, u64, u64)>>,
    /// 累计统计: access_key_id -> (total_requests, total_prompt_tokens, total_completion_tokens)
    access_totals: RwLock<HashMap<i64, (u64, u64, u64)>>,
}
```

### usage_hourly 聚合表

```sql
CREATE TABLE IF NOT EXISTS usage_hourly (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dimension TEXT NOT NULL,        -- 'pool' 或 'access'
    key_id INTEGER NOT NULL,        -- 号池 key ID 或访问 key ID
    model TEXT NOT NULL,
    hour_bucket INTEGER NOT NULL,   -- Unix 时间戳截断到小时
    request_count INTEGER DEFAULT 0,
    prompt_tokens INTEGER DEFAULT 0,
    completion_tokens INTEGER DEFAULT 0,
    created_at DATETIME,
    updated_at DATETIME,
    UNIQUE(dimension, key_id, model, hour_bucket)
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_usage_hourly_dimension_key ON usage_hourly(dimension, key_id);
CREATE INDEX IF NOT EXISTS idx_usage_hourly_hour ON usage_hourly(hour_bucket);
CREATE INDEX IF NOT EXISTS idx_usage_hourly_model ON usage_hourly(model);
```

### 刷盘机制

- 定时间隔: 60 秒
- 刷盘方式: 事务内批量 UPSERT
- 刷盘成功后清空已写入条目
- 使用 `parking_lot::RwLock` 避免恐慌级联
- **访问 Key 累计刷盘**: 同一事务内，将 `access_totals` 增量累加到 `access_keys` 表的 `total_requests`/`total_prompt_tokens`/`total_completion_tokens` 字段，保证访问 Key 的历史总计持久化

### 数据丢失防护

- 优雅关闭时（Ctrl+C 信号）触发最后一次刷盘
- 最多丢失 60 秒数据，对统计场景可接受
- 进程启动时从 `usage_hourly` 表加载当前小时数据到内存

### 数据清理

- `request_logs`: 沿用现有 7 天清理
- `usage_hourly` (dimension='pool'): 保留 7 天，号池 Key 变动频繁，历史统计价值低
- `usage_hourly` (dimension='access'): 保留 90 天，访问 Key 用量需要长期追踪

## 第三层：统计 API

### 路由

| 路由 | 方法 | 说明 |
|------|------|------|
| `/admin/stats/overview` | GET | 全局概览统计 |
| `/admin/stats/pool-keys` | GET | 号池 Key 用量列表 |
| `/admin/stats/pool-keys/{id}` | GET | 单个号池 Key 用量详情 |
| `/admin/stats/access-keys` | GET | 访问 Key 用量列表 |
| `/admin/stats/access-keys/{id}` | GET | 单个访问 Key 用量详情 |
| `/admin/stats/hourly` | GET | 小时级趋势数据 |

### 查询参数

- `hours` — 查询最近 N 小时数据（默认 24，最大 720 即 30 天）
- `dimension` — `pool` 或 `access`（仅 hourly 接口）
- `key_id` — 可选，筛选特定 Key

### 响应格式

**GET /admin/stats/overview**
```json
{
  "total_requests": 12345,
  "total_prompt_tokens": 5000000,
  "total_completion_tokens": 2000000,
  "total_tokens": 7000000,
  "active_pool_keys": 5,
  "active_access_keys": 10,
  "period": { "start": "2026-06-19T00:00:00", "end": "2026-06-19T23:59:59" }
}
```

**GET /admin/stats/pool-keys?hours=24**
```json
{
  "keys": [
    {
      "key_id": 1,
      "name": "MiMo-1",
      "platform": "mimo",
      "total_requests": 500,
      "total_prompt_tokens": 200000,
      "total_completion_tokens": 80000,
      "success_rate": 0.95,
      "avg_latency_ms": 1200,
      "last_used_at": "2026-06-19T10:30:00"
    }
  ]
}
```

**GET /admin/stats/pool-keys/{id}?hours=24**
```json
{
  "key_id": 1,
  "name": "MiMo-1",
  "platform": "mimo",
  "total_requests": 500,
  "total_prompt_tokens": 200000,
  "total_completion_tokens": 80000,
  "by_model": [
    { "model": "claude-sonnet-4-20250514", "requests": 300, "prompt_tokens": 120000, "completion_tokens": 50000 },
    { "model": "mimo-v2.5-pro", "requests": 200, "prompt_tokens": 80000, "completion_tokens": 30000 }
  ],
  "success_rate": 0.95,
  "avg_latency_ms": 1200
}
```

**GET /admin/stats/access-keys?hours=24**
```json
{
  "total": {
    "total_requests": 800,
    "total_prompt_tokens": 400000,
    "total_completion_tokens": 160000
  },
  "keys": [
    {
      "access_key_id": 1,
      "name": "user-A",
      "total_requests": 200,
      "total_prompt_tokens": 100000,
      "total_completion_tokens": 40000,
      "last_used_at": "2026-06-19T10:30:00"
    }
  ]
}
```

**GET /admin/stats/access-keys/{id}?hours=24**
```json
{
  "access_key_id": 1,
  "name": "user-A",
  "total_requests": 200,
  "total_prompt_tokens": 100000,
  "total_completion_tokens": 40000,
  "by_model": [
    { "model": "claude-sonnet-4-20250514", "requests": 150, "prompt_tokens": 70000, "completion_tokens": 28000 },
    { "model": "mimo-v2.5-pro", "requests": 50, "prompt_tokens": 30000, "completion_tokens": 12000 }
  ],
  "last_used_at": "2026-06-19T10:30:00"
}
```

**GET /admin/stats/hourly?dimension=pool&key_id=1&hours=24**
```json
{
  "data": [
    { "hour_bucket": 1718745600, "model": "claude-sonnet-4-20250514", "request_count": 50, "prompt_tokens": 20000, "completion_tokens": 8000 },
    { "hour_bucket": 1718749200, "model": "claude-sonnet-4-20250514", "request_count": 30, "prompt_tokens": 12000, "completion_tokens": 5000 }
  ]
}
```

## 第四层：前端页面

### 布局

在 StatsOverview 卡片和号池 Key 表格之间插入"用量统计"区域：

```
┌─────────────────────────────────────────────┐
│ StatsOverview (已有)                         │
├─────────────────────────────────────────────┤
│ 📊 用量统计                    [24h] [7d] [30d] │
│ ┌──────────────────┐ ┌──────────────────────┐│
│ │ 号池 Key 用量排行  │ │ 访问 Key 用量排行     ││
│ │ 名称│请求数│输入│输出│ │ 名称│请求数│输入│输出 ││
│ │                  │ │ ─────────────────── ││
│ │                  │ │ 合计│ 800│400k│160k ││
│ │                  │ │ ─────────────────── ││
│ │                  │ │ U-A │ 200│100k│ 40k ││
│ │                  │ │ U-B │ 150│ 80k│ 30k ││
│ └──────────────────┘ └──────────────────────┘│
│ ┌───────────────────────────────────────────┐│
│ │ 📈 用量趋势 (按小时)                        ││
│ │ [号池Key ▼] [模型 ▼]                       ││
│ │   堆叠柱状图: prompt_tokens + completion    ││
│ └───────────────────────────────────────────┘│
├─────────────────────────────────────────────┤
│ 号池 Key 表格 (已有)                          │
├─────────────────────────────────────────────┤
│ 访问 Key 表格 (已有)                          │
└─────────────────────────────────────────────┘
```

### 新增组件

1. **UsageStats.tsx** — 统计主容器，包含时间范围选择器、子组件编排
2. **UsageTable.tsx** — 可复用用量排行表格（号池/访问 Key 各调用一次）
3. **UsageChart.tsx** — ECharts 小时级趋势图表，支持筛选号池 Key 和模型

### 图表特性

- 堆叠柱状图: 按小时显示 prompt_tokens 和 completion_tokens
- 下拉筛选: 特定号池 Key 或模型
- Tooltip: 显示详情

### 依赖

引入 `echarts`（~300KB，gzip 后 ~100KB），通过 npm 安装。

## 文件改动清单

### Rust 后端

| 文件 | 改动类型 | 说明 |
|------|----------|------|
| `src/db/models.rs` | 修改 | `RequestLogInput` 增加 3 个字段，`AccessKeyRecord` 增加 3 个累计字段 |
| `src/db/logs.rs` | 修改 | `log_request()` 增加新字段，新增聚合查询方法 |
| `src/db/mod.rs` | 修改 | 新建 `usage_hourly` 表，`request_logs` 增加 3 列迁移，`access_keys` 增加 3 列迁移 |
| 新增 `src/db/usage.rs` | 新增 | `usage_hourly` 表 CRUD + access_keys 累计字段更新 |
| `src/proxy/forwarder.rs` | 修改 | 非流式/流式响应提取 usage |
| `src/proxy/orchestrator.rs` | 修改 | 传递 access_key_id 和 usage 数据 |
| `src/state.rs` | 修改 | 新增 `UsageCache` 字段，启动刷盘定时任务 |
| 新增 `src/usage_cache.rs` | 新增 | `UsageCache` 实现 |
| `src/server/routes.rs` | 修改 | 新增 6 个统计路由 |
| `src/server/handlers/admin.rs` | 修改 | 新增统计 handler |

### React 前端

| 文件 | 改动类型 | 说明 |
|------|----------|------|
| `frontend/src/types/index.ts` | 修改 | 新增统计相关类型 |
| `frontend/src/api/admin.ts` | 修改 | 新增统计 API 调用 |
| `frontend/src/App.tsx` | 修改 | 引入 UsageStats 组件 |
| 新增 `frontend/src/components/usage/UsageStats.tsx` | 新增 | 统计主容器 |
| 新增 `frontend/src/components/usage/UsageTable.tsx` | 新增 | 用量排行表格 |
| 新增 `frontend/src/components/usage/UsageChart.tsx` | 新增 | 趋势图表 |

### 无需改动

`config.rs`, `crypto.rs`, `error.rs`, `scheduler/*`, `health/*`, `cli.rs`

## 参考项目

- **new-api / one-api** (Calcium-Ion/new-api, songquanpeng/one-api)
  - `logs` 表: prompt_tokens, completion_tokens, quota, model_name, token_id, channel_id
  - `quota_data` 表: 按小时聚合 (user_id, model_name, hour_bucket) → token_used, count, quota
  - 内存缓存 `CacheQuotaData` + 定期刷盘
  - 前端: 小时柱状图 + 按模型/用户细分
