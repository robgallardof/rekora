//! Durable tasks and the edits agents apply to them.
//!
//! A task is the unit of work that survives a session or moves between agents:
//! objective, accepted constraints, decisions, touched files, test results,
//! failures and remaining work (master prompt §9). Edits are expressed as a
//! [`TaskPatch`] of append/complete operations rather than whole-state
//! replacement, so two agents never erase each other's list items by
//! accident; the store additionally enforces optimistic versioning.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::redact;

/// Longest single list item / objective, in bytes.
pub const MAX_ITEM_BYTES: usize = 2 * 1024;
/// Most items kept per list.
pub const MAX_ITEMS: usize = 200;

/// Lifecycle of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Created, nobody working on it.
    Open,
    /// Someone is working on it.
    InProgress,
    /// Waiting on something external.
    Blocked,
    /// Finished.
    Done,
}

impl TaskStatus {
    /// Stable identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::Done => "done",
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "open" => Ok(Self::Open),
            "in_progress" => Ok(Self::InProgress),
            "blocked" => Ok(Self::Blocked),
            "done" => Ok(Self::Done),
            _ => Err(CoreError::UnknownValue {
                field: "task status",
                value: s.to_owned(),
            }),
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The list-valued part of a task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskState {
    /// Rules accepted for this task.
    pub constraints: Vec<String>,
    /// Decisions taken (text or memory ids).
    pub decisions: Vec<String>,
    /// Files touched, root-relative.
    pub files: Vec<String>,
    /// Tests executed with their observed result, e.g. `cargo test -p x: pass`.
    pub tests: Vec<String>,
    /// Failures and dead ends worth not repeating.
    pub failures: Vec<String>,
    /// Work still to do.
    pub remaining: Vec<String>,
    /// Work completed.
    pub done: Vec<String>,
}

/// An edit to a task. Every field is optional; lists are appended to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskPatch {
    /// New status.
    pub status: Option<TaskStatus>,
    /// Replacement free-text note (latest handoff note).
    pub note: Option<String>,
    /// Who is working on it now.
    pub owner: Option<String>,
    /// Items to append to each list.
    pub add: TaskState,
    /// Remaining items to move to `done`, by exact text.
    pub complete: Vec<String>,
}

impl TaskPatch {
    /// True when the patch changes nothing.
    pub fn is_empty(&self) -> bool {
        self.status.is_none()
            && self.note.is_none()
            && self.owner.is_none()
            && self.add == TaskState::default()
            && self.complete.is_empty()
    }
}

/// Validates a task id: 1–64 chars of `[a-z0-9._-]`, starting alphanumeric.
/// Ids are human-chosen slugs (`auth-refactor`) so they can be typed and shared.
pub fn validate_task_id(id: &str) -> Result<(), CoreError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if ok {
        Ok(())
    } else {
        Err(CoreError::UnknownValue {
            field: "task id (use 1-64 chars of a-z 0-9 . _ -)",
            value: id.to_owned(),
        })
    }
}

/// Trims, bounds and redacts one text item. Returns `None` for blank items.
pub fn clean_item(text: &str) -> Result<Option<String>, CoreError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > MAX_ITEM_BYTES {
        return Err(CoreError::ContentTooLarge {
            actual: trimmed.len(),
            limit: MAX_ITEM_BYTES,
        });
    }
    Ok(Some(redact::redact(trimmed).0))
}

impl TaskState {
    fn lists_mut(&mut self) -> [&mut Vec<String>; 7] {
        [
            &mut self.constraints,
            &mut self.decisions,
            &mut self.files,
            &mut self.tests,
            &mut self.failures,
            &mut self.remaining,
            &mut self.done,
        ]
    }

    /// Applies list additions and completions from `patch`. Duplicate items are
    /// ignored so a retried patch is idempotent. Completing an unknown item is
    /// an error rather than a silent no-op: the caller's view is out of date.
    pub fn apply(&mut self, patch: &TaskPatch) -> Result<(), CoreError> {
        let mut add = patch.add.clone();
        for (target, extra) in self.lists_mut().into_iter().zip(add.lists_mut()) {
            for item in extra.iter() {
                if let Some(item) = clean_item(item)?
                    && !target.contains(&item)
                {
                    target.push(item);
                }
            }
            if target.len() > MAX_ITEMS {
                return Err(CoreError::ContentTooLarge {
                    actual: target.len(),
                    limit: MAX_ITEMS,
                });
            }
        }
        for which in &patch.complete {
            // Exact text only: positional indexes would make retries complete
            // a different item once the list has shifted.
            let position = self.remaining.iter().position(|r| r == which.trim());
            match position {
                Some(index) => {
                    let item = self.remaining.remove(index);
                    if !self.done.contains(&item) {
                        self.done.push(item);
                    }
                }
                None if self.done.iter().any(|d| d == which.trim()) => {}
                None => {
                    return Err(CoreError::UnknownValue {
                        field: "remaining item",
                        value: which.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_ids() {
        for ok in ["auth-refactor", "a", "v1.2_fix", "0abc"] {
            assert!(validate_task_id(ok).is_ok(), "{ok}");
        }
        for bad in ["", "-x", "Auth", "a b", "../etc", &"x".repeat(65), "ñ"] {
            assert!(validate_task_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn apply_is_idempotent_and_completes() {
        let mut state = TaskState::default();
        let patch = TaskPatch {
            add: TaskState {
                remaining: vec!["write tests".into(), "update docs".into(), "  ".into()],
                constraints: vec!["no new deps".into()],
                ..TaskState::default()
            },
            ..TaskPatch::default()
        };
        state.apply(&patch).unwrap();
        state.apply(&patch).unwrap();
        assert_eq!(state.remaining, ["write tests", "update docs"]);
        assert_eq!(state.constraints, ["no new deps"]);

        let done = TaskPatch {
            complete: vec!["update docs".into(), "write tests".into()],
            ..TaskPatch::default()
        };
        state.apply(&done).unwrap();
        assert!(state.remaining.is_empty());
        assert_eq!(state.done, ["update docs", "write tests"]);
        // Re-completing a done item is a harmless retry.
        state.apply(&done).unwrap();
        let unknown = TaskPatch {
            complete: vec!["nope".into()],
            ..TaskPatch::default()
        };
        assert!(state.apply(&unknown).is_err());
    }

    #[test]
    fn items_are_redacted_and_bounded() {
        let mut state = TaskState::default();
        let patch = TaskPatch {
            add: TaskState {
                failures: vec!["login failed with password=hunter2222".into()],
                ..TaskState::default()
            },
            ..TaskPatch::default()
        };
        state.apply(&patch).unwrap();
        assert!(!state.failures[0].contains("hunter2222"));
        let big = TaskPatch {
            add: TaskState {
                tests: vec!["x".repeat(MAX_ITEM_BYTES + 1)],
                ..TaskState::default()
            },
            ..TaskPatch::default()
        };
        assert!(state.apply(&big).is_err());
    }
}
