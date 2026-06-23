use axum::body::Body;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::{Protocol, UsageInfo, build_upstream_url};
use crate::db::logs::RequestLogInput;
use crate::db::models::ApiKeyRecord;
use crate::error::{AppError, Result};
use crate::state::AppState;
use crate::usage_cache::UsageCache;

/// 转发响应类型。流式响应由 body 的最终器在传输结束后记录日志。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardResponseKind {
    Immediate,
    StreamFinalized,
}

#[derive(Clone)]
pub struct StreamRequestFinalizer {
    pub state: AppState,
    pub request_id: u64,
    pub key_id: i64,
    pub access_key_id: Option<i64>,
    pub model: String,
    pub started_at: std::time::Instant,
}

#[derive(Debug, Clone)]
enum StreamEndState {
    Completed,
    UpstreamError(String),
    Cancelled,
}

#[derive(Debug, Default)]
struct StreamTrackerState {
    usage: UsageInfo,
    end_state: Option<StreamEndState>,
    finalized: bool,
}

#[derive(Clone)]
struct StreamTracker {
    inner: Arc<parking_lot::Mutex<StreamTrackerState>>,
}

impl StreamTracker {
    fn new() -> Self {
        Self {
            inner: Arc::new(parking_lot::Mutex::new(StreamTrackerState::default())),
        }
    }

    fn add_usage(&self, delta: UsageInfo) {
        if delta.prompt_tokens == 0 && delta.completion_tokens == 0 {
            return;
        }
        let mut state = self.inner.lock();
        state.usage.add_prompt(delta.prompt_tokens);
        state.usage.add_completion(delta.completion_tokens);
    }

    fn finish(&self, end_state: StreamEndState) {
        let mut state = self.inner.lock();
        if state.end_state.is_none() {
            state.end_state = Some(end_state);
        }
    }

    fn mark_completed(&self) {
        self.finish(StreamEndState::Completed);
    }
}

struct StreamFinalizeGuard {
    finalizer: StreamRequestFinalizer,
    tracker: StreamTracker,
}

impl Drop for StreamFinalizeGuard {
    fn drop(&mut self) {
        let (usage, end_state, should_finalize) = {
            let mut state = self.tracker.inner.lock();
            if state.finalized {
                return;
            }
            state.finalized = true;
            (
                state.usage.clone(),
                state.end_state.clone().unwrap_or(StreamEndState::Cancelled),
                true,
            )
        };

        if !should_finalize {
            return;
        }

        let latency = self.finalizer.started_at.elapsed().as_millis() as i64;
        let (status_code, is_success, affects_key_health, error_msg) = match end_state {
            StreamEndState::Completed => (Some(200), true, true, None),
            StreamEndState::UpstreamError(error) => (Some(502), false, true, Some(error)),
            StreamEndState::Cancelled => (
                Some(499),
                false,
                false,
                Some("stream cancelled before completion".to_string()),
            ),
        };

        self.finalizer
            .state
            .active_keys_notifier
            .deactivate(self.finalizer.request_id);

        let error_ref = error_msg.as_deref();
        if let Err(error) = self.finalizer.state.db.log_request(RequestLogInput {
            key_id: Some(self.finalizer.key_id),
            access_key_id: self.finalizer.access_key_id,
            model: &self.finalizer.model,
            status_code,
            latency_ms: Some(latency),
            is_success,
            affects_key_health,
            error_msg: error_ref,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        }) {
            tracing::warn!("写入流式请求日志失败: {}", error);
        } else if affects_key_health {
            self.finalizer.state.health_cache.invalidate();
        }

        self.finalizer.state.usage_cache.record(
            self.finalizer.key_id,
            self.finalizer.access_key_id,
            &self.finalizer.model,
            0,
            0,
            is_success,
        );

        if is_success {
            self.finalizer
                .state
                .circuit_breaker
                .record_success(self.finalizer.key_id);
        } else if affects_key_health {
            self.finalizer
                .state
                .circuit_breaker
                .record_failure(self.finalizer.key_id);
        }
    }
}

/// 请求体大小限制 (16 MB)
const MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

