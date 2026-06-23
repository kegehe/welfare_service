use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::params;

use crate::db::Database;
use crate::error::Result;
use crate::scheduler::circuit_breaker::{CircuitBreakerSnapshot, CircuitState};

#[derive(Debug, Clone)]
pub struct PersistedCircuitState {
    pub key_id: i64,
    pub state: CircuitState,
    pub failure_count: u32,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub opened_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct PersistedCooldown {
    pub key_id: i64,
    pub remaining_secs: u64,
}

fn fmt_dt(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn parse_utc(value: Option<String>) -> Option<DateTime<Utc>> {
    let value = value?;
    NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc())
}

fn circuit_state_to_str(state: &CircuitState) -> &'static str {
    match state {
        CircuitState::Closed => "closed",
        CircuitState::Open => "open",
        CircuitState::HalfOpen => "half_open",
    }
}

fn parse_circuit_state(value: &str) -> CircuitState {
    match value {
        "open" => CircuitState::Open,
        "half_open" => CircuitState::HalfOpen,
        _ => CircuitState::Closed,
    }
}

impl Database {
    pub fn save_circuit_snapshot(&self, snapshots: &[CircuitBreakerSnapshot]) -> Result<()> {
        let conn = self.conn();
        let now = fmt_dt(Utc::now());

        for snapshot in snapshots {
            let state = circuit_state_to_str(&snapshot.state);
            let last_failure_at = snapshot.last_failure_at.map(fmt_dt);
            let next_retry_at = snapshot.opened_at.map(fmt_dt);

            conn.execute(
                "INSERT INTO circuit_states (key_id, state, failure_count, last_failure_at, next_retry_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(key_id) DO UPDATE SET
                    state = ?2,
                    failure_count = ?3,
                    last_failure_at = ?4,
                    next_retry_at = ?5",
                params![
                    snapshot.key_id,
                    state,
                    snapshot.failure_count as i64,
                    last_failure_at,
                    next_retry_at,
                ],
            )?;

            if snapshot.state == CircuitState::Closed && snapshot.failure_count == 0 {
                let _ = conn.execute(
                    "UPDATE circuit_states SET last_failure_at = NULL, next_retry_at = NULL WHERE key_id = ?1",
                    params![snapshot.key_id],
                );
            }
        }

        let _ = conn.execute(
            "DELETE FROM circuit_states WHERE key_id NOT IN (SELECT id FROM api_keys)",
            [],
        );
        tracing::debug!("已持久化熔断器状态 {} 条 ({})", snapshots.len(), now);
        Ok(())
    }

    pub fn load_circuit_states(&self) -> Result<Vec<PersistedCircuitState>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT key_id, state, failure_count, last_failure_at, next_retry_at
             FROM circuit_states
             WHERE key_id IN (SELECT id FROM api_keys WHERE status = 'active')",
        )?;

        let rows = stmt
            .query_map([], |row| {
                let state_str: String = row.get(1)?;
                let failure_count: i64 = row.get(2)?;
                let last_failure_at: Option<String> = row.get(3)?;
                let opened_at: Option<String> = row.get(4)?;
                Ok(PersistedCircuitState {
                    key_id: row.get(0)?,
                    state: parse_circuit_state(&state_str),
                    failure_count: failure_count.max(0) as u32,
                    last_failure_at: parse_utc(last_failure_at),
                    opened_at: parse_utc(opened_at),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn save_rate_limit_cooldowns(&self, cooldowns: &[(i64, u64)]) -> Result<()> {
        let conn = self.conn();
        let now = Utc::now();
        let now_str = fmt_dt(now);

        conn.execute("DELETE FROM token_bucket_states", [])?;
        for (key_id, remaining_secs) in cooldowns {
            let until = fmt_dt(now + chrono::TimeDelta::seconds(*remaining_secs as i64));
            conn.execute(
                "INSERT INTO token_bucket_states (key_id, tpm_remaining, rpm_remaining, updated_at)
                 VALUES (?1, 0, ?2, ?3)",
                params![key_id, *remaining_secs as i64, until],
            )?;
        }

        tracing::debug!("已持久化 429 冷却状态 {} 条 ({})", cooldowns.len(), now_str);
        Ok(())
    }

    pub fn load_rate_limit_cooldowns(&self) -> Result<Vec<PersistedCooldown>> {
        let conn = self.conn();
        let now = Utc::now();
        let mut stmt = conn.prepare(
            "SELECT key_id, updated_at
             FROM token_bucket_states
             WHERE key_id IN (SELECT id FROM api_keys WHERE status = 'active')",
        )?;

        let rows = stmt
            .query_map([], |row| {
                let key_id: i64 = row.get(0)?;
                let until: Option<String> = row.get(1)?;
                Ok((key_id, parse_utc(until)))
            })?
            .filter_map(|row| match row {
                Ok((key_id, Some(until))) if until > now => {
                    let remaining = (until - now).num_seconds().max(1) as u64;
                    Some(Ok(PersistedCooldown {
                        key_id,
                        remaining_secs: remaining,
                    }))
                }
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }
}
