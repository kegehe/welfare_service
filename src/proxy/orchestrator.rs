use axum::http::{HeaderMap, Method};
use axum::response::Response;
use bytes::Bytes;

use crate::db::logs::RequestLogInput;
use crate::error::{AppError, Result};
use crate::proxy::{forwarder, Protocol};
use crate::scheduler::selector::KeySelector;
use crate::state::AppState;

const MAX_REQUEST_BODY: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureImpact {
    ClientRequest,
    KeyRejected,
    RateLimited,
    UpstreamUnavailable,
    Internal,
}

impl FailureImpact {
    fn retryable(self) -> bool {
        matches!(
            self,
            Self::KeyRejected | Self::RateLimited | Self::UpstreamUnavailable
        )
    }

    fn affects_key_health(self) -> bool {
        matches!(
            self,
            Self::KeyRejected | Self::UpstreamUnavailable | Self::Internal
        )
    }

    fn records_circuit_failure(self) -> bool {
        matches!(
            self,
            Self::KeyRejected | Self::UpstreamUnavailable | Self::Internal
        )
    }
}

/// 估算请求体的 token 数 (粗略: 1 token ≈ 4 bytes)
fn estimate_tokens(body_len: usize) -> u64 {
    (body_len as u64 / 4).max(1)
}

fn classify_error(error: &AppError) -> FailureImpact {
    match error {
        AppError::UpstreamResponse { status, .. } => classify_status(*status),
        AppError::Timeout | AppError::UpstreamRequest(_) => FailureImpact::UpstreamUnavailable,
        AppError::Crypto(_) => FailureImpact::Internal,
        AppError::PayloadTooLarge | AppError::BadRequest(_) | AppError::Unauthorized(_) => {
            FailureImpact::ClientRequest
        }
        AppError::NoAvailableKey
        | AppError::Config(_)
        | AppError::Database(_)
        | AppError::Internal(_) => FailureImpact::Internal,
    }
}

fn classify_status(status: u16) -> FailureImpact {
    match status {
        400 | 404 | 409 | 422 => FailureImpact::ClientRequest,
        401 | 403 => FailureImpact::KeyRejected,
        408 | 425 | 429 => FailureImpact::RateLimited,
        500..=599 => FailureImpact::UpstreamUnavailable,
        _ if (400..=499).contains(&status) => FailureImpact::ClientRequest,
        _ => FailureImpact::UpstreamUnavailable,
    }
}

fn status_code(error: &AppError) -> Option<i32> {
    match error {
        AppError::UpstreamResponse { status, .. } => Some(*status as i32),
        AppError::Timeout => Some(504),
        AppError::UpstreamRequest(_) => Some(502),
        _ => None,
    }
}

fn request_body_with_model(request: &serde_json::Value, upstream_model: &str) -> Result<Bytes> {
    let mut rewritten = request.clone();
    if let Some(object) = rewritten.as_object_mut() {
        object.insert(
            "model".to_string(),
            serde_json::Value::String(upstream_model.to_string()),
        );
    }

    serde_json::to_vec(&rewritten)
        .map(Bytes::from)
        .map_err(|e| AppError::Internal(format!("重写请求模型失败: {}", e)))
}

/// 验证访问 Key 并返回其 ID。
///
/// 从请求头中提取 Bearer token 或 x-api-key，查找对应的 access_keys 记录，
/// 检查状态、过期时间和访问侧限流。
pub fn verify_access_key(
    headers: &HeaderMap,
    state: &AppState,
    estimated_tokens: u64,
) -> Result<i64> {
    let access_key_str = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()));

    let key_str = match access_key_str {
        Some(k) => k,
        None => {
            return Err(AppError::Unauthorized(
                "请提供 API Key (Authorization: Bearer sk-xxx 或 x-api-key: sk-xxx)".to_string(),
            ));
        }
    };

    let access_key = state.db.get_access_key_by_key(key_str)?;

    if access_key.status != "active" {
        return Err(AppError::Unauthorized("访问 Key 已禁用".to_string()));
    }

    if let Some(exp) = access_key.expires_at {
        if chrono::Utc::now().naive_utc() > exp {
            return Err(AppError::Unauthorized("访问 Key 已过期".to_string()));
        }
    }

    if !state
        .access_token_bucket
        .try_acquire(access_key.id, estimated_tokens)
    {
        tracing::warn!(
            "访问 Key {} 限流 (预估 {} tokens)",
            access_key.id,
            estimated_tokens
        );
        return Err(AppError::UpstreamResponse {
            status: 429,
            body: "Rate limited".to_string(),
        });
    }

    let _ = state.db.update_access_key_last_used(access_key.id);

    Ok(access_key.id)
}

