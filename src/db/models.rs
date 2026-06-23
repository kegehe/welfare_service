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
    pub access_key_id: Option<i64>,
    pub model: String,
    pub status_code: Option<i32>,
    pub latency_ms: Option<i64>,
    pub is_success: Option<bool>,
    pub affects_key_health: Option<bool>,
    pub error_msg: Option<String>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
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
    /// 累计请求次数
    pub total_requests: i64,
    /// 累计输入 token 数
    pub total_prompt_tokens: i64,
    /// 累计输出 token 数
    pub total_completion_tokens: i64,
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

// ============================================================
// Key 健康评分 (100格网格信号连通状态条)
// ============================================================

/// 评分数据来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreSource {
    /// 实时成功率（样本≥20）
    Realtime,
    /// 24h时间窗口（样本<20但有窗口数据）
    Window,
    /// 无数据
    #[serde(rename = "nodata")]
    NoData,
}

/// 状态标签
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusLabel {
    /// 正常稳定 (80-100)
    Normal,
    /// 轻度限流 (50-79)
    LightThrottled,
    /// 重度限流 (20-49)
    HeavyThrottled,
    /// 严重异常 (0-19)
    Critical,
    /// 无数据
    #[serde(rename = "nodata")]
    NoData,
}

/// Key 健康评分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyHealthScore {
    pub key_id: i64,
    /// Key 显示名称（方便前端独立使用，无需再关联查询）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key_name: String,
    /// 0-100 整数评分
    pub health_score: u8,
    pub score_source: ScoreSource,
    pub status_label: StatusLabel,
    /// 使用的样本数
    pub sample_count: u32,
    /// 低置信度标记：Window 来源且样本 < 5 时为 true，表示评分可能不可靠
    #[serde(default)]
    pub low_confidence: bool,
}

// ============================================================
// 实时活跃密钥 (SSE 推送)
// ============================================================

/// 当前正在使用的活跃密钥条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveKeyEntry {
    /// 请求唯一标识
    pub request_id: u64,
    pub key_id: i64,
    pub key_name: String,
    pub key_prefix: String,
    /// 号池平台 (xiaomi/iflytek/anthropic)
    pub platform: String,
    /// 当前请求使用的模型
    pub model: String,
    /// 请求开始时间 (Unix 毫秒时间戳)
    pub started_at: i64,
}
