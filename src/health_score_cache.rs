use std::sync::Mutex;
use std::time::Instant;

use crate::db::models::KeyHealthScore;
use crate::db::Database;
use crate::error::Result;

/// 健康评分缓存 TTL（秒）
const HEALTH_SCORE_CACHE_TTL_SECS: u64 = 60;

/// 健康评分缓存
///
/// 避免每次 API 请求都重新计算所有 key 的健康评分。
/// 缓存 TTL 为 60 秒，过期后下次请求触发重新计算。
/// 对于小规模号池（几十个 key），批量计算仅需 2 次 SQL，
/// 因此缓存主要减少高频刷新场景下的 DB 压力。
///
/// 设计要点:
/// - Mutex 只在读写缓存的元数据（computed_at, scores）时持有
/// - DB 查询在 Mutex 之外执行，不阻塞 tokio 异步线程
/// - 如果多个请求同时发现缓存过期，都会执行计算（无锁竞争），
///   但最终写入的是同一份数据，不会产生不一致
pub struct HealthScoreCache {
    inner: Mutex<CachedScores>,
}

struct CachedScores {
    scores: Vec<KeyHealthScore>,
    computed_at: Instant,
}

impl HealthScoreCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(CachedScores {
                scores: Vec::new(),
                computed_at: Instant::now()
                    .checked_sub(std::time::Duration::from_secs(HEALTH_SCORE_CACHE_TTL_SECS + 1))
                    .unwrap_or(Instant::now()),
            }),
        }
    }

    /// 获取缓存的评分，如果过期则重新计算
    ///
    /// 注意: 此方法包含阻塞的 DB 查询，在 async 上下文中应通过
    /// `tokio::task::spawn_blocking` 调用。
    pub fn get_or_compute(&self, db: &Database) -> Result<Vec<KeyHealthScore>> {
        // 1. 快速检查缓存是否有效（临界区极短）
        {
            let cache = self.inner.lock().unwrap();
            if cache.computed_at.elapsed().as_secs() < HEALTH_SCORE_CACHE_TTL_SECS {
                return Ok(cache.scores.clone());
            }
        }

        // 2. 在 Mutex 之外执行可能耗时的 DB 查询
        let scores = db.compute_all_keys_health_scores()?;

        // 3. 写入缓存（临界区极短）
        {
            let mut cache = self.inner.lock().unwrap();
            cache.scores = scores.clone();
            cache.computed_at = Instant::now();
        }

        Ok(scores)
    }

    /// 强制使缓存失效（例如在 key 增删或状态变更后调用）
    pub fn invalidate(&self) {
        let mut cache = self.inner.lock().unwrap();
        cache.computed_at = Instant::now()
            .checked_sub(std::time::Duration::from_secs(HEALTH_SCORE_CACHE_TTL_SECS + 1))
            .unwrap_or(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{ScoreSource, StatusLabel};

    fn make_score(key_id: i64, score: u8) -> KeyHealthScore {
        KeyHealthScore {
            key_id,
            key_name: format!("Key-{}", key_id),
            health_score: score,
            score_source: if score > 0 { ScoreSource::Realtime } else { ScoreSource::NoData },
            status_label: if score > 0 { StatusLabel::Normal } else { StatusLabel::NoData },
            sample_count: if score > 0 { 20 } else { 0 },
            low_confidence: false,
        }
    }

    #[test]
    fn cache_initial_state_is_expired() {
        let cache = HealthScoreCache::new();
        let inner = cache.inner.lock().unwrap();
        // 初始状态: computed_at 设置为 TTL+1 秒前，即已过期
        assert!(inner.computed_at.elapsed().as_secs() >= HEALTH_SCORE_CACHE_TTL_SECS);
        assert!(inner.scores.is_empty());
    }

    #[test]
    fn invalidate_resets_timestamp_to_expired() {
        let cache = HealthScoreCache::new();
        // 模拟已缓存数据（标记为刚计算）
        {
            let mut inner = cache.inner.lock().unwrap();
            inner.computed_at = Instant::now();
            inner.scores = vec![make_score(1, 85)];
        }
        // invalidate 后应标记为过期
        cache.invalidate();
        let inner = cache.inner.lock().unwrap();
        assert!(inner.computed_at.elapsed().as_secs() >= HEALTH_SCORE_CACHE_TTL_SECS);
    }

    #[test]
    fn invalidate_does_not_clear_scores() {
        // invalidate 只使缓存过期，不清空数据
        // 下次 get_or_compute 会因为过期而重新计算
        let cache = HealthScoreCache::new();
        {
            let mut inner = cache.inner.lock().unwrap();
            inner.computed_at = Instant::now();
            inner.scores = vec![make_score(1, 85)];
        }
        cache.invalidate();
        let inner = cache.inner.lock().unwrap();
        // 数据仍在，只是时间戳已过期
        assert_eq!(inner.scores.len(), 1);
    }

    #[test]
    fn cache_ttl_is_60_seconds() {
        assert_eq!(HEALTH_SCORE_CACHE_TTL_SECS, 60);
    }

    #[test]
    fn multiple_invalidates_are_idempotent() {
        let cache = HealthScoreCache::new();
        cache.invalidate();
        cache.invalidate();
        cache.invalidate();
        // 不应 panic，且状态一致
        let inner = cache.inner.lock().unwrap();
        assert!(inner.computed_at.elapsed().as_secs() >= HEALTH_SCORE_CACHE_TTL_SECS);
    }

    #[test]
    fn score_struct_equality() {
        // 验证 KeyHealthScore 的 Clone 和 PartialEq (通过字段逐一比较)
        let a = make_score(1, 85);
        let b = make_score(1, 85);
        assert_eq!(a.key_id, b.key_id);
        assert_eq!(a.health_score, b.health_score);
        assert_eq!(a.score_source, b.score_source);
        assert_eq!(a.status_label, b.status_label);
        assert_eq!(a.sample_count, b.sample_count);
        assert_eq!(a.low_confidence, b.low_confidence);
    }
}
