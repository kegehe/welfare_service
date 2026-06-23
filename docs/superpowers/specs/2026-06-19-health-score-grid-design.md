# API Key 100格网格信号连通状态条设计

## 概述

为号池每条API Key增加100格网格信号连通状态条，以单条Key调用成功率作为评分，满分100分对应100个独立小方格。通过颜色和填充数量直观反映Key的健康状态。

## 评分规则

| 分数段 | 填充色 | 色值 | 状态文字 |
|--------|--------|------|----------|
| 80-100 | 绿色 | `#22C55E` | 正常稳定 |
| 50-79 | 黄绿色 | `#84CC16` | 轻度限流，偶发429 |
| 20-49 | 橙黄色 | `#F59E0B` | 重度限流，大量超时 |
| 0-19（有数据） | 红色 | `#EF4444` | 严重异常/封禁/401 |
| 无数据 | 全灰 | `#E5E7EB` | 无数据 |

未填充的方格统一使用浅灰色 `#E5E7EB`。

### 低置信度指示

当评分来源为 Window 且样本数 < 5 时，标记为 `low_confidence: true`：
- 填充格使用斜线纹理叠加（降低透明度 + 条纹背景）
- 评分数字旁显示 ⚠ 警告图标
- Tooltip 中显示 "⚠ 低置信度" 提示

## 评分计算（后端，混合方式）

### 数据来源优先级

1. **实时成功率**（优先）：调用 `get_key_health_stats_excluding_rate_limited(key_id, 20)`，基于最近20条 `affects_key_health=1` 且非 429 的日志。样本数≥20时使用。
2. **24h时间窗口**（回退）：查询 `request_logs` 中最近24小时、`affects_key_health=1` 且非 429 的记录计算成功率。样本数<20但有窗口数据时使用。
3. **无数据**：两者都没有数据。

### 计算流程

```
1. 查询 get_key_health_stats_excluding_rate_limited(key_id, 20)
2. 如果 sample_count >= 20:
     → score = round(success_rate * 100)
     → source = "realtime"
     → low_confidence = false
3. 否则，查询24h窗口成功率（排除 429）:
     → 如果窗口有数据:
         score = round(window_rate * 100)
         source = "window"
         low_confidence = (sample_count < 5)
     → 否则:
         score = 0
         source = "nodata"
         low_confidence = false
4. score clamp 到 [0, 100]
5. 根据 score 和 source 确定状态标签:
     source == "nodata" → "nodata"
     score >= 80 → "normal"
     score >= 50 → "light_throttled"
     score >= 20 → "heavy_throttled"
     score < 20  → "critical"
```

### 关键区分

- **NoData (score=0, source=nodata)**：从未有过请求，网格全灰，状态"无数据" — 表示未知状态
- **Realtime/Window + score=0**：有请求但全部失败，网格0格填充红色，状态"严重异常" — 表示已知故障

### 性能优化

批量查询使用 `compute_all_keys_health_scores()`：2 条 SQL 代替 2N 条，结果带 60 秒 TTL 缓存。
Key 增删改时自动调用 `health_cache.invalidate()` 使缓存失效。

## API设计

### 端点

```
GET /admin/keys/health-score
```

**Response**:
```json
[
  {
    "key_id": 1,
    "key_name": "OpenAI-Prod",
    "health_score": 85,
    "score_source": "realtime",
    "status_label": "normal",
    "sample_count": 42,
    "low_confidence": false
  },
  {
    "key_id": 2,
    "health_score": 0,
    "score_source": "nodata",
    "status_label": "nodata",
    "sample_count": 0,
    "low_confidence": false
  },
  {
    "key_id": 3,
    "key_name": "Claude-Test",
    "health_score": 100,
    "score_source": "window",
    "status_label": "normal",
    "sample_count": 2,
    "low_confidence": true
  }
]
```

### 数据结构

```rust
// src/db/models.rs
pub struct KeyHealthScore {
    pub key_id: i64,
    pub key_name: String,           // Key 显示名称（空时省略序列化）
    pub health_score: u8,           // 0-100
    pub score_source: ScoreSource,
    pub status_label: StatusLabel,
    pub sample_count: u32,
    pub low_confidence: bool,       // Window 来源且样本 < 5
}

pub enum ScoreSource {
    Realtime,  // 实时成功率（样本≥20）
    Window,    // 24h时间窗口（样本<20但有窗口数据）
    NoData,    // 无数据
}

pub enum StatusLabel {
    Normal,           // 正常稳定 (80-100)
    LightThrottled,   // 轻度限流 (50-79)
    HeavyThrottled,   // 重度限流 (20-49)
    Critical,         // 严重异常 (0-19)
    NoData,           // 无数据
}
```

