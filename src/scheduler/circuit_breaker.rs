use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// 熔断器状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// 正常: 允许所有请求
    Closed,
    /// 打开: 拒绝所有请求
    Open,
    /// 半开: 允许少量探测请求
    HalfOpen,
}

/// 熔断器配置
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 连续失败多少次后打开熔断器
    pub failure_threshold: u32,
    /// 熔断器打开后等待多久进入半开状态 (秒)
    pub recovery_timeout_secs: u64,
    /// 半开状态下允许的探测请求比例 (0.0-1.0)
    pub half_open_probe_ratio: f64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_secs: 60,
            half_open_probe_ratio: 0.1,
        }
    }
}

/// 单个 Key 的熔断器
struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    last_failure_at: Option<DateTime<Utc>>,
    opened_at: Option<DateTime<Utc>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            last_failure_at: None,
            opened_at: None,
            config,
        }
    }

    /// 检查是否允许请求通过
    fn is_allowed(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // 检查是否到了恢复时间
                if let Some(opened_at) = self.opened_at {
                    let elapsed = Utc::now() - opened_at;
                    if elapsed.num_seconds() >= self.config.recovery_timeout_secs as i64 {
                        self.state = CircuitState::HalfOpen;
                        return true; // 允许一个探测请求
                    }
                }
                false
            }
            CircuitState::HalfOpen => {
                // 半开状态下按比例允许
                rand::random::<f64>() < self.config.half_open_probe_ratio
            }
        }
    }

    /// 记录成功
    fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                // 半开状态下成功，恢复到关闭状态
                self.state = CircuitState::Closed;
                self.failure_count = 0;
                self.last_failure_at = None;
                self.opened_at = None;
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::Open => {
                // 不应该在 Open 状态收到成功
            }
        }
    }

    /// 记录失败
    fn record_failure(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                self.last_failure_at = Some(Utc::now());
                if self.failure_count >= self.config.failure_threshold {
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Utc::now());
                    tracing::warn!("熔断器打开: 连续失败 {} 次", self.failure_count);
                }
            }
            CircuitState::HalfOpen => {
                // 半开状态下失败，重新打开
                self.state = CircuitState::Open;
                self.opened_at = Some(Utc::now());
                self.failure_count += 1;
                self.last_failure_at = Some(Utc::now());
            }
            CircuitState::Open => {
                // 已经是打开状态
            }
        }
    }

    /// 获取当前状态
    #[allow(dead_code)]
    fn state(&self) -> &CircuitState {
        &self.state
    }

    /// 获取连续失败次数
    #[allow(dead_code)]
    fn failure_count(&self) -> u32 {
        self.failure_count
    }
}

/// 熔断器管理器
pub struct CircuitBreakerManager {
    breakers: RwLock<HashMap<i64, CircuitBreaker>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// 注册一个 Key 的熔断器
    pub fn register(&self, key_id: i64) {
        let mut breakers = self.breakers.write();
        breakers.insert(key_id, CircuitBreaker::new(self.config.clone()));
    }

    /// 移除一个 Key 的熔断器
    pub fn unregister(&self, key_id: i64) {
        let mut breakers = self.breakers.write();
        breakers.remove(&key_id);
    }

    /// 检查指定 Key 是否允许请求
    pub fn is_allowed(&self, key_id: i64) -> bool {
        let mut breakers = self.breakers.write();
        if let Some(breaker) = breakers.get_mut(&key_id) {
            breaker.is_allowed()
        } else {
            true // 未注册视为允许
        }
    }

    /// 记录请求成功
    pub fn record_success(&self, key_id: i64) {
        let mut breakers = self.breakers.write();
        if let Some(breaker) = breakers.get_mut(&key_id) {
            breaker.record_success();
        }
    }

    /// 记录请求失败
    pub fn record_failure(&self, key_id: i64) {
        let mut breakers = self.breakers.write();
        if let Some(breaker) = breakers.get_mut(&key_id) {
            breaker.record_failure();
        }
    }

    /// 获取指定 Key 的熔断器状态
    #[allow(dead_code)]
    pub fn state(&self, key_id: i64) -> CircuitState {
        let breakers = self.breakers.read();
        if let Some(breaker) = breakers.get(&key_id) {
            breaker.state().clone()
        } else {
            CircuitState::Closed
        }
    }

    /// 获取指定 Key 的连续失败次数
    #[allow(dead_code)]
    pub fn failure_count(&self, key_id: i64) -> u32 {
        let breakers = self.breakers.read();
        if let Some(breaker) = breakers.get(&key_id) {
            breaker.failure_count()
        } else {
            0
        }
    }
}
