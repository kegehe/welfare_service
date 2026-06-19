# Welfare Service — API Key 池化共享服务设计文档

> 日期：2026-06-16
> 状态：设计阶段
> 作者：tangmaoke

---

## 1. 项目背景

Linux.do 社区用户经常无偿分享自己用不完的各大平台 coding plan / token plan 的 API key。但存在两个问题：

- **热门 key 过载**：刚分享时很多人同时使用，导致 429 限流
- **冷门 key 浪费**：发帖时间不好时，key 几乎没人用

**目标**：构建一个 API Key 池化服务，将分散的 key 统一管理，提供代理接口，自动分流限流，让用户只需接入一个地址即可丝滑使用。

---

## 2. 目标平台

第一期支持：
- **小米（MiMo）** — 提供 OpenAI + Claude 双格式兼容 API
- **讯飞（星火）** — 提供 OpenAI + Claude 双格式兼容 API

两者均支持 OpenAI 和 Claude 两种协议格式，通过不同的 base_url 区分。

### 2.1 平台 API 端点

| 平台 | 协议 | Base URL |
|------|------|----------|
| 小米 MiMo | OpenAI 兼容 | `https://token-plan-sgp.xiaomimimo.com/v1` |
| 小米 MiMo | Claude 兼容 | `https://token-plan-sgp.xiaomimimo.com/anthropic` |
| 讯飞星火 | OpenAI 兼容 | `https://maas-coding-api.cn-huabei-1.xf-yun.com/v2` |
| 讯飞星火 | Claude 兼容 | `https://maas-coding-api.cn-huabei-1.xf-yun.com/anthropic` |

### 2.2 平台特性

| 项目 | 小米 MiMo | 讯飞星火 |
|------|----------|---------|
| OpenAI 兼容 | ✅ | ✅ |
| Claude 兼容 | ✅ | ✅ |
| 认证方式 | Bearer Token | Bearer Token |
| 限流机制 | TPM/RPM | TPM/RPM（按套餐区分） |
| 流式响应 | SSE | SSE |

### 2.3 主要客户端

本服务主要面向以下 CLI 工具：

| 客户端 | 协议格式 | 接入方式 |
|--------|---------|---------|
| **Claude Code CLI** | Claude Messages API (`/v1/messages`) | 配置 `ANTHROPIC_BASE_URL` 指向本服务 |
| **Codex CLI** | OpenAI 兼容 (`/v1/chat/completions`) | 配置 `OPENAI_BASE_URL` 指向本服务 |

**使用示例**：
```bash
# Claude Code CLI
ANTHROPIC_BASE_URL=http://localhost:8080 claude

# Codex CLI（OpenAI 兼容）
OPENAI_BASE_URL=http://localhost:8080 codex
```

> ⚠️ 注意：Claude Code CLI 的环境变量名需确认，可能是 `ANTHROPIC_BASE_URL` 或其他名称。Codex CLI 的环境变量名也需确认。

### 2.4 待确认事项

> ⚠️ 以下信息需要在开发前确认，影响具体实现：

1. **限流响应头** — 平台是否返回 `x-ratelimit-remaining`、`retry-after` 等标准头
2. **429 响应格式** — 是否标准 JSON，是否有 `retry-after` 头
3. **健康检查端点** — 是否有 `/v1/models` 可用于验证 key
4. **Claude 协议端点** — 讯飞的 `/anthropic` 端点具体支持哪些 Claude API 路径

**设计决策**：不硬编码平台信息，通过配置文件定义平台特性，支持运行时调整。

---

## 3. 架构方案

### 3.1 选型：Rust 单进程代理服务

```
Claude Code CLI ──→ /v1/messages (Claude 格式)
Codex CLI      ──→ /v1/chat/completions (OpenAI 格式)
       │
       ▼
┌─────────────────────────────┐
│      Axum HTTP 服务          │
│   http://localhost:8080      │
├─────────────────────────────┤
│                             │
│  ┌──────────┐  ┌─────────┐ │
│  │ 路由解析  │→│ Key 调度 │ │
│  │(协议识别) │  │(令牌桶+  │ │
│  │          │  │ 熔断器)  │ │
│  └──────────┘  └────┬────┘ │
│                     │      │
│  ┌──────────┐  ┌────▼────┐ │
│  │ 健康检查  │  │ 代理转发 │ │
│  └──────────┘  └────┬────┘ │
│                     │      │
│              ┌──────▼──────┐│
│              │   SQLite    ││
│              └─────────────┘│
└─────────────────────────────┘
       │
       ├─────→ 小米 MiMo API (OpenAI/Claude)
       └─────→ 讯飞星火 API (OpenAI/Claude)
```

