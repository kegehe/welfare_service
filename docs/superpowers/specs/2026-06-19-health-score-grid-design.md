# API Key 100格网格信号连通状态条设计

## 概述

为号池每条API Key增加100格网格信号连通状态条，以单条Key调用成功率作为评分，满分100分对应100个独立小方格。通过颜色和填充数量直观反映Key的健康状态。

## 评分规则

| 分数段 | 填充色 | 色值 | 状态文字 |
|--------|--------|------|----------|
| 80-100 | 绿色 | `#22C55E` | 正常稳定 |
| 50-79 | 黄绿色 | `#84CC16` | 轻度限流，偶发429 |
| 20-49 | 橙黄色 | `#F59E0B` | 重度限流，大量超时 |
| 0-19（有数据） | 红色 | `#EF4444` | 密钥失效/封禁/401 |
| 无数据 | 全灰 | `#E5E7EB` | 无数据 |

未填充的方格统一使用浅灰色 `#E5E7EB`。

## 评分计算（后端，混合方式）

### 数据来源优先级

1. **实时成功率**（优先）：调用 `get_key_health_stats(key_id, 20)`，基于最近20条 `affects_key_health=1` 的日志。样本数≥20时使用。
2. **24h时间窗口**（回退）：查询 `request_logs` 中最近24小时、`affects_key_health=1` 的记录计算成功率。样本数<20但有窗口数据时使用。
3. **无数据**：两者都没有数据。

### 计算流程

```
1. 查询 get_key_health_stats(key_id, 20)
2. 如果 sample_count >= 20:
     → score = round(success_rate * 100)
     → source = "realtime"
3. 否则，查询24h窗口成功率:
     → 如果窗口有数据:
         score = round(window_rate * 100)
         source = "window"
     → 否则:
         score = 0
         source = "nodata"
4. score clamp 到 [0, 100]
5. 根据 score 和 source 确定状态标签:
     source == "nodata" → "nodata"
     score >= 80 → "normal"
     score >= 50 → "light_throttled"
     score >= 20 → "heavy_throttled"
     score < 20  → "key_invalid"
```

### 关键区分

- **NoData (score=0, source=nodata)**：从未有过请求，网格全灰，状态"无数据" — 表示未知状态
- **Realtime/Window + score=0**：有请求但全部失败，网格0格填充红色，状态"密钥失效" — 表示已知故障

## API设计

### 新增端点

```
GET /admin/keys/health-score
```

**Response**:
```json
[
  {
    "key_id": 1,
    "health_score": 85,
    "score_source": "realtime",
    "status_label": "normal",
    "sample_count": 42
  },
  {
    "key_id": 2,
    "health_score": 0,
    "score_source": "nodata",
    "status_label": "nodata",
    "sample_count": 0
  }
]
```

### 数据结构

```rust
// src/db/models.rs
pub struct KeyHealthScore {
    pub key_id: i64,
    pub health_score: u8,           // 0-100
    pub score_source: ScoreSource,
    pub status_label: StatusLabel,
    pub sample_count: u32,
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
    KeyInvalid,       // 密钥失效 (0-19)
    NoData,           // 无数据
}
```

## 前端设计

### HealthScoreGrid 组件

**文件**: `frontend/src/components/HealthScoreGrid.vue`

**Props**:
```typescript
interface Props {
  score: number        // 0-100
  statusLabel: string  // 'normal' | 'light_throttled' | 'heavy_throttled' | 'key_invalid' | 'nodata'
}
```

**视觉规格**:
- 100个独立小方格，紧密排列成一条横条
- 每格 2px 宽 × 10px 高，间距 1px
- CSS Grid 布局：`grid-template-columns: repeat(100, 2px)`，`gap: 1px`
- 前 `score` 个方格填充对应颜色，剩余方格浅灰 `#E5E7EB`
- 网格条下方居中显示状态文字

**渲染逻辑**:
- `v-for` 渲染100个 `div`
- 填充/空白通过 `index < score` 判断
- 颜色通过计算属性根据 `statusLabel` 统一返回

### 前端类型扩展

```typescript
// types/index.ts 新增
interface KeyHealthScore {
  key_id: number
  health_score: number       // 0-100
  score_source: 'realtime' | 'window' | 'nodata'
  status_label: 'normal' | 'light_throttled' | 'heavy_throttled' | 'key_invalid' | 'nodata'
  sample_count: number
}
```

### API调用

```typescript
// api/admin.ts 新增
export async function getKeysHealthScore(): Promise<KeyHealthScore[]> {
  const { data } = await http.get('/admin/keys/health-score')
  return data
}
```

### PoolKeysTable 集成

- 新增 `healthScoreMap: Record<number, KeyHealthScore>` 状态
- 在 `onMounted` / 刷新时并行调用 `getKeysHealthScore()`
- **卡片视图**：替换现有 `el-progress` 成功率进度条为 `<HealthScoreGrid>`
- **表格视图**：替换"成功率"列的 `el-progress` 为 `<HealthScoreGrid>`

## 后端实现

### 新增文件

1. **`src/db/health_score.rs`** — 评分计算逻辑
   - `compute_key_health_score(db, key_id) -> KeyHealthScore`
   - `get_key_window_success_rate(db, key_id, hours) -> Option<(f64, u32)>` — 24h窗口成功率查询

2. **`src/server/handlers/health_score.rs`** — API handler
   - `keys_health_score()` — 遍历所有pool keys，计算评分，返回JSON

### 模块注册

- `src/db/mod.rs` 新增 `pub mod health_score;`
- `src/server/handlers/mod.rs` 新增 `pub mod health_score;`

### 路由注册

`src/server/routes.rs` 新增：
```
GET /admin/keys/health-score → keys_health_score
```

## 边界情况处理

| 场景 | 处理方式 |
|------|----------|
| key刚加入，无任何请求记录 | score=0, source=NoData, 全灰网格 |
| key只有1-2条记录 | 回退到24h窗口，窗口也无数据则NoData |
| score计算结果为100 | clamp到100，显示100格全绿 |
| 所有请求都失败，success_rate=0.0 | score=0, source=Realtime, 状态"密钥失效"（非NoData） |
| key被手动disabled | 仍计算评分并显示，评分反映历史健康状态 |
| key被自动标记unhealthy | 同上，评分可能很低，与unhealthy状态一致 |
| 24h窗口内只有1条记录且成功 | score=100, source=Window, 样本量少但显示为正常 |

### API错误处理

- 数据库查询失败 → 返回空数组 `[]`，不返回500
- 单个key计算失败 → 该key跳过，不影响其他key的评分返回
