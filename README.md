# Welfare Service

API Key 池化共享服务 — 汇聚 Linux.do 社区共享 API Key，提供统一代理端点，自动处理限流与故障转移。

## 功能

- **API Key 池化管理** — 多平台（小米 MiMo、讯飞星火）Key 统一入库，AES-256-GCM 加密存储
- **双协议代理** — 同时支持 OpenAI Chat Completions (`/v1/chat/completions`) 和 Claude Messages (`/v1/messages`) 格式
- **自动限流** — Token Bucket 算法控制 TPM/RPM，支持按 Key 独立配置（0 = 不限）
- **熔断保护** — 连续失败自动熔断，定时半开探测恢复，避免无效请求冲击故障 Key
- **健康检查** — 定时主动探测上游端点 + 被动监控成功率，故障 Key 自动下线与恢复
- **SSE 流式转发** — 完整支持 Server-Sent Events 实时流式响应
- **管理 API** — Key 增删查 + 健康状态查询，Token 认证保护

## 架构

```
Client → [统一代理端点] → [Key 选择器] → [上游 API]
                ↓               ↓
          [Token Bucket]   [熔断器]
                ↓               ↓
          [SQLite 存储]    [健康检查器]
```

## 快速开始

### 1. 构建

```bash
cargo build --release
```

无需系统 OpenSSL 依赖，使用 rustls 静态链接 TLS。

### 2. 生成加密主密钥

```bash
./target/release/welfare-service gen-key
```

输出类似：
```
主密钥: abc123def456...
请将此密钥保存到 config.toml 的 [encryption].master_key 字段
```

### 3. 编辑配置

```bash
cp config.toml config.toml
vim config.toml
```

必须填写的字段：
- `[encryption].master_key` — 上一步生成的主密钥
- `[server].admin_token` — 管理 API 认证令牌（自行设定一个强密码）

### 4. 添加 API Key

```bash
./target/release/welfare-service add-key \
  --platform xiaomi \
  --key "sk-xxx" \
  --openai-url "https://api.xiaomi.com/v1" \
  --claude-url "https://api.xiaomi.com/v1" \
  --models "mimo-v2,mimo-v2-mini" \
  --tpm-limit 100000 \
  --rpm-limit 60 \
  --source "linux.do" \
  --note "用户名分享"
```

参数说明：

| 参数 | 说明 |
|------|------|
| `--platform` | 平台标识：`xiaomi` 或 `iflytek` |
| `--key` | 原始 API Key |
| `--openai-url` | 该 Key 的 OpenAI 兼容端点 Base URL |
| `--claude-url` | 该 Key 的 Claude 兼容端点 Base URL |
| `--models` | 支持的模型列表，逗号分隔 |
| `--tpm-limit` | 每分钟 Token 限制（0 = 不限） |
| `--rpm-limit` | 每分钟请求数限制（0 = 不限） |
| `--source` | 来源说明（可选） |
| `--note` | 备注（可选） |

### 5. 启动服务

```bash
./target/release/welfare-service serve
```

默认监听 `127.0.0.1:8080`，可通过 `config.toml` 修改。

### 6. 使用代理

**OpenAI 格式：**

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-user-token" \
  -d '{
    "model": "mimo-v2",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

**Claude 格式：**

```bash
curl http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-user-token" \
  -d '{
    "model": "mimo-v2",
    "messages": [{"role": "user", "content": "Hello"}],
    "max_tokens": 1024
  }'
```

**SSE 流式：**

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-user-token" \
  -d '{"model": "mimo-v2", "messages": [{"role": "user", "content": "Hello"}], "stream": true}'
```

## 管理 API

所有管理接口需要 `Authorization: Bearer <admin_token>` 认证。

```bash
# 查看所有 Key
curl -H "Authorization: Bearer your-admin-token" http://localhost:8080/admin/keys

# 查看健康状态
curl -H "Authorization: Bearer your-admin-token" http://localhost:8080/admin/health

# 删除 Key
curl -X DELETE -H "Authorization: Bearer your-admin-token" http://localhost:8080/admin/keys/1
```

## CLI 命令

| 命令 | 说明 |
|------|------|
| `serve` | 启动代理服务 |
| `gen-key` | 生成 AES-256-GCM 主密钥 |
| `add-key` | 添加 API Key |
| `remove-key <id>` | 移除 API Key |
| `list-keys` | 列出所有 Key |

全局选项：`-c, --config <PATH>` 指定配置文件路径（默认 `config.toml`）

## 配置说明

`config.toml` 完整字段：

```toml
[server]
host = "127.0.0.1"        # 监听地址
port = 8080                # 监听端口
admin_token = ""           # 管理 API 认证令牌（必填）
cors_origin = ""           # CORS 允许的源（空 = 不允许跨域）

[encryption]
master_key = ""            # AES-256-GCM 主密钥（必填，用 gen-key 生成）

[database]
path = "data/welfare.db"   # SQLite 数据库文件路径

[proxy]
timeout = 60               # 上游请求超时（秒）

[scheduler]
health_check_interval = 300  # 健康检查间隔（秒）

[scheduler.token_bucket]
enabled = true

[scheduler.circuit_breaker]
enabled = true
failure_threshold = 5      # 连续失败 N 次触发熔断
recovery_timeout = 60      # 熔断恢复超时（秒）
```

## 技术栈

- **Rust** + **Axum 0.8** + **Tokio** 异步运行时
- **SQLite** (rusqlite, WAL 模式) 持久化
- **AES-256-GCM** 密钥加密
- **reqwest** + **rustls-tls** HTTP 客户端（无系统 OpenSSL 依赖）
- **Token Bucket** 限流算法
- **Circuit Breaker** 熔断状态机

## License

MIT
