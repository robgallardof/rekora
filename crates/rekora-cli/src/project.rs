//! Locating the project scope and reading Git context.
//!
//! A project is the directory that contains `.rekora/rekora.db`. `init` places
//! it at the Git top-level when run inside a repository (so every subdirectory
//! and the same checkout share one scope), otherwise at the current directory.
//! Git is invoked as a plain subprocess with fixed arguments — never through a
//! shell — and a missing `git` binary simply means "no branch information".

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use rekora_store::StoreError;

/// Directory holding Rekora's local, never-committed data.
pub const DATA_DIR: &str = ".rekora";
/// Database file name inside [`DATA_DIR`].
pub const DB_FILE: &str = "rekora.db";

/// A resolved project scope.
#[derive(Debug, Clone)]
pub struct Project {
    /// Canonical project root.
    pub root: PathBuf,
}

impl Project {
    /// Path of the scope database.
    pub fn db_path(&self) -> PathBuf {
        self.root.join(DATA_DIR).join(DB_FILE)
    }

    /// Where `init` should create a project when started from `cwd`.
    pub fn for_init(cwd: &Path) -> Result<Self> {
        let root = git(cwd, &["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .unwrap_or_else(|| cwd.to_owned());
        let root = dunce_canonicalize(&root)?;
        Ok(Self { root })
    }

    /// Walks up from `start` to the nearest directory containing a store.
    pub fn discover(start: &Path) -> Result<Self> {
        let start = dunce_canonicalize(start)?;
        for dir in start.ancestors() {
            if dir.join(DATA_DIR).join(DB_FILE).is_file() {
                return Ok(Self {
                    root: dir.to_owned(),
                });
            }
        }
        // Typed so `--format json` reports the stable `not_initialized` code.
        Err(StoreError::NotInitialized(start.join(DATA_DIR).join(DB_FILE)).into())
    }

    /// Writes `.rekora/.gitignore` so local memory is never committed by
    /// accident. Leaves an existing file untouched (the user may have opted in
    /// to sharing parts of it).
    pub fn protect_from_git(&self) -> Result<()> {
        let ignore = self.root.join(DATA_DIR).join(".gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, "# Rekora local data: never commit.\n*\n")
                .with_context(|| format!("writing {}", ignore.display()))?;
        }
        Ok(())
    }

    /// Current branch and commit, when the root is a Git checkout with commits.
    pub fn git_context(&self) -> (Option<String>, Option<String>) {
        let branch =
            git(&self.root, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|b| b != "HEAD");
        let commit = git(&self.root, &["rev-parse", "HEAD"]);
        (branch, commit)
    }
}

/// Runs `git -C dir <args>` and returns trimmed stdout on success.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// Canonicalises without the Windows `\\?\` verbatim prefix when it is not
/// needed, so paths shown to users and stored in configs stay readable while
/// still being symlink/junction-resolved.
pub fn dunce_canonicalize(path: &Path) -> Result<PathBuf> {
    let canonical =
        std::fs::canonicalize(path).with_context(|| format!("resolving {}", path.display()))?;
    #[cfg(windows)]
    {
        let text = canonical.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\")
            && !rest.starts_with("UNC\\")
            && rest.len() < 260
        {
            return Ok(PathBuf::from(rest));
        }
    }
    Ok(canonical)
}
