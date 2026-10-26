//! Memory records: kinds, verification states, provenance and creation rules.
//!
//! Invariant (master prompt §5.3): text coming from a model or an import is never
//! *verified* just because it was written down. [`NewMemory::validate`] enforces
//! which `(origin, status)` combinations are allowed at creation time.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::redact::{self, Redaction};
use crate::source::SourceRef;

/// Upper bound for a single memory body. Memories are distilled knowledge, not
/// file dumps; larger inputs should be cited as a source instead.
pub const MAX_CONTENT_BYTES: usize = 8 * 1024;

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident, $field:literal, { $($(#[$vmeta:meta])* $variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($(#[$vmeta])* $variant),+ }

        impl $name {
            /// Every accepted value, in declaration order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// Stable lowercase identifier used in storage and JSON.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = CoreError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
                    $($text => Ok($name::$variant),)+
                    _ => Err(CoreError::UnknownValue { field: $field, value: s.to_owned() }),
                }
            }
        }
    };
}

string_enum!(
    /// What kind of knowledge a memory holds. Drives ranking priority.
    MemoryKind, "kind", {
        /// An observed fact about the project.
        Fact => "fact",
        /// A decision that was taken, ideally with its rationale.
        Decision => "decision",
        /// A rule that must hold (performance budget, API contract, policy).
        Constraint => "constraint",
        /// A stylistic or workflow preference.
        Preference => "preference",
        /// A known bug or limitation.
        KnownIssue => "known_issue",
        /// A fix or workaround that worked.
        Solution => "solution",
        /// Steps to perform a recurring task.
        Procedure => "procedure",
    }
);

string_enum!(
    /// Verification lifecycle of a memory.
    MemoryStatus, "status", {
        /// Recorded but not yet backed by evidence or human confirmation.
        Candidate => "candidate",
        /// Backed by evidence or confirmed by a human.
        Verified => "verified",
        /// Contradicted by other evidence; kept for audit, ranked low.
        Disputed => "disputed",
        /// Replaced by a newer memory; never returned as current truth.
        Superseded => "superseded",
    }
);

string_enum!(
    /// Who produced the memory. Determines the trust tier.
    Origin, "origin", {
        /// Typed or explicitly approved by the user.
        Human => "human",
        /// Written by an AI agent (CLI or MCP); untrusted until verified.
        Agent => "agent",
        /// Brought in from an external document or export; untrusted data.
        Import => "import",
    }
);

impl MemoryKind {
    /// Relative ranking priority. Constraints and decisions are what agents most
    /// often lose between sessions, so they win ties against loose facts.
    pub fn priority(self) -> f64 {
        match self {
            Self::Constraint => 1.3,
            Self::Decision => 1.2,
            Self::KnownIssue | Self::Solution => 1.1,
            Self::Fact | Self::Procedure => 1.0,
            Self::Preference => 0.9,
        }
    }
}

impl MemoryStatus {
    /// Ranking weight. Superseded memories are filtered before ranking, so their
    /// weight only matters for explicit history queries.
    pub fn weight(self) -> f64 {
        match self {
            Self::Verified => 1.0,
            Self::Candidate => 0.75,
            Self::Disputed => 0.4,
            Self::Superseded => 0.0,
        }
    }

    /// Whether a memory in this state may be presented as current knowledge.
    pub fn is_current(self) -> bool {
        !matches!(self, Self::Superseded)
    }
}

impl Origin {
    /// Whether text from this origin must be labelled as untrusted data when
    /// handed to an agent, regardless of its verification status.
    pub fn is_untrusted_text(self) -> bool {
        !matches!(self, Self::Human)
    }
}

/// A memory as requested by a caller, before persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct NewMemory {
    /// Knowledge kind.
    pub kind: MemoryKind,
    /// Free text body. Redacted and trimmed by [`NewMemory::validate`].
    pub content: String,
    /// Who is writing it.
    pub origin: Origin,
    /// Requested initial status; may be downgraded by validation rules.
    pub status: MemoryStatus,
    /// Evidence backing the memory.
    pub sources: Vec<SourceRef>,
    /// Git branch the memory was observed on, if any.
    pub branch: Option<String>,
    /// Git commit the memory was observed at, if any.
    pub commit: Option<String>,
}

/// Outcome of validating a [`NewMemory`].
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedMemory {
    /// The memory with normalised content and enforced status.
    pub memory: NewMemory,
    /// Secrets that were masked before persistence.
    pub redactions: Vec<Redaction>,
    /// Set when the requested status was not allowed and was downgraded.
    pub status_downgraded: bool,
}

impl NewMemory {
    /// Normalises the content, masks secrets and enforces the creation rules:
    ///
    /// - content must be non-empty and at most [`MAX_CONTENT_BYTES`];
    /// - `superseded`/`disputed` cannot be requested at creation;
    /// - `import` memories always start as `candidate`;
    /// - `agent` memories may only start `verified` when they cite a source.
    ///
    /// Violations of the status rules downgrade to `candidate` rather than fail,
    /// so a retrying agent is never blocked from recording knowledge.
    ///
    /// ```
    /// use rekora_core::{MemoryKind, MemoryStatus, NewMemory, Origin};
    /// let m = NewMemory {
    ///     kind: MemoryKind::Decision,
    ///     content: "  Use SQLite  ".into(),
    ///     origin: Origin::Agent,
    ///     status: MemoryStatus::Verified,
    ///     sources: vec![],
    ///     branch: None,
    ///     commit: None,
    /// };
    /// let v = m.validate().unwrap();
    /// assert_eq!(v.memory.content, "Use SQLite");
    /// assert_eq!(v.memory.status, MemoryStatus::Candidate);
    /// assert!(v.status_downgraded);
    /// ```
    pub fn validate(mut self) -> Result<ValidatedMemory, CoreError> {
        let trimmed = self.content.trim();
        if trimmed.is_empty() {
            return Err(CoreError::EmptyContent);
        }
        if trimmed.len() > MAX_CONTENT_BYTES {
            return Err(CoreError::ContentTooLarge {
                actual: trimmed.len(),
                limit: MAX_CONTENT_BYTES,
            });
        }
        let (content, redactions) = redact::redact(trimmed);
        self.content = content;

        let allowed = match (self.origin, self.status) {
            (_, MemoryStatus::Superseded | MemoryStatus::Disputed) => false,
            (Origin::Import, MemoryStatus::Verified) => false,
            (Origin::Agent, MemoryStatus::Verified) => !self.sources.is_empty(),
            _ => true,
        };
        if !allowed {
            self.status = MemoryStatus::Candidate;
        }
        Ok(ValidatedMemory {
            memory: self,
            redactions,
            status_downgraded: !allowed,
        })
    }

