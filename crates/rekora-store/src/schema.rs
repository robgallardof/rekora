//! Versioned schema migrations.
//!
//! The schema version lives in SQLite's `PRAGMA user_version`. Each migration
//! runs in its own `IMMEDIATE` transaction together with the version bump, so a
//! crash leaves the database at either the old or the new version — never in
//! between (test matrix R11). Before upgrading a database that already holds
//! data, a snapshot is written with `VACUUM INTO` next to the database file.
//!
//! A database written by a newer binary is refused rather than "best effort"
//! opened, to avoid accidental downgrade writes (R30).

use std::path::{Path, PathBuf};

use rusqlite::{Connection, TransactionBehavior};

use crate::StoreError;

/// Ordered migrations; index `i` upgrades from version `i` to `i + 1`.
pub const MIGRATIONS: &[&str] = &[MIGRATION_1];

/// Schema version this binary reads and writes.
pub fn current_version() -> u32 {
    MIGRATIONS.len() as u32
}

const MIGRATION_1: &str = r#"
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE memories (
    rowid         INTEGER PRIMARY KEY,
    id            TEXT NOT NULL UNIQUE,
    scope_id      TEXT NOT NULL,
    kind          TEXT NOT NULL,
    status        TEXT NOT NULL,
    origin        TEXT NOT NULL,
    content       TEXT NOT NULL,
    fingerprint   TEXT NOT NULL,
    branch        TEXT,
    commit_sha    TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    version       INTEGER NOT NULL DEFAULT 1,
    superseded_by TEXT REFERENCES memories(id) ON DELETE SET NULL,
    UNIQUE (scope_id, fingerprint)
) STRICT;

CREATE INDEX memories_scope_status ON memories (scope_id, status);

CREATE TABLE sources (
    memory_id    TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    ord          INTEGER NOT NULL,
    path         TEXT NOT NULL,
    line         INTEGER,
    content_hash TEXT NOT NULL,
    PRIMARY KEY (memory_id, ord)
) STRICT;

-- Append-only audit log. No foreign key: events must outlive deleted memories.
CREATE TABLE events (
    seq       INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_id TEXT NOT NULL,
    action    TEXT NOT NULL,
    actor     TEXT NOT NULL,
    at        TEXT NOT NULL,
    detail    TEXT NOT NULL
) STRICT;

CREATE INDEX events_memory ON events (memory_id, seq);

-- External-content FTS index kept in sync by triggers. remove_diacritics 2 lets
-- "como" match "cómo", which matters for Spanish-language projects.
CREATE VIRTUAL TABLE memories_fts USING fts5(
    content,
    content = 'memories',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts (rowid, content) VALUES (new.rowid, new.content);
END;
CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts (memories_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
END;
CREATE TRIGGER memories_au AFTER UPDATE OF content ON memories BEGIN
    INSERT INTO memories_fts (memories_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
    INSERT INTO memories_fts (rowid, content) VALUES (new.rowid, new.content);
END;
"#;

/// Reads `PRAGMA user_version`.
pub fn version(conn: &Connection) -> Result<u32, StoreError> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

/// Brings `conn` up to `migrations.len()`. Returns the snapshot path when one
/// was taken. `db_path` is only used to place the snapshot.
///
/// Taking `migrations` as a parameter lets tests inject a failing migration.
pub fn migrate(
    conn: &mut Connection,
    db_path: &Path,
    migrations: &[&str],
) -> Result<Option<PathBuf>, StoreError> {
    let found = version(conn)?;
    let supported = migrations.len() as u32;
    if found > supported {
        return Err(StoreError::SchemaTooNew { found, supported });
    }
    if found == supported {
        return Ok(None);
    }
    let snapshot = if found > 0 {
        let path = snapshot_path(db_path, found);
        // VACUUM INTO refuses to overwrite; a stale snapshot from an earlier
        // interrupted upgrade is the same version, so replacing it is safe.
        let _ = std::fs::remove_file(&path);
        conn.execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])?;
        Some(path)
    } else {
        None
    };
    for (index, sql) in migrations.iter().enumerate().skip(found as usize) {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as u32 + 1)?;
        tx.commit()?;
    }
    Ok(snapshot)
}

/// Where the pre-upgrade snapshot for `version` is written.
pub fn snapshot_path(db_path: &Path, version: u32) -> PathBuf {
    let mut name = db_path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| "rekora.db".into());
    name.push(format!(".v{version}.bak"));
    db_path.with_file_name(name)
}
