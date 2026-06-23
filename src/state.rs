use std::collections::HashMap;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::crypto::KeyStore;
use crate::db::{models::ApiKeyRecord, Database};
use crate::health_score_cache::HealthScoreCache;
use crate::scheduler::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerManager};
use crate::scheduler::cooldown::RateLimitCooldown;
use crate::scheduler::token_bucket::TokenBucketLimiter;
use crate::usage_cache::UsageCache;
use crate::db::models::ActiveKeyEntry;

/// 活跃密钥表 + 变更通知
pub struct ActiveKeysNotifier {
    /// request_id -> ActiveKeyEntry（Mutex 保护并发访问）
    active_keys: parking_lot::Mutex<HashMap<u64, ActiveKeyEntry>>,
    /// watch 通道发送端：每次变更时 send 通知 SSE handler
    /// SSE handler 通过 subscribe() 创建接收端监听变更
    change_tx: tokio::sync::watch::Sender<bool>,
}

impl ActiveKeysNotifier {
    pub fn new() -> Self {
        let (change_tx, _change_rx) = tokio::sync::watch::channel(false);
        Self {
            active_keys: parking_lot::Mutex::new(HashMap::new()),
            change_tx,
        }
    }

    /// 激活：记录一个正在使用 key 的请求
    pub fn activate(&self, request_id: u64, entry: ActiveKeyEntry) {
        self.active_keys.lock().insert(request_id, entry);
        let _ = self.change_tx.send(true);
    }

    /// 去激活：请求完成，移除记录
    pub fn deactivate(&self, request_id: u64) {
        self.active_keys.lock().remove(&request_id);
        let _ = self.change_tx.send(true);
    }

    /// 获取当前所有活跃条目的快照
    pub fn snapshot(&self) -> Vec<ActiveKeyEntry> {
        self.active_keys.lock().values().cloned().collect()
    }

    /// 订阅变更通知（每个 SSE 连接各持有一份）
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.change_tx.subscribe()
    }
}

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
    /// 用量统计缓存
    pub usage_cache: Arc<UsageCache>,
    /// 健康评分缓存（TTL 60秒）
    pub health_cache: Arc<HealthScoreCache>,
    /// 实时活跃密钥通知器
    pub active_keys_notifier: Arc<ActiveKeysNotifier>,
    /// 优雅关闭令牌：收到关闭信号时取消，通知所有 handler 中止长连接
    pub cancel_token: CancellationToken,
}

