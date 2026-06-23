use axum::http::{HeaderMap, Method};
use axum::response::Response;
use bytes::Bytes;

use crate::db::logs::RequestLogInput;
use crate::db::models::ActiveKeyEntry;
use crate::error::{AppError, Result};
use crate::proxy::{forwarder, Protocol};
use crate::proxy::forwarder::{ForwardResponseKind, StreamRequestFinalizer};
use crate::scheduler::selector::KeySelector;
use crate::state::AppState;

/// 全局请求 ID 计数器，用于标识每个代理请求
static REQUEST_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_request_id() -> u64 {
    REQUEST_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

const MAX_REQUEST_BODY: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureImpact {
    ClientRequest,
    KeyRejected,
    RateLimited,
    UpstreamUnavailable,
    Internal,
}

pub(crate) struct AccessKeyVerification {
    pub(crate) id: i64,
    pub(crate) failure: Option<AppError>,
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
            Self::KeyRejected | Self::RateLimited | Self::UpstreamUnavailable | Self::Internal
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

/// 检测请求中是否包含工具调用相关内容。
///
/// 两种情况会标记为工具调用请求:
/// 1. 请求包含 `tools` 参数 — 定义了可用工具
/// 2. 请求消息中包含 `tool_result` 内容块 — 这是工具调用的后续请求
///
/// 调度器仍会在无精确匹配时映射到号池 Key 配置的上游模型。
fn detect_tool_usage(request: &serde_json::Value) -> bool {
    // 检查顶层 tools 参数
    if request.get("tools").is_some_and(|v| !v.is_null()) {
        return true;
    }

    // 检查消息中是否包含 tool_result 内容块
    // Claude 格式: messages[].content[].type == "tool_result"
    // OpenAI 格式: messages[].role == "tool"
    if let Some(messages) = request.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            // OpenAI 格式: role == "tool"
            if msg.get("role").and_then(|v| v.as_str()) == Some("tool") {
                return true;
            }
            // Claude 格式: content 数组中有 type == "tool_result"
            if let Some(content) = msg.get("content").and_then(|v| v.as_array()) {
                for block in content {
                    if block.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        return true;
                    }
                }
            }
        }
    }

    false
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
        429 => FailureImpact::RateLimited,
        408 => FailureImpact::UpstreamUnavailable, // 请求超时，非限流
        425 => FailureImpact::ClientRequest,       // TLS 过早请求，非限流
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
) -> Result<AccessKeyVerification> {
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
        return Ok(AccessKeyVerification {
            id: access_key.id,
            failure: Some(AppError::UpstreamResponse {
            status: 429,
            body: "Rate limited".to_string(),
            }),
        });
    }

    let _ = state.db.update_access_key_last_used(access_key.id);

    Ok(AccessKeyVerification {
        id: access_key.id,
        failure: None,
    })
}

