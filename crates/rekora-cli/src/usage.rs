//! Reading token usage from the files agent hosts already write.
//!
//! Rekora does not own model requests; the hosts record each response's usage
//! in their session files, and the hook payload says where that file is
//! (`transcript_path`). Formats (observed 2026-11-24; neither is a documented,
//! stable API, so every field is optional and unknown lines are skipped):
//!
//! - **Claude Code** transcript JSONL: `type: "assistant"` lines carry
//!   `message.model` and `message.usage` with Anthropic's exclusive split
//!   (`input_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`
//!   with `cache_creation.ephemeral_5m/1h_input_tokens`, `output_tokens`).
//!   One response can span several lines with the same `requestId`; the last
//!   one wins. Subagents write their own files under `<session>/subagents/`.
//! - **Codex** rollout JSONL: `turn_context` lines carry the `model`;
//!   `token_usage_record` lines carry one response's `usage` (OpenAI's
//!   inclusive split: `input_tokens` includes `cached_input_tokens`) and a
//!   `response_id`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use rekora_core::usage::{ModelUsage, Usage};
use serde_json::Value;

/// Lines longer than this are skipped: usage lines are small, and a huge line
/// (a pasted file) is never worth parsing.
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// Usage of a host session file, per model. Missing or unreadable files give
/// an empty result: usage is a hint, never a reason to fail a hook.
pub fn session_usage(host: &str, transcript: &Path) -> ModelUsage {
    match host {
        "codex" => codex(transcript),
        _ => {
            let mut all = claude_file(transcript);
            // Subagent transcripts: `<dir>/<session>.jsonl` → `<dir>/<session>/subagents/*.jsonl`.
            if let (Some(dir), Some(stem)) = (transcript.parent(), transcript.file_stem()) {
                let subagents = dir.join(stem).join("subagents");
                if let Ok(entries) = std::fs::read_dir(subagents) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "jsonl") {
                            rekora_core::usage::merge(&mut all, &claude_file(&path));
                        }
                    }
                }
            }
            all
        }
    }
}

fn lines(path: &Path) -> impl Iterator<Item = String> {
    File::open(path)
        .ok()
        .map(BufReader::new)
        .into_iter()
        .flat_map(|r| r.lines().map_while(Result::ok))
        .filter(|l| l.len() <= MAX_LINE_BYTES)
}

