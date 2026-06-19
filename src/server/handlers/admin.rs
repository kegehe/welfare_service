use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::time::Instant;

use crate::db::keys::hash_key;
use crate::db::models::{
    AddApiKeyInput, CreateAccessKeyInput, UpdateAccessKeyInput, UpdateApiKeyInput,
};
use crate::error::AppError;
use crate::scheduler::circuit_breaker::CircuitState;
use crate::state::AppState;

/// GET /admin/keys
///
/// 列出所有 API Key，包含编辑表单所需字段。
pub async fn list_keys(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let keys = state.db.get_all_keys()?;

    let keys_json: Vec<_> = keys
        .iter()
        .map(|k| {
            // 管理页面表格显示脱敏前缀，编辑弹窗回显完整 Key。
            let plaintext_key = state.key_store.decrypt(&k.api_key).ok();
            let key_display = plaintext_key
                .as_deref()
                .map(mask_key)
                .unwrap_or_else(|| "****".to_string());

            json!({
                "id": k.id,
                "platform": k.platform,
                "name": k.name,
                "api_key": plaintext_key.unwrap_or_default(),
                "key_prefix": key_display,
                "openai_url": k.openai_url,
                "claude_url": k.claude_url,
                "models": serde_json::from_str::<Vec<String>>(&k.models).unwrap_or_default(),
                "tpm_limit": k.tpm_limit,
                "rpm_limit": k.rpm_limit,
                "status": k.status,
                "source": k.source,
                "note": k.note,
                "created_at": k.created_at,
            })
        })
        .collect();

    Ok(Json(json!({ "keys": keys_json })))
}

/// POST /admin/keys
///
/// 添加新的 API Key
pub async fn add_key(
    State(state): State<AppState>,
    Json(mut input): Json<AddApiKeyInput>,
) -> Result<impl IntoResponse, AppError> {
    input.platform = input.platform.trim().to_string();
    input.name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    input.api_key = input.api_key.trim().to_string();
    input.models = input
        .models
        .into_iter()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .collect();
    input.tpm_limit = Some(input.tpm_limit.unwrap_or(0).max(0));
    input.rpm_limit = Some(input.rpm_limit.unwrap_or(0).max(0));
    input.source = input
        .source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    input.note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    // 验证必填字段
    if input.api_key.is_empty() {
        return Err(AppError::BadRequest("api_key 不能为空".to_string()));
    }
    // 去除 URL 前后空白，避免拼接时出错
    let openai_url = input.openai_url.trim().to_string();
    let claude_url = input.claude_url.trim().to_string();
    if openai_url.is_empty() && claude_url.is_empty() {
        return Err(AppError::BadRequest(
            "openai_url 和 claude_url 至少填写一个".to_string(),
        ));
    }
    if input.models.is_empty() {
        return Err(AppError::BadRequest("models 不能为空".to_string()));
    }

    // 验证平台
    if !crate::config::VALID_PLATFORMS.contains(&input.platform.as_str()) {
        return Err(AppError::BadRequest(format!(
            "无效的平台: {}，支持的平台: {:?}",
            input.platform,
            crate::config::VALID_PLATFORMS
        )));
    }

    // 加密 API Key
    let encrypted = state.key_store.encrypt(&input.api_key)?;

    // 将 trim 后的 URL 写回 input，确保数据库中存储的是干净的值
    input.openai_url = openai_url;
    input.claude_url = claude_url;

    // 存储到数据库
    let id = state.db.add_key(&input, &encrypted)?;

    tracing::info!(
        "添加 API Key: id={}, platform={}, models={:?}",
        id,
        input.platform,
        input.models
    );

    if let Some(key) = state.db.get_key_by_id(id)? {
        state.register_pool_key(&key);
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "API Key 添加成功"
        })),
    ))
}

