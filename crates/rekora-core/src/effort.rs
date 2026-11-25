//! Suggested model tier for a task, from evidence Rekora already records.
//!
//! Hosts own model choice; Rekora only *suggests* which tier a task (or the
//! subagent a host delegates it to) needs. The rule follows the research in
//! docs/research/MODEL_DELEGATION.md: switching models turn by turn inside a
//! task rarely pays off, while starting on a cheap tier and escalating on
//! recorded evidence of difficulty does. So the suggestion starts at
//! [`Tier::Light`] and only moves up for reasons it can name:
//!
//! - recorded failures (the strongest signal: a cheaper model already tried),
//! - how many files and top-level areas the task touches,
//! - how much work remains,
//! - words in the objective that usually mean design or debugging work (a
//!   weak signal: it can raise the tier by one step at most).
//!
//! No LLM call, no network: the same task state always gives the same answer.

use serde::Serialize;

use crate::task::TaskState;

/// Model capability tier. Hosts map it to their own models
/// (e.g. Claude Code `haiku` / `sonnet` / `opus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Small, well-specified work: docs, renames, formatting, simple tests.
    Light,
    /// Ordinary feature or fix work.
    Standard,
    /// Work a cheaper model failed at, or that spans much of the codebase.
    Deep,
}

impl Tier {
    /// Stable lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }

    fn up(self) -> Self {
        match self {
            Self::Light => Self::Standard,
            _ => Self::Deep,
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A suggested tier and the evidence behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    /// Suggested tier.
    pub tier: Tier,
    /// Why, in plain words (empty for the default light tier).
    pub reasons: Vec<String>,
}

/// Objective words that usually mean design, debugging or risky work.
const HEAVY_WORDS: &[&str] = &[
    "architect",
    "design",
    "migrat",
    "security",
    "vulnerab",
    "concurren",
    "race",
    "deadlock",
    "refactor",
    "performance",
    "optimi",
    "debug",
    "crash",
    "flaky",
    "protocol",
    "schema",
];

/// Suggests a tier for a task with `objective` and recorded `state`.
pub fn suggest(objective: &str, state: &TaskState) -> Suggestion {
    let mut tier = Tier::Light;
    let mut reasons = Vec::new();
    let mut raise_to = |to: Tier, why: String, tier: &mut Tier| {
        if to > *tier {
            *tier = to;
        }
        reasons.push(why);
    };

    let failures = state.failures.len();
    if failures >= 2 {
        raise_to(
            Tier::Deep,
            format!("{failures} recorded failures: escalate instead of retrying the same tier"),
            &mut tier,
        );
    } else if failures == 1 {
        raise_to(Tier::Standard, "1 recorded failure".into(), &mut tier);
    }

    let files = state.files.len();
    let mut areas: Vec<&str> = state
        .files
        .iter()
        .map(|f| {
            f.trim_start_matches("./")
                .split(['/', '\\'])
                .next()
                .unwrap_or("")
        })
        .filter(|a| !a.is_empty())
        .collect();
    areas.sort_unstable();
    areas.dedup();
    if files > 8 || areas.len() >= 3 {
        raise_to(
            Tier::Deep,
            format!("touches {files} files in {} top-level areas", areas.len()),
            &mut tier,
        );
    } else if files >= 3 {
        raise_to(Tier::Standard, format!("touches {files} files"), &mut tier);
    }

    let remaining = state.remaining.len();
    if remaining >= 6 {
        raise_to(
            Tier::Standard,
            format!("{remaining} items remaining"),
            &mut tier,
        );
    }

    let lower = objective.to_lowercase();
    if let Some(word) = HEAVY_WORDS.iter().find(|w| lower.contains(*w)) {
        // Weak signal: one step up from whatever the evidence says, capped.
        let to = tier.up();
        raise_to(to, format!("objective mentions \"{word}\""), &mut tier);
    }

    Suggestion { tier, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(failures: usize, files: &[&str], remaining: usize) -> TaskState {
        TaskState {
            failures: (0..failures)
                .map(|i| format!("attempt {i} failed"))
                .collect(),
            files: files.iter().map(|f| (*f).to_owned()).collect(),
            remaining: (0..remaining).map(|i| format!("item {i}")).collect(),
            ..TaskState::default()
        }
    }

    #[test]
    fn small_tasks_start_light() {
        let s = suggest("Fix typo in README", &state(0, &["README.md"], 1));
        assert_eq!(s.tier, Tier::Light);
        assert!(s.reasons.is_empty());
    }

    #[test]
    fn failures_escalate() {
        assert_eq!(
            suggest("Add endpoint", &state(1, &[], 1)).tier,
            Tier::Standard
        );
        let s = suggest("Add endpoint", &state(2, &[], 1));
        assert_eq!(s.tier, Tier::Deep);
        assert!(s.reasons[0].contains("2 recorded failures"), "{s:?}");
    }

    #[test]
    fn breadth_escalates() {
        let three = state(0, &["src/a.rs", "src/b.rs", "src/c.rs"], 0);
        assert_eq!(suggest("Add endpoint", &three).tier, Tier::Standard);
        let wide = state(0, &["crates/a.rs", "docs/b.md", "scripts/c.py"], 0);
        assert_eq!(suggest("Add endpoint", &wide).tier, Tier::Deep);
        assert_eq!(
            suggest("Add endpoint", &state(0, &[], 6)).tier,
            Tier::Standard
        );
    }

    #[test]
    fn words_raise_one_step_only() {
        let s = suggest("Refactor the parser", &state(0, &[], 0));
        assert_eq!(s.tier, Tier::Standard);
        assert!(s.reasons[0].contains("refactor"), "{s:?}");
        let s = suggest("Debug the crash", &state(1, &[], 0));
        assert_eq!(s.tier, Tier::Deep);
    }
}