**选择理由**：
- 单二进制部署，零依赖
- Rust 性能优秀，适合高并发代理
- SQLite 零配置，本地自用够用
- 后续可平滑迁移到微服务架构

### 3.2 技术栈

| 组件 | 选型 | 理由 |
|------|------|------|
| Web 框架 | Axum | 异步、类型安全、tokio 生态 |
| HTTP 客户端 | reqwest | 支持 SSE 流式、连接池 |
| 数据库 | SQLite (rusqlite) | 零配置、单文件 |
| 加密 | AES-GCM (aes-gcm) | 安全存储 API Key |
| 序列化 | serde + serde_json | JSON 处理 |
| 配置 | toml | 人类可读 |
| CLI | clap | 命令行解析 |
| 异步运行时 | tokio | Rust 异步标准 |
| 日志 | tracing | 结构化日志 |
| 定时任务 | tokio-cron-scheduler | 健康检查调度 |

### 3.3 后端分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                        接入层 (Presentation Layer)            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │  Axum Router    │  │  请求解析器      │  │  响应构建器   │ │
│  │  路由分发        │  │  协议识别        │  │  格式转换     │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬───────┘ │
│           └───────────────────┼───────────────────┘          │
└───────────────────────────────┼──────────────────────────────┘
                                │
┌───────────────────────────────┼──────────────────────────────┐
│                        服务层 (Service Layer)                  │
│  ┌─────────────────┐  ┌──────▼──────────┐  ┌──────────────┐ │
│  │  Key 调度服务    │  │  代理转发服务    │  │  健康检查服务 │ │
│  │                  │  │                  │  │              │ │
│  │  - 令牌桶管理    │  │  - 请求转发      │  │  - 被动检测   │ │
│  │  - 熔断器状态    │  │  - 流式处理      │  │  - 主动探活   │ │
│  │  - Key 选择算法  │  │  - 错误重试      │  │  - 状态更新   │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬───────┘ │
│           └───────────────────┼───────────────────┘          │
└───────────────────────────────┼──────────────────────────────┘
                                │
┌───────────────────────────────┼──────────────────────────────┐
│                        数据层 (Data Layer)                     │
│  ┌─────────────────┐  ┌──────▼──────────┐  ┌──────────────┐ │
│  │  Key 存储        │  │  请求日志        │  │  配置管理     │ │
│  │  (SQLite)        │  │  (SQLite)        │  │  (TOML)      │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
└──────────────────────────────────────────────────────────────┘
                                │
┌───────────────────────────────┼──────────────────────────────┐
│                      外部 API 层 (External API Layer)          │
│  ┌─────────────────┐  ┌──────▼──────────┐  ┌──────────────┐ │
│  │  小米 MiMo API  │  │  讯飞星火 API    │  │  其他平台     │ │
│  │  (OpenAI/Claude)│  │  (OpenAI/Claude) │  │  (扩展)      │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

**各层职责**：

| 层级 | 职责 | 关键模块 |
|------|------|---------|
| 接入层 | HTTP 路由、协议识别、请求/响应格式转换 | Axum Router, Protocol Parser |
| 服务层 | 核心业务逻辑：调度、转发、健康检查 | Scheduler, Proxy, HealthChecker |
| 数据层 | 持久化存储、配置管理 | SQLite, TOML Config |
| 外部 API 层 | 上游 API 通信、认证、错误处理 | reqwest Client |

**层间通信**：
- 接入层 → 服务层：通过 `AppState` 共享状态（Arc 包裹）
- 服务层 → 数据层：通过 Repository Trait 抽象，方便后续替换存储
- 服务层 → 外部 API 层：通过 `UpstreamClient` 封装，统一处理认证和错误

---

## 4. 核心模块设计

### 4.1 请求路由

```
/v1/chat/completions  → OpenAI 兼容格式
/v1/messages          → Claude Messages 格式
/v1/models            → 返回可用模型列表
/admin/*              → 管理接口（后续扩展）
```