/// PUT /admin/keys/:id
///
/// 更新 API Key 配置。api_key 留空时保留原密钥。
pub async fn update_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(mut input): Json<UpdateApiKeyInput>,
) -> Result<impl IntoResponse, AppError> {
    let existing = state
        .db
        .get_key_by_id(id)?
        .ok_or_else(|| AppError::BadRequest(format!("Key ID {} 不存在", id)))?;

    let api_key = input.api_key.as_deref().map(str::trim).unwrap_or("");
    input.platform = input.platform.trim().to_string();
    input.name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let openai_url = input.openai_url.trim().to_string();
    let claude_url = input.claude_url.trim().to_string();
    input.models = input
        .models
        .into_iter()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .collect();

    if openai_url.is_empty() && claude_url.is_empty() {
        return Err(AppError::BadRequest(
            "openai_url 和 claude_url 至少填写一个".to_string(),
        ));
    }
    if input.models.is_empty() {
        return Err(AppError::BadRequest("models 不能为空".to_string()));
    }
    if !crate::config::VALID_PLATFORMS.contains(&input.platform.as_str()) {
        return Err(AppError::BadRequest(format!(
            "无效的平台: {}，支持的平台: {:?}",
            input.platform,
            crate::config::VALID_PLATFORMS
        )));
    }

    input.openai_url = openai_url;
    input.claude_url = claude_url;
    input.tpm_limit = Some(input.tpm_limit.unwrap_or(0).max(0));
    input.rpm_limit = Some(input.rpm_limit.unwrap_or(0).max(0));
    input.source = input
        .source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    input.note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let encrypted_key;
    let new_key_hash;
    let (encrypted_ref, hash_ref) = if api_key.is_empty() {
        (None, None)
    } else {
        encrypted_key = state.key_store.encrypt(api_key)?;
        new_key_hash = hash_key(api_key);
        (Some(encrypted_key.as_str()), Some(new_key_hash.as_str()))
    };

    let updated = state.db.update_key(id, &input, encrypted_ref, hash_ref)?;
    if !updated {
        return Err(AppError::BadRequest(format!("Key ID {} 不存在", id)));
    }

    if let Some(updated_key) = state.db.get_key_by_id(id)? {
        if updated_key.status == "active" {
            state.register_pool_key(&updated_key);
        } else if existing.status == "active" {
            state.unregister_pool_key(id);
        }
    }

    tracing::info!("更新 API Key: id={}, platform={}", id, input.platform);
    Ok(Json(json!({
        "id": id,
        "message": "API Key 更新成功"
    })))
}

/// DELETE /admin/keys/:id
///
/// 删除指定 API Key
pub async fn remove_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let removed = state.db.remove_key(id)?;
    if removed {
        // 从令牌桶和熔断器中移除
        state.unregister_pool_key(id);
        tracing::info!("删除 API Key: id={}", id);
        Ok(Json(json!({ "message": "API Key 已删除" })))
    } else {
        Err(AppError::BadRequest(format!("Key ID {} 不存在", id)))
    }
}

/// GET /admin/health
///
/// 返回系统健康状态
pub async fn health_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let keys = state.db.get_active_keys().unwrap_or_default();
    let total_keys = keys.len();

    // 构建用户接入 base_url
    let host = &state.config.server.host;
    let port = state.config.server.port;
    // 如果绑定 0.0.0.0，用 127.0.0.1 作为示例
    let display_host = if host == "0.0.0.0" { "127.0.0.1" } else { host };

    Ok(Json(json!({
        "status": "ok",
        "active_keys": total_keys,
        "version": env!("CARGO_PKG_VERSION"),
        "base_url": format!("http://{}:{}", display_host, port),
    })))
}

/// GET /admin/keys/status
///
/// 批量获取所有 Key 的实时状态 (熔断器、令牌桶、成功率)
pub async fn keys_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let keys = state.db.get_all_keys()?;
    let statuses: Vec<_> = keys
        .iter()
        .map(|k| {
            let cb_state = state.circuit_breaker.state(k.id);
            let cb_state_str = match cb_state {
                CircuitState::Closed => "closed",
                CircuitState::Open => "open",
                CircuitState::HalfOpen => "half_open",
            };
            let cb_failures = state.circuit_breaker.failure_count(k.id);
            let (tpm_rem, rpm_rem) = state.token_bucket.remaining(k.id);
            let success_rate = state.db.get_key_success_rate(k.id, 50).unwrap_or(1.0);

            json!({
                "key_id": k.id,
                "circuit_state": cb_state_str,
                "failure_count": cb_failures,
                "tpm_remaining": tpm_rem,
                "rpm_remaining": rpm_rem,
                "success_rate": (success_rate * 100.0).round() / 100.0,
            })
        })
        .collect();

    Ok(Json(json!({ "statuses": statuses })))
}

/// POST /admin/keys/:id/toggle
///
/// 切换 Key 的启用/禁用状态
pub async fn toggle_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let keys = state.db.get_all_keys()?;
    let key = keys
        .iter()
        .find(|k| k.id == id)
        .ok_or_else(|| AppError::BadRequest(format!("Key ID {} 不存在", id)))?;

    let new_status = if key.status == "active" {
        "disabled"
    } else {
        "active"
    };
    state.db.update_key_status(id, new_status)?;

    if new_status == "active" {
        state.set_pool_key_active(key);
    } else {
        state.set_pool_key_disabled(id);
    }

    tracing::info!("切换 Key {} 状态为 {}", id, new_status);
    Ok(Json(json!({
        "id": id,
        "status": new_status,
        "message": format!("Key 已{}", if new_status == "active" { "启用" } else { "禁用" })
    })))
}