pub async fn handle_proxy_request(
    state: AppState,
    headers: HeaderMap,
    body: Bytes,
    protocol: Protocol,
    path: &str,
) -> Result<Response> {
    if body.len() > MAX_REQUEST_BODY {
        return Err(AppError::PayloadTooLarge);
    }

    let request: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("无效的 JSON 请求: {}", e)))?;

    let model = request
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let is_stream = request
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    tracing::info!("{:?} 请求: model={}, stream={}", protocol, model, is_stream);

    let estimated_tokens = estimate_tokens(body.len());
    verify_access_key(&headers, &state, estimated_tokens)?;

    let selector = KeySelector::new(&state);
    let candidates = selector.candidates(&model, protocol)?;
    let mut last_error: Option<AppError> = None;
    let mut skipped_by_local_limit = 0usize;
    let mut forwarded_attempts = 0usize;
    let candidate_count = candidates.len();

    for candidate in candidates {
        let key = &candidate.key;
        if !state.token_bucket.try_acquire(key.id, estimated_tokens) {
            skipped_by_local_limit += 1;
            tracing::warn!(
                "Key {} 本地令牌桶限流 (预估 {} tokens)，尝试下一个候选",
                key.id,
                estimated_tokens
            );
            continue;
        }
        forwarded_attempts += 1;

        let forwarded_body = if candidate.upstream_model == model {
            body.clone()
        } else {
            tracing::info!(
                "模型自动映射: requested_model={} -> upstream_model={} (key_id={}, exact_match={})",
                model,
                candidate.upstream_model,
                key.id,
                candidate.matched_requested_model
            );
            request_body_with_model(&request, &candidate.upstream_model)?
        };

        let start = std::time::Instant::now();
        let result = forwarder::forward_request(
            &state,
            key,
            protocol,
            Method::POST,
            path,
            headers.clone(),
            forwarded_body,
        )
        .await;
        let latency = start.elapsed().as_millis() as i64;

        match result {
            Ok(response) => {
                let status = response.status().as_u16() as i32;
                let _ = state.db.log_request(RequestLogInput {
                    key_id: Some(key.id),
                    model: &model,
                    status_code: Some(status),
                    latency_ms: Some(latency),
                    is_success: true,
                    affects_key_health: true,
                    error_msg: None,
                });
                state.circuit_breaker.record_success(key.id);
                return Ok(response);
            }
            Err(error) => {
                let impact = classify_error(&error);
                let error_text = error.to_string();
                if impact == FailureImpact::RateLimited {
                    state.rate_limit_cooldown.mark_limited(key.id);
                    tracing::warn!(
                        "Key {} 收到上游限流，进入 {} 秒冷却",
                        key.id,
                        state.config.proxy.rate_limit_cooldown_secs
                    );
                }
                let _ = state.db.log_request(RequestLogInput {
                    key_id: Some(key.id),
                    model: &model,
                    status_code: status_code(&error),
                    latency_ms: Some(latency),
                    is_success: false,
                    affects_key_health: impact.affects_key_health(),
                    error_msg: Some(&error_text),
                });

                if impact.records_circuit_failure() {
                    state.circuit_breaker.record_failure(key.id);
                }

                if !impact.retryable() {
                    return Err(error);
                }

                tracing::warn!(
                    "Key {} 请求失败 ({:?})，尝试下一个候选: {}",
                    key.id,
                    impact,
                    error
                );
                last_error = Some(error);
            }
        }
    }

    if let Some(error) = last_error {
        tracing::warn!(
            "所有候选 Key 均尝试失败: forwarded_attempts={}, candidate_count={}",
            forwarded_attempts,
            candidate_count
        );
        return Err(error);
    }

    if skipped_by_local_limit > 0 {
        return Err(AppError::UpstreamResponse {
            status: 429,
            body: "All candidate keys are locally rate limited".to_string(),
        });
    }

    Err(AppError::NoAvailableKey)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn client_errors_do_not_retry_or_affect_key_health() {
        for status in [400, 404, 409, 422] {
            let impact = classify_status(status);
            assert_eq!(impact, FailureImpact::ClientRequest);
            assert!(!impact.retryable());
            assert!(!impact.affects_key_health());
            assert!(!impact.records_circuit_failure());
        }
    }

    #[test]
    fn upstream_and_key_errors_are_retryable() {
        for status in [401, 403, 500, 502, 503, 504] {
            let impact = classify_status(status);
            assert!(impact.retryable());
            assert!(impact.affects_key_health());
            assert!(impact.records_circuit_failure());
        }
    }

    #[test]
    fn rate_limits_retry_without_health_penalty() {
        let impact = classify_status(429);
        assert_eq!(impact, FailureImpact::RateLimited);
        assert!(impact.retryable());
        assert!(!impact.affects_key_health());
        assert!(!impact.records_circuit_failure());
    }

    #[test]
    fn rewrites_request_model_for_upstream() {
        let body = request_body_with_model(
            &json!({
                "model": "claude-opus-4-8",
                "messages": [{"role": "user", "content": "hi"}]
            }),
            "mimo-v2.5-pro",
        )
        .unwrap();
        let rewritten: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(rewritten["model"], "mimo-v2.5-pro");
        assert_eq!(rewritten["messages"][0]["content"], "hi");
    }
}
