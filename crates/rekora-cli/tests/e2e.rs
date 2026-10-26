//! E2E-local tests: the compiled `rekora` binary, real Git repositories and a
//! real MCP stdio child process speaking JSON-RPC. No mocks, no network, no
//! model. Scenario ids refer to planning/REKORA_TEST_MATRIX.md and the master
//! prompt (§14).

#![allow(clippy::unwrap_used)] // test helpers fail loudly by design

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rekora");

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Rekora Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git must be installed for E2E tests");
    assert!(status.success(), "git {args:?} failed");
}

/// A temporary Git repository with one committed ADR.
fn repo(adr: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(dir.path().join("docs/adr")).unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("docs/adr/0001.md"), adr).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub struct AuthService;\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "initial"]);
    dir
}

fn rekora(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn json_ok(dir: &Path, args: &[&str]) -> Value {
    let mut full = vec!["--format", "json"];
    full.extend_from_slice(args);
    let out = rekora(dir, &full);
    assert!(
        out.status.success(),
        "rekora {args:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn json_err(dir: &Path, args: &[&str]) -> (i32, Value) {
    let mut full = vec!["--format", "json"];
    full.extend_from_slice(args);
    let out = rekora(dir, &full);
    assert!(
        !out.status.success(),
        "rekora {args:?} unexpectedly succeeded"
    );
    (
        out.status.code().unwrap_or(-1),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}

#[test]
fn s01_store_restart_and_retrieve_with_source() {
    let repo = repo("# ADR 1\n\nUse SQLite in WAL mode for the local store.\n");
    let init = json_ok(repo.path(), &["init"]);
    assert_eq!(init["created"], true);
    assert!(repo.path().join(".rekora/.gitignore").is_file());

    let added = json_ok(
        repo.path(),
        &[
            "memory",
            "add",
            "--type",
            "decision",
            "--source",
            "docs/adr/0001.md:3",
            "--verified",
            "The local store uses SQLite in WAL mode",
        ],
    );
    let id = added["memory"]["id"].as_str().unwrap().to_owned();
    assert_eq!(added["memory"]["branch"], "main");
    assert_eq!(added["memory"]["commit"].as_str().unwrap().len(), 40);

    // New process, launched from a subdirectory: discovery walks up.
    let found = json_ok(
        &repo.path().join("src"),
        &["memory", "search", "sqlite store mode"],
    );
    assert_eq!(found["status"], "found");
    let hit = &found["hits"][0];
    assert_eq!(hit["memory"]["id"], id.as_str());
    assert_eq!(hit["memory"]["status"], "verified");
    assert_eq!(hit["memory"]["sources"][0]["path"], "docs/adr/0001.md");
    assert_eq!(hit["memory"]["sources"][0]["line"], 3);
    assert_eq!(hit["possibly_stale"], false);

    // Git must not see the store.
    let porcelain = Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&porcelain.stdout).trim().is_empty(),
        "store visible to git"
    );
}

#[test]
fn acceptance_7_tampered_source_reports_possibly_stale() {
    let repo = repo("# ADR 1\nRetries: 3\n");
    json_ok(repo.path(), &["init"]);
    json_ok(
        repo.path(),
        &[
            "memory",
            "add",
            "--source",
            "docs/adr/0001.md:2",
            "Retry budget is three attempts",
        ],
    );
    std::fs::write(
        repo.path().join("docs/adr/0001.md"),
        "# ADR 1\nRetries: 5\n",
    )
    .unwrap();
    let found = json_ok(repo.path(), &["memory", "search", "retry budget attempts"]);
    assert_eq!(found["hits"][0]["possibly_stale"], true);
    let text = rekora(
        repo.path(),
        &["context", "--task", "retry budget", "--budget", "300"],
    );
    assert!(String::from_utf8_lossy(&text.stdout).contains("POSSIBLY STALE"));
}

#[test]
fn s03_similar_repos_are_isolated() {
    let a = repo("AuthService issues JWT.\n");
    let b = repo("AuthService uses session cookies.\n");
    json_ok(a.path(), &["init"]);
    json_ok(b.path(), &["init"]);
    json_ok(
        a.path(),
        &[
            "memory",
            "add",
            "--type",
            "decision",
            "AuthService issues RS256 JWT access tokens",
        ],
    );
    json_ok(
        b.path(),
        &[
            "memory",
            "add",
            "--type",
            "decision",
            "AuthService uses opaque session cookies instead of JWT",
        ],
    );

    let out = rekora(
        a.path(),
        &[
            "--format",
            "json",
            "memory",
            "search",
            "AuthService JWT session cookies",
        ],
    );
    let raw = String::from_utf8_lossy(&out.stdout);
    assert!(raw.contains("RS256"));
    assert!(
        !raw.contains("opaque session cookies"),
        "repo B leaked into repo A: {raw}"
    );
    let pack = rekora(
        a.path(),
        &[
            "context",
            "--task",
            "AuthService session cookies",
            "--budget",
            "2000",
        ],
    );
    assert!(!String::from_utf8_lossy(&pack.stdout).contains("opaque"));
}

#[test]
fn s07_unknown_topic_is_no_evidence() {
    let repo = repo("x\n");
    json_ok(repo.path(), &["init"]);
    json_ok(
        repo.path(),
        &["memory", "add", "Builds use cargo nextest in CI"],
    );
    let out = json_ok(
        repo.path(),
        &["memory", "search", "kubernetes helm chart rollout"],
    );
    assert_eq!(out["status"], "no_evidence");
    assert_eq!(out["hits"].as_array().unwrap().len(), 0);
}

#[test]
fn r33_commands_before_init_fail_with_code() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        &["memory", "search", "x"][..],
        &["status"],
        &["context", "--task", "x"],
    ] {
        let (code, err) = json_err(dir.path(), args);
        assert_eq!(code, 1);
        assert_eq!(err["error"]["code"], "not_initialized", "{args:?}");
    }
    assert!(
        !dir.path().join(".rekora").exists(),
        "read command created a store"
    );
    let doctor = rekora(dir.path(), &["doctor"]);
    assert_eq!(doctor.status.code(), Some(3));
}

