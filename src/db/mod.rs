pub mod access_keys;
pub mod keys;
pub mod logs;
pub mod models;

use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::Path;

use crate::error::Result;

/// 线程安全的数据库连接包装
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开数据库并初始化表结构
    pub fn open(path: &Path) -> Result<Self> {
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::error::AppError::Database(rusqlite::Error::InvalidPath(
                    format!("创建数据库目录失败: {}", e).into(),
                ))
            })?;
        }

        let conn = Connection::open(path)?;

        // 启用 WAL 模式和外键约束
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_tables()?;
        Ok(db)
    }

    /// 初始化所有表结构
    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS api_keys (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                platform    TEXT NOT NULL,
                name        TEXT NOT NULL DEFAULT '',
                api_key     TEXT NOT NULL,
                key_hash    TEXT NOT NULL DEFAULT '',
                openai_url  TEXT NOT NULL,
                claude_url  TEXT NOT NULL,
                models      TEXT NOT NULL DEFAULT '[]',
                tpm_limit   INTEGER DEFAULT 0,
                rpm_limit   INTEGER DEFAULT 0,
                status      TEXT DEFAULT 'active',
                source      TEXT,
                note        TEXT,
                created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS request_logs (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                key_id      INTEGER REFERENCES api_keys(id) ON DELETE SET NULL,
                model       TEXT NOT NULL,
                status_code INTEGER,
                latency_ms  INTEGER,
                is_success  BOOLEAN,
                affects_key_health BOOLEAN DEFAULT 1,
                error_msg   TEXT,
                created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS circuit_states (
                key_id          INTEGER PRIMARY KEY REFERENCES api_keys(id) ON DELETE CASCADE,
                state           TEXT DEFAULT 'closed',
                failure_count   INTEGER DEFAULT 0,
                last_failure_at DATETIME,
                next_retry_at   DATETIME
            );

            CREATE TABLE IF NOT EXISTS token_bucket_states (
                key_id          INTEGER PRIMARY KEY REFERENCES api_keys(id) ON DELETE CASCADE,
                tpm_remaining   INTEGER NOT NULL,
                rpm_remaining   INTEGER NOT NULL,
                updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_request_logs_key_id ON request_logs(key_id);
            CREATE INDEX IF NOT EXISTS idx_request_logs_created ON request_logs(created_at);
            CREATE INDEX IF NOT EXISTS idx_api_keys_status ON api_keys(status);
            CREATE INDEX IF NOT EXISTS idx_api_keys_platform ON api_keys(platform);

            CREATE TABLE IF NOT EXISTS access_keys (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                key          TEXT NOT NULL UNIQUE,
                name         TEXT NOT NULL DEFAULT '',
                status       TEXT DEFAULT 'active',
                rpm_limit    INTEGER DEFAULT 0,
                tpm_limit    INTEGER DEFAULT 0,
                expires_at   DATETIME,
                last_used_at DATETIME,
                created_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at   DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_access_keys_status ON access_keys(status);
            ",
        )?;

        // 迁移: 为旧数据库添加 key_hash 列
        let has_key_hash: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('api_keys') WHERE name='key_hash'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0)
            > 0;

        if !has_key_hash {
            let _ = conn.execute(
                "ALTER TABLE api_keys ADD COLUMN key_hash TEXT NOT NULL DEFAULT ''",
                [],
            );
            tracing::info!("数据库迁移: 已添加 key_hash 列");
        }

        // 迁移: 为号池 Key 添加可选显示名称
        let has_name: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('api_keys') WHERE name='name'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0)
            > 0;

        if !has_name {
            let _ = conn.execute(
                "ALTER TABLE api_keys ADD COLUMN name TEXT NOT NULL DEFAULT ''",
                [],
            );
            tracing::info!("数据库迁移: 已添加 api_keys.name 列");
        }

        // 迁移: 标记哪些请求日志会影响 Key 健康判断。旧日志默认保留原语义。
        let has_affects_key_health: bool = conn
            .prepare(
                "SELECT COUNT(*) FROM pragma_table_info('request_logs') WHERE name='affects_key_health'",
            )
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0)
            > 0;

        if !has_affects_key_health {
            let _ = conn.execute(
                "ALTER TABLE request_logs ADD COLUMN affects_key_health BOOLEAN DEFAULT 1",
                [],
            );
            tracing::info!("数据库迁移: 已添加 affects_key_health 列");
        }

        let _ = conn.execute(
            "UPDATE request_logs
             SET affects_key_health = 0
             WHERE status_code BETWEEN 400 AND 499
               AND status_code NOT IN (401, 403)",
            [],
        );

        // 确保 key_hash 唯一索引存在 (放在迁移之后，兼容新旧数据库)
        let _ = conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash) WHERE key_hash != ''", []);

        Ok(())
    }

    /// 获取数据库连接的互斥锁引用
    pub fn conn(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.conn.lock()
    }

    /// 在事务中执行操作
    pub fn with_transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let conn = self.conn.lock();
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = f(&conn);
        match &result {
            Ok(_) => conn.execute_batch("COMMIT")?,
            Err(_) => conn.execute_batch("ROLLBACK")?,
        }
        result
    }
}
