use rusqlite::params;

use crate::db::Database;
use crate::db::models::{KeyHealthScore, ScoreSource, StatusLabel};
use crate::error::Result;

impl Database {
    /// 计算指定 Key 的健康评分（混合方式）
    ///
    /// 优先使用实时成功率（样本≥20），不足时回退到24h窗口，都没有则标记无数据。
    /// 健康评分衡量近期可用性，429 限流会降低评分；自动下线逻辑仍单独排除 429。
    ///
    /// 注: 批量场景优先使用 compute_all_keys_health_scores()，此方法保留用于单 key 查询。
    #[allow(dead_code)]
    pub fn compute_key_health_score(&self, key_id: i64) -> Result<KeyHealthScore> {
        // 1. 尝试实时成功率（包含 429，用于展示限流降级）
        let health_stats = self.get_key_health_stats(key_id, 20)?;
        if health_stats.total >= 20 {
            let score = (health_stats.success_rate * 100.0)
                .round()
                .clamp(0.0, 100.0) as u8;
            return Ok(KeyHealthScore {
                key_id,
                key_name: String::new(),
                health_score: score,
                score_source: ScoreSource::Realtime,
                status_label: score_to_label(score, ScoreSource::Realtime),
                sample_count: health_stats.total,
                low_confidence: false,
            });
        }

        // 2. 回退到24h窗口（包含 429，用于展示限流降级）
        if let Some((rate, count)) = self.get_key_window_success_rate(key_id, 24)? {
            let score = (rate * 100.0).round().clamp(0.0, 100.0) as u8;
            return Ok(KeyHealthScore {
                key_id,
                key_name: String::new(),
                health_score: score,
                score_source: ScoreSource::Window,
                status_label: score_to_label(score, ScoreSource::Window),
                sample_count: count,
                // Window 来源且样本 < 5 → 低置信度
                low_confidence: count < 5,
            });
        }

        // 3. 无数据
        Ok(KeyHealthScore {
            key_id,
            key_name: String::new(),
            health_score: 0,
            score_source: ScoreSource::NoData,
            status_label: StatusLabel::NoData,
            sample_count: 0,
            low_confidence: false,
        })
    }

    /// 批量计算所有 Key 的健康评分
    ///
    /// 优化策略：
    /// - 24h 窗口统计：单条批量 SQL（无需 LIMIT，天然是全量聚合）
    /// - 实时最近 N 条统计：对每个 key 执行单独查询（带 LIMIT）
    /// - 合并：优先使用实时数据（样本≥20），否则回退到窗口数据
    ///
    /// 注意：不使用关联子查询（WHERE id IN (SELECT ... LIMIT 20)），
    /// 因为在 SQLite 中其复杂度为 O(N²)，50K 行时耗时 > 100s。
    /// 逐 key 查询虽为 N 次 SQL，但每次有索引加持 + LIMIT 20，总体远快于关联子查询。
    pub fn compute_all_keys_health_scores(&self) -> Result<Vec<KeyHealthScore>> {
        let conn = self.conn();

        // 获取所有 key 的 id 和 name
        let mut stmt_keys = conn.prepare("SELECT id, name FROM api_keys ORDER BY id")?;
        let keys: Vec<(i64, String)> = stmt_keys
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // 批量查询: 24h 窗口统计（无需 LIMIT，单条 SQL 即可）
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::TimeDelta::hours(24))
            .ok_or_else(|| crate::error::AppError::Internal("时间窗口计算溢出".to_string()))?
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let mut stmt_window = conn.prepare(
            "SELECT key_id,
                    COUNT(*) as total,
                    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success
             FROM request_logs
             WHERE affects_key_health = 1
               AND created_at >= ?1
               AND key_id IN (SELECT id FROM api_keys)
             GROUP BY key_id",
        )?;

