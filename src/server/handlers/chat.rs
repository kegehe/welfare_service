use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;

use crate::proxy::orchestrator::handle_proxy_request;
use crate::proxy::Protocol;
use crate::state::AppState;

/// POST /v1/chat/completions
///
/// OpenAI 兼容接口，代理到上游 API
pub async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match handle_proxy_request(
        state,
        headers,
        body,
        Protocol::OpenAI,
        "/v1/chat/completions",
    )
    .await
    {
        Ok(response) => response,
        Err(e) => e.into_response(),
    }
}
