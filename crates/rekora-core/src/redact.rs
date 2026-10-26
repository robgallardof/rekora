//! Best-effort secret masking applied before anything is persisted.
//!
//! This is a pattern list, not a guarantee: it catches common credential shapes
//! (cloud keys, tokens, PEM private keys, `password=` assignments) and will miss
//! bespoke formats. False positives are preferred over leaks, and every masked
//! span is reported back to the caller so the behaviour is visible.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

/// One masked span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Redaction {
    /// Pattern family that matched (e.g. `aws_access_key`).
    pub kind: &'static str,
}

struct Rule {
    kind: &'static str,
    pattern: Regex,
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    [
        (
            "private_key",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----|$)",
        ),
        ("aws_access_key", r"\b(AKIA|ASIA)[0-9A-Z]{16}\b"),
        ("github_token", r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}\b"),
        ("github_pat", r"\bgithub_pat_[A-Za-z0-9_]{40,}\b"),
        ("api_key", r"\bsk-[A-Za-z0-9_\-]{20,}\b"),
        ("slack_token", r"\bxox[abprs]-[A-Za-z0-9\-]{10,}\b"),
        ("jwt", r"\beyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\b"),
        (
            "assignment",
            r#"(?i)\b(password|passwd|secret|api[_-]?key|access[_-]?token|client[_-]?secret)\s*[:=]\s*["']?[^\s"']{6,}"#,
        ),
    ]
    .into_iter()
    .map(|(kind, pattern)| Rule {
        kind,
        // Patterns are compile-time constants covered by unit tests.
        pattern: Regex::new(pattern).expect("static redaction pattern is valid"),
    })
    .collect()
});

/// Returns `text` with every detected secret replaced by `[REDACTED:<kind>]`.
///
/// ```
/// let (clean, found) = rekora_core::redact::redact("token ghp_abcdefghijklmnopqrstuvwxyz0123456789");
/// assert_eq!(clean, "token [REDACTED:github_token]");
/// assert_eq!(found.len(), 1);
/// ```
pub fn redact(text: &str) -> (String, Vec<Redaction>) {
    let mut out = text.to_owned();
    let mut found = Vec::new();
    for rule in RULES.iter() {
        let count = rule.pattern.find_iter(&out).count();
        if count == 0 {
            continue;
        }
        found.extend(std::iter::repeat_n(Redaction { kind: rule.kind }, count));
        let replacement = format!("[REDACTED:{}]", rule.kind);
        out = rule
            .pattern
            .replace_all(&out, regex::NoExpand(&replacement))
            .into_owned();
    }
    (out, found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_known_shapes() {
        let samples = [
            ("AKIAABCDEFGHIJKLMNOP", "aws_access_key"),
            ("sk-proj-abcdefghijklmnopqrstuv", "api_key"),
            ("xoxb-1234567890-abcdef", "slack_token"),
            ("password = hunter22!", "assignment"),
            ("API_KEY: \"abcdef123456\"", "assignment"),
            (
                "-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----",
                "private_key",
            ),
        ];
        for (secret, kind) in samples {
            let (clean, found) = redact(&format!("before {secret} after"));
            assert!(!clean.contains(secret), "{kind} leaked: {clean}");
            assert!(found.iter().any(|r| r.kind == kind), "{kind} not reported");
            assert!(clean.starts_with("before "));
        }
    }

    #[test]
    fn ordinary_prose_untouched() {
        let text = "Use SQLite with WAL; the password policy lives in docs/security.md.";
        let (clean, found) = redact(text);
        assert_eq!(clean, text);
        assert!(found.is_empty());
    }

    #[test]
    fn truncated_private_key_still_masked() {
        let (clean, _) = redact("-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA");
        assert!(!clean.contains("MIIEpAIBAAKCAQEA"));
    }
}
