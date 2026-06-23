use rusqlite::params;

use crate::db::Database;
use crate::error::Result;

#[derive(Debug, Clone, Copy)]
pub struct KeyHealthStats {
    pub total: u32,
    pub success_rate: f64,
}

pub struct RequestLogInput<'a> {
    pub key_id: Option<i64>,
    pub access_key_id: Option<i64>,
    pub model: &'a str,
    pub status_code: Option<i32>,
    pub latency_ms: Option<i64>,
    pub is_success: bool,
    pub affects_key_health: bool,
    pub error_msg: Option<&'a str>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

impl Database {
    /// 记录请求日志
    pub fn log_request(&self, input: RequestLogInput<'_>) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO request_logs (key_id, access_key_id, model, status_code, latency_ms, is_success, affects_key_health, error_msg, prompt_tokens, completion_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                input.key_id,
                input.access_key_id,
                input.model,
                input.status_code,
                input.latency_ms,
                input.is_success,
                input.affects_key_health,
                input.error_msg,
                input.prompt_tokens,
                input.completion_tokens,
            ],
        )?;
        Ok(())
    }

    /// 获取指定 Key 最近 N 条会影响健康判断的请求统计。
    pub fn get_key_health_stats(&self, key_id: i64, recent_count: u32) -> Result<KeyHealthStats> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) as total, SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success
             FROM (
                SELECT is_success FROM request_logs
                WHERE key_id = ?1 AND affects_key_health = 1
                ORDER BY created_at DESC, id DESC LIMIT ?2
             )",
        )?;

        let result = stmt.query_row(params![key_id, recent_count], |row| {
            let total: i64 = row.get(0)?;
            let success: Option<i64> = row.get(1)?;
            Ok((total, success.unwrap_or(0)))
        })?;

        if result.0 == 0 {
            return Ok(KeyHealthStats {
                total: 0,
                success_rate: 1.0,
            });
        }

        Ok(KeyHealthStats {
            total: result.0 as u32,
            success_rate: result.1 as f64 / result.0 as f64,
        })
    }

    /// 获取指定 Key 的最近 N 条请求的成功率
    pub fn get_key_success_rate(&self, key_id: i64, recent_count: u32) -> Result<f64> {
        Ok(self
            .get_key_health_stats(key_id, recent_count)?
            .success_rate)
    }

    /// 获取指定 Key 最近 N 条会影响健康判断的请求统计（排除 429 限流）。
    ///
    /// 用于被动自动下线：429 是临时限流，不代表 Key 本身失效，
    /// 不应导致 key 被自动下线。
    pub fn get_key_health_stats_excluding_rate_limited(
        &self,
        key_id: i64,
        recent_count: u32,
    ) -> Result<KeyHealthStats> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) as total, SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success
             FROM (
                SELECT is_success FROM request_logs
                WHERE key_id = ?1 AND affects_key_health = 1 AND (status_code != 429 OR status_code IS NULL)
                ORDER BY created_at DESC, id DESC LIMIT ?2
             )",
        )?;

        let result = stmt.query_row(params![key_id, recent_count], |row| {
            let total: i64 = row.get(0)?;
            let success: Option<i64> = row.get(1)?;
            Ok((total, success.unwrap_or(0)))
        })?;

        if result.0 == 0 {
            return Ok(KeyHealthStats {
                total: 0,
                success_rate: 1.0,
            });
        }

        Ok(KeyHealthStats {
            total: result.0 as u32,
            success_rate: result.1 as f64 / result.0 as f64,
        })
    }

    /// 获取指定 Key 的最近连续失败次数（排除 429 限流）。
    ///
    /// 用于被动监测：429 是临时限流，连续 429 不应导致 key 被自动下线。
    pub fn get_key_consecutive_failures_excluding_rate_limited(
        &self,
        key_id: i64,
    ) -> Result<u32> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT is_success FROM request_logs
             WHERE key_id = ?1 AND affects_key_health = 1 AND (status_code != 429 OR status_code IS NULL)
             ORDER BY created_at DESC, id DESC LIMIT 20",
        )?;

        let mut failures = 0u32;
        let rows = stmt.query_map(params![key_id], |row| {
            let success: Option<bool> = row.get(0)?;
            Ok(success.unwrap_or(false))
        })?;

        for row in rows {
            if !row? {
                failures += 1;
            } else {
                break;
            }
        }

        Ok(failures)
    }

    /// 查询指定 Key 在最近 N 小时内的成功率（排除 429 限流）。
    ///
    /// 用于需要排除临时限流的策略口径：429 不代表 Key 本身失效。
    /// 返回 (success_rate, sample_count)，无记录返回 None。
    #[allow(dead_code)]
    pub fn get_key_window_success_rate_excluding_rate_limited(
        &self,
        key_id: i64,
        hours: u32,
    ) -> Result<Option<(f64, u32)>> {
        let conn = self.conn();
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::TimeDelta::hours(hours as i64))
            .ok_or_else(|| {
                crate::error::AppError::Internal("时间窗口计算溢出".to_string())
            })?
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let (total, success) = conn.query_row(
            "SELECT COUNT(*) as total,
                    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success
             FROM request_logs
             WHERE key_id = ?1 AND affects_key_health = 1
               AND (status_code != 429 OR status_code IS NULL)
               AND created_at >= ?2",
            rusqlite::params![key_id, cutoff],
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

    /// 清理过期日志 (保留最近 N 天)
    pub fn cleanup_old_logs(&self, keep_days: u32) -> Result<usize> {
        let conn = self.conn();
        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::TimeDelta::days(keep_days as i64))
            .ok_or_else(|| crate::error::AppError::Internal("日志保留天数计算溢出".to_string()))?
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let affected = conn.execute(
            "DELETE FROM request_logs WHERE created_at < ?1",
            params![cutoff],
        )?;
        Ok(affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!(
            "welfare-service-log-test-{}-{}.db",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_file(&path);
        let db = Database::open(&path).unwrap();
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO api_keys (id, platform, api_key, key_hash, openai_url, claude_url, models)
                 VALUES (1, 'test', 'encrypted', 'hash', 'https://openai.test', 'https://claude.test', '[]')",
                [],
            )
            .unwrap();
        }
        db
    }

    #[test]
    fn health_stats_ignore_client_request_failures() {
        let db = test_db();
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(400),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: false,
            error_msg: Some("bad request"),
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(500),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: true,
            error_msg: Some("server error"),
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();

        let stats = db.get_key_health_stats(1, 20).unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn health_stats_excluding_rate_limited_filters_429() {
        let db = test_db();
        // 429 限流：affects_key_health=true 但应被 excluding 方法排除
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(429),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: true,
            error_msg: Some("rate limited"),
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();
        // 500 错误：affects_key_health=true，不应被排除
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(500),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: true,
            error_msg: Some("server error"),
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();
        // 成功请求
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(200),
            latency_ms: Some(1),
            is_success: true,
            affects_key_health: true,
            error_msg: None,
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();

        // get_key_health_stats 包含 429: total=3, success=1 → rate=1/3
        let stats_all = db.get_key_health_stats(1, 20).unwrap();
        assert_eq!(stats_all.total, 3);
        assert!((stats_all.success_rate - 1.0 / 3.0).abs() < 0.01);

        // get_key_health_stats_excluding_rate_limited 排除 429: total=2, success=1 → rate=0.5
        let stats_no429 = db.get_key_health_stats_excluding_rate_limited(1, 20).unwrap();
        assert_eq!(stats_no429.total, 2);
        assert!((stats_no429.success_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn consecutive_failures_excluding_rate_limited_skips_429() {
        let db = test_db();
        // 1 次成功，然后 1 次 500，然后连续 3 次 429（最新）
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(200),
            latency_ms: Some(1),
            is_success: true,
            affects_key_health: true,
            error_msg: None,
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(500),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: true,
            error_msg: Some("server error"),
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();
        for _ in 0..3 {
            db.log_request(RequestLogInput {
                key_id: Some(1),
                access_key_id: None,
                model: "m",
                status_code: Some(429),
                latency_ms: Some(1),
                is_success: false,
                affects_key_health: true,
                error_msg: Some("rate limited"),
                prompt_tokens: 0,
                completion_tokens: 0,
            })
            .unwrap();
        }

        // 排除 429: 可见记录 = [500, 200]，最新的是 500（失败），连续失败 1 次
        assert_eq!(db.get_key_consecutive_failures_excluding_rate_limited(1).unwrap(), 1);
    }

    #[test]
    fn excluding_rate_limited_includes_null_status_code() {
        let db = test_db();
        // status_code=NULL 的内部错误不应被排除
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: None, // AppError::Internal 等场景
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: true,
            error_msg: Some("internal error"),
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();

        // NULL 不等于 429，应被包含
        let stats = db.get_key_health_stats_excluding_rate_limited(1, 20).unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.success_rate, 0.0);

        let failures = db.get_key_consecutive_failures_excluding_rate_limited(1).unwrap();
        assert_eq!(failures, 1);
    }

    #[test]
    fn window_success_rate_excluding_rate_limited_filters_429() {
        let db = test_db();
        // 插入：1 次 200 成功，2 次 429 限流，1 次 500 错误
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(200),
            latency_ms: Some(1),
            is_success: true,
            affects_key_health: true,
            error_msg: None,
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();
        for _ in 0..2 {
            db.log_request(RequestLogInput {
                key_id: Some(1),
                access_key_id: None,
                model: "m",
                status_code: Some(429),
                latency_ms: Some(1),
                is_success: false,
                affects_key_health: true,
                error_msg: Some("rate limited"),
                prompt_tokens: 0,
                completion_tokens: 0,
            })
            .unwrap();
        }
        db.log_request(RequestLogInput {
            key_id: Some(1),
            access_key_id: None,
            model: "m",
            status_code: Some(500),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: true,
            error_msg: Some("server error"),
            prompt_tokens: 0,
            completion_tokens: 0,
        })
        .unwrap();

        // 包含 429: total=4, success=1 → rate=0.25
        let rate_all = db.get_key_window_success_rate(1, 24).unwrap();
        assert!(rate_all.is_some());
        let (rate, count) = rate_all.unwrap();
        assert_eq!(count, 4);
        assert!((rate - 0.25).abs() < 0.01);

        // 排除 429: total=2, success=1 → rate=0.5
        let rate_no429 = db.get_key_window_success_rate_excluding_rate_limited(1, 24).unwrap();
        assert!(rate_no429.is_some());
        let (rate, count) = rate_no429.unwrap();
        assert_eq!(count, 2);
        assert!((rate - 0.5).abs() < 0.01);
    }
}
