use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// 统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("数据库错误")]
    Database(#[from] rusqlite::Error),

    #[error("加密错误: {0}")]
    Crypto(String),

    #[error("没有可用的 API Key")]
    NoAvailableKey,

    #[error("上游请求失败")]
    UpstreamRequest(#[from] reqwest::Error),

    #[error("上游返回错误")]
    UpstreamResponse { status: u16, body: String },

    #[error("请求超时")]
    Timeout,

    #[error("无效请求: {0}")]
    BadRequest(String),

    #[error("内部错误")]
    Internal(String),

    #[error("认证失败: {0}")]
    Unauthorized(String),

    #[error("请求体过大")]
    PayloadTooLarge,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            AppError::Config(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "config_error",
                "服务配置错误".to_string(),
            ),
            AppError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                "数据库内部错误".to_string(),
            ),
            AppError::Crypto(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "crypto_error",
                "加密操作失败".to_string(),
            ),
            AppError::NoAvailableKey => (
                StatusCode::SERVICE_UNAVAILABLE,
                "no_available_key",
                "没有可用的 API Key，请稍后重试".to_string(),
            ),
            AppError::UpstreamRequest(_) => (
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                "上游服务不可达".to_string(),
            ),
            AppError::UpstreamResponse { status, body } => {
                let upstream_status =
                    StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY);
                // 尝试从上游响应体中提取更有用的错误信息
                let detail = extract_upstream_error(body);
                (
                    upstream_status,
                    "upstream_error",
                    if detail.is_empty() {
                        format!("上游服务返回错误 (HTTP {})", status)
                    } else {
                        format!("上游服务返回错误 (HTTP {}): {}", status, detail)
                    },
                )
            }
            AppError::Timeout => (
                StatusCode::GATEWAY_TIMEOUT,
                "timeout",
                "上游请求超时".to_string(),
            ),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务器内部错误".to_string(),
            ),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg.clone()),
            AppError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "请求体过大".to_string(),
            ),
        };

        let body = Json(json!({
            "error": {
                "type": error_type,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

/// 从上游 JSON 错误响应中提取有意义的错误信息
fn extract_upstream_error(body: &str) -> String {
    // 尝试解析 JSON，提取嵌套的 message 字段
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        // OpenAI/Claude 格式: {"error": {"message": "..."}}
        if let Some(msg) = val
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return msg.to_string();
        }
        // 简单 message 字段
        if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
    }
    // 非 JSON 或无法提取，截断返回原始文本
    if body.len() > 200 {
        format!("{}...", &body[..200])
    } else {
        body.to_string()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
