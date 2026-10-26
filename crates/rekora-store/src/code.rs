//! Storage for the code index: file hashes and declared symbols per snapshot.
//!
//! The indexer (crate `rekora-code`) decides *what* changed; this module only
//! persists a batch atomically, so an interrupted scan leaves the previous
//! index intact rather than half of each.

use std::collections::HashMap;

use rusqlite::{TransactionBehavior, params};
use serde::Serialize;

use crate::{Store, StoreError, now};

/// A declared symbol found by a parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Symbol {
    /// Identifier as written.
    pub name: String,
    /// Declaration kind (`function`, `class`, `struct`, `method`, ...).
    pub kind: String,
    /// 1-based line of the declaration.
    pub line: u32,
}

/// New or changed file to persist.
#[derive(Debug, Clone, PartialEq)]
pub struct FileUpdate {
    /// Root-relative path with `/` separators.
    pub path: String,
    /// BLAKE3 of the content.
    pub hash: String,
    /// Language id, or `None` when only file-level evidence is kept.
    pub language: Option<String>,
    /// Size in bytes.
    pub size: u64,
    /// Declared symbols (empty for unsupported languages).
    pub symbols: Vec<Symbol>,
}

/// A symbol search result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SymbolHit {
    /// Snapshot (branch) the symbol was indexed from.
    pub snapshot: String,
    /// File path.
    pub path: String,
    /// Symbol.
    pub name: String,
    /// Kind.
    pub kind: String,
    /// Line.
    pub line: u32,
    /// Language of the file.
    pub language: Option<String>,
}

impl Store {
    /// Stored `path -> hash` for a snapshot.
    pub fn indexed_hashes(&self, snapshot: &str) -> Result<HashMap<String, String>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, hash FROM code_files WHERE scope_id = ?1 AND snapshot = ?2")?;
        let rows = stmt.query_map(params![self.scope_id, snapshot], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Replaces changed files and removes deleted ones in one transaction.
    pub fn apply_index(
        &mut self,
        snapshot: &str,
        updates: &[FileUpdate],
        removed: &[String],
    ) -> Result<(), StoreError> {
        let at = now();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for path in removed {
            tx.execute(
                "DELETE FROM code_files WHERE scope_id = ?1 AND snapshot = ?2 AND path = ?3",
                params![self.scope_id, snapshot, path],
            )?;
        }
        for file in updates {
            // ON DELETE CASCADE clears the old symbols with the old row.
            tx.execute(
                "DELETE FROM code_files WHERE scope_id = ?1 AND snapshot = ?2 AND path = ?3",
                params![self.scope_id, snapshot, file.path],
            )?;
            tx.execute(
                "INSERT INTO code_files (scope_id, snapshot, path, hash, language, size, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    self.scope_id,
                    snapshot,
                    file.path,
                    file.hash,
                    file.language,
                    file.size as i64,
                    at
                ],
            )?;
            let mut insert = tx.prepare_cached(
                "INSERT INTO code_symbols (scope_id, snapshot, path, name, kind, line)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for symbol in &file.symbols {
                insert.execute(params![
                    self.scope_id,
                    snapshot,
                    file.path,
                    symbol.name,
                    symbol.kind,
                    symbol.line
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Case-insensitive symbol lookup: exact matches first, then prefix, then
    /// substring. `snapshot = None` searches every indexed branch.
    pub fn find_symbols(
        &self,
        name: &str,
        snapshot: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SymbolHit>, StoreError> {
        let needle = name.trim();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let escaped = needle
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let mut stmt = self.conn.prepare_cached(
            "SELECT s.snapshot, s.path, s.name, s.kind, s.line, f.language
             FROM code_symbols s
             JOIN code_files f ON f.scope_id = s.scope_id AND f.snapshot = s.snapshot AND f.path = s.path
             WHERE s.scope_id = ?1 AND (?2 IS NULL OR s.snapshot = ?2)
               AND s.name LIKE '%' || ?3 || '%' ESCAPE '\\'
             ORDER BY (s.name = ?4 COLLATE NOCASE) DESC,
                      (s.name LIKE ?3 || '%' ESCAPE '\\') DESC,
                      s.name, s.path, s.line
             LIMIT ?5",
        )?;
        let rows = stmt.query_map(
            params![
                self.scope_id,
                snapshot,
                escaped,
                needle,
                limit.clamp(1, 200) as i64
            ],
            |r| {
                Ok(SymbolHit {
                    snapshot: r.get(0)?,
                    path: r.get(1)?,
                    name: r.get(2)?,
                    kind: r.get(3)?,
                    line: r.get(4)?,
                    language: r.get(5)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Symbols declared in one file of a snapshot, in line order.
    pub fn file_symbols(&self, snapshot: &str, path: &str) -> Result<Vec<Symbol>, StoreError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT name, kind, line FROM code_symbols
             WHERE scope_id = ?1 AND snapshot = ?2 AND path = ?3 ORDER BY line, name",
        )?;
        let rows = stmt.query_map(params![self.scope_id, snapshot, path], |r| {
            Ok(Symbol {
                name: r.get(0)?,
                kind: r.get(1)?,
                line: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
