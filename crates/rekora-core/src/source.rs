//! Source evidence: which files may be cited and how freshness is checked.
//!
//! Paths are untrusted input (master prompt §5.7). A citation is accepted only
//! when its *canonical* path — after resolving `..`, symlinks and Windows
//! junctions — stays inside the canonical project root and is not on the
//! exclusion list. Stored paths are root-relative with `/` separators so a
//! database stays meaningful across machines and operating systems.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Files larger than this are cited by path but not hashed in full; the hash then
/// covers only the first `MAX_HASH_BYTES`. Keeps `memory add` bounded.
pub const MAX_HASH_BYTES: u64 = 16 * 1024 * 1024;

/// A pointer to evidence inside the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    /// Root-relative path using `/` separators.
    pub path: String,
    /// Optional 1-based line number.
    pub line: Option<u32>,
    /// BLAKE3 hex digest of the file when the memory was recorded.
    pub content_hash: String,
}

impl SourceRef {
    /// `path` or `path:line`, the form shown to users and used in fingerprints.
    pub fn location(&self) -> String {
        match self.line {
            Some(line) => format!("{}:{line}", self.path),
            None => self.path.clone(),
        }
    }
}

/// Whether cited evidence still matches what was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// The file hash is unchanged.
    Fresh,
    /// The file exists but its content changed: the memory is possibly stale.
    Changed,
    /// The file no longer exists (deleted, renamed, or on another branch).
    Missing,
}

/// Resolves and checks citations against one project root.
#[derive(Debug, Clone)]
pub struct SourcePolicy {
    root: PathBuf,
}

impl SourcePolicy {
    /// Creates a policy for `root`, which must exist; it is canonicalised once.
    pub fn new(root: &Path) -> io::Result<Self> {
        Ok(Self {
            root: fs::canonicalize(root)?,
        })
    }

    /// Canonical project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Parses `PATH` or `PATH:LINE` (relative paths resolve against `cwd`),
    /// enforces the root boundary and exclusion list, and hashes the file.
    pub fn resolve(&self, spec: &str, cwd: &Path) -> Result<SourceRef, CoreError> {
        let (raw_path, line) = split_line(spec)?;
        let joined = if Path::new(raw_path).is_absolute() {
            PathBuf::from(raw_path)
        } else {
            cwd.join(raw_path)
        };
        let canonical =
            fs::canonicalize(&joined).map_err(|_| CoreError::SourceMissing(raw_path.to_owned()))?;
        let relative = self.relative(&canonical, raw_path)?;
        let content_hash =
            hash_file(&canonical).map_err(|_| CoreError::SourceMissing(raw_path.to_owned()))?;
        Ok(SourceRef {
            path: relative,
            line,
            content_hash,
        })
    }

    /// Re-hashes a stored citation. The stored path is re-validated against the
    /// root because the database itself is not trusted to be untampered.
    pub fn freshness(&self, source: &SourceRef) -> Freshness {
        let candidate = self.root.join(&source.path);
        let Ok(canonical) = fs::canonicalize(&candidate) else {
            return Freshness::Missing;
        };
        if self.relative(&canonical, &source.path).is_err() {
            return Freshness::Missing;
        }
        match hash_file(&canonical) {
            Ok(hash) if hash == source.content_hash => Freshness::Fresh,
            Ok(_) => Freshness::Changed,
            Err(_) => Freshness::Missing,
        }
    }

    fn relative(&self, canonical: &Path, shown: &str) -> Result<String, CoreError> {
        let rel = canonical
            .strip_prefix(&self.root)
            .map_err(|_| CoreError::SourceOutsideRoot(shown.to_owned()))?;
        let mut parts = Vec::new();
        for component in rel.components() {
            match component {
                Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
                // strip_prefix on canonical paths cannot yield these, but a
                // defensive reject is cheaper than reasoning about every OS.
                _ => return Err(CoreError::SourceOutsideRoot(shown.to_owned())),
            }
        }
        if parts.is_empty() {
            return Err(CoreError::SourceMissing(shown.to_owned()));
        }
        if let Some(reason) = excluded(&parts) {
            return Err(CoreError::SourceExcluded {
                path: shown.to_owned(),
                reason,
            });
        }
        Ok(parts.join("/"))
    }
}

