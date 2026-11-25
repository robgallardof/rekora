//! Usage ledger: tokens per host session and per task.
//!
//! The host hook reports a session's *running totals* (read from the host's
//! own transcript) at the end of every turn. The ledger keeps the last totals
//! per session and adds the growth since the previous report to the task that
//! was active during that turn, so a task's usage accumulates across sessions,
//! hosts and agents. Records live in `meta` (`usage:<host>:<session>`,
//! `usage-task:<id>`), need no migration, and are bounded.
//!
//! Tokens are facts reported by the host; dollar amounts are computed from
//! them on display (`rekora_core::usage`) and are always estimates.

use rekora_core::usage::{ModelUsage, Usage, merge};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{Store, StoreError, now};

/// Most session records kept; older ones are pruned (task totals stay).
pub const USAGE_MAX_SESSIONS: usize = 200;

/// Last reported totals of one host session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionUsage {
    /// Host (`claude`, `codex`).
    pub host: String,
    /// Host session id.
    pub session: String,
    /// When the totals were last reported (UTC RFC 3339).
    pub at: String,
    /// Task the latest growth was attributed to, if any.
    #[serde(default)]
    pub task: Option<String>,
    /// Running totals per model.
    pub models: ModelUsage,
}

/// Accumulated usage of one task across sessions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskUsage {
    /// Task id.
    pub task: String,
    /// Last update (UTC RFC 3339).
    pub at: String,
    /// Usage per model.
    pub models: ModelUsage,
    /// Sessions that contributed.
    pub sessions: Vec<String>,
}

fn key_part(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(128)
        .collect()
}

impl Store {
    /// Records the running totals of a host session and attributes their
    /// growth since the previous report to `task` (when one is active).
    /// Returns the growth. Totals that shrink (a host rewrote its transcript)
    /// count as zero growth, never as negative usage.
    pub fn record_session_usage(
        &mut self,
        host: &str,
        session: &str,
        totals: &ModelUsage,
        task: Option<&str>,
    ) -> Result<ModelUsage, StoreError> {
        let host = key_part(host);
        let session = key_part(session);
        if host.is_empty() || session.is_empty() {
            return Ok(ModelUsage::new());
        }
        let key = format!("usage:{host}:{session}");
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let read = |key: &str| -> Result<Option<String>, rusqlite::Error> {
            tx.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
                .optional()
        };
        let previous: Option<SessionUsage> =
            read(&key)?.and_then(|v| serde_json::from_str(&v).ok());
        let mut growth = ModelUsage::new();
        for (model, now_usage) in totals {
            let before = previous
                .as_ref()
                .and_then(|p| p.models.get(model))
                .copied()
                .unwrap_or_default();
            let delta = now_usage.since(&before);
            if !delta.is_empty() {
                growth.insert(model.clone(), delta);
            }
        }
        let at = now();
        if let Some(task) = task.map(key_part).filter(|t| !t.is_empty())
            && !growth.is_empty()
        {
            let task_key = format!("usage-task:{task}");
            let mut record: TaskUsage = read(&task_key)?
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_else(|| TaskUsage {
                    task: task.clone(),
                    ..TaskUsage::default()
                });
            merge(&mut record.models, &growth);
            record.at = at.clone();
            let id = format!("{host}:{session}");
            if !record.sessions.contains(&id) {
                record.sessions.push(id);
            }
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![task_key, serde_json::to_string(&record).unwrap_or_default()],
            )?;
        }
        let record = SessionUsage {
            host,
            session,
            at,
            task: task.map(key_part).filter(|t| !t.is_empty()),
            models: totals.clone(),
        };
        tx.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(&record).unwrap_or_default()],
        )?;
        tx.execute(
            "DELETE FROM meta WHERE key LIKE 'usage:%' AND key NOT IN (
                 SELECT key FROM meta WHERE key LIKE 'usage:%'
                 ORDER BY json_extract(value, '$.at') DESC LIMIT ?1)",
            [USAGE_MAX_SESSIONS as i64],
        )?;
        tx.commit()?;
        Ok(growth)
    }

    /// Last reported totals of one host session, if any.
    pub fn session_usage(
        &self,
        host: &str,
        session: &str,
    ) -> Result<Option<SessionUsage>, StoreError> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [format!("usage:{}:{}", key_part(host), key_part(session))],
                |r| r.get(0),
            )
            .optional()?;
        Ok(value.and_then(|v| serde_json::from_str(&v).ok()))
    }

    /// Usage accumulated by a task, if any was recorded.
    pub fn task_usage(&self, task: &str) -> Result<Option<TaskUsage>, StoreError> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [format!("usage-task:{}", key_part(task))],
                |r| r.get(0),
            )
            .optional()?;
        Ok(value.and_then(|v| serde_json::from_str(&v).ok()))
    }

    /// Recorded sessions, most recent first.
    pub fn session_usages(&self, limit: usize) -> Result<Vec<SessionUsage>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT value FROM meta WHERE key LIKE 'usage:%'
             ORDER BY json_extract(value, '$.at') DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(record) = serde_json::from_str(&row?) {
                out.push(record);
            }
        }
        Ok(out)
    }

    /// Every task with recorded usage, most recently updated first.
    pub fn task_usages(&self) -> Result<Vec<TaskUsage>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT value FROM meta WHERE key LIKE 'usage-task:%'
             ORDER BY json_extract(value, '$.at') DESC",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(record) = serde_json::from_str(&row?) {
                out.push(record);
            }
        }
        Ok(out)
    }
}

/// Total usage across models (for compact displays).
pub fn total(models: &ModelUsage) -> Usage {
    let mut all = Usage::default();
    for usage in models.values() {
        all.add(usage);
    }
    all
}
