pub mod handlers;
pub mod routes;

use axum::http::HeaderValue;
use axum::Router;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// 创建 Axum 应用
pub fn create_app(state: AppState) -> Router {
    // CORS: 使用配置中的允许源
    let cors = if state.config.server.cors_origin.is_empty() {
        // 未配置 CORS 源，允许所有来源
        CorsLayer::very_permissive()
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::exact(
                state
                    .config
                    .server
                    .cors_origin
                    .parse::<HeaderValue>()
                    .expect("cors_origin 配置格式错误"),
            ))
            .allow_methods(Any)
            .allow_headers(Any)
    };

    Router::new()
        .merge(routes::api_routes())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
