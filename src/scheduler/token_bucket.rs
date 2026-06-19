use std::collections::HashMap;

use chrono::Utc;
use parking_lot::RwLock;

/// 令牌桶限流器
///
/// 每个 Key 有独立的 TPM 和 RPM 令牌桶。
/// 每秒自动补充令牌，请求时消耗令牌。
/// limit=0 表示不限制。
pub struct TokenBucketLimiter {
    /// Key ID -> (tpm_bucket, rpm_bucket)
    buckets: RwLock<HashMap<i64, (TokenBucket, TokenBucket)>>,
}

struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_rate: f64, // 每秒补充的令牌数
    last_refill: chrono::DateTime<Utc>,
}

impl TokenBucket {
    fn new(capacity: u64) -> Self {
        Self {
            capacity: capacity as f64,
            tokens: capacity as f64,
            refill_rate: capacity as f64 / 60.0, // TPM -> 每秒补充量
            last_refill: Utc::now(),
        }
    }

    /// 创建一个无限制的令牌桶
    fn unlimited() -> Self {
        Self {
            capacity: f64::MAX,
            tokens: f64::MAX,
            refill_rate: 0.0,
            last_refill: Utc::now(),
        }
    }

    /// 是否无限制
    fn is_unlimited(&self) -> bool {
        self.capacity == f64::MAX
    }

    /// 尝试消耗一个令牌
    fn try_acquire(&mut self, tokens: f64) -> bool {
        if self.is_unlimited() {
            return true;
        }
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    /// 补充令牌
    fn refill(&mut self) {
        let now = Utc::now();
        let elapsed = (now - self.last_refill).num_milliseconds() as f64 / 1000.0;
        if elapsed > 0.0 {
            self.tokens = (self.tokens + self.refill_rate * elapsed).min(self.capacity);
            self.last_refill = now;
        }
    }

    /// 获取剩余令牌数
    /// 返回 None 表示无限制
    #[allow(dead_code)]
    fn remaining(&self) -> Option<u64> {
        if self.is_unlimited() {
            return None;
        }
        Some(self.tokens as u64)
    }
}

impl TokenBucketLimiter {
    pub fn new() -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
        }
    }

    /// 注册一个 Key 的令牌桶
    /// tpm_limit=0 或 rpm_limit=0 表示不限制
    pub fn register(&self, key_id: i64, tpm_limit: u64, rpm_limit: u64) {
        let mut buckets = self.buckets.write();
        let tpm = if tpm_limit == 0 {
            TokenBucket::unlimited()
        } else {
            TokenBucket::new(tpm_limit)
        };
        let rpm = if rpm_limit == 0 {
            TokenBucket::unlimited()
        } else {
            TokenBucket::new(rpm_limit)
        };
        buckets.insert(key_id, (tpm, rpm));
    }

    /// 移除一个 Key 的令牌桶
    pub fn unregister(&self, key_id: i64) {
        let mut buckets = self.buckets.write();
        buckets.remove(&key_id);
    }

    /// 尝试为指定 Key 消耗令牌
    ///
    /// estimated_tokens: 预估本次请求消耗的 token 数 (用于 TPM 限流)
    /// 返回 true 表示允许请求，false 表示限流
    pub fn try_acquire(&self, key_id: i64, estimated_tokens: u64) -> bool {
        let mut buckets = self.buckets.write();
        if let Some((tpm, rpm)) = buckets.get_mut(&key_id) {
            // TPM: 按预估 token 数消耗
            if !tpm.try_acquire(estimated_tokens as f64) {
                return false;
            }
            // RPM: 每次请求消耗 1 个令牌
            if !rpm.try_acquire(1.0) {
                // 回滚 TPM
                if !tpm.is_unlimited() {
                    tpm.tokens = (tpm.tokens + estimated_tokens as f64).min(tpm.capacity);
                }
                return false;
            }
            true
        } else {
            // 未注册的 Key 不限流
            true
        }
    }

    /// 获取指定 Key 的剩余配额
    /// 返回 (Option<tpm>, Option<rpm>)，None 表示不限流或未注册
    #[allow(dead_code)]
    pub fn remaining(&self, key_id: i64) -> (Option<u64>, Option<u64>) {
        let buckets = self.buckets.read();
        if let Some((tpm, rpm)) = buckets.get(&key_id) {
            (tpm.remaining(), rpm.remaining())
        } else {
            (None, None)
        }
    }
}