    /// Content fingerprint used for idempotent writes: the same kind, text and
    /// cited locations always hash to the same value, so retries deduplicate.
    pub fn fingerprint(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.kind.as_str().as_bytes());
        hasher.update(&[0]);
        hasher.update(self.content.as_bytes());
        let mut locations: Vec<String> = self.sources.iter().map(SourceRef::location).collect();
        locations.sort();
        for location in locations {
            hasher.update(&[0]);
            hasher.update(location.as_bytes());
        }
        hasher.finalize().to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(origin: Origin, status: MemoryStatus, sources: Vec<SourceRef>) -> NewMemory {
        NewMemory {
            kind: MemoryKind::Decision,
            content: "Use optimistic concurrency".into(),
            origin,
            status,
            sources,
            branch: None,
            commit: None,
        }
    }

    fn source() -> SourceRef {
        SourceRef {
            path: "docs/adr/0001.md".into(),
            line: Some(3),
            content_hash: "abc".into(),
        }
    }

    #[test]
    fn enums_round_trip_and_accept_dashes() {
        for kind in MemoryKind::ALL {
            assert_eq!(kind.as_str().parse::<MemoryKind>(), Ok(*kind));
        }
        assert_eq!("known-issue".parse(), Ok(MemoryKind::KnownIssue));
        assert_eq!(" Verified ".parse(), Ok(MemoryStatus::Verified));
        assert!(matches!(
            "gossip".parse::<MemoryKind>(),
            Err(CoreError::UnknownValue { field: "kind", .. })
        ));
    }

    #[test]
    fn empty_and_oversized_content_rejected() {
        let mut m = memory(Origin::Human, MemoryStatus::Candidate, vec![]);
        m.content = " \n\t ".into();
        assert_eq!(m.clone().validate(), Err(CoreError::EmptyContent));
        m.content = "x".repeat(MAX_CONTENT_BYTES + 1);
        assert!(matches!(
            m.validate(),
            Err(CoreError::ContentTooLarge { .. })
        ));
    }

    #[test]
    fn content_at_exact_limit_is_accepted() {
        let mut m = memory(Origin::Human, MemoryStatus::Candidate, vec![]);
        m.content = "x".repeat(MAX_CONTENT_BYTES);
        assert!(m.validate().is_ok());
    }

    #[test]
    fn status_rules_by_origin() {
        let cases = [
            (
                Origin::Human,
                MemoryStatus::Verified,
                vec![],
                MemoryStatus::Verified,
            ),
            (
                Origin::Agent,
                MemoryStatus::Verified,
                vec![],
                MemoryStatus::Candidate,
            ),
            (
                Origin::Agent,
                MemoryStatus::Verified,
                vec![source()],
                MemoryStatus::Verified,
            ),
            (
                Origin::Import,
                MemoryStatus::Verified,
                vec![source()],
                MemoryStatus::Candidate,
            ),
            (
                Origin::Human,
                MemoryStatus::Superseded,
                vec![],
                MemoryStatus::Candidate,
            ),
            (
                Origin::Human,
                MemoryStatus::Disputed,
                vec![],
                MemoryStatus::Candidate,
            ),
        ];
        for (origin, requested, sources, expected) in cases {
            let v = memory(origin, requested, sources).validate().unwrap();
            assert_eq!(v.memory.status, expected, "{origin} requesting {requested}");
            assert_eq!(v.status_downgraded, requested != expected);
        }
    }

    #[test]
    fn fingerprint_ignores_source_order_but_not_content() {
        let a = SourceRef {
            path: "a.rs".into(),
            line: None,
            content_hash: "1".into(),
        };
        let b = SourceRef {
            path: "b.rs".into(),
            line: Some(2),
            content_hash: "2".into(),
        };
        let m1 = memory(
            Origin::Human,
            MemoryStatus::Candidate,
            vec![a.clone(), b.clone()],
        );
        let m2 = memory(Origin::Human, MemoryStatus::Candidate, vec![b, a]);
        assert_eq!(m1.fingerprint(), m2.fingerprint());
        let mut m3 = m1.clone();
        m3.content.push('!');
        assert_ne!(m1.fingerprint(), m3.fingerprint());
    }

    #[test]
    fn secrets_are_redacted_during_validation() {
        let mut m = memory(Origin::Agent, MemoryStatus::Candidate, vec![]);
        m.content = "deploy key is AKIAABCDEFGHIJKLMNOP".into();
        let v = m.validate().unwrap();
        assert!(!v.memory.content.contains("AKIAABCDEFGHIJKLMNOP"));
        assert_eq!(v.redactions.len(), 1);
    }
}
