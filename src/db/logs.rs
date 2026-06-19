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
    pub model: &'a str,
    pub status_code: Option<i32>,
    pub latency_ms: Option<i64>,
    pub is_success: bool,
    pub affects_key_health: bool,
    pub error_msg: Option<&'a str>,
}

impl Database {
    /// 记录请求日志
    pub fn log_request(&self, input: RequestLogInput<'_>) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO request_logs (key_id, model, status_code, latency_ms, is_success, affects_key_health, error_msg)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                input.key_id,
                input.model,
                input.status_code,
                input.latency_ms,
                input.is_success,
                input.affects_key_health,
                input.error_msg
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

    /// 获取指定 Key 的最近连续失败次数
    pub fn get_key_consecutive_failures(&self, key_id: i64) -> Result<u32> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT is_success FROM request_logs
             WHERE key_id = ?1 AND affects_key_health = 1
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
            model: "m",
            status_code: Some(400),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: false,
            error_msg: Some("bad request"),
        })
        .unwrap();
        db.log_request(RequestLogInput {
            key_id: Some(1),
            model: "m",
            status_code: Some(500),
            latency_ms: Some(1),
            is_success: false,
            affects_key_health: true,
            error_msg: Some("server error"),
        })
        .unwrap();

        let stats = db.get_key_health_stats(1, 20).unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn consecutive_failures_use_insertion_order_for_same_second_logs() {
        let db = test_db();
        for success in [false, true, false] {
            db.log_request(RequestLogInput {
                key_id: Some(1),
                model: "m",
                status_code: Some(if success { 200 } else { 500 }),
                latency_ms: Some(1),
                is_success: success,
                affects_key_health: true,
                error_msg: None,
            })
            .unwrap();
        }

        assert_eq!(db.get_key_consecutive_failures(1).unwrap(), 1);
    }
}