**路由逻辑**：
1. 根据 URL 路径判断协议类型（`/v1/chat/completions` → OpenAI，`/v1/messages` → Claude）
2. 从请求体提取 `model` 字段
3. 查找支持该模型的可用 key
4. 根据协议选择对应的上游 URL（`openai_url` 或 `claude_url`）
5. 转发到上游 API，替换认证头

### 4.2 Key 调度引擎

#### 4.2.1 令牌桶

每个 key 维护独立的令牌桶：

```rust
struct TokenBucket {
    tpm_capacity: u64,      // TPM 桶容量
    tpm_tokens: AtomicU64,  // 当前 TPM 令牌数
    rpm_capacity: u64,      // RPM 桶容量
    rpm_tokens: AtomicU64,  // 当前 RPM 令牌数
    last_refill: Instant,   // 上次补充时间
}
```

**补充策略**：
- 每秒补充 `capacity / 60` 个令牌
- 令牌数不超过容量上限
- 使用原子操作保证并发安全

**调度流程**：
1. 筛选支持请求模型的 key
2. 排除熔断中的 key
3. 排除令牌不足的 key
4. 按令牌剩余量降序排序，选择最多的
5. 消耗 1 个 TPM 令牌 + 1 个 RPM 令牌

#### 4.2.2 熔断器

```
状态机：Closed → Open → Half-Open → Closed

┌──────────┐  连续失败≥N次  ┌──────────┐
│  Closed  │ ──────────────→ │   Open   │
│ (正常)   │                │ (熔断)   │
└──────────┘                └────┬─────┘
      ↑                          │
      │                  等待 T 秒后尝试
      │                          │
      │                          ▼
      │                    ┌──────────┐
      └────────────────────│Half-Open │
            试探成功        │(试探)    │
                           └──────────┘
```

**参数**：
- `failure_threshold`: 3（连续失败次数触发熔断）
- `recovery_timeout`: 60s（熔断后等待时间）
- `half_open_max_try`: 1（半开状态最大试探次数）

#### 4.2.3 调度算法伪代码

```rust
fn select_key(model: &str) -> Option<&KeyState> {
    let candidates: Vec<_> = keys.iter()
        .filter(|k| k.models.contains(model))
        .filter(|k| k.circuit_breaker.state != Open)
        .filter(|k| k.token_bucket.has_tokens(1))
        .collect();

    if candidates.is_empty() {
        return None; // 所有 key 不可用
    }

    // 按令牌剩余量排序，选最多的
    candidates.sort_by(|a, b| {
        b.token_bucket.remaining().cmp(&a.token_bucket.remaining())
    });

    let selected = candidates[0];
    selected.token_bucket.consume(1);
    Some(selected)
}
```

### 4.3 代理转发层

#### 4.3.1 请求转发

```rust
/// 协议类型
enum Protocol {
    OpenAI,  // /v1/chat/completions
    Claude,  // /v1/messages
}

/// 从请求路径中提取上游路径（去掉代理前缀）
/// 例如：/v1/chat/completions → /chat/completions
///       /v1/messages → /messages
fn extract_upstream_path(request_path: &str) -> &str {
    request_path
        .strip_prefix("/v1")
        .unwrap_or(request_path)
}

async fn proxy_request(
    key: &KeyState,
    protocol: &Protocol,
    request_path: &str,   // 完整请求路径，如 /v1/chat/completions
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let client = reqwest::Client::new();

    // 根据协议选择上游 base_url（已包含路径前缀）
    let base_url = match protocol {
        Protocol::OpenAI => &key.openai_url,   // https://...xiaomimimo.com/v1
        Protocol::Claude => &key.claude_url,   // https://...xiaomimimo.com/anthropic
    };

    // 拼接：base_url + 去掉前缀的请求路径
    // 例：https://.../v1 + /chat/completions = https://.../v1/chat/completions
    let upstream_path = extract_upstream_path(request_path);
    let upstream_url = format!("{}{}", base_url, upstream_path);

    // 构建上游请求
    let mut req = client.post(&upstream_url)
        .headers(headers)
        .body(body);

    // 替换认证头（两种协议都用 Bearer Token）
    req = req.header("Authorization", format!("Bearer {}", key.api_key));

    // Claude 协议额外添加版本头
    if matches!(protocol, Protocol::Claude) {
        req = req.header("anthropic-version", "2023-06-01");
    }

    req.send().await
}
```

#### 4.3.2 流式响应处理