fn log_request(state: &AppState, input: RequestLogInput<'_>) {
    let affects_key_health = input.affects_key_health;
    if let Err(error) = state.db.log_request(input) {
        tracing::warn!("写入请求日志失败: {}", error);
        return;
    }
    if affects_key_health {
        state.health_cache.invalidate();
    }
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

    // 检测请求中是否包含工具调用相关内容:
    // - 顶层 `tools` 参数 (定义可用工具)
    // - 消息中的 `tool_result` 内容块 (工具调用后续请求)
    // 调度器会优先精确匹配；无精确匹配时映射到号池 Key 配置模型。
    let has_tools = detect_tool_usage(&request);

    tracing::info!(
        "{:?} 请求: model={}, stream={}, has_tools={}",
        protocol, model, is_stream, has_tools
    );

    let estimated_tokens = estimate_tokens(body.len());
    let access_check = verify_access_key(&headers, &state, estimated_tokens)?;
    let access_key_id = access_check.id;
    if let Some(error) = access_check.failure {
        let error_text = error.to_string();
        log_request(&state, RequestLogInput {
            key_id: None,
            access_key_id: Some(access_key_id),
            model: &model,
            status_code: status_code(&error),
            latency_ms: None,
            is_success: false,
            affects_key_health: false,
            error_msg: Some(&error_text),
            prompt_tokens: 0,
            completion_tokens: 0,
        });
        return Err(error);
    }

    let selector = KeySelector::new(&state);
    let candidates = selector.candidates(&model, protocol, has_tools)?;
    let mut last_error: Option<AppError> = None;
    let mut skipped_by_local_limit = 0usize;
    let mut forwarded_attempts = 0usize;
    let candidate_count = candidates.len();
    let request_id = next_request_id();

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

        // 记录活跃密钥
        let key_prefix = {
            let plaintext = state.key_store.decrypt(&key.api_key).unwrap_or_default();
            if plaintext.len() > 12 {
                format!("{}****{}", &plaintext[..6], &plaintext[plaintext.len() - 4..])
            } else if plaintext.len() > 4 {
                format!("{}****", &plaintext[..2])
            } else {
                "****".to_string()
            }
        };
        let active_entry = ActiveKeyEntry {
            request_id,
            key_id: key.id,
            key_name: if key.name.is_empty() { format!("Key #{}", key.id) } else { key.name.clone() },
            key_prefix,
            platform: key.platform.clone(),
            model: model.clone(),
            started_at: chrono::Utc::now().timestamp_millis(),
        };
        state.active_keys_notifier.activate(request_id, active_entry);

        let start = std::time::Instant::now();
        let stream_finalizer = if is_stream {
            Some(StreamRequestFinalizer {
                state: state.clone(),
                request_id,
                key_id: key.id,
                access_key_id: Some(access_key_id),
                model: model.clone(),
                started_at: start,
            })
        } else {
            None
        };
        let result = forwarder::forward_request(
            &state,
            key,
            protocol,
            Method::POST,
            path,
            headers.clone(),
            forwarded_body,
            &model,
            Some(access_key_id),
            stream_finalizer,
        )
        .await;
        let latency = start.elapsed().as_millis() as i64;

        match result {
            Ok((response, usage, response_kind)) => {
                if response_kind == ForwardResponseKind::StreamFinalized {
                    return Ok(response);
                }

                state.active_keys_notifier.deactivate(request_id);
                let status = response.status().as_u16() as i32;
                log_request(&state, RequestLogInput {
                    key_id: Some(key.id),
                    access_key_id: Some(access_key_id),
                    model: &model,
                    status_code: Some(status),
                    latency_ms: Some(latency),
                    is_success: true,
                    affects_key_health: true,
                    error_msg: None,
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                });

                // 更新用量缓存
                // count_request=true: 每次请求只计一次请求次数
                // 对于非流式: tokens 从 usage 获取
                // 对于流式: tokens 由 SSE 帧提取补充 (count_request=false)
                state.usage_cache.record(
                    key.id,
                    Some(access_key_id),
                    &model,
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    true,
                );

                state.circuit_breaker.record_success(key.id);

                return Ok(response);
            }
            Err(error) => {
                let impact = classify_error(&error);
                let error_text = error.to_string();
                if impact == FailureImpact::RateLimited {
                    state.rate_limit_cooldown.mark_limited(key.id);
                    if let Err(error) = state
                        .db
                        .save_rate_limit_cooldowns(&state.rate_limit_cooldown.snapshot_remaining_secs())
                    {
                        tracing::warn!("保存 429 冷却状态失败: {}", error);
                    }
                    tracing::warn!(
                        "Key {} 收到上游限流，进入 {} 秒冷却",
                        key.id,
                        state.config.proxy.rate_limit_cooldown_secs
                    );
                }
                log_request(&state, RequestLogInput {
                    key_id: Some(key.id),
                    access_key_id: Some(access_key_id),
                    model: &model,
                    status_code: status_code(&error),
                    latency_ms: Some(latency),
                    is_success: false,
                    affects_key_health: impact.affects_key_health(),
                    error_msg: Some(&error_text),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                });

                if impact.records_circuit_failure() {
                    state.circuit_breaker.record_failure(key.id);
                }

                if !impact.retryable() {
                    // 不可重试，移除活跃记录并返回错误
                    state.active_keys_notifier.deactivate(request_id);
                    return Err(error);
                }

                tracing::warn!(
                    "Key {} 请求失败 ({:?})，尝试下一个候选: {}",
                    key.id,
                    impact,
                    error
                );
                last_error = Some(error);
                // 可重试：不 deactivate，下一次 activate 会覆盖当前条目
            }
        }
    }

    // 所有候选已遍历完毕，确保移除可能残留的活跃记录
    state.active_keys_notifier.deactivate(request_id);

    if let Some(error) = last_error {
        tracing::warn!(
            "所有候选 Key 均尝试失败: forwarded_attempts={}, candidate_count={}",
            forwarded_attempts,
            candidate_count
        );
        // 记录此次失败的请求日志（所有候选 key 均失败）
        log_request(&state, RequestLogInput {
            key_id: None,
            access_key_id: Some(access_key_id),
            model: &model,
            status_code: status_code(&error),
            latency_ms: None,
            is_success: false,
            affects_key_health: false,
            error_msg: Some(&error.to_string()),
            prompt_tokens: 0,
            completion_tokens: 0,
        });
        return Err(error);
    }

    if skipped_by_local_limit > 0 {
        // 记录全局限流失败的请求日志
        log_request(&state, RequestLogInput {
            key_id: None,
            access_key_id: Some(access_key_id),
            model: &model,
            status_code: Some(429),
            latency_ms: None,
            is_success: false,
            affects_key_health: false,
            error_msg: Some("All candidate keys are locally rate limited"),
            prompt_tokens: 0,
            completion_tokens: 0,
        });
        return Err(AppError::UpstreamResponse {
            status: 429,
            body: "All candidate keys are locally rate limited".to_string(),
        });
    }

    // 记录无可用 Key 的请求日志
    log_request(&state, RequestLogInput {
        key_id: None,
        access_key_id: Some(access_key_id),
        model: &model,
        status_code: Some(503),
        latency_ms: None,
        is_success: false,
        affects_key_health: false,
        error_msg: Some("No available key"),
        prompt_tokens: 0,
        completion_tokens: 0,
    });
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
    fn rate_limits_are_retryable_and_affect_health_but_no_circuit_failure() {
        let impact = classify_status(429);
        assert_eq!(impact, FailureImpact::RateLimited);
        assert!(impact.retryable());
        assert!(impact.affects_key_health());
        assert!(!impact.records_circuit_failure());
    }

    #[test]
    fn http_408_timeout_is_upstream_unavailable() {
        let impact = classify_status(408);
        assert_eq!(impact, FailureImpact::UpstreamUnavailable);
        assert!(impact.retryable());
        assert!(impact.affects_key_health());
        assert!(impact.records_circuit_failure());
    }

    #[test]
    fn http_425_too_early_is_client_request() {
        let impact = classify_status(425);
        assert_eq!(impact, FailureImpact::ClientRequest);
        assert!(!impact.retryable());
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

    #[test]
    fn request_ids_are_unique_and_increasing() {
        let id1 = next_request_id();
        let id2 = next_request_id();
        let id3 = next_request_id();
        assert!(id2 > id1, "request IDs should be increasing");
        assert!(id3 > id2, "request IDs should be increasing");
    }

    #[test]
    fn non_retryable_errors_include_client_errors() {
        // 400 系客户端错误不应重试
        for status in [400, 404, 409, 422] {
            let impact = classify_status(status);
            assert!(!impact.retryable(), "status {} should not be retryable", status);
        }
    }

    #[test]
    fn retryable_errors_include_upstream_and_key_errors() {
        // 401/403 鉴权失败、5xx 上游错误应该重试
        for status in [401, 403, 500, 502, 503, 504] {
            let impact = classify_status(status);
            assert!(impact.retryable(), "status {} should be retryable", status);
        }
    }

    // ========== detect_tool_usage 测试 ==========

    #[test]
    fn detect_tools_parameter() {
        let request = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{"name": "get_weather", "description": "Get weather"}]
        });
        assert!(detect_tool_usage(&request));
    }

    #[test]
    fn detect_tools_null_is_not_tool_usage() {
        let request = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": null
        });
        assert!(!detect_tool_usage(&request));
    }

    #[test]
    fn detect_tool_result_claude_format() {
        let request = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {"role": "assistant", "content": [{"type": "tool_use", "id": "tu_1", "name": "get_weather"}]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "tu_1", "content": "Sunny"}]}
            ]
        });
        assert!(detect_tool_usage(&request));
    }

    #[test]
    fn detect_tool_result_openai_format() {
        let request = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "What's the weather?"},
                {"role": "assistant", "content": null, "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "get_weather"}}]},
                {"role": "tool", "tool_call_id": "call_1", "content": "Sunny"}
            ]
        });
        assert!(detect_tool_usage(&request));
    }

    #[test]
    fn no_tool_usage_plain_chat() {
        let request = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "hi"}]
        });
        assert!(!detect_tool_usage(&request));
    }

    #[test]
    fn no_tool_usage_empty_messages() {
        let request = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": []
        });
        assert!(!detect_tool_usage(&request));
    }
}
