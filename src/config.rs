use serde::Deserialize;
use std::path::Path;

use crate::error::{AppError, Result};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub encryption: EncryptionConfig,
    pub health: HealthConfig,
    pub scheduler: SchedulerConfig,
    pub rate_limit: RateLimitConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub proxy: ProxyConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// CORS 允许的源 (留空则不限制)
    #[serde(default)]
    pub cors_origin: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EncryptionConfig {
    /// AES-256-GCM 密钥 (32 bytes, base64 编码)
    pub master_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthConfig {
    pub check_interval_secs: u64,
    pub passive_failure_threshold: u32,
    pub passive_error_rate_threshold: f64,
    pub probe_timeout_secs: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerConfig {
    pub global_tpm: u64,
    pub global_rpm: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    pub default_tpm: u64,
    pub default_rpm: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout_secs: u64,
    pub half_open_probe_ratio: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub upstream_timeout_secs: u64,
    #[serde(default = "default_rate_limit_cooldown_secs")]
    pub rate_limit_cooldown_secs: u64,
    pub pool_size: usize,
}

fn default_rate_limit_cooldown_secs() -> u64 {
    30
}

/// 支持的平台列表
pub const VALID_PLATFORMS: &[&str] = &["xiaomi", "iflytek"];

impl Config {
    /// 从文件加载配置
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AppError::Config(format!("读取配置文件失败: {}", e)))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| AppError::Config(format!("解析配置文件失败: {}", e)))?;
        Ok(config)
    }

    /// 从文件加载配置，文件不存在则使用默认值
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            tracing::warn!("配置文件 {:?} 不存在，使用默认配置", path);
            Ok(Self::default())
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                cors_origin: String::new(),
            },
            database: DatabaseConfig {
                path: "./data/welfare.db".to_string(),
            },
            encryption: EncryptionConfig {
                master_key: String::new(),
            },
            health: HealthConfig {
                check_interval_secs: 300,
                passive_failure_threshold: 5,
                passive_error_rate_threshold: 0.5,
                probe_timeout_secs: 10,
            },
            scheduler: SchedulerConfig {
                global_tpm: 0,
                global_rpm: 0,
            },
            rate_limit: RateLimitConfig {
                default_tpm: 0,
                default_rpm: 0,
            },
            circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 5,
                recovery_timeout_secs: 60,
                half_open_probe_ratio: 0.1,
            },
            proxy: ProxyConfig {
                upstream_timeout_secs: 300,
                rate_limit_cooldown_secs: default_rate_limit_cooldown_secs(),
                pool_size: 100,
            },
        }
    }
}