        let window_map: std::collections::HashMap<i64, (u32, f64)> = stmt_window
            .query_map(params![cutoff], |row| {
                let key_id: i64 = row.get(0)?;
                let total: i64 = row.get(1)?;
                let success: Option<i64> = row.get(2)?;
                let success_val = success.unwrap_or(0) as f64;
                let rate = if total > 0 {
                    success_val / total as f64
                } else {
                    1.0
                };
                Ok((key_id, (total as u32, rate)))
            })?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("批量窗口统计行解析失败: {}", e);
                    None
                }
            })
            .collect();

        // 预编译实时统计查询（带 LIMIT 20，按 key 逐个查询）
        let mut stmt_realtime = conn.prepare(
            "SELECT COUNT(*) as total,
                    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success
             FROM (
                SELECT is_success FROM request_logs
                WHERE key_id = ?1 AND affects_key_health = 1
                ORDER BY created_at DESC, id DESC LIMIT 20
             )",
        )?;

        // 逐 key 查询实时统计，与窗口数据合并
        let mut scores = Vec::with_capacity(keys.len());
        for (key_id, key_name) in &keys {
            // 实时统计（预编译语句，按 key 查询）
            let realtime_result = stmt_realtime
                .query_row(params![key_id], |row| {
                    let total: i64 = row.get(0)?;
                    let success: Option<i64> = row.get(1)?;
                    Ok((total as u32, success.unwrap_or(0) as f64))
                })
                .ok();

            let score = if let Some((total, success_val)) = realtime_result {
                if total >= 20 {
                    let rate = success_val / total as f64;
                    let s = (rate * 100.0).round().clamp(0.0, 100.0) as u8;
                    KeyHealthScore {
                        key_id: *key_id,
                        key_name: key_name.clone(),
                        health_score: s,
                        score_source: ScoreSource::Realtime,
                        status_label: score_to_label(s, ScoreSource::Realtime),
                        sample_count: total,
                        low_confidence: false,
                    }
                } else if let Some((w_total, w_rate)) = window_map.get(key_id) {
                    let s = (w_rate * 100.0).round().clamp(0.0, 100.0) as u8;
                    KeyHealthScore {
                        key_id: *key_id,
                        key_name: key_name.clone(),
                        health_score: s,
                        score_source: ScoreSource::Window,
                        status_label: score_to_label(s, ScoreSource::Window),
                        sample_count: *w_total,
                        low_confidence: *w_total < 5,
                    }
                } else {
                    KeyHealthScore {
                        key_id: *key_id,
                        key_name: key_name.clone(),
                        health_score: 0,
                        score_source: ScoreSource::NoData,
                        status_label: StatusLabel::NoData,
                        sample_count: 0,
                        low_confidence: false,
                    }
                }
            } else if let Some((w_total, w_rate)) = window_map.get(key_id) {
                let s = (w_rate * 100.0).round().clamp(0.0, 100.0) as u8;
                KeyHealthScore {
                    key_id: *key_id,
                    key_name: key_name.clone(),
                    health_score: s,
                    score_source: ScoreSource::Window,
                    status_label: score_to_label(s, ScoreSource::Window),
                    sample_count: *w_total,
                    low_confidence: *w_total < 5,
                }
            } else {
                KeyHealthScore {
                    key_id: *key_id,
                    key_name: key_name.clone(),
                    health_score: 0,
                    score_source: ScoreSource::NoData,
                    status_label: StatusLabel::NoData,
                    sample_count: 0,
                    low_confidence: false,
                }
            };
            scores.push(score);
        }

        Ok(scores)
    }

    /// 查询指定 Key 在最近 N 小时内的成功率
    ///
    /// 返回 (success_rate, sample_count)，无记录返回 None
    /// 注: 批量场景优先使用 compute_all_keys_health_scores()，此方法保留用于单 key 查询。
    #[allow(dead_code)]
    pub fn get_key_window_success_rate(
        &self,
        key_id: i64,
        hours: u32,
    ) -> Result<Option<(f64, u32)>> {
        let conn = self.conn();
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::TimeDelta::hours(hours as i64))
            .ok_or_else(|| crate::error::AppError::Internal("时间窗口计算溢出".to_string()))?
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let (total, success) = conn.query_row(
            "SELECT COUNT(*) as total,
                    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success
             FROM request_logs
             WHERE key_id = ?1 AND affects_key_health = 1 AND created_at >= ?2",
            params![key_id, cutoff],
            |row| {
                let total: i64 = row.get(0)?;
                let success: Option<i64> = row.get(1)?;
                Ok((total, success.unwrap_or(0)))
            },
        )?;

        if total > 0 {
            Ok(Some((success as f64 / total as f64, total as u32)))
        } else {
            Ok(None)
        }
    }
}