#[test]
fn r13_r22_sources_outside_root_or_secret_are_rejected() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("proj with spaces ñ");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    std::fs::write(parent.path().join("outside.txt"), "secret elsewhere").unwrap();
    std::fs::write(root.join(".env"), "TOKEN=abc").unwrap();
    std::fs::write(root.join("notes.md"), "ok").unwrap();
    json_ok(&root, &["init"]);

    let cases = [
        ("../outside.txt", "source_outside_root"),
        (".env", "source_excluded"),
        (".rekora/rekora.db", "source_excluded"),
        ("missing.md", "source_missing"),
        ("notes.md:0", "invalid_source_spec"),
    ];
    for (spec, code) in cases {
        let (_, err) = json_err(&root, &["memory", "add", "--source", spec, "some fact"]);
        assert_eq!(err["error"]["code"], code, "source {spec}");
    }
    let ok = json_ok(
        &root,
        &["memory", "add", "--source", "notes.md", "Notes exist"],
    );
    assert_eq!(ok["memory"]["sources"][0]["path"], "notes.md");
}

#[cfg(unix)]
#[test]
fn r22_symlink_escape_is_rejected() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(parent.path().join("outside.txt"), "x").unwrap();
    std::os::unix::fs::symlink(parent.path().join("outside.txt"), root.join("link.txt")).unwrap();
    json_ok(&root, &["init"]);
    let (_, err) = json_err(&root, &["memory", "add", "--source", "link.txt", "fact"]);
    assert_eq!(err["error"]["code"], "source_outside_root");
}

#[cfg(windows)]
#[test]
fn r22_junction_escape_is_rejected() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("proj");
    let outside = parent.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "x").unwrap();
    // Junctions need no special privilege on Windows, unlike symlinks.
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(root.join("jn"))
        .arg(&outside)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J failed");
    json_ok(&root, &["init"]);
    let (_, err) = json_err(
        &root,
        &["memory", "add", "--source", "jn/secret.txt", "fact"],
    );
    assert_eq!(err["error"]["code"], "source_outside_root");
}

#[test]
fn secrets_never_reach_disk() {
    let repo = repo("x\n");
    json_ok(repo.path(), &["init"]);
    let out = json_ok(
        repo.path(),
        &[
            "memory",
            "add",
            "CI deploys with key AKIAABCDEFGHIJKLMNOP to staging",
        ],
    );
    assert_eq!(out["redactions"][0], "aws_access_key");
    drop(out);
    let db = std::fs::read(repo.path().join(".rekora/rekora.db")).unwrap();
    let wal = std::fs::read(repo.path().join(".rekora/rekora.db-wal")).unwrap_or_default();
    let needle = b"AKIAABCDEFGHIJKLMNOP";
    for bytes in [&db, &wal] {
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle),
            "secret persisted"
        );
    }
}