impl AppState {
    pub fn new(config: Config, db: Database, key_store: KeyStore, cancel_token: CancellationToken) -> Self {
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
            usage_cache: Arc::new(UsageCache::new()),
            health_cache: Arc::new(HealthScoreCache::new()),
            active_keys_notifier: Arc::new(ActiveKeysNotifier::new()),
            cancel_token,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(request_id: u64, key_id: i64, platform: &str, model: &str) -> ActiveKeyEntry {
        ActiveKeyEntry {
            request_id,
            key_id,
            key_name: format!("Key #{}", key_id),
            key_prefix: "sk-ab****cd".to_string(),
            platform: platform.to_string(),
            model: model.to_string(),
            started_at: 1718800000000,
        }
    }

    #[test]
    fn activate_and_snapshot() {
        let notifier = ActiveKeysNotifier::new();
        assert!(notifier.snapshot().is_empty());

        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));
        let snap = notifier.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].request_id, 1);
        assert_eq!(snap[0].key_id, 100);
        assert_eq!(snap[0].platform, "anthropic");
        assert_eq!(snap[0].model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn deactivate_removes_entry() {
        let notifier = ActiveKeysNotifier::new();
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));
        assert_eq!(notifier.snapshot().len(), 1);

        notifier.deactivate(1);
        assert!(notifier.snapshot().is_empty());
    }

    #[test]
    fn deactivate_nonexistent_is_noop() {
        let notifier = ActiveKeysNotifier::new();
        // 应该不会 panic
        notifier.deactivate(999);
        assert!(notifier.snapshot().is_empty());
    }

    #[test]
    fn activate_overwrites_same_request_id() {
        let notifier = ActiveKeysNotifier::new();
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));
        notifier.activate(1, make_entry(1, 200, "xiaomi", "gpt-4o"));

        let snap = notifier.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].key_id, 200);
        assert_eq!(snap[0].platform, "xiaomi");
    }

    #[test]
    fn multiple_concurrent_entries() {
        let notifier = ActiveKeysNotifier::new();
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));
        notifier.activate(2, make_entry(2, 101, "xiaomi", "gpt-4o"));
        notifier.activate(3, make_entry(3, 102, "iflytek", "spark-v3"));

        let snap = notifier.snapshot();
        assert_eq!(snap.len(), 3);

        // deactivate 中间那个
        notifier.deactivate(2);
        let snap = notifier.snapshot();
        assert_eq!(snap.len(), 2);
        let key_ids: Vec<i64> = snap.iter().map(|e| e.key_id).collect();
        assert!(key_ids.contains(&100));
        assert!(key_ids.contains(&102));
    }

    #[tokio::test]
    async fn subscribe_receives_change_on_activate() {
        let notifier = ActiveKeysNotifier::new();
        let mut rx = notifier.subscribe();

        // 初始值应该是 false
        assert!(!*rx.borrow_and_update());

        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));

        // rx.changed() 应该感知到变更
        let changed = rx.changed().await;
        assert!(changed.is_ok());
        assert!(*rx.borrow());
    }

    #[tokio::test]
    async fn subscribe_receives_change_on_deactivate() {
        let notifier = ActiveKeysNotifier::new();
        let mut rx = notifier.subscribe();

        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));
        let _ = rx.changed().await; // consume activate notification

        notifier.deactivate(1);
        let changed = rx.changed().await;
        assert!(changed.is_ok());
    }

    #[test]
    fn send_without_receiver_is_ok() {
        let notifier = ActiveKeysNotifier::new();
        // 初始 _change_rx 在 new() 中被 drop，所以没有 receiver
        // activate 不应该 panic
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));
        assert_eq!(notifier.snapshot().len(), 1);
    }

    #[tokio::test]
    async fn subscribe_after_initial_rx_dropped() {
        let notifier = ActiveKeysNotifier::new();
        // new() 中 _change_rx 已被 drop

        // 激活一个条目（此时无 receiver，send 返回 Err，但被忽略）
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));

        // 现在 subscribe 创建新 receiver
        let mut rx = notifier.subscribe();

        // 再次 activate，新的 rx 应该能收到通知
        notifier.activate(2, make_entry(2, 101, "xiaomi", "gpt-4o"));
        let changed = rx.changed().await;
        assert!(changed.is_ok());
    }

    #[test]
    fn snapshot_returns_cloned_data() {
        let notifier = ActiveKeysNotifier::new();
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));
        let snap = notifier.snapshot();

        // 修改 snapshot 不影响原始数据
        drop(snap);
        assert_eq!(notifier.snapshot().len(), 1);
    }

    // ============================================================
    // SSE stream 逻辑测试（模拟 active_keys_stream handler）
    // ============================================================

    /// 模拟 SSE handler 中 unfold stream 的逻辑，
    /// 验证 snapshot → update → update 的完整事件序列
    #[tokio::test]
    async fn sse_stream_produces_events_on_changes() {
        let notifier = Arc::new(ActiveKeysNotifier::new());

        // 先 activate 一个条目，让 snapshot 不为空
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));

        // 模拟 SSE handler 创建 stream
        let snapshot = notifier.snapshot();
        let snapshot_data = serde_json::to_string(&snapshot).unwrap();

        let watch_rx = notifier.subscribe();
        let notifier_clone = notifier.clone();

        let stream = futures::stream::unfold(
            (Some(snapshot_data), watch_rx, notifier_clone),
            |(initial, mut rx, notifier)| async move {
                if let Some(data) = initial {
                    return Some((data, (None, rx, notifier)));
                }

                if rx.changed().await.is_err() {
                    return None;
                }

                let current = notifier.snapshot();
                let data = serde_json::to_string(&current).unwrap_or_else(|_| "[]".to_string());
                Some((data, (None, rx, notifier)))
            },
        );

        use futures::StreamExt;
        let mut stream = Box::pin(stream);

        // 事件 1: snapshot（包含 key_id=100）
        let first = stream.next().await.unwrap();
        let entries: Vec<ActiveKeyEntry> = serde_json::from_str(&first).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key_id, 100);

        // 后台 activate key_id=101
        let n = notifier.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            n.activate(2, make_entry(2, 101, "xiaomi", "gpt-4o"));
        });

        // 事件 2: update（2 个活跃 key）
        let second = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .expect("timeout waiting for activate event")
            .unwrap();
        let entries: Vec<ActiveKeyEntry> = serde_json::from_str(&second).unwrap();
        assert_eq!(entries.len(), 2);

        // 后台 deactivate key_id=100
        let n = notifier.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            n.deactivate(1);
        });

        // 事件 3: update（只剩 key_id=101）
        let third = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .expect("timeout waiting for deactivate event")
            .unwrap();
        let entries: Vec<ActiveKeyEntry> = serde_json::from_str(&third).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key_id, 101);
    }

    /// 测试多个 SSE 客户端并行接收事件
    #[tokio::test]
    async fn multiple_sse_clients_receive_events() {
        let notifier = Arc::new(ActiveKeysNotifier::new());

        // 创建两个客户端
        let mut rx1 = notifier.subscribe();
        let mut rx2 = notifier.subscribe();

        // activate 一个条目
        notifier.activate(1, make_entry(1, 100, "anthropic", "claude-sonnet-4-20250514"));

        // 两个客户端都应该收到变更通知
        let r1 = tokio::time::timeout(std::time::Duration::from_secs(1), rx1.changed()).await;
        let r2 = tokio::time::timeout(std::time::Duration::from_secs(1), rx2.changed()).await;
        assert!(r1.is_ok());
        assert!(r2.is_ok());

        // 快照应该一致
        let snap = notifier.snapshot();
        assert_eq!(snap.len(), 1);
    }

    /// 测试快速连续 activate/deactivate 不导致死锁或崩溃
    #[tokio::test]
    async fn rapid_activate_deactivate() {
        let notifier = Arc::new(ActiveKeysNotifier::new());
        let mut rx = notifier.subscribe();

        // 快速连续 activate 和 deactivate
        for i in 1..=100u64 {
            notifier.activate(i, make_entry(i, i as i64, "anthropic", "model"));
            notifier.deactivate(i);
        }

        // 最终 snapshot 应该为空
        assert!(notifier.snapshot().is_empty());

        // watch 应该至少感知到一次变更
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), rx.changed()).await;
        assert!(result.is_ok());
    }

    /// 测试 SSE 事件的 JSON 格式正确性（包括中文字符）
    #[test]
    fn sse_event_json_format() {
        let notifier = ActiveKeysNotifier::new();

        let entry = ActiveKeyEntry {
            request_id: 42,
            key_id: 7,
            key_name: "测试密钥".to_string(),
            key_prefix: "sk-ab****cd".to_string(),
            platform: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            started_at: 1718800000123,
        };
        notifier.activate(42, entry);

        let snap = notifier.snapshot();
        let json = serde_json::to_string(&snap).unwrap();

        // 验证 JSON 可以正确反序列化
        let parsed: Vec<ActiveKeyEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].request_id, 42);
        assert_eq!(parsed[0].key_id, 7);
        assert_eq!(parsed[0].key_name, "测试密钥");
        assert_eq!(parsed[0].platform, "anthropic");
        assert_eq!(parsed[0].model, "claude-sonnet-4-20250514");
        assert_eq!(parsed[0].started_at, 1718800000123);

        // 验证 JSON 中包含中文（确保 UTF-8 正确）
        assert!(json.contains("测试密钥"));
    }

    /// 测试并发 activate/deactivate 的线程安全性
    #[tokio::test]
    async fn concurrent_activate_deactivate() {
        let notifier = Arc::new(ActiveKeysNotifier::new());
        let mut handles = vec![];

        // 启动 10 个并发任务，每个执行 100 次 activate/deactivate
        for task_id in 0..10u64 {
            let n = notifier.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..100u64 {
                    let request_id = task_id * 1000 + i;
                    n.activate(request_id, make_entry(request_id, request_id as i64, "anthropic", "model"));
                    // 短暂持有活跃状态
                    tokio::task::yield_now().await;
                    n.deactivate(request_id);
                }
            }));
        }

        // 等待所有任务完成
        for handle in handles {
            handle.await.unwrap();
        }

        // 所有任务完成后，活跃表应该为空
        let snap = notifier.snapshot();
        assert!(
            snap.is_empty(),
            "Expected empty active keys after all tasks complete, got {} entries",
            snap.len()
        );
    }
}
