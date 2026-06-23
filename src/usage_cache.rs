use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio_util::sync::CancellationToken;

use crate::db::usage::UsageFlushEntry;
use crate::db::Database;
use crate::error::Result;

/// 内存中的用量缓存条目
#[derive(Debug, Clone, Default)]
struct UsageCacheEntry {
    request_count: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
}

/// 增量累计条目（刷盘时同步到 access_keys 表）
#[derive(Debug, Clone, Default)]
struct TotalIncrement {
    requests: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
}

/// 号池 key 缓存键: (key_id, model, hour_bucket)
type PoolKey = (i64, String, i64);
/// 访问 key 缓存键: (access_key_id, model, hour_bucket)
type AccessKey = (i64, String, i64);

/// 全局用量缓存
///
/// 请求完成时仅更新内存，后台定时任务每 60 秒刷盘到数据库。
pub struct UsageCache {
    /// 号池 key 维度: (key_id, model, hour_bucket) -> UsageCacheEntry
    pool_usage: RwLock<HashMap<PoolKey, UsageCacheEntry>>,
    /// 访问 key 维度: (access_key_id, model, hour_bucket) -> UsageCacheEntry
    access_usage: RwLock<HashMap<AccessKey, UsageCacheEntry>>,
    /// 访问 key 累计增量（刷盘时同步到 access_keys 表）
    access_totals: RwLock<HashMap<i64, TotalIncrement>>,
}

impl UsageCache {
    pub fn new() -> Self {
        Self {
            pool_usage: RwLock::new(HashMap::new()),
            access_usage: RwLock::new(HashMap::new()),
            access_totals: RwLock::new(HashMap::new()),
        }
    }

    /// 从数据库加载当前小时的缓存数据（启动时调用）
    ///
    /// **已禁用**: 加载后在下次 flush 时会导致数据翻倍（缓存值 + 数据库已有值被 UPSERT 累加）。
    /// 优雅关闭时已做最后一次 flush，最多丢失非正常关闭时 60 秒内的缓存数据。
    /// 如需恢复此功能，需在加载后删除数据库中当前小时的行，或改用增量追踪。
    pub fn load_from_db(&self, _db: &Database) -> Result<()> {
        tracing::info!("用量缓存跳过数据库加载（避免重启数据翻倍）");
        Ok(())
    }

    /// 记录一次请求的用量
    ///
    /// `count_request`: 是否计入请求次数。
    /// - orchestrator 调用时设为 true（每次请求只计一次）
    /// - SSE 帧提取调用时设为 false（只补充 token 数据，不重复计数）
    pub fn record(
        &self,
        pool_key_id: i64,
        access_key_id: Option<i64>,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        count_request: bool,
    ) {
        let hour_bucket = chrono::Utc::now().timestamp() / 3600;

        // 更新号池 key 缓存
        {
            let mut pool = self.pool_usage.write();
            let entry = pool
                .entry((pool_key_id, model.to_string(), hour_bucket))
                .or_default();
            if count_request {
                entry.request_count += 1;
            }
            entry.prompt_tokens += prompt_tokens as u64;
            entry.completion_tokens += completion_tokens as u64;
        }

        // 更新访问 key 缓存
        if let Some(access_id) = access_key_id {
            {
                let mut access = self.access_usage.write();
                let entry = access
                    .entry((access_id, model.to_string(), hour_bucket))
                    .or_default();
                if count_request {
                    entry.request_count += 1;
                }
                entry.prompt_tokens += prompt_tokens as u64;
                entry.completion_tokens += completion_tokens as u64;
            }

            // 更新访问 key 累计增量
            if count_request || prompt_tokens > 0 || completion_tokens > 0 {
                let mut totals = self.access_totals.write();
                let inc = totals.entry(access_id).or_default();
                if count_request {
                    inc.requests += 1;
                }
                inc.prompt_tokens += prompt_tokens as u64;
                inc.completion_tokens += completion_tokens as u64;
            }
        }
    }

