//! Durable, transactional storage for Rekora memories on SQLite.
//!
//! **One scope per database file.** A project's memories live in
//! `<root>/.rekora/rekora.db` and nothing else does. Every query still filters
//! by `scope_id`, but the real isolation guarantee is physical: FTS5 ranking
//! statistics (BM25 document frequencies) are computed per table, so keeping
//! other scopes out of the file is what guarantees they cannot influence ranks
//! (master prompt §5.1, test matrix R03).
//!
//! Writes use `BEGIN IMMEDIATE` so concurrent processes serialise on the write
//! lock up front instead of failing on lock upgrade; WAL mode lets readers
//! proceed during writes. SQLite is bundled (a native C dependency) so FTS5 is
//! always available regardless of the system library.

mod code;
mod context;
mod schema;
mod search;
mod tasks;
mod transfer;

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rekora_core::memory::ValidatedMemory;
use rekora_core::{CoreError, MemoryKind, MemoryStatus, Origin, SourceRef};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub use code::{FileUpdate, Symbol, SymbolHit};
pub use context::{ContextPack, PACK_HEADER, PackItem, render_line, short_id};
pub use schema::{MIGRATIONS, current_version, migrate, snapshot_path};
pub use search::{Freshness, Hit, ScoreExplain, SearchOptions, SearchOutcome, SearchStatus};
pub use tasks::{HANDOFF_SCHEMA, Handoff, Task};
pub use transfer::{
    EXPORT_FORMAT, ExportFile, ExportedMemory, ImportReport, MAX_IMPORT_BYTES, Rejected,
};

/// How long a writer waits for another process's write lock before failing.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(10);

/// Storage-level failure. `code()` is stable for structured output.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// SQLite reported an error.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A domain rule rejected the input.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// The database was written by a newer Rekora.
    #[error(
        "database schema v{found} is newer than this binary supports (v{supported}); upgrade rekora"
    )]
    SchemaTooNew {
        /// Version found on disk.
        found: u32,
        /// Highest version this binary knows.
        supported: u32,
    },
    /// No database exists at the expected path.
    #[error("no Rekora store at {0}; run `rekora init` first")]
    NotInitialized(PathBuf),
    /// No memory matches the id.
    #[error("no memory with id `{0}` in this scope")]
    NotFound(String),
    /// An id prefix matched several memories.
    #[error("id prefix `{0}` is ambiguous; use more characters")]
    Ambiguous(String),
    /// The requested state change is not allowed.
    #[error("invalid transition: {0}")]
    InvalidTransition(String),
    /// Optimistic concurrency check failed: someone else changed the memory.
    #[error("version conflict: expected v{expected}, current v{current}")]
    VersionConflict {
        /// Version the caller based its change on.
        expected: i64,
        /// Version currently stored.
        current: i64,
    },
    /// An import file is not a valid Rekora export.
    #[error("invalid import: {0}")]
    InvalidImport(String),
    /// A filesystem error outside SQLite.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl StoreError {
    /// Stable error code for structured output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Sqlite(_) => "database_error",
            Self::Core(e) => e.code(),
            Self::SchemaTooNew { .. } => "schema_too_new",
            Self::NotInitialized(_) => "not_initialized",
            Self::NotFound(_) => "not_found",
            Self::Ambiguous(_) => "ambiguous_id",
            Self::InvalidTransition(_) => "invalid_transition",
            Self::VersionConflict { .. } => "version_conflict",
            Self::InvalidImport(_) => "invalid_import",
            Self::Io(_) => "io_error",
        }
    }
}

/// A persisted memory as read back from the store.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryRecord {
    /// Stable identifier (UUIDv7).
    pub id: String,
    /// Scope the memory belongs to.
    pub scope_id: String,
    /// Knowledge kind.
    pub kind: MemoryKind,
    /// Verification state.
    pub status: MemoryStatus,
    /// Producer / trust tier.
    pub origin: Origin,
    /// Redacted body.
    pub content: String,
    /// Evidence, in citation order.
    pub sources: Vec<SourceRef>,
    /// Branch observed at creation.
    pub branch: Option<String>,
    /// Commit observed at creation.
    pub commit: Option<String>,
    /// UTC RFC 3339 creation time.
    pub created_at: String,
    /// UTC RFC 3339 last update time.
    pub updated_at: String,
    /// Incremented on every state change (optimistic concurrency).
    pub version: i64,
    /// Replacement memory when superseded.
    pub superseded_by: Option<String>,
}