/// GET /admin/ui
///
/// 提供 Web 管理页面
pub async fn serve_ui() -> impl IntoResponse {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../../../static/index.html"),
    )
}

/// 密钥脱敏: sk-1234567890 -> sk-12****90
fn mask_key(plaintext: &str) -> String {
    if plaintext.len() > 12 {
        format!(
            "{}****{}",
            &plaintext[..6],
            &plaintext[plaintext.len() - 4..]
        )
    } else if plaintext.len() > 4 {
        format!("{}****", &plaintext[..2])
    } else {
        "****".to_string()
    }
}

// ============================================================
// 访问 Key 管理 (用户用来访问号池的凭证)
// ============================================================

/// 生成 sk-xxxx 格式的随机 Key (32 位 hex = 128 bit 随机性)
fn generate_sk_key() -> String {
    use rand::Rng;
    let bytes: [u8; 16] = rand::thread_rng().gen();
    format!("sk-{}", hex::encode(bytes))
}

/// GET /admin/access-keys
///
/// 列出所有访问 Key
pub async fn list_access_keys(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let keys = state.db.get_all_access_keys()?;

    let keys_json: Vec<_> = keys
        .iter()
        .map(|k| {
            json!({
                "id": k.id,
                "key": k.key,
                "name": k.name,
                "status": k.status,
                "rpm_limit": k.rpm_limit,
                "tpm_limit": k.tpm_limit,
                "expires_at": k.expires_at,
                "last_used_at": k.last_used_at,
                "created_at": k.created_at,
            })
        })
        .collect();

    Ok(Json(json!({ "keys": keys_json })))
}

/// POST /admin/access-keys
///
/// 创建新的访问 Key
pub async fn add_access_key(
    State(state): State<AppState>,
    Json(mut input): Json<CreateAccessKeyInput>,
) -> Result<impl IntoResponse, AppError> {
    input.name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    input.expires_at = input
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    input.tpm_limit = Some(input.tpm_limit.unwrap_or(0).max(0));
    input.rpm_limit = Some(input.rpm_limit.unwrap_or(0).max(0));

    let key = generate_sk_key();
    let id = state.db.add_access_key(&key, &input)?;

    // 注册到访问 Key 限流器
    let tpm = input.tpm_limit.unwrap_or(0) as u64;
    let rpm = input.rpm_limit.unwrap_or(0) as u64;
    state.access_token_bucket.register(id, tpm, rpm);

    tracing::info!("创建访问 Key: id={}, key={}", id, key);

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "key": key,
            "message": "访问 Key 创建成功"
        })),
    ))
}

/// PUT /admin/access-keys/:id
///
/// 更新访问 Key 配置。
pub async fn update_access_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(mut input): Json<UpdateAccessKeyInput>,
) -> Result<impl IntoResponse, AppError> {
    let existing = state
        .db
        .get_all_access_keys()?
        .into_iter()
        .find(|key| key.id == id)
        .ok_or_else(|| AppError::BadRequest(format!("访问 Key ID {} 不存在", id)))?;

    input.name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    input.expires_at = input
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    input.tpm_limit = Some(input.tpm_limit.unwrap_or(0).max(0));
    input.rpm_limit = Some(input.rpm_limit.unwrap_or(0).max(0));

    let updated = state.db.update_access_key(id, &input)?;
    if !updated {
        return Err(AppError::BadRequest(format!("访问 Key ID {} 不存在", id)));
    }

    if existing.status == "active" {
        let tpm = input.tpm_limit.unwrap_or(0).max(0) as u64;
        let rpm = input.rpm_limit.unwrap_or(0).max(0) as u64;
        state.access_token_bucket.register(id, tpm, rpm);
    }

    tracing::info!("更新访问 Key: id={}", id);
    Ok(Json(json!({
        "id": id,
        "message": "访问 Key 更新成功"
    })))
}

/// DELETE /admin/access-keys/:id
///
/// 删除访问 Key
pub async fn remove_access_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let removed = state.db.remove_access_key(id)?;
    if removed {
        state.access_token_bucket.unregister(id);
        tracing::info!("删除访问 Key: id={}", id);
        Ok(Json(json!({ "message": "访问 Key 已删除" })))
    } else {
        Err(AppError::BadRequest(format!("访问 Key ID {} 不存在", id)))
    }
}

