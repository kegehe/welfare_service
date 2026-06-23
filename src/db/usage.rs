use rusqlite::params;

use crate::db::Database;
use crate::error::Result;
use serde::Serialize;

/// usage_hourly 表中一条聚合记录
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct UsageHourlyRecord {
    pub id: i64,
    pub dimension: String,
    pub key_id: i64,
    pub model: String,
    pub hour_bucket: i64,
    pub request_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

/// 批量刷盘用的缓存条目
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct UsageFlushEntry {
    pub dimension: String,
    pub key_id: i64,
    pub model: String,
    pub hour_bucket: i64,
    pub request_count: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// 号池 Key 用量统计行
#[derive(Debug, Clone, Serialize)]
pub struct PoolKeyStatsRow {
    pub key_id: i64,
    pub name: String,
    pub platform: String,
    pub total_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub last_used_at: Option<String>,
}

/// 访问 Key 用量统计行
#[derive(Debug, Clone, Serialize)]
pub struct AccessKeyStatsRow {
    pub access_key_id: i64,
    pub name: String,
    pub total_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub last_used_at: Option<String>,
}

/// 按模型细分的统计行
#[derive(Debug, Clone, Serialize)]
pub struct ModelStatsRow {
    pub model: String,
    pub requests: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

/// 全局概览统计
#[derive(Debug, Clone, Serialize)]
pub struct OverviewStats {
    pub total_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub active_pool_keys: i64,
    pub active_access_keys: i64,
}

/// 小时趋势数据行
#[derive(Debug, Clone, Serialize)]
pub struct HourlyStatsRow {
    pub hour_bucket: i64,
    pub model: String,
    pub request_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

/// 计算 cutoff 时间对应的 hour_bucket 整数
fn cutoff_hour_bucket(hours: i64) -> i64 {
    let cutoff = chrono::Utc::now() - chrono::TimeDelta::hours(hours);
    cutoff.timestamp() / 3600
}

/// 计算 cutoff 日期时间字符串（用于 request_logs 的时间过滤）
fn cutoff_datetime_str(hours: i64) -> String {
    let cutoff = chrono::Utc::now() - chrono::TimeDelta::hours(hours);
    cutoff.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 计算 cutoff 日期对应的 hour_bucket 整数（按天）
fn cutoff_day_hour_bucket(days: u32) -> i64 {
    let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::TimeDelta::days(days as i64))
        .unwrap_or_else(chrono::Utc::now);
    cutoff.timestamp() / 3600
}

impl Database {
    /// 批量刷盘: UPSERT 到 usage_hourly 表 (供外部直接调用)
    #[allow(dead_code)]
    pub fn flush_usage_hourly(&self, entries: &[UsageFlushEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let conn = self.conn();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        for entry in entries {
            conn.execute(
                "INSERT INTO usage_hourly (dimension, key_id, model, hour_bucket, request_count, prompt_tokens, completion_tokens, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                 ON CONFLICT(dimension, key_id, model, hour_bucket) DO UPDATE SET
                    request_count = request_count + ?5,
                    prompt_tokens = prompt_tokens + ?6,
                    completion_tokens = completion_tokens + ?7,
                    updated_at = ?8",
                params![
                    entry.dimension,
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

        Ok(())
    }

    /// 获取全局概览统计
    pub fn get_stats_overview(&self, hours: i64) -> Result<OverviewStats> {
        let conn = self.conn();
        let cutoff_hb = cutoff_hour_bucket(hours);

        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(SUM(request_count), 0),
                COALESCE(SUM(prompt_tokens), 0),
                COALESCE(SUM(completion_tokens), 0)
             FROM usage_hourly
             WHERE dimension = 'pool'
               AND hour_bucket >= ?1",
        )?;

        let (total_requests, total_prompt_tokens, total_completion_tokens) = stmt
            .query_row(params![cutoff_hb], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
            })
            .unwrap_or((0, 0, 0));

        // 活跃 key 数量
        let active_pool_keys: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM api_keys WHERE status = 'active'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let active_access_keys: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM access_keys WHERE status = 'active'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(OverviewStats {
            total_requests,
            total_prompt_tokens,
            total_completion_tokens,
            active_pool_keys,
            active_access_keys,
        })
    }

    /// 获取号池 Key 用量列表
    pub fn get_pool_key_stats(&self, hours: i64) -> Result<Vec<PoolKeyStatsRow>> {
        let conn = self.conn();
        let cutoff_hb = cutoff_hour_bucket(hours);
        let cutoff_str = cutoff_datetime_str(hours);

        let mut stmt = conn.prepare(
            "SELECT
                ak.id,
                COALESCE(ak.name, '') as name,
                COALESCE(ak.platform, '') as platform,
                COALESCE(SUM(uh.request_count), 0) as total_requests,
                COALESCE(SUM(uh.prompt_tokens), 0) as total_prompt_tokens,
                COALESCE(SUM(uh.completion_tokens), 0) as total_completion_tokens,
                COALESCE(
                    (SELECT CAST(SUM(CASE WHEN rl.is_success THEN 1 ELSE 0 END) AS REAL) / COUNT(*)
                     FROM request_logs rl
                     WHERE rl.key_id = ak.id
                       AND rl.created_at >= ?1
                       AND rl.affects_key_health = 1),
                    1.0
                ) as success_rate,
                COALESCE(
                    (SELECT AVG(CAST(rl.latency_ms AS REAL))
                     FROM request_logs rl
                     WHERE rl.key_id = ak.id
                       AND rl.is_success = 1
                       AND rl.created_at >= ?1),
                    0.0
                ) as avg_latency_ms,
                (SELECT MAX(rl.created_at)
                 FROM request_logs rl
                 WHERE rl.key_id = ak.id) as last_used_at
             FROM api_keys ak
             LEFT JOIN usage_hourly uh ON uh.dimension = 'pool' AND uh.key_id = ak.id
                 AND uh.hour_bucket >= ?2
             GROUP BY ak.id
             ORDER BY total_prompt_tokens DESC",
        )?;

        let rows = stmt
            .query_map(params![cutoff_str, cutoff_hb], |row| {
                Ok(PoolKeyStatsRow {
                    key_id: row.get(0)?,
                    name: row.get(1)?,
                    platform: row.get(2)?,
                    total_requests: row.get(3)?,
                    total_prompt_tokens: row.get(4)?,
                    total_completion_tokens: row.get(5)?,
                    success_rate: row.get(6)?,
                    avg_latency_ms: row.get(7)?,
                    last_used_at: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// 获取单个号池 Key 的按模型统计
    pub fn get_pool_key_model_stats(&self, key_id: i64, hours: i64) -> Result<Vec<ModelStatsRow>> {
        let conn = self.conn();
        let cutoff_hb = cutoff_hour_bucket(hours);

        let mut stmt = conn.prepare(
            "SELECT
                uh.model,
                SUM(uh.request_count) as requests,
                SUM(uh.prompt_tokens),
                SUM(uh.completion_tokens)
             FROM usage_hourly uh
             WHERE uh.dimension = 'pool'
               AND uh.key_id = ?1
               AND uh.hour_bucket >= ?2
             GROUP BY uh.model
             ORDER BY requests DESC",
        )?;

        let rows = stmt
            .query_map(params![key_id, cutoff_hb], |row| {
                Ok(ModelStatsRow {
                    model: row.get(0)?,
                    requests: row.get(1)?,
                    prompt_tokens: row.get(2)?,
                    completion_tokens: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// 获取访问 Key 用量列表
    pub fn get_access_key_stats(&self, hours: i64) -> Result<Vec<AccessKeyStatsRow>> {
        let conn = self.conn();
        let cutoff_hb = cutoff_hour_bucket(hours);

        let mut stmt = conn.prepare(
            "SELECT
                ak.id,
                COALESCE(ak.name, '') as name,
                COALESCE(SUM(uh.request_count), 0) as total_requests,
                COALESCE(SUM(uh.prompt_tokens), 0) as total_prompt_tokens,
                COALESCE(SUM(uh.completion_tokens), 0) as total_completion_tokens,
                ak.last_used_at
             FROM access_keys ak
             LEFT JOIN usage_hourly uh ON uh.dimension = 'access' AND uh.key_id = ak.id
                 AND uh.hour_bucket >= ?1
             GROUP BY ak.id
             ORDER BY total_prompt_tokens DESC",
        )?;

        let rows = stmt
            .query_map(params![cutoff_hb], |row| {
                Ok(AccessKeyStatsRow {
                    access_key_id: row.get(0)?,
                    name: row.get(1)?,
                    total_requests: row.get(2)?,
                    total_prompt_tokens: row.get(3)?,
                    total_completion_tokens: row.get(4)?,
                    last_used_at: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// 获取单个访问 Key 的按模型统计
    pub fn get_access_key_model_stats(&self, key_id: i64, hours: i64) -> Result<Vec<ModelStatsRow>> {
        let conn = self.conn();
        let cutoff_hb = cutoff_hour_bucket(hours);

        let mut stmt = conn.prepare(
            "SELECT
                uh.model,
                SUM(uh.request_count) as requests,
                SUM(uh.prompt_tokens),
                SUM(uh.completion_tokens)
             FROM usage_hourly uh
             WHERE uh.dimension = 'access'
               AND uh.key_id = ?1
               AND uh.hour_bucket >= ?2
             GROUP BY uh.model
             ORDER BY requests DESC",
        )?;

        let rows = stmt
            .query_map(params![key_id, cutoff_hb], |row| {
                Ok(ModelStatsRow {
                    model: row.get(0)?,
                    requests: row.get(1)?,
                    prompt_tokens: row.get(2)?,
                    completion_tokens: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// 获取小时级趋势数据
    pub fn get_hourly_stats(
        &self,
        dimension: &str,
        key_id: Option<i64>,
        hours: i64,
    ) -> Result<Vec<HourlyStatsRow>> {
        let conn = self.conn();
        let cutoff_hb = cutoff_hour_bucket(hours);

        let sql = match key_id {
            Some(_) => {
                "SELECT hour_bucket, model, SUM(request_count), SUM(prompt_tokens), SUM(completion_tokens)
                 FROM usage_hourly
                 WHERE dimension = ?1
                   AND key_id = ?2
                   AND hour_bucket >= ?3
                 GROUP BY hour_bucket, model
                 ORDER BY hour_bucket ASC"
            }
            None => {
                "SELECT hour_bucket, model, SUM(request_count), SUM(prompt_tokens), SUM(completion_tokens)
                 FROM usage_hourly
                 WHERE dimension = ?1
                   AND hour_bucket >= ?2
                 GROUP BY hour_bucket, model
                 ORDER BY hour_bucket ASC"
            }
        };

        let mut stmt = conn.prepare(sql)?;

        let rows = match key_id {
            Some(kid) => stmt
                .query_map(params![dimension, kid, cutoff_hb], |row| {
                    Ok(HourlyStatsRow {
                        hour_bucket: row.get(0)?,
                        model: row.get(1)?,
                        request_count: row.get(2)?,
                        prompt_tokens: row.get(3)?,
                        completion_tokens: row.get(4)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            None => stmt
                .query_map(params![dimension, cutoff_hb], |row| {
                    Ok(HourlyStatsRow {
                        hour_bucket: row.get(0)?,
                        model: row.get(1)?,
                        request_count: row.get(2)?,
                        prompt_tokens: row.get(3)?,
                        completion_tokens: row.get(4)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };

        Ok(rows)
    }

    /// 清理过期的 usage_hourly 数据
    pub fn cleanup_usage_hourly(&self, pool_keep_days: u32, access_keep_days: u32) -> Result<usize> {
        let conn = self.conn();

        let pool_cutoff_hb = cutoff_day_hour_bucket(pool_keep_days);
        let access_cutoff_hb = cutoff_day_hour_bucket(access_keep_days);

        let pool_affected = conn.execute(
            "DELETE FROM usage_hourly WHERE dimension = 'pool' AND hour_bucket < ?1",
            params![pool_cutoff_hb],
        )?;

        let access_affected = conn.execute(
            "DELETE FROM usage_hourly WHERE dimension = 'access' AND hour_bucket < ?1",
            params![access_cutoff_hb],
        )?;

        Ok(pool_affected + access_affected)
    }

    /// 加载当前小时的缓存数据（启动时用）
    /// 当前未使用（load_from_db 已禁用），保留以备将来恢复
    #[allow(dead_code, clippy::type_complexity)]
    pub fn load_current_hour_usage(
        &self,
    ) -> Result<Vec<(String, i64, String, i64, u64, u64, u64)>> {
        let conn = self.conn();
        let hour_bucket = chrono::Utc::now().timestamp() / 3600;

        let mut stmt = conn.prepare(
            "SELECT dimension, key_id, model, hour_bucket, request_count, prompt_tokens, completion_tokens
             FROM usage_hourly
             WHERE hour_bucket = ?1",
        )?;

        let rows = stmt
            .query_map(params![hour_bucket], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, u64>(4)?,
                    row.get::<_, u64>(5)?,
                    row.get::<_, u64>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }
}