```rust
async fn proxy_stream(
    key: &KeyState,
    request: Request,
) -> Sse<impl Stream<Item = Result<Event>>> {
    let upstream_response = proxy_request(key, ...).await?;

    // 将上游 SSE 流转换为下游 SSE 流
    let stream = upstream_response.bytes_stream()
        .map(|chunk| {
            // 解析上游 SSE 事件
            // 转换为标准 SSE 格式
            // 返回给客户端
        });

    Sse::new(stream)
}
```

#### 4.3.3 错误处理

| 上游状态码 | 处理策略 |
|-----------|---------|
| 200 | 正常返回，重置熔断计数 |
| 429 | 标记限流，暂停使用 60s，尝试下一个 key |
| 401/403 | 标记失效，停止使用，通知管理员 |
| 5xx | 累加失败计数，触发熔断判断 |
| 超时 | 累加失败计数，触发熔断判断 |

**重试策略**：
- 429 → 立即尝试下一个 key
- 5xx → 尝试下一个 key（最多重试 2 次）
- 401/403 → 不重试，直接返回错误
- 所有 key 都失败 → 返回 503 + 明确错误信息

### 4.4 健康检查

#### 4.4.1 被动检测（实时）

请求过程中实时更新 key 状态：
- 成功 → 重置失败计数
- 429 → 标记限流中，暂停 60s
- 401/403 → 标记失效
- 其他错误 → 累加失败计数

#### 4.4.2 主动探活（定时）

```
每 5 分钟执行：

1. 遍历所有 active 的 key
   - 发送轻量请求（如 GET /v1/models）
   - 成功 → 重置失败计数，更新 last_check 时间
   - 401/403 → 标记 expired
   - 其他错误 → 记录日志，不立即下线

2. 遍历所有 expired 的 key
   - 尝试探活
   - 成功 → 恢复为 active
```

#### 4.4.3 指标统计

以下指标通过查询 `request_logs` 表实时计算，不单独存储：
- `success_count`: 成功请求数（`SELECT COUNT(*) WHERE key_id = ? AND is_success = true`）
- `failure_count`: 失败请求数
- `avg_latency_ms`: 最近 100 次请求的平均延迟
- `last_used_at`: 最后使用时间（`SELECT MAX(created_at) WHERE key_id = ?`）
- `last_checked_at`: 最后健康检查时间（记录在内存中，定期同步到 `circuit_states` 表）

---

## 5. 数据模型

### 5.1 SQLite 表结构

```sql
-- API Key 存储
CREATE TABLE api_keys (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    platform    TEXT NOT NULL,               -- "xiaomi" | "iflytek"
    api_key     TEXT NOT NULL,               -- AES-GCM 加密存储
    openai_url  TEXT NOT NULL,               -- OpenAI 兼容端点
    claude_url  TEXT NOT NULL,               -- Claude 兼容端点
    models      TEXT NOT NULL,               -- 支持的模型列表 JSON
    tpm_limit   INTEGER DEFAULT 0,          -- TPM 限制（0=不限制）
    rpm_limit   INTEGER DEFAULT 0,          -- RPM 限制（0=不限制）
    status      TEXT DEFAULT 'active',       -- active | disabled | expired
    source      TEXT,                        -- 来源链接（Linux.do 帖子）
    note        TEXT,                        -- 备注
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 请求日志
CREATE TABLE request_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id      INTEGER REFERENCES api_keys(id),
    model       TEXT NOT NULL,
    status_code INTEGER,
    latency_ms  INTEGER,
    is_success  BOOLEAN,
    error_msg   TEXT,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 熔断状态持久化
CREATE TABLE circuit_states (
    key_id          INTEGER PRIMARY KEY REFERENCES api_keys(id),
    state           TEXT DEFAULT 'closed',   -- closed | open | half_open
    failure_count   INTEGER DEFAULT 0,
    last_failure_at DATETIME,
    next_retry_at   DATETIME
);

-- 令牌桶状态持久化（服务重启恢复）
CREATE TABLE token_bucket_states (
    key_id          INTEGER PRIMARY KEY REFERENCES api_keys(id),
    tpm_remaining   INTEGER NOT NULL,         -- 当前剩余 TPM 令牌
    rpm_remaining   INTEGER NOT NULL,         -- 当前剩余 RPM 令牌
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 索引
CREATE INDEX idx_request_logs_created_at ON request_logs(created_at);
CREATE INDEX idx_request_logs_key_id ON request_logs(key_id);
CREATE INDEX idx_api_keys_status ON api_keys(status);
```

