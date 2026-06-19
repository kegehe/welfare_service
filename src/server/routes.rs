use axum::routing::{get, post, put};
use axum::Router;

use super::handlers;
use crate::state::AppState;

/// 根路径直接返回管理页面
async fn root() -> axum::response::Response {
    axum::response::IntoResponse::into_response((
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../../static/index.html"),
    ))
}

/// API 路由
pub fn api_routes() -> Router<AppState> {
    Router::new()
        // 欢迎页
        .route("/", get(root))
        // 管理页面
        .route("/admin/ui", get(handlers::admin::serve_ui))
        // OpenAI 兼容接口
        .route(
            "/v1/chat/completions",
            post(handlers::chat::chat_completions),
        )
        .route("/v1/messages", post(handlers::messages::messages))
        .route("/v1/models", get(handlers::models::list_models))
        // 管理接口
        .route("/admin/keys", get(handlers::admin::list_keys))
        .route("/admin/keys", post(handlers::admin::add_key))
        .route("/admin/keys/status", get(handlers::admin::keys_status))
        .route(
            "/admin/keys/{id}",
            put(handlers::admin::update_key).delete(handlers::admin::remove_key),
        )
        .route("/admin/keys/{id}/toggle", post(handlers::admin::toggle_key))
        .route("/admin/keys/{id}/test", post(handlers::admin::test_key))
        .route("/admin/health", get(handlers::admin::health_status))
        // 访问 Key 管理
        .route("/admin/access-keys", get(handlers::admin::list_access_keys))
        .route("/admin/access-keys", post(handlers::admin::add_access_key))
        .route(
            "/admin/access-keys/{id}",
            put(handlers::admin::update_access_key).delete(handlers::admin::remove_access_key),
        )
        .route(
            "/admin/access-keys/{id}/toggle",
            post(handlers::admin::toggle_access_key),
        )
}
