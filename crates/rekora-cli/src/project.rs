//! Locating the project scope and reading Git context.
//!
//! A project is the directory that contains `.rekora/rekora.db`. Inside Git,
//! that is the **main worktree** of the repository, found through
//! `git rev-parse --git-common-dir`; linked worktrees (`git worktree add`)
//! share the same store, and memories carry the branch they were recorded on
//! so branch-specific facts stay distinguishable (S04). Source paths are
//! resolved against the *current* checkout, since that is where the cited
//! file is.
//!
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
    /// Where the store lives (main worktree or plain directory).
    pub root: PathBuf,
    /// The checkout the command runs in (a linked worktree, or `root`).
    pub checkout: PathBuf,
}

impl Project {
    /// Path of the scope database.
    pub fn db_path(&self) -> PathBuf {
        self.root.join(DATA_DIR).join(DB_FILE)
    }

    /// Rekora-owned directory for backups and adapter state.
    pub fn data_dir(&self) -> PathBuf {
        self.root.join(DATA_DIR)
    }

    /// Where `init` should create a project when started from `start`.
    pub fn for_init(start: &Path) -> Result<Self> {
        let start = dunce_canonicalize(start)?;
        let checkout = git(&start, &["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .map(|p| dunce_canonicalize(&p))
            .transpose()?
            .unwrap_or_else(|| start.clone());
        let root = main_worktree(&checkout).unwrap_or_else(|| checkout.clone());
        Ok(Self { root, checkout })
    }

    /// Finds the store for a command started in `start`: first the
    /// repository's main worktree, then the nearest ancestor with a store.
    pub fn discover(start: &Path) -> Result<Self> {
        let start = dunce_canonicalize(start)?;
        let checkout = git(&start, &["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .and_then(|p| dunce_canonicalize(&p).ok());
        if let Some(checkout) = &checkout
            && let Some(main) = main_worktree(checkout)
            && main.join(DATA_DIR).join(DB_FILE).is_file()
        {
            return Ok(Self {
                root: main,
                checkout: checkout.clone(),
            });
        }
        for dir in start.ancestors() {
            if dir.join(DATA_DIR).join(DB_FILE).is_file() {
                let checkout = checkout
                    .filter(|c| c.starts_with(dir))
                    .unwrap_or_else(|| dir.to_owned());
                return Ok(Self {
                    root: dir.to_owned(),
                    checkout,
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
        let ignore = self.data_dir().join(".gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, "# Rekora local data: never commit.\n*\n")
                .with_context(|| format!("writing {}", ignore.display()))?;
        }
        Ok(())
    }

    /// Current branch and commit of the checkout, when it is a Git checkout.
    pub fn git_context(&self) -> (Option<String>, Option<String>) {
        let branch =
            git(&self.checkout, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|b| b != "HEAD");
        let commit = git(&self.checkout, &["rev-parse", "HEAD"]);
        (branch, commit)
    }

    /// `git status --porcelain` lines of the checkout: work an agent left
    /// uncommitted. Empty outside Git or when the tree is clean.
    pub fn uncommitted(&self) -> Vec<String> {
        git_raw(&self.checkout, &["status", "--porcelain=v1"])
            .map(|out| {
                out.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Snapshot key for the code index: branch, `detached@<sha12>`, or
    /// `worktree` outside Git.
    pub fn snapshot(&self) -> String {
        match self.git_context() {
            (Some(branch), _) => branch,
            (None, Some(commit)) => format!("detached@{}", &commit[..commit.len().min(12)]),
            (None, None) => "worktree".to_owned(),
        }
    }
}

/// Main worktree of the repository containing `checkout`, if any.
fn main_worktree(checkout: &Path) -> Option<PathBuf> {
    let common = git(
        checkout,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common = PathBuf::from(common);
    // `<main>/.git` for normal repositories; bare repositories have no main
    // worktree, in which case each checkout keeps its own store.
    if common.file_name().is_some_and(|n| n == ".git") {
        common.parent().and_then(|p| dunce_canonicalize(p).ok())
    } else {
        None
    }
}

/// Runs `git -C dir <args>` and returns trimmed stdout on success.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let text = git_raw(dir, args)?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// Runs `git -C dir <args>` and returns stdout untouched (porcelain output
/// starts with significant spaces).
fn git_raw(dir: &Path, args: &[&str]) -> Option<String> {
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
    String::from_utf8(output.stdout).ok()
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