/// Result of [`Store::add`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AddOutcome {
    /// The stored memory (new or pre-existing).
    pub memory: MemoryRecord,
    /// True when an identical memory already existed and nothing was written.
    pub deduplicated: bool,
    /// Kinds of secrets masked before storage.
    pub redactions: Vec<&'static str>,
    /// True when the requested status was downgraded to `candidate`.
    pub status_downgraded: bool,
}

/// One audit-log entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    /// Monotonic sequence number.
    pub seq: i64,
    /// Memory the event refers to (may no longer exist).
    pub memory_id: String,
    /// `create`, `status`, `supersede`, `delete`.
    pub action: String,
    /// Who performed it (e.g. `cli`, `mcp`).
    pub actor: String,
    /// UTC RFC 3339 timestamp.
    pub at: String,
    /// JSON details.
    pub detail: serde_json::Value,
}

/// Counts for `status` / `doctor`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Stats {
    /// Memories per status.
    pub by_status: Vec<(String, i64)>,
    /// Total memories.
    pub total: i64,
    /// Total audit events.
    pub events: i64,
}

/// An open handle on one scope's database.
pub struct Store {
    conn: Connection,
    path: PathBuf,
    scope_id: String,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("path", &self.path)
            .field("scope_id", &self.scope_id)
            .finish()
    }
}

