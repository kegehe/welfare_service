use axum::routing::{get, post, put};
use axum::Router;

use super::handlers;
use crate::state::AppState;

/// 根路径直接返回管理页面
async fn root() -> axum::response::Response {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(html) => axum::response::IntoResponse::into_response((
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )),
        Err(_) => axum::response::IntoResponse::into_response((
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            include_str!("../../static/index.html"),
        )),
    }
}

/// 静态资源文件服务 (JS/CSS)
async fn serve_asset(axum::extract::Path(path): axum::extract::Path<String>) -> impl axum::response::IntoResponse {
    // 安全检查：防止路径遍历攻击
    if path.contains("..") || path.starts_with('/') || path.contains('\\') {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            b"Bad Request".to_vec(),
        );
    }

    let file_path = format!("static/assets/{}", path);

    // 再次校验：canonicalize 后确保路径在 static/assets/ 目录下
    match std::path::Path::new(&file_path).canonicalize() {
        Ok(canonical) => {
            // 如果 static/assets 目录存在，获取其 canonical 路径进行比较
            // 如果不存在，说明没有静态资源，直接返回 404
            let assets_dir = match std::path::Path::new("static/assets").canonicalize() {
                Ok(dir) => dir,
                Err(_) => {
                    // static/assets 目录不存在，无法提供静态文件
                    return (
                        axum::http::StatusCode::NOT_FOUND,
                        [(axum::http::header::CONTENT_TYPE, "text/plain")],
                        b"Not Found".to_vec(),
                    );
                }
            };

            if !canonical.starts_with(&assets_dir) {
                return (
                    axum::http::StatusCode::FORBIDDEN,
                    [(axum::http::header::CONTENT_TYPE, "text/plain")],
                    b"Forbidden".to_vec(),
                );
            }
        }
        Err(_) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "text/plain")],
                b"Not Found".to_vec(),
            );
        }
    }

    match tokio::fs::read(&file_path).await {
        Ok(data) => {
            let content_type = match path.rsplit('.').next() {
                Some("js") => "application/javascript",
                Some("css") => "text/css",
                Some("html") => "text/html",
                Some("json") => "application/json",
                Some("png") => "image/png",
                Some("svg") => "image/svg+xml",
                Some("ico") => "image/x-icon",
                _ => "application/octet-stream",
            };
            (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, content_type)],
                data,
            )
        }
        Err(_) => (
            axum::http::StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            b"Not Found".to_vec(),
        ),
    }
}

/// API 路由
pub fn api_routes() -> Router<AppState> {
    Router::new()
        // 根路径 - 管理页面
        .route("/", get(root))
        // 静态资源
        .route("/assets/{*path}", get(serve_asset))
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
        .route("/admin/keys/health-score", get(handlers::health_score::keys_health_score))
        .route("/admin/keys/active-stream", get(handlers::admin::active_keys_stream))
        .route(
            "/admin/keys/{id}",
            put(handlers::admin::update_key).delete(handlers::admin::remove_key),
        )
        .route("/admin/keys/{id}/toggle", post(handlers::admin::toggle_key))
        .route("/admin/keys/{id}/test", post(handlers::admin::test_key))
        .route("/admin/health", get(handlers::admin::health_status))
        // 模型预设
        .route("/admin/models/presets", get(handlers::admin::model_presets))
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
        // 用量统计
        .route("/admin/stats/overview", get(handlers::admin::stats_overview))
        .route("/admin/stats/pool-keys", get(handlers::admin::stats_pool_keys))
        .route(
            "/admin/stats/pool-keys/{id}",
            get(handlers::admin::stats_pool_key_detail),
        )
        .route(
            "/admin/stats/access-keys",
            get(handlers::admin::stats_access_keys),
        )
        .route(
            "/admin/stats/access-keys/{id}",
            get(handlers::admin::stats_access_key_detail),
        )
        .route("/admin/stats/hourly", get(handlers::admin::stats_hourly))
}
