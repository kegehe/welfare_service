use std::sync::Arc;

use crate::config::Config;
use crate::crypto::KeyStore;
use crate::db::{models::ApiKeyRecord, Database};
use crate::scheduler::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerManager};
use crate::scheduler::cooldown::RateLimitCooldown;
use crate::scheduler::token_bucket::TokenBucketLimiter;

/// 全局应用状态
///
/// 通过 Axum 的 State extractor 注入到所有 handler
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<Database>,
    pub key_store: Arc<KeyStore>,
    pub http_client: reqwest::Client,
    pub token_bucket: Arc<TokenBucketLimiter>,
    pub circuit_breaker: Arc<CircuitBreakerManager>,
    pub rate_limit_cooldown: Arc<RateLimitCooldown>,
    /// 访问 Key 的限流器 (独立于号池 Key 的限流)
    pub access_token_bucket: Arc<TokenBucketLimiter>,
}

impl AppState {
    pub fn new(config: Config, db: Database, key_store: KeyStore) -> Self {
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(config.proxy.pool_size)
            .timeout(std::time::Duration::from_secs(
                config.proxy.upstream_timeout_secs,
            ))
            .build()
            .expect("创建 HTTP 客户端失败");

        let cb_config = CircuitBreakerConfig {
            failure_threshold: config.circuit_breaker.failure_threshold,
            recovery_timeout_secs: config.circuit_breaker.recovery_timeout_secs,
            half_open_probe_ratio: config.circuit_breaker.half_open_probe_ratio,
        };
        let rate_limit_cooldown_secs = config.proxy.rate_limit_cooldown_secs;

        Self {
            config: Arc::new(config),
            db: Arc::new(db),
            key_store: Arc::new(key_store),
            http_client,
            token_bucket: Arc::new(TokenBucketLimiter::new()),
            circuit_breaker: Arc::new(CircuitBreakerManager::new(cb_config)),
            rate_limit_cooldown: Arc::new(RateLimitCooldown::new(rate_limit_cooldown_secs)),
            access_token_bucket: Arc::new(TokenBucketLimiter::new()),
        }
    }

    pub fn effective_pool_limits(&self, key: &ApiKeyRecord) -> (u64, u64) {
        let tpm = if key.tpm_limit > 0 {
            key.tpm_limit as u64
        } else {
            self.config.rate_limit.default_tpm
        };
        let rpm = if key.rpm_limit > 0 {
            key.rpm_limit as u64
        } else {
            self.config.rate_limit.default_rpm
        };
        (tpm, rpm)
    }

    pub fn register_pool_key(&self, key: &ApiKeyRecord) {
        let (tpm, rpm) = self.effective_pool_limits(key);
        self.token_bucket.register(key.id, tpm, rpm);
        self.circuit_breaker.register(key.id);
    }

    pub fn unregister_pool_key(&self, key_id: i64) {
        self.token_bucket.unregister(key_id);
        self.circuit_breaker.unregister(key_id);
        self.rate_limit_cooldown.unregister(key_id);
    }

    pub fn set_pool_key_active(&self, key: &ApiKeyRecord) {
        self.register_pool_key(key);
    }

    pub fn set_pool_key_disabled(&self, key_id: i64) {
        self.unregister_pool_key(key_id);
    }
}
