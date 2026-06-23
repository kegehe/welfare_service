use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::error::AppError;
use crate::state::AppState;

/// GET /admin/keys/health-score
///
/// 批量获取所有 Key 的健康评分（100格网格信号连通状态条）
/// 使用批量查询优化: 2次 SQL 代替 2N 次，结果带 TTL 缓存
///
/// 通过 spawn_blocking 执行缓存+DB 操作，避免阻塞 tokio 异步线程
pub async fn keys_health_score(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let db = state.db.clone();
    let cache = state.health_cache.clone();

    let scores = tokio::task::spawn_blocking(move || cache.get_or_compute(&db))
        .await
        .map_err(|e| AppError::Internal(format!("健康评分计算任务失败: {}", e)))??;

    // 直接序列化: KeyHealthScore 的 serde derive 已处理
    // skip_serializing_if="String::is_empty" 会自动省略空 key_name
    Ok(Json(json!(scores)))
}