### 5.2 Key 加密存储

`api_keys.api_key` 字段使用 AES-256-GCM 加密存储，密钥从环境变量 `WELFARE_SECRET_KEY` 读取。

```rust
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::{Aead, OsRng};

struct KeyStore {
    cipher: Aes256Gcm,
}

impl KeyStore {
    /// 加密 API Key，返回 nonce + ciphertext
    fn encrypt(&self, plaintext: &str) -> Vec<u8> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 随机 nonce
        let ciphertext = self.cipher.encrypt(&nonce, plaintext.as_bytes())
            .expect("encryption failed");
        // 拼接：nonce (12 bytes) + ciphertext
        [nonce.as_slice(), &ciphertext].concat()
    }

    /// 解密 API Key
    fn decrypt(&self, data: &[u8]) -> String {
        let (nonce, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce);
        let plaintext = self.cipher.decrypt(nonce, ciphertext)
            .expect("decryption failed");
        String::from_utf8(plaintext).expect("invalid utf8")
    }
}
```

**说明**：
- 每次加密使用随机 nonce，nonce 存储在密文前 12 字节
- `WELFARE_SECRET_KEY` 必须是 32 字节（256 位）的 base64 编码字符串
- 首次运行时自动生成密钥并保存到 `.env` 文件

---

## 6. 配置文件

```toml
# config.toml

[server]
host = "0.0.0.0"
port = 8080
# workers = 4  # 默认使用 CPU 核心数

[database]
path = "./data/welfare.db"

[encryption]
# AES 密钥从环境变量读取
key_env = "WELFARE_SECRET_KEY"

[health_check]
# 主动探活间隔（秒）
interval_secs = 300
# 连续失败次数触发熔断
failure_threshold = 3
# 熔断后等待时间（秒）
recovery_timeout_secs = 60
# 限流暂停时间（秒）
rate_limit_pause_secs = 60

[rate_limit]
# 默认 TPM（key 未指定时使用）
default_tpm = 100000
# 默认 RPM
default_rpm = 1000

[logging]
# 日志级别：trace | debug | info | warn | error
level = "info"
# 日志文件路径（不配置则输出到 stdout）
# file = "./logs/welfare.log"

# 平台配置
[[platforms]]
name = "xiaomi"
display_name = "小米 MiMo"
auth_type = "bearer"
default_openai_url = "https://token-plan-sgp.xiaomimimo.com/v1"
default_claude_url = "https://token-plan-sgp.xiaomimimo.com/anthropic"

[[platforms]]
name = "iflytek"
display_name = "讯飞星火"
auth_type = "bearer"
default_openai_url = "https://maas-coding-api.cn-huabei-1.xf-yun.com/v2"
default_claude_url = "https://maas-coding-api.cn-huabei-1.xf-yun.com/anthropic"
```

---

## 7. CLI 命令

```bash
# 启动服务
welfare serve --config config.toml

# Key 管理
welfare key add \
  --platform xiaomi \
  --key "sk-xxxx" \
  --openai-url "https://token-plan-sgp.xiaomimimo.com/v1" \
  --claude-url "https://token-plan-sgp.xiaomimimo.com/anthropic" \
  --models "claude-3-5-sonnet,gpt-4o" \
  --tpm 100000 \
  --rpm 1000

welfare key list                    # 列出所有 key
welfare key list --status active    # 按状态筛选
welfare key disable <id>           # 禁用 key
welfare key enable <id>            # 启用 key
welfare key remove <id>            # 删除 key
welfare key test <id>              # 测试 key 是否可用

# 状态查看
welfare status                     # 查看所有 key 健康状态
welfare stats                      # 查看请求统计
welfare stats --key-id <id>        # 查看指定 key 统计
welfare logs --tail 50             # 查看最近日志
```

---

## 8. 项目结构

