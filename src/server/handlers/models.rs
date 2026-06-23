use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::state::AppState;

/// GET /v1/models
///
/// 返回所有可用模型列表 (需要提供访问 Key)
pub async fn list_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    // 验证访问 Key (模型列表也需要认证)
    let access_check = crate::proxy::orchestrator::verify_access_key(&headers, &state, 0)?;
    if let Some(error) = access_check.failure {
        return Err(error);
    }

    let keys = state.db.get_active_keys()?;

    // 收集所有唯一的模型名
    let mut models: Vec<String> = Vec::new();
    for key in &keys {
        if let Ok(model_list) = serde_json::from_str::<Vec<String>>(&key.models) {
            for m in model_list {
                let model = m.trim().to_string();
                if !model.is_empty() && !models.contains(&model) {
                    models.push(model);
                }
            }
        }
    }

    // 按字母排序
    models.sort();

    let model_objects: Vec<Value> = models
        .iter()
        .map(|m| {
            json!({
                "id": m,
                "object": "model",
                "created": 0,
                "owned_by": "welfare-service"
            })
        })
        .collect();

    Ok(Json(json!({
        "object": "list",
        "data": model_objects
    })))
}