impl Store {
    /// Creates the database (and parent directory) if missing, migrates it and
    /// assigns a scope id on first creation. Returns `(store, created)`.
    /// Safe to call repeatedly.
    pub fn init(path: &Path) -> Result<(Self, bool), StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existed = path.exists();
        let mut conn = connect(path, true)?;
        schema::migrate(&mut conn, path, MIGRATIONS)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row("SELECT value FROM meta WHERE key = 'scope_id'", [], |r| {
                r.get(0)
            })
            .optional()?;
        let (scope_id, created) = match existing {
            Some(id) => (id, false),
            None => {
                let id = uuid::Uuid::now_v7().to_string();
                tx.execute(
                    "INSERT INTO meta (key, value) VALUES ('scope_id', ?1), ('created_at', ?2)",
                    params![id, now()],
                )?;
                (id, true)
            }
        };
        tx.commit()?;
        Ok((
            Self {
                conn,
                path: path.to_owned(),
                scope_id,
            },
            created || !existed,
        ))
    }

    /// Opens an existing database. Never creates one: a missing store is a
    /// user-visible `not_initialized` error, not a silent empty result.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if !path.is_file() {
            return Err(StoreError::NotInitialized(path.to_owned()));
        }
        let mut conn = connect(path, false)?;
        schema::migrate(&mut conn, path, MIGRATIONS)?;
        let scope_id: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = 'scope_id'", [], |r| {
                r.get(0)
            })
            .optional()?;
        let scope_id = scope_id.ok_or_else(|| StoreError::NotInitialized(path.to_owned()))?;
        Ok(Self {
            conn,
            path: path.to_owned(),
            scope_id,
        })
    }

    /// Scope identifier of this database.
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }

    /// Database file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Persists a validated memory, or returns the existing one when the same
    /// content fingerprint is already stored (idempotent retry).
    pub fn add(
        &mut self,
        validated: ValidatedMemory,
        actor: &str,
    ) -> Result<AddOutcome, StoreError> {
        let ValidatedMemory {
            memory,
            redactions,
            status_downgraded,
        } = validated;
        let redactions: Vec<&'static str> = redactions.iter().map(|r| r.kind).collect();
        let fingerprint = memory.fingerprint();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM memories WHERE scope_id = ?1 AND fingerprint = ?2",
                params![self.scope_id, fingerprint],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            tx.commit()?;
            return Ok(AddOutcome {
                memory: self.get(&id)?,
                deduplicated: true,
                redactions,
                status_downgraded,
            });
        }
        let id = uuid::Uuid::now_v7().to_string();
        let at = now();
        tx.execute(
            "INSERT INTO memories (id, scope_id, kind, status, origin, content, fingerprint,
                                   branch, commit_sha, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                id,
                self.scope_id,
                memory.kind.as_str(),
                memory.status.as_str(),
                memory.origin.as_str(),
                memory.content,
                fingerprint,
                memory.branch,
                memory.commit,
                at,
            ],
        )?;
        for (ord, source) in memory.sources.iter().enumerate() {
            tx.execute(
                "INSERT INTO sources (memory_id, ord, path, line, content_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, ord as i64, source.path, source.line, source.content_hash],
            )?;
        }
        let detail = serde_json::json!({
            "kind": memory.kind,
            "status": memory.status,
            "origin": memory.origin,
            "sources": memory.sources.iter().map(SourceRef::location).collect::<Vec<_>>(),
            "redactions": redactions,
        });
        record_event(&tx, &id, "create", actor, &at, &detail)?;
        tx.commit()?;
        Ok(AddOutcome {
            memory: self.get(&id)?,
            deduplicated: false,
            redactions,
            status_downgraded,
        })
    }

    /// Resolves a full id or a unique prefix (at least 8 characters).
    pub fn resolve_id(&self, id_or_prefix: &str) -> Result<String, StoreError> {
        let needle = id_or_prefix.trim();
        if needle.len() < 8 || !needle.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            return Err(StoreError::NotFound(needle.to_owned()));
        }
        let mut stmt = self.conn.prepare(
            "SELECT id FROM memories WHERE scope_id = ?1 AND id LIKE ?2 || '%' ESCAPE '\\' LIMIT 2",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![self.scope_id, needle.to_ascii_lowercase()], |r| {
                r.get(0)
            })?
            .collect::<Result<_, _>>()?;
        match ids.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(StoreError::NotFound(needle.to_owned())),
            _ => Err(StoreError::Ambiguous(needle.to_owned())),
        }
    }

    /// Reads one memory by full id or unique prefix.
    pub fn get(&self, id_or_prefix: &str) -> Result<MemoryRecord, StoreError> {
        let id = self.resolve_id(id_or_prefix)?;
        let mut record = self.conn.query_row(
            "SELECT id, scope_id, kind, status, origin, content, branch, commit_sha,
                    created_at, updated_at, version, superseded_by
             FROM memories WHERE scope_id = ?1 AND id = ?2",
            params![self.scope_id, id],
            read_record,
        )?;
        record.sources = self.sources(&record.id)?;
        Ok(record)
    }

    fn sources(&self, id: &str) -> Result<Vec<SourceRef>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT path, line, content_hash FROM sources WHERE memory_id = ?1 ORDER BY ord",
        )?;
        let rows = stmt.query_map([id], |r| {
            Ok(SourceRef {
                path: r.get(0)?,
                line: r.get(1)?,
                content_hash: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Changes the verification status (`verified`, `candidate`, `disputed`).
    /// Use [`Store::supersede`] for replacement. `expected_version`, when given,
    /// makes the update fail with `version_conflict` if someone else changed
    /// the memory first (optimistic concurrency).
    pub fn set_status(
        &mut self,
        id_or_prefix: &str,
        status: MemoryStatus,
        expected_version: Option<i64>,
        actor: &str,
    ) -> Result<MemoryRecord, StoreError> {
        if status == MemoryStatus::Superseded {
            return Err(StoreError::InvalidTransition(
                "use supersede to mark a memory as replaced".into(),
            ));
        }
        let id = self.resolve_id(id_or_prefix)?;
        let at = now();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (from, version): (String, i64) = tx.query_row(
            "SELECT status, version FROM memories WHERE scope_id = ?1 AND id = ?2",
            params![self.scope_id, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if from == MemoryStatus::Superseded.as_str() {
            return Err(StoreError::InvalidTransition(format!(
                "memory {id} is superseded and cannot change status"
            )));
        }
        if let Some(expected) = expected_version
            && expected != version
        {
            return Err(StoreError::VersionConflict {
                expected,
                current: version,
            });
        }
        tx.execute(
            "UPDATE memories SET status = ?1, updated_at = ?2, version = version + 1 WHERE id = ?3",
            params![status.as_str(), at, id],
        )?;
        record_event(
            &tx,
            &id,
            "status",
            actor,
            &at,
            &serde_json::json!({ "from": from, "to": status }),
        )?;
        tx.commit()?;
        self.get(&id)
    }

    /// Marks `old` as superseded by `new`. Both must be in this scope and `new`
    /// must itself be current. The old memory stays readable for history but is
    /// excluded from search results.
    pub fn supersede(
        &mut self,
        old: &str,
        new: &str,
        actor: &str,
    ) -> Result<MemoryRecord, StoreError> {
        let old_id = self.resolve_id(old)?;
        let new_id = self.resolve_id(new)?;
        if old_id == new_id {
            return Err(StoreError::InvalidTransition(
                "a memory cannot supersede itself".into(),
            ));
        }
        let new_status = self.get(&new_id)?.status;
        if !new_status.is_current() {
            return Err(StoreError::InvalidTransition(format!(
                "replacement {new_id} is itself superseded"
            )));
        }
        let at = now();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE memories SET status = 'superseded', superseded_by = ?1, updated_at = ?2,
                    version = version + 1
             WHERE scope_id = ?3 AND id = ?4 AND status != 'superseded'",
            params![new_id, at, self.scope_id, old_id],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidTransition(format!(
                "memory {old_id} is already superseded"
            )));
        }
        record_event(
            &tx,
            &old_id,
            "supersede",
            actor,
            &at,
            &serde_json::json!({ "by": new_id }),
        )?;
        tx.commit()?;
        self.get(&old_id)
    }

    /// Permanently deletes a memory and its sources. The audit log keeps that
    /// the deletion happened (id, kind, actor, time) but **not the content**:
    /// deleting is how users remove knowledge they do not want retained.
    pub fn delete(&mut self, id_or_prefix: &str, actor: &str) -> Result<(), StoreError> {
        let id = self.resolve_id(id_or_prefix)?;
        let at = now();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let kind: String = tx.query_row(
            "SELECT kind FROM memories WHERE scope_id = ?1 AND id = ?2",
            params![self.scope_id, id],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE memories SET superseded_by = NULL WHERE superseded_by = ?1",
            [&id],
        )?;
        tx.execute("DELETE FROM memories WHERE id = ?1", [&id])?;
        record_event(
            &tx,
            &id,
            "delete",
            actor,
            &at,
            &serde_json::json!({ "kind": kind }),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Audit history of one memory id (works after deletion with the full id).
    pub fn history(&self, id: &str) -> Result<Vec<Event>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, memory_id, action, actor, at, detail FROM events WHERE memory_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map([id], |r| {
            let detail: String = r.get(5)?;
            Ok(Event {
                seq: r.get(0)?,
                memory_id: r.get(1)?,
                action: r.get(2)?,
                actor: r.get(3)?,
                at: r.get(4)?,
                detail: serde_json::from_str(&detail).unwrap_or(serde_json::Value::Null),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Counts per status and audit events.
    pub fn stats(&self) -> Result<Stats, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT status, COUNT(*) FROM memories WHERE scope_id = ?1 GROUP BY status ORDER BY status",
        )?;
        let by_status: Vec<(String, i64)> = stmt
            .query_map([&self.scope_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let total = by_status.iter().map(|(_, n)| n).sum();
        let events = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
        Ok(Stats {
            by_status,
            total,
            events,
        })
    }

    /// Runs SQLite's `integrity_check` and the FTS5 index consistency check.
    /// Returns human-readable problems; empty means healthy.
    pub fn check(&self) -> Result<Vec<String>, StoreError> {
        let mut problems = Vec::new();
        let mut stmt = self.conn.prepare("PRAGMA integrity_check")?;
        for row in stmt.query_map([], |r| r.get::<_, String>(0))? {
            let row = row?;
            if row != "ok" {
                problems.push(format!("integrity: {row}"));
            }
        }
        if let Err(e) = self.conn.execute(
            "INSERT INTO memories_fts (memories_fts) VALUES ('integrity-check')",
            [],
        ) {
            problems.push(format!("fts index: {e}"));
        }
        Ok(problems)
    }

    /// Schema version of the open database.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        schema::version(&self.conn)
    }
}

fn connect(path: &Path, create: bool) -> Result<Connection, StoreError> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let conn = Connection::open_with_flags(path, flags)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

fn read_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRecord> {
    fn parse<T: std::str::FromStr>(r: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<T> {
        let raw: String = r.get(idx)?;
        raw.parse().map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                idx,
                rusqlite::types::Type::Text,
                format!("unknown enum value `{raw}`").into(),
            )
        })
    }
    Ok(MemoryRecord {
        id: r.get(0)?,
        scope_id: r.get(1)?,
        kind: parse(r, 2)?,
        status: parse(r, 3)?,
        origin: parse(r, 4)?,
        content: r.get(5)?,
        sources: Vec::new(),
        branch: r.get(6)?,
        commit: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
        version: r.get(10)?,
        superseded_by: r.get(11)?,
    })
}

fn record_event(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
    action: &str,
    actor: &str,
    at: &str,
    detail: &serde_json::Value,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO events (memory_id, action, actor, at, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, action, actor, at, detail.to_string()],
    )?;
    Ok(())
}

/// Current UTC time as RFC 3339; storage never holds local time.
fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}
