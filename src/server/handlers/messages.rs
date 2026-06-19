use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;

use crate::proxy::orchestrator::handle_proxy_request;
use crate::proxy::Protocol;
use crate::state::AppState;

/// POST /v1/messages
///
/// Claude Messages API 兼容接口
pub async fn messages(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    match handle_proxy_request(state, headers, body, Protocol::Claude, "/v1/messages").await {
        Ok(response) => response,
        Err(e) => e.into_response(),
    }
}
