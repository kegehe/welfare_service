use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// 数据库中的 API Key 记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    pub id: i64,
    pub platform: String,
    /// 可选显示名称
    pub name: String,
    /// AES-GCM 加密后的密文 (base64)
    pub api_key: String,
    pub openai_url: String,
    pub claude_url: String,
    /// JSON 数组: ["claude-sonnet-4-20250514", ...]
    pub models: String,
    pub tpm_limit: i64,
    pub rpm_limit: i64,
    pub status: String, // active | disabled | unhealthy | expired
    pub source: Option<String>,
    pub note: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

/// 添加 API Key 时的输入
#[derive(Debug, Deserialize)]
pub struct AddApiKeyInput {
    pub platform: String,
    pub name: Option<String>,
    pub api_key: String,
    pub openai_url: String,
    pub claude_url: String,
    pub models: Vec<String>,
    pub tpm_limit: Option<i64>,
    pub rpm_limit: Option<i64>,
    pub source: Option<String>,
    pub note: Option<String>,
}

/// 更新 API Key 时的输入
#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyInput {
    pub platform: String,
    pub name: Option<String>,
    /// 留空或缺省表示保留原 API Key
    pub api_key: Option<String>,
    pub openai_url: String,
    pub claude_url: String,
    pub models: Vec<String>,
    pub tpm_limit: Option<i64>,
    pub rpm_limit: Option<i64>,
    pub source: Option<String>,
    pub note: Option<String>,
}

/// 请求日志
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct RequestLog {
    pub id: i64,
    pub key_id: Option<i64>,
    pub model: String,
    pub status_code: Option<i32>,
    pub latency_ms: Option<i64>,
    pub is_success: Option<bool>,
    pub affects_key_health: Option<bool>,
    pub error_msg: Option<String>,
    pub created_at: Option<NaiveDateTime>,
}

/// 熔断器状态
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitState {
    pub key_id: i64,
    pub state: String, // closed | open | half_open
    pub failure_count: i32,
    pub last_failure_at: Option<NaiveDateTime>,
    pub next_retry_at: Option<NaiveDateTime>,
}

/// 令牌桶状态
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBucketState {
    pub key_id: i64,
    pub tpm_remaining: i64,
    pub rpm_remaining: i64,
    pub updated_at: Option<NaiveDateTime>,
}

/// 访问 Key 记录 (用户用来访问号池的凭证)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKeyRecord {
    pub id: i64,
    /// sk-xxxx 格式的访问凭证
    pub key: String,
    /// 名称/备注
    pub name: String,
    /// active | disabled
    pub status: String,
    /// RPM 限制 (0=不限)
    pub rpm_limit: i64,
    /// TPM 限制 (0=不限)
    pub tpm_limit: i64,
    /// 过期时间 (NULL=永不过期)
    pub expires_at: Option<NaiveDateTime>,
    /// 最后使用时间
    pub last_used_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

/// 创建访问 Key 的输入
#[derive(Debug, Deserialize)]
pub struct CreateAccessKeyInput {
    pub name: Option<String>,
    pub rpm_limit: Option<i64>,
    pub tpm_limit: Option<i64>,
    /// 过期时间字符串 "2026-12-31 23:59:59" 或空
    pub expires_at: Option<String>,
}

/// 更新访问 Key 的输入
#[derive(Debug, Deserialize)]
pub struct UpdateAccessKeyInput {
    pub name: Option<String>,
    pub rpm_limit: Option<i64>,
    pub tpm_limit: Option<i64>,
    /// 过期时间字符串 "2026-12-31 23:59:59" 或空
    pub expires_at: Option<String>,
}
