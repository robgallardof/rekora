//! Incremental, syntax-only code indexing.
//!
//! [`index`] walks a checkout (honouring `.gitignore`, `.ignore` and Rekora's
//! own exclusions), hashes each candidate file with BLAKE3 and re-parses only
//! files whose hash differs from the stored one for the same snapshot
//! (branch). Deleted files are removed from that snapshot. Nothing is executed:
//! no build scripts, no package managers, no language servers.
//!
//! The snapshot is checked before and after the walk; if the branch or commit
//! changed meanwhile, the scan is retried once and then refused, so an index
//! never mixes two branches (test matrix R25).

pub mod lang;

use std::path::Path;

use rekora_core::source::check_relative;
use rekora_store::{FileUpdate, Store, StoreError};
use serde::Serialize;

pub use lang::{Detected, Extractor, Lang, detect};

/// Files larger than this are kept as file-level evidence but not parsed.
pub const MAX_PARSE_BYTES: u64 = 1024 * 1024;
/// Files larger than this are skipped entirely.
pub const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;

/// Counters reported by [`index`], used to prove incrementality (R36).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IndexReport {
    /// Snapshot (branch or `detached@<sha>`) that was indexed.
    pub snapshot: String,
    /// Candidate files seen on disk.
    pub scanned: usize,
    /// Files whose content was hashed.
    pub hashed: usize,
    /// Files parsed with tree-sitter.
    pub parsed: usize,
    /// Files recorded at file level only (unsupported language or too large).
    pub file_only: usize,
    /// Files unchanged since the last index.
    pub unchanged: usize,
    /// Files removed from the index.
    pub removed: usize,
    /// Symbols written in this run.
    pub symbols_written: usize,
    /// Paths skipped, with reason (binary, too large, policy).
    pub skipped: Vec<(String, &'static str)>,
    /// What the index does not know.
    pub limits: &'static str,
}

/// Index failure.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// Storage error.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Filesystem walk error.
    #[error("io error: {0}")]
    Io(String),
    /// A grammar failed to load (a bug in this build).
    #[error("parser setup failed: {0}")]
    Parser(String),
    /// The checkout changed branch/commit during both scan attempts.
    #[error("checkout changed from {before} to {after} during indexing; re-run when it is stable")]
    SnapshotChanged {
        /// Snapshot before the scan.
        before: String,
        /// Snapshot after the scan.
        after: String,
    },
}

impl IndexError {
    /// Stable error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Store(e) => e.code(),
            Self::Io(_) => "io_error",
            Self::Parser(_) => "parser_error",
            Self::SnapshotChanged { .. } => "snapshot_changed",
        }
    }
}

const LIMITS: &str = "syntax-only: declarations (functions, methods, types, classes, interfaces, \
enums, modules) for rust, python, javascript, typescript/tsx and c#; other languages are \
file-level only; no references, call graphs, type resolution or dynamic behaviour";

/// Indexes `checkout` into `store` under the snapshot returned by `snapshot`
/// (called before and after the walk; see module docs).
pub fn index(
    store: &mut Store,
    checkout: &Path,
    snapshot: &dyn Fn() -> String,
) -> Result<IndexReport, IndexError> {
    let mut extractor = Extractor::new().map_err(IndexError::Parser)?;
    let mut last = None;
    for _attempt in 0..2 {
        let before = snapshot();
        let plan = scan(store, checkout, &before, &mut extractor)?;
        let after = snapshot();
        if before == after {
            let (updates, removed, report) = plan;
            store.apply_index(&before, &updates, &removed)?;
            return Ok(report);
        }
        last = Some((before, after));
    }
    let (before, after) = last.unwrap_or_default();
    Err(IndexError::SnapshotChanged { before, after })
}

type Plan = (Vec<FileUpdate>, Vec<String>, IndexReport);

fn scan(
    store: &Store,
    checkout: &Path,
    snapshot: &str,
    extractor: &mut Extractor,
) -> Result<Plan, IndexError> {
    let mut known = store.indexed_hashes(snapshot)?;
    let mut report = IndexReport {
        snapshot: snapshot.to_owned(),
        limits: LIMITS,
        ..IndexReport::default()
    };
    let mut updates = Vec::new();
    let walker = ignore::WalkBuilder::new(checkout)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git" | ".rekora" | "node_modules" | "target" | "bin" | "obj"
            )
        })
        .build();
    for entry in walker {
        let entry = entry.map_err(|e| IndexError::Io(e.to_string()))?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(checkout) else {
            continue;
        };
        let rel = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let detected = detect(&rel);
        if detected == Detected::Ignored {
            continue;
        }
        if check_relative(&rel).is_err() {
            report.skipped.push((rel, "excluded by policy"));
            continue;
        }
        report.scanned += 1;
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if size > MAX_INDEX_BYTES {
            report.skipped.push((rel, "larger than 8 MiB"));
            continue;
        }
        let bytes = std::fs::read(entry.path()).map_err(|e| IndexError::Io(e.to_string()))?;
        report.hashed += 1;
        if bytes.iter().take(8192).any(|b| *b == 0) {
            report.skipped.push((rel, "binary"));
            continue;
        }
        let hash = blake3::hash(&bytes).to_hex().to_string();
        if known.remove(&rel).is_some_and(|old| old == hash) {
            report.unchanged += 1;
            continue;
        }
        let (language, symbols) = match detected {
            Detected::Parsed(lang) if size <= MAX_PARSE_BYTES => {
                report.parsed += 1;
                (Some(lang.id().to_owned()), extractor.symbols(lang, &bytes))
            }
            Detected::Parsed(lang) => {
                report.file_only += 1;
                (Some(lang.id().to_owned()), Vec::new())
            }
            Detected::FileOnly(id) => {
                report.file_only += 1;
                (Some(id.to_owned()), Vec::new())
            }
            Detected::Ignored => continue,
        };
        report.symbols_written += symbols.len();
        updates.push(FileUpdate {
            path: rel,
            hash,
            language,
            size,
            symbols,
        });
    }
    // Whatever is still in `known` was indexed before but is gone now.
    let mut removed: Vec<String> = known.into_keys().collect();
    removed.sort();
    report.removed = removed.len();
    Ok((updates, removed, report))
}