/// 根据评分和来源确定状态标签
fn score_to_label(score: u8, source: ScoreSource) -> StatusLabel {
    if source == ScoreSource::NoData {
        return StatusLabel::NoData;
    }
    match score {
        80..=100 => StatusLabel::Normal,
        50..=79 => StatusLabel::LightThrottled,
        20..=49 => StatusLabel::HeavyThrottled,
        0..=19 => StatusLabel::Critical,
        // u8 不会超过 100（已 clamp），但编译器需要穷尽匹配
        _ => StatusLabel::Critical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::db::logs::RequestLogInput;

    /// 创建临时测试数据库并插入一个 key（id=1）
    fn test_db() -> Database {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!(
            "welfare-service-hs-test-{}-{}.db",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_file(&path);
        let db = Database::open(&path).unwrap();
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO api_keys (id, platform, name, api_key, key_hash, openai_url, claude_url, models)
                 VALUES (1, 'test', 'TestKey', 'encrypted', 'hash', 'https://openai.test', 'https://claude.test', '[]')",
                [],
            )
            .unwrap();
        }
        db
    }

    /// 创建临时测试数据库并插入多个 key
    fn test_db_multi_key() -> Database {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!(
            "welfare-service-hs-multi-{}-{}.db",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_file(&path);
        let db = Database::open(&path).unwrap();
        {
            let conn = db.conn();
            for (id, name, hash) in [
                (1, "Key-A", "hash-a"),
                (2, "Key-B", "hash-b"),
                (3, "Key-C", "hash-c"),
            ] {
                conn.execute(
                    "INSERT INTO api_keys (id, platform, name, api_key, key_hash, openai_url, claude_url, models)
                     VALUES (?1, 'test', ?2, 'encrypted', ?3, 'https://openai.test', 'https://claude.test', '[]')",
                    rusqlite::params![id, name, hash],
                )
                .unwrap();
            }
        }
        db
    }

    /// 辅助: 记录一条日志
    fn log(db: &Database, key_id: i64, is_success: bool, status_code: Option<i32>, affects: bool) {
        db.log_request(RequestLogInput {
            key_id: Some(key_id),
            access_key_id: None,
            model: "m",
            status_code,
            latency_ms: Some(1),
            is_success,
            affects_key_health: affects,
            error_msg: None,
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();
    }

    // ============================================================
    // score_to_label 单元测试
    // ============================================================

    #[test]
    fn test_score_to_label_boundary_values() {
        // 每个分段的边界值
        assert_eq!(
            score_to_label(100, ScoreSource::Realtime),
            StatusLabel::Normal
        );
        assert_eq!(
            score_to_label(80, ScoreSource::Realtime),
            StatusLabel::Normal
        );
        assert_eq!(
            score_to_label(79, ScoreSource::Realtime),
            StatusLabel::LightThrottled
        );
        assert_eq!(
            score_to_label(50, ScoreSource::Realtime),
            StatusLabel::LightThrottled
        );
        assert_eq!(
            score_to_label(49, ScoreSource::Realtime),
            StatusLabel::HeavyThrottled
        );
        assert_eq!(
            score_to_label(20, ScoreSource::Realtime),
            StatusLabel::HeavyThrottled
        );
        assert_eq!(
            score_to_label(19, ScoreSource::Realtime),
            StatusLabel::Critical
        );
        assert_eq!(
            score_to_label(0, ScoreSource::Realtime),
            StatusLabel::Critical
        );
    }

    #[test]
    fn test_score_to_label_nodata_overrides_score() {
        // NoData 无论分数如何都返回 NoData
        assert_eq!(
            score_to_label(100, ScoreSource::NoData),
            StatusLabel::NoData
        );
        assert_eq!(score_to_label(50, ScoreSource::NoData), StatusLabel::NoData);
        assert_eq!(score_to_label(0, ScoreSource::NoData), StatusLabel::NoData);
    }

    #[test]
    fn test_score_to_label_window_source() {
        // Window 和 Realtime 使用相同的分数阈值
        assert_eq!(score_to_label(90, ScoreSource::Window), StatusLabel::Normal);
        assert_eq!(
            score_to_label(60, ScoreSource::Window),
            StatusLabel::LightThrottled
        );
        assert_eq!(
            score_to_label(30, ScoreSource::Window),
            StatusLabel::HeavyThrottled
        );
        assert_eq!(
            score_to_label(10, ScoreSource::Window),
            StatusLabel::Critical
        );
    }

    // ============================================================
    // compute_key_health_score 单 key 集成测试
    // ============================================================

    #[test]
    fn test_single_key_no_data() {
        // 新 key，无任何日志 → NoData
        let db = test_db();
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.key_id, 1);
        assert_eq!(hs.health_score, 0);
        assert_eq!(hs.score_source, ScoreSource::NoData);
        assert_eq!(hs.status_label, StatusLabel::NoData);
        assert_eq!(hs.sample_count, 0);
        assert!(!hs.low_confidence);
    }

    #[test]
    fn test_single_key_realtime_normal() {
        // 20条成功 + 0条失败 → score=100, Normal
        let db = test_db();
        for _ in 0..20 {
            log(&db, 1, true, Some(200), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.health_score, 100);
        assert_eq!(hs.score_source, ScoreSource::Realtime);
        assert_eq!(hs.status_label, StatusLabel::Normal);
        assert_eq!(hs.sample_count, 20);
        assert!(!hs.low_confidence);
    }

    #[test]
    fn test_single_key_realtime_partial_failure() {
        // 15条成功 + 5条失败 → score=75, LightThrottled
        let db = test_db();
        for _ in 0..15 {
            log(&db, 1, true, Some(200), true);
        }
        for _ in 0..5 {
            log(&db, 1, false, Some(500), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.health_score, 75);
        assert_eq!(hs.score_source, ScoreSource::Realtime);
        assert_eq!(hs.status_label, StatusLabel::LightThrottled);
        assert_eq!(hs.sample_count, 20);
        assert!(!hs.low_confidence);
    }

    #[test]
    fn test_single_key_realtime_all_fail() {
        // 20条全部失败 → score=0, Critical
        let db = test_db();
        for _ in 0..20 {
            log(&db, 1, false, Some(500), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.health_score, 0);
        assert_eq!(hs.score_source, ScoreSource::Realtime);
        assert_eq!(hs.status_label, StatusLabel::Critical);
        assert_eq!(hs.sample_count, 20);
        assert!(!hs.low_confidence);
    }

    #[test]
    fn test_single_key_realtime_includes_429() {
        // 15条成功 + 5条429 → 实时样本20条，score=75, LightThrottled
        let db = test_db();
        for _ in 0..15 {
            log(&db, 1, true, Some(200), true);
        }
        for _ in 0..5 {
            log(&db, 1, false, Some(429), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.score_source, ScoreSource::Realtime);
        assert_eq!(hs.health_score, 75);
        assert_eq!(hs.status_label, StatusLabel::LightThrottled);
        assert_eq!(hs.sample_count, 20);
    }

    #[test]
    fn test_single_key_window_low_confidence() {
        // 只有3条成功记录 → Window来源, low_confidence=true
        let db = test_db();
        for _ in 0..3 {
            log(&db, 1, true, Some(200), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.score_source, ScoreSource::Window);
        assert_eq!(hs.health_score, 100);
        assert!(hs.low_confidence, "样本<5应为低置信度");
        assert_eq!(hs.sample_count, 3);
    }

    #[test]
    fn test_single_key_window_not_low_confidence() {
        // 5条成功记录 → Window来源, low_confidence=false（5是边界值，不算低置信度）
        let db = test_db();
        for _ in 0..5 {
            log(&db, 1, true, Some(200), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.score_source, ScoreSource::Window);
        assert!(!hs.low_confidence, "样本>=5不应为低置信度");
        assert_eq!(hs.sample_count, 5);
    }

    #[test]
    fn test_single_key_429_lowers_score_without_unhealthy_status() {
        // 10条成功 + 10条429 → score=50, LightThrottled
        let db = test_db();
        for _ in 0..10 {
            log(&db, 1, true, Some(200), true);
        }
        for _ in 0..10 {
            log(&db, 1, false, Some(429), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.score_source, ScoreSource::Realtime);
        assert_eq!(hs.health_score, 50);
        assert_eq!(hs.status_label, StatusLabel::LightThrottled);
    }

    #[test]
    fn test_single_key_affects_key_health_false_ignored() {
        // affects_key_health=false 的日志不计入评分
        let db = test_db();
        for _ in 0..20 {
            log(&db, 1, false, Some(400), false); // 客户端错误，不影响健康
        }
        for _ in 0..5 {
            log(&db, 1, true, Some(200), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        // 只有5条 affects_key_health=true 的成功记录 → Window来源
        assert_eq!(hs.score_source, ScoreSource::Window);
        assert_eq!(hs.health_score, 100);
    }

    // ============================================================
    // compute_all_keys_health_scores 批量集成测试
    // ============================================================

    #[test]
    fn test_batch_empty_db() {
        // 无 key 时返回空数组
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!(
            "welfare-service-hs-empty-{}-{}.db",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_file(&path);
        let db = Database::open(&path).unwrap();
        let scores = db.compute_all_keys_health_scores().unwrap();
        assert!(scores.is_empty());
    }

    #[test]
    fn test_batch_multiple_keys_different_states() {
        // 3个 key: A=全部成功(20条), B=少量记录(3条), C=无数据
        let db = test_db_multi_key();

        // Key-A: 20条成功 → Realtime, Normal, 100分
        for _ in 0..20 {
            log(&db, 1, true, Some(200), true);
        }
        // Key-B: 3条成功 → Window, Normal, 100分, low_confidence=true
        for _ in 0..3 {
            log(&db, 2, true, Some(200), true);
        }
        // Key-C: 无日志 → NoData

        let scores = db.compute_all_keys_health_scores().unwrap();
        assert_eq!(scores.len(), 3);

        // Key-A
        let a = scores.iter().find(|s| s.key_id == 1).unwrap();
        assert_eq!(a.key_name, "Key-A");
        assert_eq!(a.health_score, 100);
        assert_eq!(a.score_source, ScoreSource::Realtime);
        assert_eq!(a.status_label, StatusLabel::Normal);
        assert!(!a.low_confidence);

        // Key-B
        let b = scores.iter().find(|s| s.key_id == 2).unwrap();
        assert_eq!(b.key_name, "Key-B");
        assert_eq!(b.health_score, 100);
        assert_eq!(b.score_source, ScoreSource::Window);
        assert!(b.low_confidence);

        // Key-C
        let c = scores.iter().find(|s| s.key_id == 3).unwrap();
        assert_eq!(c.key_name, "Key-C");
        assert_eq!(c.health_score, 0);
        assert_eq!(c.score_source, ScoreSource::NoData);
        assert_eq!(c.status_label, StatusLabel::NoData);
        assert!(!c.low_confidence);
    }

    #[test]
    fn test_batch_consistent_with_single_key() {
        // 批量计算与逐个计算的结果一致
        let db = test_db_multi_key();
        // Key-A: 混合成功和失败
        for _ in 0..12 {
            log(&db, 1, true, Some(200), true);
        }
        for _ in 0..8 {
            log(&db, 1, false, Some(500), true);
        }
        // Key-B: 全部429
        for _ in 0..20 {
            log(&db, 2, false, Some(429), true);
        }
        // Key-C: 少量失败
        for _ in 0..2 {
            log(&db, 3, false, Some(500), true);
        }

        let batch_scores = db.compute_all_keys_health_scores().unwrap();
        for key_id in [1i64, 2, 3] {
            let single = db.compute_key_health_score(key_id).unwrap();
            let batch = batch_scores.iter().find(|s| s.key_id == key_id).unwrap();

            assert_eq!(
                single.health_score, batch.health_score,
                "key_id={}: 批量score {} != 单条score {}",
                key_id, batch.health_score, single.health_score
            );
            assert_eq!(
                single.score_source, batch.score_source,
                "key_id={}: 批量source {:?} != 单条source {:?}",
                key_id, batch.score_source, single.score_source
            );
            assert_eq!(
                single.status_label, batch.status_label,
                "key_id={}: 批量label {:?} != 单条label {:?}",
                key_id, batch.status_label, single.status_label
            );
            // 注意: sample_count 可能不同（批量用窗口，单条用实时），仅比较 score/source/label
        }
    }

    #[test]
    fn test_batch_realtime_only_takes_latest_20() {
        // 验证批量实时统计确实只取最近20条
        let db = test_db();
        // 先插入20条失败，再插入20条成功
        // 最新的20条应该全是成功 → score=100
        for _ in 0..20 {
            log(&db, 1, false, Some(500), true);
        }
        for _ in 0..20 {
            log(&db, 1, true, Some(200), true);
        }

        let scores = db.compute_all_keys_health_scores().unwrap();
        let hs = scores.iter().find(|s| s.key_id == 1).unwrap();
        assert_eq!(hs.health_score, 100, "实时统计应只取最近20条（全成功）");
        assert_eq!(hs.score_source, ScoreSource::Realtime);
        assert_eq!(hs.sample_count, 20);
    }

    #[test]
    fn test_batch_realtime_429_included() {
        // 混合429和500，验证429计入可用性评分
        let db = test_db();
        for _ in 0..10 {
            log(&db, 1, true, Some(200), true);
        }
        for _ in 0..5 {
            log(&db, 1, false, Some(429), true);
        }
        for _ in 0..5 {
            log(&db, 1, false, Some(500), true);
        }
        // 10成功 + 10失败 = 20条 → Realtime, score=50
        let scores = db.compute_all_keys_health_scores().unwrap();
        let hs = scores.iter().find(|s| s.key_id == 1).unwrap();
        assert_eq!(hs.score_source, ScoreSource::Realtime);
        assert_eq!(hs.health_score, 50);
        assert_eq!(hs.status_label, StatusLabel::LightThrottled);
        assert_eq!(hs.sample_count, 20);
    }

    #[test]
    fn test_batch_key_name_populated() {
        // 验证批量结果包含 key_name
        let db = test_db_multi_key();
        // 不插入日志，所有key都是NoData
        let scores = db.compute_all_keys_health_scores().unwrap();
        let names: Vec<&str> = scores.iter().map(|s| s.key_name.as_str()).collect();
        assert!(names.contains(&"Key-A"), "应包含 Key-A");
        assert!(names.contains(&"Key-B"), "应包含 Key-B");
        assert!(names.contains(&"Key-C"), "应包含 Key-C");
    }

    // ============================================================
    // serde 序列化测试
    // ============================================================

    #[test]
    fn test_serde_serialization_matches_frontend_types() {
        // 验证 serde snake_case 序列化结果与前端类型定义一致
        let hs = KeyHealthScore {
            key_id: 1,
            key_name: String::new(),
            health_score: 85,
            score_source: ScoreSource::Realtime,
            status_label: StatusLabel::Normal,
            sample_count: 42,
            low_confidence: false,
        };
        let json = serde_json::to_value(&hs).unwrap();

        // 验证枚举的 snake_case 输出
        assert_eq!(json["score_source"], "realtime");
        assert_eq!(json["status_label"], "normal");
        assert_eq!(json["low_confidence"], false);
        // 空 key_name 应被省略
        assert!(
            json.get("key_name").is_none(),
            "空 key_name 应被 skip_serializing_if 省略"
        );

        // 验证有 key_name 时正常序列化
        let hs_named = KeyHealthScore {
            key_id: 10,
            key_name: "OpenAI-Prod".to_string(),
            health_score: 95,
            score_source: ScoreSource::Realtime,
            status_label: StatusLabel::Normal,
            sample_count: 30,
            low_confidence: false,
        };
        let json_named = serde_json::to_value(&hs_named).unwrap();
        assert_eq!(json_named["key_name"], "OpenAI-Prod");

        // 验证 Window 分支
        let hs2 = KeyHealthScore {
            key_id: 2,
            key_name: String::new(),
            health_score: 65,
            score_source: ScoreSource::Window,
            status_label: StatusLabel::LightThrottled,
            sample_count: 5,
            low_confidence: false, // 5 samples → not low confidence
        };
        let json2 = serde_json::to_value(&hs2).unwrap();
        assert_eq!(json2["score_source"], "window");
        assert_eq!(json2["status_label"], "light_throttled");

        // 验证 Window + 低置信度
        let hs2b = KeyHealthScore {
            key_id: 20,
            key_name: String::new(),
            health_score: 100,
            score_source: ScoreSource::Window,
            status_label: StatusLabel::Normal,
            sample_count: 2,
            low_confidence: true, // < 5 samples → low confidence
        };
        let json2b = serde_json::to_value(&hs2b).unwrap();
        assert_eq!(json2b["low_confidence"], true);

        // 验证 NoData 分支
        let hs3 = KeyHealthScore {
            key_id: 3,
            key_name: String::new(),
            health_score: 0,
            score_source: ScoreSource::NoData,
            status_label: StatusLabel::NoData,
            sample_count: 0,
            low_confidence: false,
        };
        let json3 = serde_json::to_value(&hs3).unwrap();
        assert_eq!(json3["score_source"], "nodata");
        assert_eq!(json3["status_label"], "nodata");

        // 验证其他状态标签
        assert_eq!(
            serde_json::to_value(StatusLabel::HeavyThrottled).unwrap(),
            "heavy_throttled"
        );
        assert_eq!(
            serde_json::to_value(StatusLabel::Critical).unwrap(),
            "critical"
        );
    }

    #[test]
    fn test_serde_deserialization_with_defaults() {
        // 验证反序列化时低置信度和 key_name 的默认值
        let json = r#"{"key_id":1,"health_score":85,"score_source":"realtime","status_label":"normal","sample_count":42}"#;
        let hs: KeyHealthScore = serde_json::from_str(json).unwrap();
        assert_eq!(hs.key_name, "");
        assert!(!hs.low_confidence, "缺失 low_confidence 应默认 false");
    }

    #[test]
    fn test_serde_roundtrip() {
        // 序列化后再反序列化应完全一致
        let original = KeyHealthScore {
            key_id: 42,
            key_name: "RoundTrip-Key".to_string(),
            health_score: 73,
            score_source: ScoreSource::Window,
            status_label: StatusLabel::LightThrottled,
            sample_count: 4,
            low_confidence: true,
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: KeyHealthScore = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.key_id, original.key_id);
        assert_eq!(restored.key_name, original.key_name);
        assert_eq!(restored.health_score, original.health_score);
        assert_eq!(restored.score_source, original.score_source);
        assert_eq!(restored.status_label, original.status_label);
        assert_eq!(restored.sample_count, original.sample_count);
        assert_eq!(restored.low_confidence, original.low_confidence);
    }

    // ============================================================
    // 边界条件测试
    // ============================================================

    #[test]
    fn test_score_clamp_100() {
        // 即使成功率计算恰好为100，score也clamp到100
        let db = test_db();
        for _ in 0..20 {
            log(&db, 1, true, Some(200), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.health_score, 100);
        assert!(hs.health_score <= 100, "score 不应超过100");
    }

    #[test]
    fn test_score_zero_realtime_vs_nodata() {
        // score=0 + Realtime (已知全部失败) vs NoData (未知) 的区分
        let db = test_db();

        // Key-1: 20条全失败 → Realtime, score=0
        for _ in 0..20 {
            log(&db, 1, false, Some(500), true);
        }
        let hs_realtime_zero = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs_realtime_zero.health_score, 0);
        assert_eq!(hs_realtime_zero.score_source, ScoreSource::Realtime);
        assert_eq!(hs_realtime_zero.status_label, StatusLabel::Critical);
        assert_ne!(
            hs_realtime_zero.status_label,
            StatusLabel::NoData,
            "score=0 + Realtime 应为 Critical，不是 NoData"
        );
    }

    #[test]
    fn test_heavy_throttled_score_boundary() {
        // score=20 → HeavyThrottled, score=19 → Critical
        let db = test_db();
        // 需要 20条中4条成功16条失败 → 20%
        for _ in 0..4 {
            log(&db, 1, true, Some(200), true);
        }
        for _ in 0..16 {
            log(&db, 1, false, Some(500), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.health_score, 20);
        assert_eq!(hs.status_label, StatusLabel::HeavyThrottled);
    }

    #[test]
    fn test_light_throttled_score_boundary() {
        // score=50 → LightThrottled, score=49 → HeavyThrottled
        let db = test_db();
        for _ in 0..10 {
            log(&db, 1, true, Some(200), true);
        }
        for _ in 0..10 {
            log(&db, 1, false, Some(500), true);
        }
        let hs = db.compute_key_health_score(1).unwrap();
        assert_eq!(hs.health_score, 50);
        assert_eq!(hs.status_label, StatusLabel::LightThrottled);
    }
}