    /// 将缓存刷盘到数据库
    ///
    /// 先取快照执行事务，成功后再清空内存，避免事务失败导致数据丢失。
    pub fn flush(&self, db: &Database) -> Result<()> {
        // 1. 取快照（不清空内存）
        let pool_snapshot = self.snapshot_pool();
        let access_snapshot = self.snapshot_access();
        let totals_snapshot = self.snapshot_access_totals();

        if pool_snapshot.is_empty() && access_snapshot.is_empty() && totals_snapshot.is_empty() {
            return Ok(());
        }

        // 2. 在事务中执行所有写入
        db.with_transaction(|conn| {
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

            // 刷盘 usage_hourly (pool)
            for entry in &pool_snapshot {
                conn.execute(
                    "INSERT INTO usage_hourly (dimension, key_id, model, hour_bucket, request_count, prompt_tokens, completion_tokens, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                     ON CONFLICT(dimension, key_id, model, hour_bucket) DO UPDATE SET
                        request_count = request_count + ?5,
                        prompt_tokens = prompt_tokens + ?6,
                        completion_tokens = completion_tokens + ?7,
                        updated_at = ?8",
                    rusqlite::params![
                        "pool",
                        entry.key_id,
                        entry.model,
                        entry.hour_bucket,
                        entry.request_count as i64,
                        entry.prompt_tokens as i64,
                        entry.completion_tokens as i64,
                        now,
                    ],
                )?;
            }

            // 刷盘 usage_hourly (access)
            for entry in &access_snapshot {
                conn.execute(
                    "INSERT INTO usage_hourly (dimension, key_id, model, hour_bucket, request_count, prompt_tokens, completion_tokens, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                     ON CONFLICT(dimension, key_id, model, hour_bucket) DO UPDATE SET
                        request_count = request_count + ?5,
                        prompt_tokens = prompt_tokens + ?6,
                        completion_tokens = completion_tokens + ?7,
                        updated_at = ?8",
                    rusqlite::params![
                        "access",
                        entry.key_id,
                        entry.model,
                        entry.hour_bucket,
                        entry.request_count as i64,
                        entry.prompt_tokens as i64,
                        entry.completion_tokens as i64,
                        now,
                    ],
                )?;
            }

            // 更新 access_keys 累计字段
            for (access_id, inc) in &totals_snapshot {
                conn.execute(
                    "UPDATE access_keys
                     SET total_requests = total_requests + ?1,
                         total_prompt_tokens = total_prompt_tokens + ?2,
                         total_completion_tokens = total_completion_tokens + ?3
                     WHERE id = ?4",
                    rusqlite::params![
                        inc.requests as i64,
                        inc.prompt_tokens as i64,
                        inc.completion_tokens as i64,
                        access_id,
                    ],
                )?;
            }

            Ok(())
        })?;

        // 3. 事务成功后再清空已刷盘的数据
        self.clear_flushed(&pool_snapshot, &access_snapshot, &totals_snapshot);

        tracing::debug!(
            "用量缓存刷盘完成: pool={}, access={}, totals={}",
            pool_snapshot.len(),
            access_snapshot.len(),
            totals_snapshot.len()
        );

        Ok(())
    }

    /// 取号池 key 缓存快照（不清空内存）
    fn snapshot_pool(&self) -> Vec<UsageFlushEntry> {
        let pool = self.pool_usage.read();
        pool.iter()
            .map(|((key_id, model, hour_bucket), entry)| UsageFlushEntry {
                dimension: "pool".to_string(),
                key_id: *key_id,
                model: model.clone(),
                hour_bucket: *hour_bucket,
                request_count: entry.request_count,
                prompt_tokens: entry.prompt_tokens,
                completion_tokens: entry.completion_tokens,
            })
            .collect()
    }