/// POST /admin/access-keys/:id/toggle
///
/// 切换访问 Key 的启用/禁用状态
pub async fn toggle_access_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let keys = state.db.get_all_access_keys()?;
    let key = keys
        .iter()
        .find(|k| k.id == id)
        .ok_or_else(|| AppError::BadRequest(format!("访问 Key ID {} 不存在", id)))?;

    let new_status = if key.status == "active" {
        "disabled"
    } else {
        "active"
    };
    state.db.update_access_key_status(id, new_status)?;

    if new_status == "active" {
        let tpm = if key.tpm_limit > 0 {
            key.tpm_limit as u64
        } else {
            0
        };
        let rpm = if key.rpm_limit > 0 {
            key.rpm_limit as u64
        } else {
            0
        };
        state.access_token_bucket.register(id, tpm, rpm);
    } else {
        state.access_token_bucket.unregister(id);
    }

    tracing::info!("切换访问 Key {} 状态为 {}", id, new_status);
    Ok(Json(json!({
        "id": id,
        "status": new_status,
        "message": format!("访问 Key 已{}", if new_status == "active" { "启用" } else { "禁用" })
    })))
}

// ============================================================
// 号池 Key 连通性测试
// ============================================================

/// POST /admin/keys/:id/test
///
/// 测试号池 Key 的连通性和可用性。
/// 向上游发送一个轻量级请求 (GET /v1/models)，测量响应时间和状态。
pub async fn test_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let keys = state.db.get_all_keys()?;
    let key = keys
        .iter()
        .find(|k| k.id == id)
        .ok_or_else(|| AppError::BadRequest(format!("Key ID {} 不存在", id)))?;

    // 解密 API Key
    let decrypted = state
        .key_store
        .decrypt(&key.api_key)
        .map_err(|e| AppError::Internal(format!("解密 Key 失败: {}", e)))?;

    // 只测试已配置的端点。
    let openai_fut = test_configured_upstream(&state, key.openai_url.trim(), &decrypted, "openai");
    let claude_fut = test_configured_upstream(&state, key.claude_url.trim(), &decrypted, "claude");
    let (openai_result, claude_result) = tokio::join!(openai_fut, claude_fut);
    let openai_result = openai_result?;
    let claude_result = claude_result?;

    // 综合判断：只要有一个端点可用就认为 Key 可用
    let available = openai_result.as_ref().map(|r| r.success).unwrap_or(false)
        || claude_result.as_ref().map(|r| r.success).unwrap_or(false);

    Ok(Json(json!({
        "key_id": id,
        "platform": key.platform,
        "available": available,
        "openai": openai_result.map(|r| json!({
            "success": r.success,
            "latency_ms": r.latency_ms,
            "status": r.status,
            "error": r.error,
        })),
        "claude": claude_result.map(|r| json!({
            "success": r.success,
            "latency_ms": r.latency_ms,
            "status": r.status,
            "error": r.error,
        })),
    })))
}

/// 上游测试结果
struct TestResult {
    success: bool,
    latency_ms: u64,
    status: Option<u16>,
    error: Option<String>,
}

/// 向上游发送轻量级测试请求
///
/// 使用 GET /v1/models 端点验证连通性（这是最轻量的 API 调用）
async fn test_upstream(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    protocol: &str,
) -> Result<TestResult, AppError> {
    // 构建测试 URL: base_url + /models (去除前导空格和尾部斜杠)
    let test_url = format!("{}/models", base_url.trim().trim_end_matches('/'));

    let mut req = state.http_client.get(&test_url);

    // 设置认证头
    match protocol {
        "openai" => {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
        "claude" => {
            req = req.header("x-api-key", api_key);
            req = req.header("anthropic-version", "2023-06-01");
        }
        _ => {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
    }

    // 使用较短的测试超时 (10 秒)
    req = req.timeout(std::time::Duration::from_secs(10));

    let start = Instant::now();
    match req.send().await {
        Ok(response) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = response.status().as_u16();
            let success = response.status().is_success();

            // 读取响应体 (小请求，不会很大)
            let _body = response.text().await;

            Ok(TestResult {
                success,
                latency_ms: latency,
                status: Some(status),
                error: if success {
                    None
                } else {
                    Some(format!("HTTP {}", status))
                },
            })
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            let error_msg = if e.is_timeout() {
                "连接超时".to_string()
            } else if e.is_connect() {
                "连接失败".to_string()
            } else {
                format!("{}", e)
            };

            Ok(TestResult {
                success: false,
                latency_ms: latency,
                status: None,
                error: Some(error_msg),
            })
        }
    }
}

async fn test_configured_upstream(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    protocol: &str,
) -> Result<Option<TestResult>, AppError> {
    if base_url.trim().is_empty() {
        return Ok(None);
    }

    test_upstream(state, base_url, api_key, protocol)
        .await
        .map(Some)
}