/// Validates a root-relative path that did not come from [`SourcePolicy::resolve`]
/// (for example from an import file) without touching the filesystem: `/`
/// separators only, no absolute paths, drive letters, `.`/`..` or empty
/// segments, and not on the exclusion list.
///
/// ```
/// use rekora_core::source::check_relative;
/// assert!(check_relative("docs/adr/0001.md").is_ok());
/// assert!(check_relative("../outside.md").is_err());
/// assert!(check_relative("C:/Windows/win.ini").is_err());
/// ```
pub fn check_relative(path: &str) -> Result<(), CoreError> {
    let outside = || CoreError::SourceOutsideRoot(path.to_owned());
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return Err(outside());
    }
    let parts: Vec<String> = path.split('/').map(str::to_owned).collect();
    if parts.iter().any(|p| p.is_empty() || p == "." || p == "..") {
        return Err(outside());
    }
    if let Some(reason) = excluded(&parts) {
        return Err(CoreError::SourceExcluded {
            path: path.to_owned(),
            reason,
        });
    }
    Ok(())
}

/// Splits a trailing `:LINE`. A colon followed by non-digits is part of the path
/// (Windows drive letters), but `file:` with an empty or zero line is rejected.
fn split_line(spec: &str) -> Result<(&str, Option<u32>), CoreError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(CoreError::InvalidSourceSpec(spec.to_owned()));
    }
    match spec.rsplit_once(':') {
        Some((path, line))
            if !path.is_empty() && !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit()) =>
        {
            match line.parse::<u32>() {
                Ok(n) if n > 0 => Ok((path, Some(n))),
                _ => Err(CoreError::InvalidSourceSpec(spec.to_owned())),
            }
        }
        Some((path, "")) if !path.is_empty() => Err(CoreError::InvalidSourceSpec(spec.to_owned())),
        _ => Ok((spec, None)),
    }
}

/// Returns why a root-relative path may not be cited, if it may not.
fn excluded(parts: &[String]) -> Option<&'static str> {
    const DIRS: &[&str] = &[".git", ".rekora", "node_modules", "target"];
    for dir in &parts[..parts.len() - 1] {
        if DIRS.iter().any(|d| dir.eq_ignore_ascii_case(d)) {
            return Some("VCS internals, Rekora data or dependency/build output");
        }
    }
    let name = parts[parts.len() - 1].to_ascii_lowercase();
    let is_env = name == ".env" || (name.starts_with(".env.") && !is_env_template(&name));
    let is_key = [".pem", ".key", ".pfx", ".p12", ".keystore"]
        .iter()
        .any(|ext| name.ends_with(ext))
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.starts_with("id_ecdsa");
    if is_env {
        Some("environment files may contain secrets")
    } else if is_key {
        Some("key material")
    } else {
        None
    }
}

fn is_env_template(name: &str) -> bool {
    [".example", ".sample", ".template"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn hash_file(path: &Path) -> io::Result<String> {
    use std::io::Read;
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file.take(MAX_HASH_BYTES))?;
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_line_cases() {
        assert_eq!(split_line("a.rs"), Ok(("a.rs", None)));
        assert_eq!(split_line("a.rs:12"), Ok(("a.rs", Some(12))));
        assert_eq!(split_line(r"C:\x\a.rs"), Ok((r"C:\x\a.rs", None)));
        assert_eq!(split_line(r"C:\x\a.rs:7"), Ok((r"C:\x\a.rs", Some(7))));
        assert!(split_line("a.rs:0").is_err());
        assert!(split_line("a.rs:").is_err());
        assert!(split_line("a.rs:99999999999").is_err());
        assert!(split_line("   ").is_err());
    }

    #[test]
    fn exclusion_list() {
        let check = |p: &str| excluded(&p.split('/').map(String::from).collect::<Vec<_>>());
        assert!(check(".env").is_some());
        assert!(check("api/.env.production").is_some());
        assert!(check(".env.example").is_none());
        assert!(check(".git/config").is_some());
        assert!(check("web/node_modules/x/index.js").is_some());
        assert!(check("keys/server.PEM").is_some());
        assert!(check("home/id_ed25519.pub").is_some());
        assert!(check("src/target.rs").is_none());
        assert!(check("docs/adr/0001.md").is_none());
    }
}