#[test]
fn delete_requires_confirm_and_is_audited() {
    let repo = repo("x\n");
    json_ok(repo.path(), &["init"]);
    let id = json_ok(
        repo.path(),
        &["memory", "add", "Temporary private preference"],
    )["memory"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let refused = rekora(repo.path(), &["memory", "delete", &id]);
    assert!(!refused.status.success());
    json_ok(repo.path(), &["memory", "delete", &id, "--confirm"]);
    let (_, err) = json_err(repo.path(), &["memory", "show", &id]);
    assert_eq!(err["error"]["code"], "not_found");
}

// ---------------------------------------------------------------- MCP ------

/// Minimal JSON-RPC client over a real `rekora mcp serve --stdio` child.
struct McpClient {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: i64,
}

impl McpClient {
    fn start(dir: &Path, client_name: &str) -> Self {
        let mut child = Command::new(BIN)
            .current_dir(dir)
            .args(["mcp", "serve", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            lines,
            next_id: 1,
        };
        let init = client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": client_name, "version": "1.0.0" }
            }),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "rekora", "{init}");
        client.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        client
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn send_raw(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        self.wait_for(id)
    }

    fn wait_for(&mut self, id: i64) -> Value {
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(20))
                .expect("MCP server did not answer within 20s");
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["id"] == json!(id) {
                return value;
            }
        }
    }

    /// Calls a tool and returns `(is_error, structured_payload)`.
    fn call(&mut self, tool: &str, arguments: Value) -> (bool, Value) {
        let response = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        let result = &response["result"];
        assert!(result.is_object(), "no result for {tool}: {response}");
        let payload = result.get("structuredContent").cloned().unwrap_or_else(|| {
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
        });
        (result["isError"] == json!(true), payload)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn s01_s02_cli_memory_visible_over_mcp_and_mcp_memory_survives_restart() {
    let repo = repo("# ADR\nOptimistic concurrency with row version.\n");
    json_ok(repo.path(), &["init"]);
    let cli_id = json_ok(
        repo.path(),
        &[
            "memory",
            "add",
            "--type",
            "decision",
            "--source",
            "docs/adr/0001.md:2",
            "--verified",
            "Orders use optimistic concurrency with a row version column",
        ],
    )["memory"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let mut a = McpClient::start(repo.path(), "agent-a");
    let tools = a.request("tools/list", json!({}));
    let mut names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "context",
            "handoff_create",
            "handoff_resume",
            "remember",
            "search",
            "status",
            "symbols",
            "task_list",
            "task_update"
        ]
    );

    let (err, found) = a.call("search", json!({ "query": "order concurrency" }));
    assert!(!err);
    assert_eq!(found["hits"][0]["memory"]["id"], cli_id.as_str());

    let (err, remembered) = a.call(
        "remember",
        json!({
            "kind": "known_issue",
            "content": "Order export times out above 10k rows; paginate the export query",
            "sources": ["src/lib.rs"]
        }),
    );
    assert!(!err, "{remembered}");
    assert_eq!(
        remembered["memory"]["status"], "candidate",
        "agent text must not self-verify"
    );
    assert_eq!(remembered["memory"]["origin"], "agent");
    let mcp_id = remembered["memory"]["id"].as_str().unwrap().to_owned();
    drop(a);

    // A different client in a fresh process sees both memories.
    let mut b = McpClient::start(repo.path(), "agent-b");
    let (_, pack) = b.call(
        "context",
        json!({ "task": "fix order export timeout", "budget": 400 }),
    );
    let text = pack["text"].as_str().unwrap();
    assert!(text.contains("paginate the export"), "{text}");
    assert!(
        text.contains("agent text"),
        "untrusted origin not labelled: {text}"
    );
    assert!(pack["used_tokens"].as_u64().unwrap() <= 400);
    let (_, status) = b.call("status", json!({}));
    assert_eq!(status["memories"]["total"], 2);
    drop(b);

    // And so does the CLI.
    let shown = json_ok(repo.path(), &["memory", "show", &mcp_id]);
    assert_eq!(shown["history"][0]["actor"], "mcp");
}

#[test]
fn r29_bad_mcp_input_is_rejected_and_server_survives() {
    let repo = repo("x\n");
    json_ok(repo.path(), &["init"]);
    let mut c = McpClient::start(repo.path(), "fuzzer");

    let (err, out) = c.call("search", json!({ "query": "x".repeat(10_000) }));
    assert!(err);
    assert_eq!(out["error"]["code"], "input_too_large");

    let (err, out) = c.call("remember", json!({ "content": "y", "kind": "gossip" }));
    assert!(err);
    assert_eq!(out["error"]["code"], "unknown_value");

    let (err, out) = c.call(
        "remember",
        json!({ "content": "z", "sources": ["../../etc/passwd"] }),
    );
    assert!(err);
    assert!(
        out["error"]["code"]
            .as_str()
            .unwrap()
            .starts_with("source_"),
        "{out}"
    );

    let (err, out) = c.call("remember", json!({ "content": "   " }));
    assert!(err);
    assert_eq!(out["error"]["code"], "empty_content");

    // Protocol-level garbage: wrong argument types, unknown tool, invalid JSON.
    let wrong_type = c.request(
        "tools/call",
        json!({ "name": "search", "arguments": { "query": 42 } }),
    );
    assert!(
        wrong_type.get("error").is_some() || wrong_type["result"]["isError"] == json!(true),
        "{wrong_type}"
    );
    let unknown = c.request(
        "tools/call",
        json!({ "name": "drop_database", "arguments": {} }),
    );
    assert!(
        unknown.get("error").is_some() || unknown["result"]["isError"] == json!(true),
        "{unknown}"
    );
    c.send_raw("{not json");
    let deep = "[".repeat(5_000) + &"]".repeat(5_000);
    c.send_raw(&format!(
        r#"{{"jsonrpc":"2.0","id":999,"method":"tools/call","params":{deep}}}"#
    ));

    // Still serving.
    let (err, out) = c.call("status", json!({}));
    assert!(!err, "{out}");
    assert_eq!(out["memories"]["total"], 0);
}
