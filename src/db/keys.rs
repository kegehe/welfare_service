use rusqlite::params;
use sha2::{Digest, Sha256};

use super::models::{AddApiKeyInput, ApiKeyRecord, UpdateApiKeyInput};
use crate::db::Database;
use crate::error::Result;

/// 计算 API Key 的 SHA-256 哈希 (用于去重)
pub(crate) fn hash_key(plaintext: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl Database {
    /// 添加一个新的 API Key (密钥已加密)
    pub fn add_key(&self, input: &AddApiKeyInput, encrypted_key: &str) -> Result<i64> {
        let conn = self.conn();
        let models_json = serde_json::to_string(&input.models).unwrap_or_default();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let key_hash = hash_key(&input.api_key);

        // 通过 SHA-256 哈希检查重复 (AES-GCM 随机 nonce 导致密文不同)
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM api_keys WHERE key_hash = ?1",
            params![key_hash],
            |row| {
                let count: i64 = row.get(0)?;
                Ok(count > 0)
            },
        )?;

        if exists {
            return Err(crate::error::AppError::BadRequest(
                "该 API Key 已存在".to_string(),
            ));
        }

        conn.execute(
            "INSERT INTO api_keys (platform, name, api_key, key_hash, openai_url, claude_url, models, tpm_limit, rpm_limit, source, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
            params![
                input.platform,
                input.name.as_deref().unwrap_or(""),
                encrypted_key,
                key_hash,
                input.openai_url,
                input.claude_url,
                models_json,
                input.tpm_limit.unwrap_or(0),
                input.rpm_limit.unwrap_or(0),
                input.source,
                input.note,
                now,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// 获取所有 active 状态的 Key
    pub fn get_active_keys(&self) -> Result<Vec<ApiKeyRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, platform, name, api_key, openai_url, claude_url, models, tpm_limit, rpm_limit, status, source, note, created_at, updated_at
             FROM api_keys WHERE status = 'active'",
        )?;

        let keys = stmt
            .query_map([], |row| {
                Ok(ApiKeyRecord {
                    id: row.get(0)?,
                    platform: row.get(1)?,
                    name: row.get(2)?,
                    api_key: row.get(3)?,
                    openai_url: row.get(4)?,
                    claude_url: row.get(5)?,
                    models: row.get(6)?,
                    tpm_limit: row.get(7)?,
                    rpm_limit: row.get(8)?,
                    status: row.get(9)?,
                    source: row.get(10)?,
                    note: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(keys)
    }

    /// 根据 ID 获取 Key
    #[allow(dead_code)]
    pub fn get_key_by_id(&self, id: i64) -> Result<Option<ApiKeyRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, platform, name, api_key, openai_url, claude_url, models, tpm_limit, rpm_limit, status, source, note, created_at, updated_at
             FROM api_keys WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(params![id], |row| {
            Ok(ApiKeyRecord {
                id: row.get(0)?,
                platform: row.get(1)?,
                name: row.get(2)?,
                api_key: row.get(3)?,
                openai_url: row.get(4)?,
                claude_url: row.get(5)?,
                models: row.get(6)?,
                tpm_limit: row.get(7)?,
                rpm_limit: row.get(8)?,
                status: row.get(9)?,
                source: row.get(10)?,
                note: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 删除 Key (事务保证原子性)
    pub fn remove_key(&self, id: i64) -> Result<bool> {
        self.with_transaction(|conn| {
            let affected = conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
            // 外键 ON DELETE CASCADE 自动清理关联记录
            Ok(affected > 0)
        })
    }

    /// 更新 Key 状态
    pub fn update_key_status(&self, id: i64, status: &str) -> Result<bool> {
        let conn = self.conn();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let affected = conn.execute(
            "UPDATE api_keys SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, id],
        )?;
        Ok(affected > 0)
    }

    /// 更新 Key 配置。encrypted_key/key_hash 为 None 时保留原密钥。
    pub fn update_key(
        &self,
        id: i64,
        input: &UpdateApiKeyInput,
        encrypted_key: Option<&str>,
        key_hash: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn();
        let models_json = serde_json::to_string(&input.models).unwrap_or_default();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        if let Some(hash) = key_hash {
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) FROM api_keys WHERE key_hash = ?1 AND id != ?2",
                params![hash, id],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )?;

            if exists {
                return Err(crate::error::AppError::BadRequest(
                    "该 API Key 已存在".to_string(),
                ));
            }
        }

        let affected = if let (Some(encrypted), Some(hash)) = (encrypted_key, key_hash) {
            conn.execute(
                "UPDATE api_keys
                 SET platform = ?1, name = ?2, api_key = ?3, key_hash = ?4, openai_url = ?5, claude_url = ?6,
                     models = ?7, tpm_limit = ?8, rpm_limit = ?9, source = ?10, note = ?11,
                     updated_at = ?12
                 WHERE id = ?13",
                params![
                    input.platform,
                    input.name.as_deref().unwrap_or(""),
                    encrypted,
                    hash,
                    input.openai_url,
                    input.claude_url,
                    models_json,
                    input.tpm_limit.unwrap_or(0),
                    input.rpm_limit.unwrap_or(0),
                    input.source,
                    input.note,
                    now,
                    id,
                ],
            )?
        } else {
            conn.execute(
                "UPDATE api_keys
                 SET platform = ?1, name = ?2, openai_url = ?3, claude_url = ?4, models = ?5,
                     tpm_limit = ?6, rpm_limit = ?7, source = ?8, note = ?9, updated_at = ?10
                 WHERE id = ?11",
                params![
                    input.platform,
                    input.name.as_deref().unwrap_or(""),
                    input.openai_url,
                    input.claude_url,
                    models_json,
                    input.tpm_limit.unwrap_or(0),
                    input.rpm_limit.unwrap_or(0),
                    input.source,
                    input.note,
                    now,
                    id,
                ],
            )?
        };

        Ok(affected > 0)
    }

    /// 获取所有 Key (包括非 active 的)
    pub fn get_all_keys(&self) -> Result<Vec<ApiKeyRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, platform, name, api_key, openai_url, claude_url, models, tpm_limit, rpm_limit, status, source, note, created_at, updated_at
             FROM api_keys ORDER BY id",
        )?;

        let keys = stmt
            .query_map([], |row| {
                Ok(ApiKeyRecord {
                    id: row.get(0)?,
                    platform: row.get(1)?,
                    name: row.get(2)?,
                    api_key: row.get(3)?,
                    openai_url: row.get(4)?,
                    claude_url: row.get(5)?,
                    models: row.get(6)?,
                    tpm_limit: row.get(7)?,
                    rpm_limit: row.get(8)?,
                    status: row.get(9)?,
                    source: row.get(10)?,
                    note: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(keys)
    }
}