/// 转发请求到上游 API
///
/// 返回上游的响应 (可能是 SSE 流) 和提取的 usage 信息。
/// 对于非流式响应，usage 从 JSON body 中提取；
/// 对于流式响应，usage 在 SSE 帧中被提取并直接写入 UsageCache。
#[allow(clippy::too_many_arguments)]
pub async fn forward_request(
    state: &AppState,
    key: &ApiKeyRecord,
    protocol: Protocol,
    method: Method,
    request_path: &str,
    headers: HeaderMap,
    body: Bytes,
    model: &str,
    access_key_id: Option<i64>,
    stream_finalizer: Option<StreamRequestFinalizer>,
) -> Result<(Response, UsageInfo, ForwardResponseKind)> {
    // 检查请求体大小
    if body.len() > MAX_BODY_SIZE {
        return Err(AppError::PayloadTooLarge);
    }

    let decrypted_key = state.key_store.decrypt(&key.api_key)?;

    let base_url = match protocol {
        Protocol::OpenAI => &key.openai_url,
        Protocol::Claude => &key.claude_url,
    };
    if base_url.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "Key {} 未配置 {:?} URL",
            key.id, protocol
        )));
    }

    let upstream_url = build_upstream_url(base_url, request_path);

    tracing::debug!(
        "转发请求: {} {} -> {} (key_id={})",
        method,
        request_path,
        upstream_url,
        key.id
    );

    // 构建上游请求
    let mut req = state.http_client.request(method.clone(), &upstream_url);

    // 只转发安全的请求头
    // 注意: anthropic-version 不在此白名单中，由下方协议块单独处理，
    // 避免与默认值重复 (reqwest .header() 是 append 行为)
    let allowed_headers = [
        "content-type",
        "accept",
        "accept-encoding",
        "accept-language",
        "cache-control",
        "user-agent",
        "anthropic-beta",
        "x-request-id",
    ];
    for name in &allowed_headers {
        if let Some(value) = headers.get(*name) {
            req = req.header(*name, value.clone());
        }
    }

    // 设置认证头和协议特定头
    match protocol {
        Protocol::OpenAI => {
            req = req.header("Authorization", format!("Bearer {}", decrypted_key));
        }
        Protocol::Claude => {
            // Claude API 使用 x-api-key 头
            req = req.header("x-api-key", &decrypted_key);
            // 优先使用客户端指定的 anthropic-version，未指定时使用默认值
            if let Some(value) = headers.get("anthropic-version") {
                req = req.header("anthropic-version", value.clone());
            } else {
                req = req.header("anthropic-version", "2023-06-01");
            }
        }
    }

    // 发送请求
    req = req.body(body);

    let upstream_response = req.send().await.map_err(|e| {
        if e.is_timeout() {
            AppError::Timeout
        } else {
            AppError::UpstreamRequest(e)
        }
    })?;

    let upstream_status = upstream_response.status();
    let upstream_headers = upstream_response.headers().clone();

    // 无论是否是 SSE，只要上游 HTTP 状态非成功，就先按错误处理。
    if !upstream_status.is_success() {
        let error_body = upstream_response
            .text()
            .await
            .unwrap_or_else(|_| "无法读取上游错误信息".to_string());
        tracing::warn!(
            "上游错误: status={}, url={}, body={}",
            upstream_status,
            upstream_url,
            &error_body[..error_body.len().min(500)]
        );
        return Err(AppError::UpstreamResponse {
            status: upstream_status.as_u16(),
            body: error_body,
        });
    }

    // 检查是否是 SSE 流式响应
    let content_type = upstream_headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.contains("text/event-stream") {
        // 流式响应: 逐块转发，同时在 SSE 帧中提取 usage 并直接写入缓存
        let Some(finalizer) = stream_finalizer else {
            return Ok((
                forward_stream_response(
                    upstream_response,
                    upstream_status,
                    content_type,
                    protocol,
                    state.usage_cache.clone(),
                    key.id,
                    access_key_id,
                    model.to_string(),
                    state.cancel_token.clone(),
                    None,
                )
                .await,
                UsageInfo::default(),
                ForwardResponseKind::Immediate,
            ));
        };

        return Ok((
            forward_stream_response(
                upstream_response,
                upstream_status,
                content_type,
                protocol,
                state.usage_cache.clone(),
                key.id,
                access_key_id,
                model.to_string(),
                state.cancel_token.clone(),
                Some(finalizer),
            )
            .await,
            UsageInfo::default(),
            ForwardResponseKind::StreamFinalized,
        ));
    }

    // 非流式响应: 读取完整 body，提取 usage，然后返回
    let response_body = upstream_response
        .bytes()
        .await
        .map_err(AppError::UpstreamRequest)?;

    let usage = extract_usage_from_json(&response_body);

    // 构建响应
    let mut response = Response::builder().status(upstream_status);

    // 复制上游响应头
    for (name, value) in upstream_headers.iter() {
        if name == "transfer-encoding" || name == "content-encoding" {
            continue;
        }
        response = response.header(name.clone(), value.clone());
    }

    Ok((
        response
            .body(Body::from(response_body))
            .unwrap_or_else(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "构建响应失败").into_response()
            }),
        usage,
        ForwardResponseKind::Immediate,
    ))
}

