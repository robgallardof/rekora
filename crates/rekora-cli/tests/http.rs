//! Real HTTP MCP child-process tests with temporary on-disk project stores.

#![allow(clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rekora");
const TOKEN: &str = "test-token-with-at-least-32-characters";

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn request(address: SocketAddr, extra: &str, body: &str) -> (u16, String, String) {
    request_host(address, "localhost", extra, body)
}

fn request_host(address: SocketAddr, host: &str, extra: &str, body: &str) -> (u16, String, String) {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(stream, "POST /mcp HTTP/1.1\r\nHost: {host}\r\nAccept: application/json, text/event-stream\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n{extra}\r\n{body}", body.len()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, payload) = response.split_once("\r\n\r\n").unwrap();
    let code = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    let payload = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut remaining = payload.as_bytes();
        let mut decoded = Vec::new();
        loop {
            let end = remaining.windows(2).position(|b| b == b"\r\n").unwrap();
            let length =
                usize::from_str_radix(std::str::from_utf8(&remaining[..end]).unwrap(), 16).unwrap();
            if length == 0 {
                break;
            }
            remaining = &remaining[end + 2..];
            decoded.extend_from_slice(&remaining[..length]);
            remaining = &remaining[length + 2..];
        }
        String::from_utf8(decoded).unwrap()
    } else {
        payload.to_owned()
    };
    let payload = if head
        .to_ascii_lowercase()
        .contains("content-type: text/event-stream")
    {
        payload
            .lines()
            .filter_map(|line| {
                line.strip_prefix("data: ")
                    .or_else(|| line.strip_prefix("data:"))
            })
            .find(|data| !data.trim().is_empty())
            .unwrap_or("")
            .to_owned()
    } else {
        payload
    };
    (code, head.to_owned(), payload)
}

#[test]
fn http_auth_session_persistence_and_origin_validation() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new(BIN)
            .current_dir(dir.path())
            .arg("init")
            .output()
            .unwrap()
            .status
            .success()
    );
    let child = Command::new(BIN)
        .current_dir(dir.path())
        .args([
            "mcp",
            "serve",
            "--http",
            "--listen",
            "127.0.0.1:0",
            "--allow-host",
            "container.test",
        ])
        .env("REKORA_MCP_TOKEN", TOKEN)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server(child);
    let stderr = server.0.stderr.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(address) = line
                .strip_prefix("MCP HTTP listening on http://")
                .and_then(|s| s.strip_suffix("/mcp"))
            {
                let _ = tx.send(address.parse::<SocketAddr>().unwrap());
            }
        }
    });
    let address = rx.recv_timeout(Duration::from_secs(20)).unwrap();
    let init = json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{"protocolVersion":"2025-11-25", "capabilities":{}, "clientInfo":{"name":"http-test", "version":"1"}}}).to_string();
    assert_eq!(request(address, "", &init).0, 401);
    assert_eq!(
        request(address, "Authorization: Bearer wrong\r\n", &init).0,
        401
    );
    let auth = format!("Authorization: Bearer {TOKEN}\r\n");
    assert_eq!(
        request_host(address, "hostile.invalid", &auth, &init).0,
        403
    );
    assert_eq!(request_host(address, "container.test", &auth, &init).0, 200);
    assert_eq!(
        request(
            address,
            &format!("{auth}Origin: https://hostile.invalid\r\n"),
            &init
        )
        .0,
        403
    );
    let (code, headers, body) = request(address, &auth, &init);
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["result"]["serverInfo"]["name"],
        "rekora"
    );
    let session = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("mcp-session-id")
                .then(|| value.trim().to_owned())
        })
        .unwrap();
    let headers =
        format!("{auth}Mcp-Session-Id: {session}\r\nMCP-Protocol-Version: 2025-11-25\r\n");
    let notified = request(
        address,
        &headers,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    assert_eq!(notified.0, 202);
    let remember = json!({"jsonrpc":"2.0", "id":2, "method":"tools/call", "params":{"name":"remember", "arguments":{"content":"Invoices archived for ten years"}}}).to_string();
    let (code, _, body) = request(address, &headers, &remember);
    assert_eq!(code, 200, "{body}");
    let result: Value = serde_json::from_str(&body).unwrap();
    assert_ne!(result["result"]["isError"], true, "{result}");
    let output = Command::new(BIN)
        .current_dir(dir.path())
        .args(["--format", "json", "memory", "search", "invoices archived"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["hits"][0]["memory"]["status"], "candidate");
    assert_eq!(
        result["hits"][0]["memory"]["content"],
        "Invoices archived for ten years"
    );
    let modern = json!({"jsonrpc":"2.0", "id":3, "method":"tools/call", "params": {
        "name":"search", "arguments":{"query":"invoices archived"}, "_meta": {
            "io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientInfo":{"name":"http-modern", "version":"1"},
            "io.modelcontextprotocol/clientCapabilities":{}
        }
    }})
    .to_string();
    let (code, _, body) = request(
        address,
        &format!(
            "{auth}MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/call\r\nMcp-Name: search\r\n"
        ),
        &modern,
    );
    assert_eq!(code, 200, "{body}");
    let result: Value = serde_json::from_str(&body).unwrap();
    assert_ne!(result["result"]["isError"], true, "{result}");
    assert!(body.contains("Invoices archived for ten years"), "{body}");
}

#[test]
fn http_rejects_missing_or_weak_token_and_conflicting_transport() {
    for token in [None, Some("short")] {
        let mut command = Command::new(BIN);
        command.args(["mcp", "serve", "--http"]);
        command.env_remove("REKORA_MCP_TOKEN");
        if let Some(token) = token {
            command.env("REKORA_MCP_TOKEN", token);
        }
        assert!(!command.output().unwrap().status.success());
    }
    assert_eq!(
        Command::new(BIN)
            .args(["mcp", "serve", "--http", "--stdio"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}
