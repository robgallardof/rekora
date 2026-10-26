//! Domain model and invariants for Rekora.
//!
//! This crate is deliberately free of I/O-heavy dependencies (no SQLite, no MCP,
//! no async runtime) so that the rules that matter most — what a memory is, which
//! paths may be cited as evidence, what gets redacted and how a context pack is
//! bounded — can be unit-tested deterministically and reused by every adapter.
//!
//! Boundaries:
//! - persistence lives in `rekora-store`;
//! - protocol adapters (CLI, MCP) live in `rekora-cli`.

pub mod budget;
pub mod error;
pub mod memory;
pub mod query;
pub mod redact;
pub mod source;
pub mod task;

pub use error::CoreError;
pub use memory::{MemoryKind, MemoryStatus, NewMemory, Origin};
pub use source::{SourcePolicy, SourceRef};