/// 从非流式 JSON 响应中提取 usage 信息
fn extract_usage_from_json(body: &[u8]) -> UsageInfo {
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return UsageInfo::default(),
    };

    let usage_obj = value.get("usage");

    match usage_obj {
        Some(usage) => UsageInfo {
            prompt_tokens: usage
                .get("prompt_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            completion_tokens: usage
                .get("completion_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        },
        None => UsageInfo::default(),
    }
}

/// 转发 SSE 流式响应，在帧中提取 usage 并写入缓存
///
/// 收到取消信号后流会提前终止，确保优雅关闭时不会因 SSE 长连接卡住进程。
#[allow(clippy::too_many_arguments)]
async fn forward_stream_response(
    upstream_response: reqwest::Response,
    status: StatusCode,
    upstream_content_type: &str,
    protocol: Protocol,
    usage_cache: Arc<UsageCache>,
    pool_key_id: i64,
    access_key_id: Option<i64>,
    model: String,
    cancel: CancellationToken,
    stream_finalizer: Option<StreamRequestFinalizer>,
) -> Response {
    use axum::body::Body;
    use futures::StreamExt;

    let tracker = StreamTracker::new();
    let finalize_guard = stream_finalizer.map(|finalizer| {
        Arc::new(StreamFinalizeGuard {
            finalizer,
            tracker: tracker.clone(),
        })
    });

    let stream = match protocol {
        Protocol::Claude => {
            let upstream_stream = upstream_response.bytes_stream();
            futures::stream::try_unfold(
                (
                    upstream_stream,
                    Vec::<u8>::new(),
                    ClaudeSseSanitizer::default(),
                    usage_cache,
                    pool_key_id,
                    access_key_id,
                    model,
                    cancel,
                    tracker.clone(),
                    finalize_guard.clone(),
                    false, // input_recorded
                ),
                |(mut upstream_stream, mut buffer, mut sanitizer, uc, pk, ak, mdl, cancel, tracker, guard, mut input_recorded)| async move {
                    loop {
                        if let Some((frame_end, separator_len)) = find_sse_frame(&buffer) {
                            let frame_bytes = buffer[..frame_end].to_vec();
                            buffer.drain(..frame_end + separator_len);

                            // 提取 usage 并写入缓存
                            let delta = extract_and_cache_usage_from_sse_frame(
                                &frame_bytes, Protocol::Claude, &uc, pk, ak, &mdl, &mut input_recorded,
                            );
                            tracker.add_usage(delta);

                            if let Some(sanitized) = sanitizer.sanitize_frame(&frame_bytes) {
                                return Ok(Some((
                                    Bytes::from(sanitized),
                                    (upstream_stream, buffer, sanitizer, uc, pk, ak, mdl, cancel, tracker, guard, input_recorded),
                                )));
                            }
                            continue;
                        }

                        // 等待上游数据或取消信号
                        tokio::select! {
                            chunk = upstream_stream.next() => {
                                match chunk {
                                    Some(Ok(chunk)) => {
                                        buffer.extend_from_slice(&chunk);
                                    }
                                    Some(Err(error)) => {
                                        tracker.finish(StreamEndState::UpstreamError(error.to_string()));
                                        return Err(std::io::Error::other(error));
                                    }
                                    None => {
                                        if buffer.is_empty() {
                                            tracker.finish(StreamEndState::Completed);
                                            return Ok(None);
                                        }

                                        let frame_bytes = std::mem::take(&mut buffer);
                                        let delta = extract_and_cache_usage_from_sse_frame(
                                            &frame_bytes, Protocol::Claude, &uc, pk, ak, &mdl, &mut input_recorded,
                                        );
                                        tracker.add_usage(delta);
                                        if let Some(sanitized) = sanitizer.sanitize_frame(&frame_bytes) {
                                            tracker.mark_completed();
                                            return Ok(Some((
                                                Bytes::from(sanitized),
                                                (upstream_stream, buffer, sanitizer, uc, pk, ak, mdl, cancel, tracker, guard, input_recorded),
                                            )));
                                        }
                                        tracker.finish(StreamEndState::Completed);
                                        return Ok(None);
                                    }
                                }
                            }
                            _ = cancel.cancelled() => {
                                tracing::debug!("SSE 流收到关闭信号，提前终止");
                                tracker.finish(StreamEndState::Cancelled);
                                return Ok(None);
                            }
                        }
                    }
                },
            )
            .boxed()
        }
        Protocol::OpenAI => {
            let upstream_stream = upstream_response.bytes_stream();
            futures::stream::try_unfold(
                (upstream_stream, Vec::<u8>::new(), usage_cache, pool_key_id, access_key_id, model, cancel, tracker.clone(), finalize_guard.clone()),
                |(mut upstream_stream, mut buffer, uc, pk, ak, mdl, cancel, tracker, guard)| async move {
                    let mut _unused = false;
                    loop {
                        if let Some((frame_end, separator_len)) = find_sse_frame(&buffer) {
                            let frame_bytes = buffer[..frame_end].to_vec();
                            buffer.drain(..frame_end + separator_len);

                            // 提取 usage 并写入缓存
                            let delta = extract_and_cache_usage_from_sse_frame(
                                &frame_bytes, Protocol::OpenAI, &uc, pk, ak, &mdl, &mut _unused,
                            );
                            tracker.add_usage(delta);

                            return Ok(Some((
                                Bytes::from(frame_bytes),
                                (upstream_stream, buffer, uc, pk, ak, mdl, cancel, tracker, guard),
                            )));
                        }

                        // 等待上游数据或取消信号
                        tokio::select! {
                            chunk = upstream_stream.next() => {
                                match chunk {
                                    Some(Ok(chunk)) => {
                                        buffer.extend_from_slice(&chunk);
                                    }
                                    Some(Err(error)) => {
                                        tracker.finish(StreamEndState::UpstreamError(error.to_string()));
                                        return Err(std::io::Error::other(error));
                                    }
                                    None => {
                                        if buffer.is_empty() {
                                            tracker.finish(StreamEndState::Completed);
                                            return Ok(None);
                                        }
                                        let frame_bytes = std::mem::take(&mut buffer);
                                        let delta = extract_and_cache_usage_from_sse_frame(
                                            &frame_bytes, Protocol::OpenAI, &uc, pk, ak, &mdl, &mut _unused,
                                        );
                                        tracker.add_usage(delta);
                                        tracker.mark_completed();
                                        return Ok(Some((
                                            Bytes::from(frame_bytes),
                                            (upstream_stream, buffer, uc, pk, ak, mdl, cancel, tracker, guard),
                                        )));
                                    }
                                }
                            }
                            _ = cancel.cancelled() => {
                                tracing::debug!("SSE 流收到关闭信号，提前终止");
                                tracker.finish(StreamEndState::Cancelled);
                                return Ok(None);
                            }
                        }
                    }
                },
            )
            .boxed()
        }
    };

    let body = Body::from_stream(stream);

    let mut response = Response::builder().status(status);
    response = response.header("content-type", upstream_content_type);
    response = response.header("cache-control", "no-cache");
    response = response.header("connection", "keep-alive");

    // 流式响应: usage 在 SSE 帧中提取并直接写入缓存，这里返回 default
    // orchestrator 会用 default(0,0) 的 usage 记录日志（流式场景 token 记录由缓存补足）
    response
        .body(body)
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "构建流式响应失败").into_response())
}