    /// 取访问 key 缓存快照（不清空内存）
    fn snapshot_access(&self) -> Vec<UsageFlushEntry> {
        let access = self.access_usage.read();
        access.iter()
            .map(|((key_id, model, hour_bucket), entry)| UsageFlushEntry {
                dimension: "access".to_string(),
                key_id: *key_id,
                model: model.clone(),
                hour_bucket: *hour_bucket,
                request_count: entry.request_count,
                prompt_tokens: entry.prompt_tokens,
                completion_tokens: entry.completion_tokens,
            })
            .collect()
    }

    /// 取访问 key 累计增量快照（不清空内存）
    fn snapshot_access_totals(&self) -> HashMap<i64, TotalIncrement> {
        let totals = self.access_totals.read();
        totals.iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    /// 事务成功后，从内存中减去已刷盘的数量
    fn clear_flushed(
        &self,
        pool_entries: &[UsageFlushEntry],
        access_entries: &[UsageFlushEntry],
        total_increments: &HashMap<i64, TotalIncrement>,
    ) {
        // 减去号池 key 已刷盘的数量
        {
            let mut pool = self.pool_usage.write();
            for entry in pool_entries {
                if let Some(existing) = pool.get_mut(&(entry.key_id, entry.model.clone(), entry.hour_bucket)) {
                    existing.request_count = existing.request_count.saturating_sub(entry.request_count);
                    existing.prompt_tokens = existing.prompt_tokens.saturating_sub(entry.prompt_tokens);
                    existing.completion_tokens = existing.completion_tokens.saturating_sub(entry.completion_tokens);
                    if existing.request_count == 0 && existing.prompt_tokens == 0 && existing.completion_tokens == 0 {
                        pool.remove(&(entry.key_id, entry.model.clone(), entry.hour_bucket));
                    }
                }
            }
        }

        // 减去访问 key 已刷盘的数量
        {
            let mut access = self.access_usage.write();
            for entry in access_entries {
                if let Some(existing) = access.get_mut(&(entry.key_id, entry.model.clone(), entry.hour_bucket)) {
                    existing.request_count = existing.request_count.saturating_sub(entry.request_count);
                    existing.prompt_tokens = existing.prompt_tokens.saturating_sub(entry.prompt_tokens);
                    existing.completion_tokens = existing.completion_tokens.saturating_sub(entry.completion_tokens);
                    if existing.request_count == 0 && existing.prompt_tokens == 0 && existing.completion_tokens == 0 {
                        access.remove(&(entry.key_id, entry.model.clone(), entry.hour_bucket));
                    }
                }
            }
        }

        // 减去访问 key 累计增量已刷盘的部分
        {
            let mut totals = self.access_totals.write();
            for (access_id, inc) in total_increments {
                if let Some(existing) = totals.get_mut(access_id) {
                    existing.requests = existing.requests.saturating_sub(inc.requests);
                    existing.prompt_tokens = existing.prompt_tokens.saturating_sub(inc.prompt_tokens);
                    existing.completion_tokens = existing.completion_tokens.saturating_sub(inc.completion_tokens);
                    if existing.requests == 0 && existing.prompt_tokens == 0 && existing.completion_tokens == 0 {
                        totals.remove(access_id);
                    }
                }
            }
        }
    }
}

/// 启动后台刷盘定时任务
///
/// 收到取消信号后会做最后一次刷盘再退出，确保数据不丢失。
pub fn start_flush_task(
    usage_cache: Arc<UsageCache>,
    db: Arc<Database>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = usage_cache.flush(&db) {
                        tracing::error!("用量缓存刷盘失败: {}", e);
                    }
                }
                _ = cancel.cancelled() => {
                    tracing::info!("用量缓存刷盘任务收到关闭信号，执行最后一次刷盘...");
                    if let Err(e) = usage_cache.flush(&db) {
                        tracing::error!("最终刷盘失败: {}", e);
                    } else {
                        tracing::info!("用量缓存最终刷盘完成");
                    }
                    break;
                }
            }
        }
    })
}
