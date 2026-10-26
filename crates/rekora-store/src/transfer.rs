//! Export and import of a scope's memories as versioned JSON.
//!
//! Export writes every memory of the scope (superseded ones included, for
//! history) plus tasks. Import is deliberately conservative:
//!
//! - imported text is untrusted: origin becomes `import`, status `candidate`
//!   (or `disputed` if it was disputed); nothing arrives `verified`;
//! - superseded memories are skipped so old truth does not come back as current;
//! - source paths are re-validated without touching the filesystem;
//! - memories are deduplicated by content fingerprint, so applying the same
//!   file twice changes nothing (test matrix R14);
//! - `dry_run` reports exactly what would happen and writes nothing.
//!
//! Tasks are exported for completeness but not imported: a task's state is
//! owned by whoever is working on it, and merging foreign task state silently
//! is the kind of overwrite §9 forbids.

use rekora_core::memory::ValidatedMemory;
use rekora_core::source::check_relative;
use rekora_core::{MemoryKind, MemoryStatus, NewMemory, Origin, SourceRef};
use rusqlite::{TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::tasks::Task;
use crate::{MemoryRecord, Store, StoreError, now, record_event};

/// Format tag written to and required in export files.
pub const EXPORT_FORMAT: &str = "rekora.export/1";
/// Largest import file accepted, in bytes.
pub const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

/// The export document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFile {
    /// Must equal [`EXPORT_FORMAT`].
    pub format: String,
    /// Scope the data came from (informational).
    pub scope_id: String,
    /// UTC RFC 3339.
    pub exported_at: String,
    /// Memories.
    pub memories: Vec<ExportedMemory>,
    /// Tasks (informational; not imported).
    #[serde(default)]
    pub tasks: Vec<serde_json::Value>,
}

/// One memory in an export file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMemory {
    /// Original id (not reused on import).
    pub id: String,
    /// Kind.
    pub kind: MemoryKind,
    /// Status at export time.
    pub status: MemoryStatus,
    /// Origin at export time.
    pub origin: Origin,
    /// Body.
    pub content: String,
    /// Evidence.
    #[serde(default)]
    pub sources: Vec<SourceRef>,
    /// Branch.
    #[serde(default)]
    pub branch: Option<String>,
    /// Commit.
    #[serde(default)]
    pub commit: Option<String>,
    /// Creation time.
    #[serde(default)]
    pub created_at: Option<String>,
}

impl From<&MemoryRecord> for ExportedMemory {
    fn from(m: &MemoryRecord) -> Self {
        Self {
            id: m.id.clone(),
            kind: m.kind,
            status: m.status,
            origin: m.origin,
            content: m.content.clone(),
            sources: m.sources.clone(),
            branch: m.branch.clone(),
            commit: m.commit.clone(),
            created_at: Some(m.created_at.clone()),
        }
    }
}

/// Per-entry outcome of an import.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportReport {
    /// True when nothing was written.
    pub dry_run: bool,
    /// New memories (would be) created.
    pub created: usize,
    /// Entries already present (same fingerprint).
    pub duplicates: usize,
    /// Superseded entries skipped.
    pub skipped_superseded: usize,
    /// Entries rejected, with reason.
    pub rejected: Vec<Rejected>,
}

/// A rejected import entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rejected {
    /// Position in the file (0-based).
    pub index: usize,
    /// Original id, if present.
    pub id: String,
    /// Stable error code.
    pub code: &'static str,
    /// Human-readable reason.
    pub reason: String,
}

impl Store {
    /// Builds the export document for this scope.
    pub fn export(&self) -> Result<ExportFile, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM memories WHERE scope_id = ?1 ORDER BY created_at, id")?;
        let ids: Vec<String> = stmt
            .query_map([&self.scope_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut memories = Vec::with_capacity(ids.len());
        for id in ids {
            memories.push(ExportedMemory::from(&self.get(&id)?));
        }
        let tasks = self
            .tasks(true)?
            .iter()
            .map(|t: &Task| serde_json::to_value(t).unwrap_or_default())
            .collect();
        Ok(ExportFile {
            format: EXPORT_FORMAT.to_owned(),
            scope_id: self.scope_id.clone(),
            exported_at: now(),
            memories,
            tasks,
        })
    }

    /// Imports `file` (see module docs). All accepted entries are written in a
    /// single transaction: a failure leaves the store unchanged.
    pub fn import(
        &mut self,
        file: &ExportFile,
        dry_run: bool,
        actor: &str,
    ) -> Result<ImportReport, StoreError> {
        if file.format != EXPORT_FORMAT {
            return Err(StoreError::InvalidImport(format!(
                "format `{}` is not `{EXPORT_FORMAT}`",
                file.format
            )));
        }
        let mut report = ImportReport {
            dry_run,
            created: 0,
            duplicates: 0,
            skipped_superseded: 0,
            rejected: Vec::new(),
        };
        let mut accepted: Vec<(String, ValidatedMemory)> = Vec::new();
        for (index, entry) in file.memories.iter().enumerate() {
            if entry.status == MemoryStatus::Superseded {
                report.skipped_superseded += 1;
                continue;
            }
            match prepare(entry) {
                Ok(validated) => accepted.push((entry.id.clone(), validated)),
                Err(e) => report.rejected.push(Rejected {
                    index,
                    id: entry.id.clone(),
                    code: e.code(),
                    reason: e.to_string(),
                }),
            }
        }

        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let at = now();
        let mut seen = std::collections::HashSet::new();
        for (original_id, validated) in accepted {
            let memory = &validated.memory;
            let fingerprint = memory.fingerprint();
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM memories WHERE scope_id = ?1 AND fingerprint = ?2)",
                params![self.scope_id, fingerprint],
                |r| r.get(0),
            )?;
            if exists || !seen.insert(fingerprint.clone()) {
                report.duplicates += 1;
                continue;
            }
            report.created += 1;
            if dry_run {
                continue;
            }
            let id = uuid::Uuid::now_v7().to_string();
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
                    "INSERT INTO sources (memory_id, ord, path, line, content_hash)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        id,
                        ord as i64,
                        source.path,
                        source.line,
                        source.content_hash
                    ],
                )?;
            }
            record_event(
                &tx,
                &id,
                "import",
                actor,
                &at,
                &serde_json::json!({ "original_id": original_id, "from_scope": file.scope_id }),
            )?;
        }
        if dry_run {
            tx.rollback()?;
        } else {
            tx.commit()?;
        }
        Ok(report)
    }
}

/// Validates one entry as untrusted imported text.
fn prepare(entry: &ExportedMemory) -> Result<ValidatedMemory, StoreError> {
    for source in &entry.sources {
        check_relative(&source.path)?;
    }
    let disputed = entry.status == MemoryStatus::Disputed;
    let mut validated = NewMemory {
        kind: entry.kind,
        content: entry.content.clone(),
        origin: Origin::Import,
        status: MemoryStatus::Candidate,
        sources: entry.sources.clone(),
        branch: entry.branch.clone(),
        commit: entry.commit.clone(),
    }
    .validate()?;
    if disputed {
        // Lowering trust is always allowed; raising it never is.
        validated.memory.status = MemoryStatus::Disputed;
    }
    Ok(validated)
}