```
welfare_service/
├── Cargo.toml
├── config.toml
├── src/
│   ├── main.rs                    # 入口：解析 CLI，启动服务
│   ├── cli.rs                     # CLI 命令定义
│   ├── config.rs                  # 配置文件解析
│   ├── error.rs                   # 统一错误类型
│   ├── crypto.rs                  # 加密/解密工具
│   ├── server/
│   │   ├── mod.rs
│   │   ├── routes.rs              # 路由定义
│   │   ├── handlers/
│   │   │   ├── mod.rs
│   │   │   ├── chat.rs            # /v1/chat/completions
│   │   │   ├── messages.rs        # /v1/messages
│   │   │   └── models.rs          # /v1/models
│   │   └── middleware.rs          # 日志、错误处理中间件
│   ├── proxy/
│   │   ├── mod.rs
│   │   ├── forwarder.rs           # 请求转发
│   │   └── stream.rs              # SSE 流式处理
│   ├── scheduler/
│   │   ├── mod.rs
│   │   ├── token_bucket.rs        # 令牌桶实现
│   │   ├── circuit_breaker.rs     # 熔断器实现
│   │   └── selector.rs            # Key 选择算法
│   ├── health/
│   │   ├── mod.rs
│   │   └── checker.rs             # 健康检查逻辑
│   ├── db/
│   │   ├── mod.rs
│   │   ├── models.rs              # 数据模型
│   │   ├── keys.rs                # Key CRUD
│   │   └── logs.rs                # 日志查询
│   └── state.rs                   # 应用状态（共享数据）
├── data/                          # SQLite 数据目录
├── docs/
│   └── 2026-06-16-welfare-service-design.md
└── tests/
    ├── integration/
    └── unit/
```

---

## 9. 开发路线

| 阶段 | 内容 | 优先级 |
|------|------|--------|
| P1 | 基础框架：Axum + SQLite + 配置解析 | 高 |
| P2 | 单 key 转发：手动添加 key，代理 OpenAI + Claude 双协议 | 高 |
| P3 | 多 key 调度：令牌桶 + key 选择算法 | 高 |
| P4 | 熔断器：失败检测 + 状态机 + 持久化 | 中 |
| P5 | 健康检查：被动检测 + 主动探活 | 中 |
| P6 | CLI 工具：key 管理 + 状态查看 | 中 |
| P7 | SSE 流式响应支持 | 中 |
| P8 | 请求日志 + 统计 | 低 |
| P9 | Key 加密存储 | 低 |

---

## 10. 潜在风险和应对

### 10.1 技术风险

| 风险 | 影响 | 应对 |
|------|------|------|
| 平台 API 变更 | 接口不兼容 | 抽象平台层，配置化端点和认证方式 |
| 限流策略不透明 | 无法精确计数 | 先被动检测，再根据 429 响应头优化 |
| SQLite 并发瓶颈 | 高并发写入慢 | WAL 模式 + 写入队列 |
| Key 失效不及时 | 用户请求失败 | 双重健康检查保障 |

### 10.2 非技术风险

| 风险 | 影响 | 应对 |
|------|------|------|
| Key 来源不稳定 | 池子枯竭 | 支持自助提交 + 社区激励 |
| 平台封禁共享 key | Key 批量失效 | 分散使用、限制单 key 并发 |
| 滥用风险 | 被平台封禁 | 后续加用户认证 + 用量限制 |

### 10.3 待确认事项（开发前必须解决）

1. **限流响应头** — 平台是否返回 `x-ratelimit-remaining`、`retry-after` 等标准头
2. **429 响应格式** — 是否标准 JSON，是否有 `retry-after` 头
3. **健康检查端点** — 是否有 `/v1/models` 可用于验证 key
4. **Claude 协议端点** — 讯飞的 `/anthropic` 端点具体支持哪些 Claude API 路径

---

## 11. 后续扩展方向

- **用户系统**：注册、登录、用量统计、配额管理
- **Web 管理后台**：可视化管理 key、查看统计
- **自动抓取**：爬取 Linux.do 帖子中的 key
- **多平台支持**：Cursor、GitHub Copilot、更多国内平台
- **分布式部署**：Redis + PostgreSQL，支持多实例
- **社区功能**：key 贡献排行榜、使用反馈

---

## 附录 A：API 格式参考

### OpenAI 兼容格式

```http
POST /v1/chat/completions
Content-Type: application/json
Authorization: Bearer <api_key>

{
  "model": "claude-3-5-sonnet",
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "stream": true
}
```

### Claude Messages 格式

```http
POST /v1/messages
Content-Type: application/json
x-api-key: <api_key>
anthropic-version: 2023-06-01

{
  "model": "claude-3-5-sonnet",
  "max_tokens": 1024,
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "stream": true
}
```

### SSE 流式响应格式

```
data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"delta":{"content":"Hello"},"index":0}]}

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"delta":{"content":" world"},"index":0}]}

data: [DONE]
```
