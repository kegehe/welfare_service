use rand::seq::SliceRandom;

use crate::db::models::ApiKeyRecord;
use crate::error::{AppError, Result};
use crate::proxy::Protocol;
use crate::state::AppState;

/// 一次调度得到的候选 Key，以及请求发往上游时应使用的真实模型名。
#[derive(Debug, Clone)]
pub struct KeyCandidate {
    pub key: ApiKeyRecord,
    pub upstream_model: String,
    pub matched_requested_model: bool,
}

/// Key 选择器
///
/// 从活跃 Key 池中选择一个可用的 Key 处理请求。
/// 选择策略:
/// 1. 过滤不可用 Key（熔断、冷却、协议不匹配、未配置模型）
/// 2. 优先选择精确支持目标模型的 Key
/// 3. 如果没有精确匹配，则自动映射到号池 Key 配置的第一个可用模型
/// 4. 随机打散候选
pub struct KeySelector<'a> {
    state: &'a AppState,
}

impl<'a> KeySelector<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// 获取当前可用候选 Key。返回结果会被随机打散，调用方可按顺序尝试。
    pub fn candidates(
        &self,
        model: &str,
        protocol: Protocol,
    ) -> Result<Vec<KeyCandidate>> {
        let keys = self.state.db.get_active_keys()?;
        self.candidates_from_keys(keys, model, protocol)
    }

    fn candidates_from_keys(
        &self,
        keys: Vec<ApiKeyRecord>,
        requested_model: &str,
        protocol: Protocol,
    ) -> Result<Vec<KeyCandidate>> {
        if keys.is_empty() {
            return Err(AppError::NoAvailableKey);
        }

        let requested_model = requested_model.trim();

        let available: Vec<(ApiKeyRecord, Vec<String>)> = keys
            .into_iter()
            .filter_map(|k| {
                // 检查熔断器
                if !self.state.circuit_breaker.is_allowed(k.id) {
                    return None;
                }
                if !self.state.rate_limit_cooldown.is_allowed(k.id) {
                    return None;
                }
                if !supports_protocol(&k, protocol) {
                    return None;
                }
                let models = parse_models(&k.models);
                if models.is_empty() {
                    return None;
                }
                Some((k, models))
            })
            .collect();

        let mut exact_candidates: Vec<KeyCandidate> = available
            .iter()
            .filter_map(|(key, models)| {
                let matched = models
                    .iter()
                    .find(|model| model.as_str() == requested_model)?;
                Some(KeyCandidate {
                    key: key.clone(),
                    upstream_model: matched.clone(),
                    matched_requested_model: true,
                })
            })
            .collect();

        if !exact_candidates.is_empty() {
            exact_candidates.shuffle(&mut rand::thread_rng());
            return Ok(exact_candidates);
        }

        let mut candidates: Vec<KeyCandidate> = available
            .into_iter()
            .map(|(key, models)| {
                let upstream_model = models
                    .first()
                    .cloned()
                    .expect("available candidates always have at least one model");
                KeyCandidate {
                    key,
                    upstream_model,
                    matched_requested_model: false,
                }
            })
            .collect();

        if candidates.is_empty() {
            return Err(AppError::NoAvailableKey);
        }

        candidates.shuffle(&mut rand::thread_rng());
        Ok(candidates)
    }

    /// 选择一个可用的 Key
    #[allow(dead_code)]
    pub fn select_key(&self, model: &str) -> Result<ApiKeyRecord> {
        self.candidates(model, Protocol::OpenAI)?
            .into_iter()
            .map(|candidate| candidate.key)
            .next()
            .ok_or(AppError::NoAvailableKey)
    }
}

fn parse_models(models_json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(models_json)
        .unwrap_or_default()
        .into_iter()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .collect()
}

fn supports_protocol(key: &ApiKeyRecord, protocol: Protocol) -> bool {
    match protocol {
        Protocol::OpenAI => !key.openai_url.trim().is_empty(),
        Protocol::Claude => !key.claude_url.trim().is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_models, supports_protocol, KeySelector};
    use crate::config::Config;
    use crate::crypto::KeyStore;
    use crate::db::Database;
    use crate::db::models::ApiKeyRecord;
    use crate::proxy::Protocol;
    use crate::state::AppState;
    use tokio_util::sync::CancellationToken;

    fn make_key(id: i64, models: &str, claude_url: &str) -> ApiKeyRecord {
        ApiKeyRecord {
            id,
            platform: "test".to_string(),
            name: String::new(),
            api_key: "encrypted".to_string(),
            openai_url: String::new(),
            claude_url: claude_url.to_string(),
            models: models.to_string(),
            tpm_limit: 0,
            rpm_limit: 0,
            status: "active".to_string(),
            source: None,
            note: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn parse_models_trims_and_drops_empty_values() {
        assert_eq!(
            parse_models(r#"[" gpt-4 ",""," claude "]"#),
            vec!["gpt-4".to_string(), "claude".to_string()]
        );
    }

    #[test]
    fn parse_models_tolerates_invalid_json() {
        assert!(parse_models("not-json").is_empty());
    }

    #[test]
    fn supports_only_configured_protocol_urls() {
        let mut key = make_key(1, "[]", "");
        key.openai_url = "https://openai.test/v1".to_string();

        assert!(supports_protocol(&key, Protocol::OpenAI));
        assert!(!supports_protocol(&key, Protocol::Claude));
    }

    #[test]
    fn tools_requests_fallback_to_key_configured_model() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "welfare_selector_tools_fallback_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let db = Database::open(&path).unwrap();
        let key_store = KeyStore::from_base64_key(&KeyStore::generate_key()).unwrap();
        let state = AppState::new(Config::default(), db, key_store, CancellationToken::new());
        state.register_pool_key(&make_key(1, r#"["mimo-v2.5-pro"]"#, "https://claude.test"));

        let selector = KeySelector::new(&state);
        let candidates = selector
            .candidates_from_keys(
                vec![make_key(1, r#"["mimo-v2.5-pro"]"#, "https://claude.test")],
                "claude-opus-4-8",
                Protocol::Claude,
            )
            .unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].upstream_model, "mimo-v2.5-pro");
        assert!(!candidates[0].matched_requested_model);

        let _ = std::fs::remove_file(path);
    }
}
