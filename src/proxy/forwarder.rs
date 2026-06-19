use axum::body::Body;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use std::collections::{HashMap, HashSet};

use super::{build_upstream_url, Protocol};
use crate::db::models::ApiKeyRecord;
use crate::error::{AppError, Result};
use crate::state::AppState;

/// 请求体大小限制 (16 MB)
const MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

/// 转发请求到上游 API
///
/// 返回上游的响应 (可能是 SSE 流)
pub async fn forward_request(
    state: &AppState,
    key: &ApiKeyRecord,
    protocol: Protocol,
    method: Method,
    request_path: &str,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
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
    let allowed_headers = [
        "content-type",
        "accept",
        "accept-encoding",
        "accept-language",
        "cache-control",
        "user-agent",
        "anthropic-version",
        "anthropic-beta",
        "x-request-id",
    ];
    for name in &allowed_headers {
        if let Some(value) = headers.get(*name) {
            req = req.header(*name, value.clone());
        }
    }

    // 设置认证头
    match protocol {
        Protocol::OpenAI => {
            req = req.header("Authorization", format!("Bearer {}", decrypted_key));
        }
        Protocol::Claude => {
            // Claude API 使用 x-api-key 头
            req = req.header("x-api-key", &decrypted_key);
            req = req.header("anthropic-version", "2023-06-01");
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
    // 有些上游会给错误响应也设置 text/event-stream，不能因此记为成功。
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
        // 流式响应: 逐块转发
        return Ok(forward_stream_response(
            upstream_response,
            upstream_status,
            content_type,
            protocol,
        )
        .await);
    }

    // 读取响应 body (限制大小)
    let response_body = upstream_response
        .bytes()
        .await
        .map_err(AppError::UpstreamRequest)?;

    // 构建响应
    let mut response = Response::builder().status(upstream_status);

    // 复制上游响应头
    for (name, value) in upstream_headers.iter() {
        if name == "transfer-encoding" || name == "content-encoding" {
            continue;
        }
        response = response.header(name.clone(), value.clone());
    }

    Ok(response
        .body(Body::from(response_body))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "构建响应失败").into_response()))
}

/// 转发 SSE 流式响应
async fn forward_stream_response(
    upstream_response: reqwest::Response,
    status: StatusCode,
    upstream_content_type: &str,
    protocol: Protocol,
) -> Response {
    use axum::body::Body;
    use futures::StreamExt;

    let stream = match protocol {
        Protocol::Claude => {
            let upstream_stream = upstream_response.bytes_stream();
            futures::stream::try_unfold(
                (
                    upstream_stream,
                    Vec::<u8>::new(),
                    ClaudeSseSanitizer::default(),
                ),
                |(mut upstream_stream, mut buffer, mut sanitizer)| async move {
                    loop {
                        if let Some((frame_end, separator_len)) = find_sse_frame(&buffer) {
                            let frame_bytes = buffer[..frame_end].to_vec();
                            buffer.drain(..frame_end + separator_len);
                            if let Some(sanitized) = sanitizer.sanitize_frame(&frame_bytes) {
                                return Ok(Some((
                                    Bytes::from(sanitized),
                                    (upstream_stream, buffer, sanitizer),
                                )));
                            }
                            continue;
                        }

                        match upstream_stream.next().await {
                            Some(Ok(chunk)) => {
                                buffer.extend_from_slice(&chunk);
                            }
                            Some(Err(error)) => return Err(std::io::Error::other(error)),
                            None => {
                                if buffer.is_empty() {
                                    return Ok(None);
                                }

                                let frame_bytes = std::mem::take(&mut buffer);
                                if let Some(sanitized) = sanitizer.sanitize_frame(&frame_bytes) {
                                    return Ok(Some((
                                        Bytes::from(sanitized),
                                        (upstream_stream, buffer, sanitizer),
                                    )));
                                }
                                return Ok(None);
                            }
                        }
                    }
                },
            )
            .boxed()
        }
        Protocol::OpenAI => upstream_response
            .bytes_stream()
            .map(|result| result.map_err(std::io::Error::other))
            .boxed(),
    };

    let body = Body::from_stream(stream);

    let mut response = Response::builder().status(status);
    response = response.header("content-type", upstream_content_type);
    response = response.header("cache-control", "no-cache");
    response = response.header("connection", "keep-alive");

    response
        .body(body)
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "构建流式响应失败").into_response())
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
}