fn n(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn claude_file(path: &Path) -> ModelUsage {
    let mut last: HashMap<String, (String, Usage)> = HashMap::new();
    let mut anonymous: Vec<(String, Usage)> = Vec::new();
    for line in lines(path) {
        // Cheap pre-filter: only assistant lines with usage matter.
        if !line.contains("\"usage\"") || !line.contains("\"assistant\"") {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if entry.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let message = &entry["message"];
        let Some(usage) = message.get("usage") else {
            continue;
        };
        let model = message
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        // Synthetic messages (errors, interruptions) are not model requests.
        if model.starts_with('<') {
            continue;
        }
        let created = n(usage, "cache_creation_input_tokens");
        let long = usage
            .pointer("/cache_creation/ephemeral_1h_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(created);
        let counted = Usage {
            requests: 1,
            uncached_input: n(usage, "input_tokens"),
            cache_read: n(usage, "cache_read_input_tokens"),
            cache_write_short: created - long,
            cache_write_long: long,
            output: n(usage, "output_tokens"),
        };
        let id = entry
            .get("requestId")
            .and_then(Value::as_str)
            .or_else(|| message.get("id").and_then(Value::as_str));
        match id {
            Some(id) => {
                last.insert(id.to_owned(), (model.to_owned(), counted));
            }
            None => anonymous.push((model.to_owned(), counted)),
        }
    }
    let mut out = ModelUsage::new();
    for (model, usage) in last.into_values().chain(anonymous) {
        out.entry(model).or_default().add(&usage);
    }
    out
}

fn codex(path: &Path) -> ModelUsage {
    let mut model = String::from("unknown");
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut out = ModelUsage::new();
    for line in lines(path) {
        let context = line.contains("\"turn_context\"");
        if !context && !line.contains("\"token_usage_record\"") {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let payload = &entry["payload"];
        match entry.get("type").and_then(Value::as_str) {
            Some("turn_context") => {
                if let Some(m) = payload.get("model").and_then(Value::as_str) {
                    model = m.to_owned();
                }
            }
            Some("token_usage_record") => {
                if let Some(id) = payload.get("response_id").and_then(Value::as_str)
                    && seen.insert(id.to_owned(), ()).is_some()
                {
                    continue;
                }
                let usage = &payload["usage"];
                let input = n(usage, "input_tokens");
                let cached = n(usage, "cached_input_tokens");
                let written = n(usage, "cache_write_input_tokens");
                out.entry(model.clone()).or_default().add(&Usage {
                    requests: 1,
                    uncached_input: input.saturating_sub(cached).saturating_sub(written),
                    cache_read: cached,
                    cache_write_short: written,
                    cache_write_long: 0,
                    output: n(usage, "output_tokens"),
                });
            }
            _ => {}
        }
    }
    out
}

/// `$1.23`, or `<$0.01` for tiny amounts.
pub fn usd(amount: f64) -> String {
    if amount > 0.0 && amount < 0.01 {
        "<$0.01".to_owned()
    } else {
        format!("${amount:.2}")
    }
}

/// An estimate as text: dollars, plus a marker when some models had no price.
pub fn describe(usage: &ModelUsage) -> String {
    let e = rekora_core::usage::estimate(usage);
    if e.unpriced.is_empty() {
        format!("~{}", usd(e.usd))
    } else {
        format!("~{} + unpriced {}", usd(e.usd), e.unpriced.join(", "))
    }
}

/// Status line for Claude Code (`statusLine` command): reads the JSON the
/// host sends on stdin and prints one line with the active task's and the
/// session's estimated cost. Silent (empty) on any problem: a status line must
/// never show an error to the user.
pub fn statusline(stdin: impl std::io::Read) -> String {
    let mut text = String::new();
    if stdin.take(1024 * 1024).read_to_string(&mut text).is_err() {
        return String::new();
    }
    let payload: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let field = |pointer: &str| payload.pointer(pointer).and_then(Value::as_str);
    let Some(transcript) = field("/transcript_path") else {
        return String::new();
    };
    let session_totals = session_usage("claude", Path::new(transcript));
    let mut parts = vec![format!("session {}", describe(&session_totals))];
    let cwd = field("/workspace/current_dir").or_else(|| field("/cwd"));
    if let Some(project) = cwd.and_then(|c| crate::project::Project::discover(Path::new(c)).ok())
        && let Ok(store) = rekora_store::Store::open(&project.db_path())
        && let Ok(tasks) = store.tasks(false)
    {
        let branch = project.git_context().0;
        if let Some(task) = crate::hook::active_task(&tasks, branch.as_deref()) {
            let mut task_usage = store
                .task_usage(&task.id)
                .ok()
                .flatten()
                .map(|t| t.models)
                .unwrap_or_default();
            // Growth since the last Stop hook is not in the ledger yet.
            if let Some(session) = field("/session_id") {
                let recorded = store
                    .session_usage("claude", session)
                    .ok()
                    .flatten()
                    .map(|s| s.models)
                    .unwrap_or_default();
                for (model, now) in &session_totals {
                    let before = recorded.get(model).copied().unwrap_or_default();
                    let growth = now.since(&before);
                    if !growth.is_empty() {
                        task_usage.entry(model.clone()).or_default().add(&growth);
                    }
                }
            }
            parts.insert(0, format!("task {} {}", task.id, describe(&task_usage)));
        }
    }
    format!(
        "rekora · {} · API est. {}",
        parts.join(" · "),
        rekora_core::usage::PRICES_AS_OF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_lines_are_deduplicated_by_request_and_split_by_cache_ttl() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("s1.jsonl");
        let line = |req: &str, out: u64| {
            format!(
                r#"{{"type":"assistant","requestId":"{req}","message":{{"model":"claude-opus-5-5","usage":{{"input_tokens":2,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":{out},"cache_creation":{{"ephemeral_1h_input_tokens":60,"ephemeral_5m_input_tokens":40}}}}}}}}"#
            )
        };
        let text = [
            line("r1", 5),
            line("r1", 50), // same response, final count
            line("r2", 7),
            r#"{"type":"user","message":{"content":"usage of assistant"}}"#.to_owned(),
            r#"{"type":"assistant","message":{"model":"<synthetic>","usage":{"output_tokens":9}}}"#
                .to_owned(),
        ]
        .join("\n");
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{e}"));
        let sub = dir.path().join("s1").join("subagents");
        std::fs::create_dir_all(&sub).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(sub.join("agent-a.jsonl"), line("r3", 1)).unwrap_or_else(|e| panic!("{e}"));

        let usage = session_usage("claude", &path);
        let opus = usage["claude-opus-5-5"];
        assert_eq!(usage.len(), 1, "{usage:?}");
        assert_eq!(opus.requests, 3);
        assert_eq!(opus.output, 50 + 7 + 1);
        assert_eq!(opus.cache_write_long, 180);
        assert_eq!(opus.cache_write_short, 120);
        assert_eq!(opus.cache_read, 3000);
        assert_eq!(opus.uncached_input, 6);
    }

    #[test]
    fn codex_usage_follows_the_turn_model_and_inclusive_input() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("rollout.jsonl");
        let record = |id: &str| {
            format!(
                r#"{{"type":"token_usage_record","payload":{{"response_id":"{id}","usage":{{"input_tokens":1000,"cached_input_tokens":800,"cache_write_input_tokens":0,"output_tokens":20}}}}}}"#
            )
        };
        let text = [
            r#"{"type":"turn_context","payload":{"model":"gpt-6.1-sol"}}"#.to_owned(),
            record("a"),
            record("a"),
            r#"{"type":"turn_context","payload":{"model":"gpt-6-luna"}}"#.to_owned(),
            record("b"),
        ]
        .join("\n");
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{e}"));
        let usage = session_usage("codex", &path);
        assert_eq!(usage["gpt-6.1-sol"].requests, 1);
        assert_eq!(usage["gpt-6.1-sol"].uncached_input, 200);
        assert_eq!(usage["gpt-6.1-sol"].cache_read, 800);
        assert_eq!(usage["gpt-6-luna"].output, 20);
        assert!(session_usage("codex", &dir.path().join("missing")).is_empty());
    }
}