/// 从 SSE 帧中提取 usage 信息并直接写入 UsageCache
///
/// `input_recorded`: 跨帧追踪 Claude 协议的 input tokens 是否已记录，
/// 因为 message_start 和 message_delta 中可能包含相同的 input_tokens 值，
/// 通过此标记避免双重计数。
fn extract_and_cache_usage_from_sse_frame(
    frame_bytes: &[u8],
    protocol: Protocol,
    usage_cache: &UsageCache,
    pool_key_id: i64,
    access_key_id: Option<i64>,
    model: &str,
    input_recorded: &mut bool,
) -> UsageInfo {
    let frame = match String::from_utf8_lossy(frame_bytes).into_owned() {
        f if f.trim().is_empty() => return UsageInfo::default(),
        f => f,
    };

    let mut total = UsageInfo::default();
    for line in frame.lines() {
        let data = match line.strip_prefix("data:").map(str::trim) {
            Some("[DONE]") => continue,
            Some(d) => d,
            None => continue,
        };

        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };

        match protocol {
            Protocol::OpenAI => {
                // OpenAI stream: 最后一个 chunk 的 usage 字段
                if let Some(usage_obj) = value.get("usage") {
                    let prompt = usage_obj
                        .get("prompt_tokens")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let completion = usage_obj
                        .get("completion_tokens")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    if prompt > 0 || completion > 0 {
                        usage_cache.record(
                            pool_key_id,
                            access_key_id,
                            model,
                            prompt,
                            completion,
                            false,
                        );
                        total.add_prompt(prompt);
                        total.add_completion(completion);
                        tracing::debug!(
                            "SSE usage 提取 (OpenAI): prompt={}, completion={}",
                            prompt,
                            completion
                        );
                    }
                }
            }
            Protocol::Claude => {
                let event_type = value.get("type").and_then(|v| v.as_str());

                // message_start: 包含 input_tokens
                if event_type == Some("message_start") {
                    if let Some(msg) = value.get("message") {
                        if let Some(usage_obj) = msg.get("usage") {
                            let input = usage_obj
                                .get("input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            let cache_creation = usage_obj
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            let cache_read = usage_obj
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            let total_input = input + cache_creation + cache_read;
                            if total_input > 0 && !*input_recorded {
                                *input_recorded = true;
                                usage_cache.record(
                                    pool_key_id,
                                    access_key_id,
                                    model,
                                    total_input,
                                    0,
                                    false,
                                );
                                total.add_prompt(total_input);
                                tracing::debug!(
                                    "SSE usage 提取 (Claude message_start): input={}, cache_creation={}, cache_read={}, total={}",
                                    input, cache_creation, cache_read, total_input
                                );
                            }
                        }
                    }
                }

                // message_delta: 包含 output_tokens 和可能的 input_tokens
                //
                // 某些上游 API（如 xiaomimimo）在 message_start 中 input_tokens=0，
                // 实际的 input_tokens 在 message_delta 的 usage 中返回。
                // 通过 input_recorded 标记避免与 message_start 重复记录。
                if event_type == Some("message_delta") {
                    if let Some(usage_obj) = value.get("usage") {
                        let input = usage_obj
                            .get("input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let cache_creation = usage_obj
                            .get("cache_creation_input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let cache_read = usage_obj
                            .get("cache_read_input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let total_input = input + cache_creation + cache_read;
                        let output = usage_obj
                            .get("output_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let input_delta = if total_input > 0 && !*input_recorded {
                            *input_recorded = true;
                            total_input
                        } else {
                            0
                        };
                        if input_delta > 0 || output > 0 {
                            usage_cache.record(pool_key_id, access_key_id, model, input_delta, output, false);
                            total.add_prompt(input_delta);
                            total.add_completion(output);
                            tracing::debug!(
                                "SSE usage 提取 (Claude message_delta): input={}, cache_creation={}, cache_read={}, input_delta={}, output={}",
                                input, cache_creation, cache_read, input_delta, output
                            );
                        }
                    }
                }
            }
        }
    }
    total
}

fn find_sse_frame(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|window| window == b"\n\n");
    let crlf = buffer.windows(4).position(|window| window == b"\r\n\r\n");

    match (lf, crlf) {
        (Some(lf_idx), Some(crlf_idx)) => {
            if lf_idx < crlf_idx {
                Some((lf_idx, 2))
            } else {
                Some((crlf_idx, 4))
            }
        }
        (Some(found), None) => Some((found, 2)),
        (None, Some(found)) => Some((found, 4)),
        (None, None) => None,
    }
}

#[derive(Default)]
struct ClaudeSseSanitizer {
    dropped_thinking_blocks: HashSet<i64>,
    index_map: HashMap<i64, i64>,
    next_index: i64,
}

impl ClaudeSseSanitizer {
    fn sanitize_frame(&mut self, frame_bytes: &[u8]) -> Option<String> {
        let frame = String::from_utf8_lossy(frame_bytes);
        self.sanitize_frame_str(&frame)
    }

    fn sanitize_frame_str(&mut self, frame: &str) -> Option<String> {
        if frame.trim().is_empty() {
            return None;
        }

        let Some(data) = frame
            .lines()
            .find_map(|line| line.strip_prefix("data:").map(str::trim))
        else {
            return Some(format!("{}\n\n", frame));
        };

        if data == "[DONE]" {
            return Some(format!("{}\n\n", frame));
        }

        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(data) else {
            return Some(format!("{}\n\n", frame));
        };

        let event_type = value.get("type").and_then(|value| value.as_str());
        let index = value.get("index").and_then(|value| value.as_i64());

        match event_type {
            Some("content_block_start") => {
                if value
                    .get("content_block")
                    .and_then(|block| block.get("type"))
                    .and_then(|value| value.as_str())
                    == Some("thinking")
                {
                    if let Some(index) = index {
                        self.dropped_thinking_blocks.insert(index);
                    }
                    return None;
                }
                self.rewrite_new_content_block_index(&mut value, index);
            }
            Some("content_block_delta") => {
                let delta_type = value
                    .get("delta")
                    .and_then(|delta| delta.get("type"))
                    .and_then(|value| value.as_str());
                if matches!(delta_type, Some("thinking_delta" | "signature_delta"))
                    || index.is_some_and(|index| self.dropped_thinking_blocks.contains(&index))
                {
                    return None;
                }
                self.rewrite_existing_content_block_index(&mut value, index);
            }
            Some("content_block_stop")
                if index.is_some_and(|index| self.dropped_thinking_blocks.remove(&index)) =>
            {
                return None;
            }
            Some("content_block_stop") => {
                self.rewrite_existing_content_block_index(&mut value, index);
            }
            _ => {}
        }

        Some(replace_sse_data(frame, &value))
    }

    fn rewrite_new_content_block_index(
        &mut self,
        value: &mut serde_json::Value,
        original_index: Option<i64>,
    ) {
        let Some(original_index) = original_index else {
            return;
        };
        let mapped_index = self.next_index;
        self.next_index += 1;
        self.index_map.insert(original_index, mapped_index);
        set_sse_index(value, mapped_index);
    }

    fn rewrite_existing_content_block_index(
        &mut self,
        value: &mut serde_json::Value,
        original_index: Option<i64>,
    ) {
        let Some(original_index) = original_index else {
            return;
        };
        if let Some(mapped_index) = self.index_map.get(&original_index).copied() {
            set_sse_index(value, mapped_index);
        }
    }
}

fn set_sse_index(value: &mut serde_json::Value, index: i64) {
    if let Some(object) = value.as_object_mut() {
        object.insert("index".to_string(), serde_json::Value::from(index));
    }
}

fn replace_sse_data(frame: &str, value: &serde_json::Value) -> String {
    let rewritten_data = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    let lines = frame
        .lines()
        .map(|line| {
            if line.starts_with("data:") {
                format!("data: {}", rewritten_data)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}\n\n", lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_text_delta_frames() {
        let mut sanitizer = ClaudeSseSanitizer::default();
        let frame = r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}"#;

        let sanitized = sanitizer.sanitize_frame_str(frame).unwrap();

        assert!(sanitized.contains("text_delta"));
        assert!(sanitized.ends_with("\n\n"));
    }

    #[test]
    fn drops_thinking_delta_frames() {
        let mut sanitizer = ClaudeSseSanitizer::default();
        let frame = r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hidden"}}"#;

        assert!(sanitizer.sanitize_frame_str(frame).is_none());
    }

    #[test]
    fn drops_full_thinking_blocks() {
        let mut sanitizer = ClaudeSseSanitizer::default();
        let start = r#"event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"thinking","thinking":""}}"#;
        let stop = r#"event: content_block_stop
data: {"type":"content_block_stop","index":1}"#;

        assert!(sanitizer.sanitize_frame_str(start).is_none());
        assert!(sanitizer.sanitize_frame_str(stop).is_none());
    }

    #[test]
    fn rewrites_indexes_after_dropped_thinking_block() {
        let mut sanitizer = ClaudeSseSanitizer::default();
        let thinking = r#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#;
        let text = r#"event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#;
        let delta = r#"event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"hi"}}"#;

        assert!(sanitizer.sanitize_frame_str(thinking).is_none());
        let text = sanitizer.sanitize_frame_str(text).unwrap();
        let delta = sanitizer.sanitize_frame_str(delta).unwrap();

        assert!(text.contains(r#""index":0"#));
        assert!(delta.contains(r#""index":0"#));
    }

    #[test]
    fn extract_usage_from_openai_json() {
        let body = r#"{"id":"chatcmpl-123","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":20}}"#;
        let usage = extract_usage_from_json(body.as_bytes());
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
    }

    #[test]
    fn extract_usage_from_json_missing_usage() {
        let body = r#"{"id":"chatcmpl-123","choices":[]}"#;
        let usage = extract_usage_from_json(body.as_bytes());
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
    }
}
