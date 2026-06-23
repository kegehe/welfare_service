use rusqlite::params;

use super::models::{AccessKeyRecord, CreateAccessKeyInput, UpdateAccessKeyInput};
use crate::db::Database;
use crate::error::Result;

/// 从行中读取 AccessKeyRecord（所有查询共用）
fn read_access_key_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AccessKeyRecord> {
    Ok(AccessKeyRecord {
        id: row.get(0)?,
        key: row.get(1)?,
        name: row.get(2)?,
        status: row.get(3)?,
        rpm_limit: row.get(4)?,
        tpm_limit: row.get(5)?,
        expires_at: row.get(6)?,
        last_used_at: row.get(7)?,
        total_requests: row.get(8).unwrap_or(0),
        total_prompt_tokens: row.get(9).unwrap_or(0),
        total_completion_tokens: row.get(10).unwrap_or(0),
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

const ACCESS_KEY_COLUMNS: &str = "id, key, name, status, rpm_limit, tpm_limit, expires_at, last_used_at, total_requests, total_prompt_tokens, total_completion_tokens, created_at, updated_at";

impl Database {
    /// 添加一个新的访问 Key
    pub fn add_access_key(&self, key: &str, input: &CreateAccessKeyInput) -> Result<i64> {
        let conn = self.conn();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let name = input.name.as_deref().unwrap_or("");
        let rpm_limit = input.rpm_limit.unwrap_or(0);
        let tpm_limit = input.tpm_limit.unwrap_or(0);

        // 解析过期时间
        let expires_at: Option<String> = input
            .expires_at
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        conn.execute(
            "INSERT INTO access_keys (key, name, rpm_limit, tpm_limit, expires_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![key, name, rpm_limit, tpm_limit, expires_at, now],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// 根据 key 字符串查找访问 Key
    pub fn get_access_key_by_key(&self, key: &str) -> Result<AccessKeyRecord> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM access_keys WHERE key = ?1", ACCESS_KEY_COLUMNS),
        )?;

        stmt.query_row(params![key], read_access_key_row)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    crate::error::AppError::Unauthorized("无效的 API Key".to_string())
                }
                other => crate::error::AppError::Database(other),
            })
    }

    /// 获取所有访问 Key
    pub fn get_all_access_keys(&self) -> Result<Vec<AccessKeyRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM access_keys ORDER BY id", ACCESS_KEY_COLUMNS),
        )?;

        let keys = stmt
            .query_map([], read_access_key_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(keys)
    }

    /// 删除访问 Key
    pub fn remove_access_key(&self, id: i64) -> Result<bool> {
        let conn = self.conn();
        let affected = conn.execute("DELETE FROM access_keys WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    /// 更新访问 Key 状态
    pub fn update_access_key_status(&self, id: i64, status: &str) -> Result<bool> {
        let conn = self.conn();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let affected = conn.execute(
            "UPDATE access_keys SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, id],
        )?;
        Ok(affected > 0)
    }

    /// 更新访问 Key 配置
    pub fn update_access_key(&self, id: i64, input: &UpdateAccessKeyInput) -> Result<bool> {
        let conn = self.conn();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let name = input.name.as_deref().unwrap_or("");
        let rpm_limit = input.rpm_limit.unwrap_or(0);
        let tpm_limit = input.tpm_limit.unwrap_or(0);
        let expires_at: Option<String> = input
            .expires_at
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let affected = conn.execute(
            "UPDATE access_keys
             SET name = ?1, rpm_limit = ?2, tpm_limit = ?3, expires_at = ?4, updated_at = ?5
             WHERE id = ?6",
            params![name, rpm_limit, tpm_limit, expires_at, now, id],
        )?;
        Ok(affected > 0)
    }

    /// 更新最后使用时间
    pub fn update_access_key_last_used(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        conn.execute(
            "UPDATE access_keys SET last_used_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// 获取所有活跃的访问 Key (用于启动时注册限流器)
    pub fn get_active_access_keys(&self) -> Result<Vec<AccessKeyRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM access_keys WHERE status = 'active'", ACCESS_KEY_COLUMNS),
        )?;

        let keys = stmt
            .query_map([], read_access_key_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(keys)
    }

    /// 增量更新访问 Key 的累计统计字段 (供外部直接调用)
    #[allow(dead_code)]
    pub fn increment_access_key_totals(
        &self,
        id: i64,
        requests: i64,
        prompt_tokens: i64,
        completion_tokens: i64,
    ) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE access_keys
             SET total_requests = total_requests + ?1,
                 total_prompt_tokens = total_prompt_tokens + ?2,
                 total_completion_tokens = total_completion_tokens + ?3
             WHERE id = ?4",
            params![requests, prompt_tokens, completion_tokens, id],
        )?;
        Ok(())
    }
}
