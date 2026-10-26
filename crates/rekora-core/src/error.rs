//! Typed validation errors raised by domain rules.
//!
//! Every variant carries a stable machine-readable `code()` so CLI `--format json`
//! and MCP tool errors can be matched by callers without parsing prose.

use thiserror::Error;

/// A domain rule rejected the input. Never caused by I/O.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    /// The memory text is empty after trimming.
    #[error("memory content is empty")]
    EmptyContent,
    /// The memory text exceeds [`crate::memory::MAX_CONTENT_BYTES`].
    #[error("memory content is {actual} bytes; the limit is {limit}")]
    ContentTooLarge {
        /// Observed size in bytes.
        actual: usize,
        /// Configured limit in bytes.
        limit: usize,
    },
    /// An enum value (kind, status, origin) was not recognised.
    #[error("unknown {field} value `{value}`")]
    UnknownValue {
        /// Field being parsed.
        field: &'static str,
        /// Raw value supplied by the caller.
        value: String,
    },
    /// The cited source resolves outside the project root (traversal, symlink, junction).
    #[error("source `{0}` is outside the project root")]
    SourceOutsideRoot(String),
    /// The cited source is excluded by policy (secrets, VCS internals, build output).
    #[error("source `{path}` is excluded by policy: {reason}")]
    SourceExcluded {
        /// Path as supplied.
        path: String,
        /// Human-readable policy reason.
        reason: &'static str,
    },
    /// The cited source does not exist or cannot be read.
    #[error("source `{0}` does not exist or is not a readable file")]
    SourceMissing(String),
    /// The source specifier is syntactically invalid (e.g. `file:abc`).
    #[error("invalid source specifier `{0}`; expected PATH or PATH:LINE")]
    InvalidSourceSpec(String),
}

impl CoreError {
    /// Stable error code for structured output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptyContent => "empty_content",
            Self::ContentTooLarge { .. } => "content_too_large",
            Self::UnknownValue { .. } => "unknown_value",
            Self::SourceOutsideRoot(_) => "source_outside_root",
            Self::SourceExcluded { .. } => "source_excluded",
            Self::SourceMissing(_) => "source_missing",
            Self::InvalidSourceSpec(_) => "invalid_source_spec",
        }
    }
}