## 前端设计

### HealthScoreGrid 组件

**文件**: `frontend/src/components/pool/HealthScoreGrid.tsx`（React/TSX）

**Props**:
```typescript
interface Props {
  score: number
  statusLabel: string
  scoreSource: string
  sampleCount?: number
  lowConfidence?: boolean
}
```

**视觉规格**:
- 100个独立小方格，紧密排列成一条横条
- 每格 2px 宽 × 10px 高，间距 1px
- CSS Grid 布局：`grid-template-columns: repeat(100, 2px)`，`gap: 1px`
- 前 `score` 个方格填充对应颜色，剩余方格浅灰 `#E5E7EB`
- 网格条下方居中显示评分数字，颜色与填充色对应
- 低置信度时：填充格叠加斜线纹理 + 评分旁显示 ⚠ 图标

**颜色映射**:
```typescript
const STATUS_COLORS: Record<string, string> = {
  normal: '#22C55E',           // 绿色 (80-100)
  light_throttled: '#84CC16',  // 黄绿色 (50-79)
  heavy_throttled: '#F59E0B',  // 琥珀色 (20-49)
  critical: '#EF4444',         // 红色 (0-19)
  nodata: '#E5E7EB',           // 浅灰色
}
```

### 前端类型扩展

```typescript
// types/index.ts
interface KeyHealthScore {
  key_id: number
  key_name?: string
  health_score: number
  score_source: 'realtime' | 'window' | 'nodata'
  status_label: 'normal' | 'light_throttled' | 'heavy_throttled' | 'critical' | 'nodata'
  sample_count: number
  low_confidence: boolean
}
```

### PoolKeysTable 集成

- `healthScoreMap: Record<number, KeyHealthScore>` 传入组件
- **表格视图**：新增"健康评分"列，宽度适配 300px（100格 + 标签）
- **卡片视图**：在状态/熔断器标签旁显示 `<HealthScoreGrid>`

## 后端实现

### 文件

1. **`src/db/health_score.rs`** — 评分计算逻辑
   - `compute_key_health_score(db, key_id) -> KeyHealthScore` — 单个 Key 评分
   - `compute_all_keys_health_scores(db) -> Vec<KeyHealthScore>` — 批量计算（2次 SQL）

2. **`src/health_score_cache.rs`** — 评分缓存（TTL 60s）
   - `get_or_compute(db) -> Vec<KeyHealthScore>` — 获取缓存或重新计算
   - `invalidate()` — 强制失效（Key 增删改时调用）

3. **`src/server/handlers/health_score.rs`** — API handler
   - `keys_health_score()` — 使用缓存批量返回评分

### 模块注册

- `src/main.rs` 新增 `mod health_score_cache;`
- `src/db/mod.rs` 新增 `pub mod health_score;`
- `src/server/handlers/mod.rs` 新增 `pub mod health_score;`

### 缓存失效触发

- `add_key` → `health_cache.invalidate()`
- `update_key` → `health_cache.invalidate()`
- `remove_key` → `health_cache.invalidate()`
- `toggle_key` → `health_cache.invalidate()`

## 边界情况处理

| 场景 | 处理方式 |
|------|----------|
| key刚加入，无任何请求记录 | score=0, source=NoData, 全灰网格 |
| key只有1-2条记录 | 回退到24h窗口，窗口也无数据则NoData |
| score计算结果为100 | clamp到100，显示100格全绿 |
| 所有请求都失败，success_rate=0.0 | score=0, source=Realtime, 状态"严重异常"（非NoData） |
| key被手动disabled | 仍计算评分并显示，评分反映历史健康状态 |
| key被自动标记unhealthy | 同上，评分可能很低，与unhealthy状态一致 |
| 24h窗口内只有1条记录且成功 | score=100, source=Window, low_confidence=true |
| 日志清理后key失去所有窗口数据 | score=0, source=NoData（缓存可短暂保留旧值） |

### API错误处理

- 数据库查询失败 → 返回空数组 `[]`，不返回500
- 单个key计算失败 → 该key跳过，不影响其他key的评分返回
