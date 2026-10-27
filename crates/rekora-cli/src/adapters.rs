//! Host adapters for Claude Code, Codex, OpenCode 1.x and DeepSeek Harness.
//!
//! Each adapter knows one config file and one entry it owns (`rekora`):
//!
//! | Host | File | Entry |
//! |---|---|---|
//! | Claude Code | `<project>/.mcp.json` (project scope) | `mcpServers.rekora` |
//! | Codex | `$CODEX_HOME/config.toml`, default `~/.codex/config.toml` (user scope) | `[mcp_servers.rekora]` |
//!
//! Safety contract (master prompt §8, test matrix R10, R24, R32):
//! - `--dry-run` (the default) prints the planned change and writes nothing;
//! - a malformed or unreadable file is never rewritten;
//! - unrelated entries are preserved (JSON key order kept, TOML edited in
//!   place with `toml_edit`, so comments and formatting survive);
//! - the original is backed up under `.rekora/backups/` before the first write;
//! - the file is re-read right before writing and the write is aborted if it
//!   changed since planning; the write itself is temp-file + rename;
//! - uninstall removes only the `rekora` entry, and deletes the file only if
//!   Rekora created it and nothing else was added since.
//!
//! Nothing here runs a shell: the command and arguments are stored as
//! structured values, so paths with spaces or quotes cannot inject anything.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::project::Project;

/// Name of the entry Rekora owns in every host config.
pub const ENTRY: &str = "rekora";

/// Supported hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Host {
    /// Anthropic Claude Code.
    Claude,
    /// OpenAI Codex CLI.
    Codex,
    /// OpenCode 1.x.
    OpenCode,
    /// Official DeepSeek Harness.
    Deepseek,
}

impl std::str::FromStr for Host {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "opencode" => Ok(Self::OpenCode),
            "deepseek" | "dsh" => Ok(Self::Deepseek),
            other => bail!("unknown agent `{other}`; supported: claude, codex, opencode, deepseek"),
        }
    }
}

impl Host {
    /// All hosts with an adapter.
    pub const ALL: [Host; 4] = [Host::Claude, Host::Codex, Host::OpenCode, Host::Deepseek];

    /// Stable id.
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Deepseek => "deepseek",
        }
    }

    fn binary(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Deepseek => "dsh",
        }
    }

    /// Config file this adapter edits.
    pub fn config_path(self, project: &Project) -> Result<PathBuf> {
        match self {
            Self::Claude => Ok(project.checkout.join(".mcp.json")),
            Self::OpenCode => {
                if project.checkout.join("opencode.jsonc").exists() {
                    bail!(
                        "opencode.jsonc exists; merge the Rekora entry manually to preserve comments"
                    );
                }
                Ok(project.checkout.join("opencode.json"))
            }
            Self::Deepseek => Ok(project.checkout.join(".dsh/rekora.cordis.yml")),
            Self::Codex => {
                let home = match std::env::var_os("CODEX_HOME") {
                    Some(dir) if !dir.is_empty() => PathBuf::from(dir),
                    _ => home_dir()
                        .ok_or_else(|| anyhow!("cannot find the home directory"))?
                        .join(".codex"),
                };
                Ok(home.join("config.toml"))
            }
        }
    }

    /// `project` or `user`: how far the change reaches.
    pub fn scope(self) -> &'static str {
        match self {
            Self::Claude | Self::OpenCode | Self::Deepseek => "project",
            Self::Codex => "user",
        }
    }

    /// Installed version from `<binary> --version`, if the host is on PATH.
    pub fn detected_version(self) -> Option<String> {
        let out = Command::new(self.binary())
            .arg("--version")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .filter(|v| !v.is_empty())
    }

    /// Honest support level (see docs/architecture/ADAPTERS.md).
    pub fn support(self) -> &'static str {
        match self {
            Self::OpenCode => {
                "implemented; OpenCode 1.x project MCP configuration; see docs/architecture/ADAPTERS.md"
            }
            Self::Deepseek => {
                "implemented; explicit dsh --patch overlay; no model inference claimed"
            }
            Self::Claude => {
                "implemented; config contract-tested; connection checked with `claude mcp list` on 2.1.310"
            }
            Self::Codex => {
                "implemented; config contract-tested; listing checked with `codex mcp list` on 0.164.0"
            }
        }
    }
}

/// How the host should launch Rekora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Launch {
    /// Executable (absolute path, or a bare name resolved through PATH).
    pub command: String,
    /// Arguments.
    pub args: Vec<String>,
}

impl Launch {
    /// Default launch: this executable's absolute path, discovering the
    /// project from the host's working directory at each call.
    pub fn current(command: Option<String>) -> Result<Self> {
        let command = match command {
            Some(c) => c,
            None => std::env::current_exe()
                .context("locating the rekora executable")?
                .to_string_lossy()
                .into_owned(),
        };
        Ok(Self {
            command,
            args: vec!["mcp".into(), "serve".into(), "--stdio".into()],
        })
    }
}

/// What an install/uninstall would do.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// Host.
    pub host: Host,
    /// `project` or `user`.
    pub scope: &'static str,
    /// File to change.
    pub path: PathBuf,
    /// `create_file`, `add_entry`, `update_entry`, `remove_entry`,
    /// `delete_file`, or `none`.
    pub action: &'static str,
    /// The owned entry after the change (absent for removals).
    pub entry: Option<Value>,
    /// File content after the change (`None` when the file is deleted).
    #[serde(skip)]
    after: Option<String>,
    /// File content when planned (`None` when it did not exist).
    #[serde(skip)]
    before: Option<String>,
}

/// Adapter bookkeeping in `.rekora/adapters.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    installs: Vec<InstallRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallRecord {
    host: Host,
    path: PathBuf,
    created_file: bool,
    backup: Option<PathBuf>,
}

/// Plans an install of `launch` for `host`.
pub fn plan_install(host: Host, project: &Project, launch: &Launch) -> Result<Plan> {
    let path = host.config_path(project)?;
    let before = read_optional(&path)?;
    let (after, entry, existed_entry) = match host {
        Host::Claude => json_install(before.as_deref(), launch)?,
        Host::Codex => toml_install(before.as_deref(), launch)?,
        Host::OpenCode | Host::Deepseek => {
            project_install(host, before.as_deref(), launch, project)?
        }
    };
    let action = match (&before, existed_entry) {
        (Some(b), _) if *b == after => "none",
        (None, _) => "create_file",
        (Some(_), true) => "update_entry",
        (Some(_), false) => "add_entry",
    };
    Ok(Plan {
        host,
        scope: host.scope(),
        path,
        action,
        entry: Some(entry),
        after: Some(after),
        before,
    })
}

/// Plans removal of Rekora's entry for `host`.
pub fn plan_uninstall(host: Host, project: &Project) -> Result<Plan> {
    let path = host.config_path(project)?;
    let before = read_optional(&path)?;
    let Some(text) = before.as_deref() else {
        return Ok(Plan {
            host,
            scope: host.scope(),
            path,
            action: "none",
            entry: None,
            after: None,
            before,
        });
    };
    let (after, removed, now_empty) = match host {
        Host::Claude => json_uninstall(text)?,
        Host::Codex => toml_uninstall(text)?,
        Host::OpenCode | Host::Deepseek => project_uninstall(host, text)?,
    };
    let created_by_us = load_state(project)?
        .installs
        .iter()
        .any(|r| r.host == host && r.path == path && r.created_file);
    let (action, after) = if !removed {
        ("none", Some(text.to_owned()))
    } else if now_empty && created_by_us {
        ("delete_file", None)
    } else {
        ("remove_entry", Some(after))
    };
    Ok(Plan {
        host,
        scope: host.scope(),
        path,
        action,
        entry: None,
        after,
        before,
    })
}

/// Applies a plan with backup, concurrent-modification check and atomic write.
pub fn apply(plan: &Plan, project: &Project) -> Result<()> {
    if plan.action == "none" {
        return Ok(());
    }
    let current = read_optional(&plan.path)?;
    if current != plan.before {
        bail!(
            "{} changed while Rekora was planning; nothing was written, re-run the command",
            plan.path.display()
        );
    }
    let mut state = load_state(project)?;
    let backup = match &plan.before {
        Some(content) => {
            let dir = project.data_dir().join("backups");
            std::fs::create_dir_all(&dir)?;
            let stamp = time_stamp();
            let file = dir.join(format!("{}-{stamp}.bak", plan.host.id()));
            std::fs::write(&file, content)
                .with_context(|| format!("writing backup {}", file.display()))?;
            Some(file)
        }
        None => None,
    };
    match &plan.after {
        Some(content) => write_atomic(&plan.path, content)?,
        None => std::fs::remove_file(&plan.path)
            .with_context(|| format!("removing {}", plan.path.display()))?,
    }
    match plan.action {
        "remove_entry" | "delete_file" => {
            state
                .installs
                .retain(|r| !(r.host == plan.host && r.path == plan.path));
        }
        _ => {
            let created_file = plan.action == "create_file"
                || state
                    .installs
                    .iter()
                    .any(|r| r.host == plan.host && r.path == plan.path && r.created_file);
            state
                .installs
                .retain(|r| !(r.host == plan.host && r.path == plan.path));
            state.installs.push(InstallRecord {
                host: plan.host,
                path: plan.path.clone(),
                created_file,
                backup,
            });
        }
    }
    save_state(project, &state)
}

/// Read-only adapter health for `doctor`: the file parses and, when Rekora's
/// entry exists, its command still exists on disk.
pub fn check(host: Host, project: &Project) -> Result<Option<String>> {
    let path = host.config_path(project)?;
    let Some(text) = read_optional(&path)? else {
        return Ok(None);
    };
    let command = match host {
        Host::OpenCode | Host::Deepseek => {
            let doc: Value = serde_json::from_str(&text)?;
            if host == Host::OpenCode {
                doc.pointer("/mcp/rekora/command/0")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            } else {
                doc.as_array()
                    .and_then(|rows| rows.iter().find(|r| r["id"] == "mcp-rekora"))
                    .and_then(|r| r.pointer("/config/command"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            }
        }
        Host::Claude => {
            let doc: Value = serde_json::from_str(&text)
                .map_err(|e| anyhow!("{} is not valid JSON: {e}", path.display()))?;
            doc.get("mcpServers")
                .and_then(|s| s.get(ENTRY))
                .and_then(|e| e.get("command"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        }
        Host::Codex => {
            let doc: toml_edit::DocumentMut = text
                .parse()
                .map_err(|e| anyhow!("{} is not valid TOML: {e}", path.display()))?;
            doc.get("mcp_servers")
                .and_then(|s| s.get(ENTRY))
                .and_then(|e| e.get("command"))
                .and_then(|c| c.as_str())
                .map(str::to_owned)
        }
    };
    Ok(match command {
        None => Some(format!("{}: not installed", path.display())),
        Some(cmd) if Path::new(&cmd).is_absolute() && !Path::new(&cmd).is_file() => Some(format!(
            "{}: entry points to missing executable {cmd} (re-run `rekora agent install {}`)",
            path.display(),
            host.id()
        )),
        Some(cmd) => Some(format!("{}: installed ({cmd})", path.display())),
    })
}

// Project hosts use structured commands; the DeepSeek overlay is JSON-valid YAML.
fn project_install(
    host: Host,
    before: Option<&str>,
    launch: &Launch,
    project: &Project,
) -> Result<(String, Value, bool)> {
    let mut doc: Value = match before {
        Some(text) => serde_json::from_str(text)
            .context("existing host configuration is not valid JSON; not modified")?,
        None if host == Host::OpenCode => json!({}),
        None => json!([]),
    };
    let (entry, existed) = if host == Host::OpenCode {
        let root = doc
            .as_object_mut()
            .ok_or_else(|| anyhow!("OpenCode config must be an object"))?;
        let servers = root
            .entry("mcp")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("OpenCode mcp must be an object"))?;
        if servers.contains_key("servers") {
            bail!("OpenCode 2.x schema is not supported by this adapter");
        }
        let mut command = vec![launch.command.clone()];
        command.extend(launch.args.clone());
        let entry = json!({"type":"local", "command":command, "enabled":true});
        let existed = servers.insert(ENTRY.into(), entry.clone()).is_some();
        (entry, existed)
    } else {
        let rows = doc
            .as_array_mut()
            .ok_or_else(|| anyhow!("DeepSeek overlay must be an array"))?;
        let existed = rows.iter().any(|r| r["id"] == "mcp-rekora");
        rows.retain(|r| r["id"] != "mcp-rekora");
        let entry = json!({"id":"mcp-rekora", "name":"@deepseek-ai/dsh-mcp-client", "config":{
            "serverName":ENTRY, "transport":"stdio", "command":launch.command, "args":launch.args,
            "cwd":project.checkout
        }});
        rows.push(entry.clone());
        (entry, existed)
    };
    let after = if let Some(text) =
        before.filter(|t| serde_json::from_str::<Value>(t).ok().as_ref() == Some(&doc))
    {
        text.to_owned()
    } else {
        format!("{}\n", serde_json::to_string_pretty(&doc)?)
    };
    Ok((after, entry, existed))
}

fn project_uninstall(host: Host, text: &str) -> Result<(String, bool, bool)> {
    let mut doc: Value =
        serde_json::from_str(text).context("invalid host configuration; not modified")?;
    let (removed, empty) = if host == Host::OpenCode {
        let root = doc
            .as_object_mut()
            .ok_or_else(|| anyhow!("OpenCode config must be an object"))?;
        let removed = root
            .get_mut("mcp")
            .and_then(Value::as_object_mut)
            .is_some_and(|servers| servers.remove(ENTRY).is_some());
        let empty = root.len() == 1
            && root
                .get("mcp")
                .and_then(Value::as_object)
                .is_some_and(|s| s.is_empty());
        (removed, empty)
    } else {
        let rows = doc
            .as_array_mut()
            .ok_or_else(|| anyhow!("DeepSeek overlay must be an array"))?;
        let len = rows.len();
        rows.retain(|r| r["id"] != "mcp-rekora");
        (rows.len() != len, rows.is_empty())
    };
    Ok((
        format!("{}\n", serde_json::to_string_pretty(&doc)?),
        removed,
        empty,
    ))
}

// ---------------------------------------------------------------- JSON ----

fn json_entry(launch: &Launch) -> Value {
    json!({ "type": "stdio", "command": launch.command, "args": launch.args })
}

fn json_install(before: Option<&str>, launch: &Launch) -> Result<(String, Value, bool)> {
    let mut doc: Value = match before {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text)
            .map_err(|e| anyhow!("existing .mcp.json is not valid JSON ({e}); not modified"))?,
        _ => json!({}),
    };
    let root = doc
        .as_object_mut()
        .ok_or_else(|| anyhow!("existing .mcp.json is not a JSON object; not modified"))?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow!("`mcpServers` is not an object; not modified"))?;
    let existed = servers.contains_key(ENTRY);
    let entry = json_entry(launch);
    servers.insert(ENTRY.to_owned(), entry.clone());
    let mut text = serde_json::to_string_pretty(&doc)?;
    text.push('\n');
    // Keep the file byte-identical when nothing changed (idempotent install).
    if let Some(b) = before
        && serde_json::from_str::<Value>(b).ok().as_ref() == Some(&doc)
    {
        text = b.to_owned();
    }
    Ok((text, entry, existed))
}

fn json_uninstall(text: &str) -> Result<(String, bool, bool)> {
    let mut doc: Value = serde_json::from_str(text)
        .map_err(|e| anyhow!("existing .mcp.json is not valid JSON ({e}); not modified"))?;
    let removed = doc
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .is_some_and(|s| s.remove(ENTRY).is_some());
    let now_empty = doc
        .as_object()
        .is_some_and(|o| o.len() == 1 && o["mcpServers"].as_object().is_some_and(|s| s.is_empty()));
    let mut out = serde_json::to_string_pretty(&doc)?;
    out.push('\n');
    Ok((out, removed, now_empty))
}

// ---------------------------------------------------------------- TOML ----

fn toml_install(before: Option<&str>, launch: &Launch) -> Result<(String, Value, bool)> {
    let mut doc: toml_edit::DocumentMut = before
        .unwrap_or_default()
        .parse()
        .map_err(|e| anyhow!("existing config.toml is not valid TOML ({e}); not modified"))?;
    if !doc.contains_key("mcp_servers") {
        let mut table = toml_edit::Table::new();
        table.set_implicit(true);
        doc.insert("mcp_servers", toml_edit::Item::Table(table));
    }
    let servers = doc["mcp_servers"]
        .as_table_mut()
        .ok_or_else(|| anyhow!("`mcp_servers` is not a table; not modified"))?;
    let existed = servers.contains_key(ENTRY);
    let mut entry = toml_edit::Table::new();
    entry.insert("command", toml_edit::value(launch.command.as_str()));
    let mut args = toml_edit::Array::new();
    for a in &launch.args {
        args.push(a.as_str());
    }
    entry.insert("args", toml_edit::value(args));
    let unchanged = servers
        .get(ENTRY)
        .and_then(|e| e.as_table())
        .is_some_and(|t| {
            t.get("command").and_then(|c| c.as_str()) == Some(launch.command.as_str())
                && t.get("args").and_then(|a| a.as_array()).is_some_and(|a| {
                    a.iter().map(|v| v.as_str()).collect::<Vec<_>>()
                        == launch
                            .args
                            .iter()
                            .map(|s| Some(s.as_str()))
                            .collect::<Vec<_>>()
                })
                && t.len() == 2
        });
    if !unchanged {
        servers.insert(ENTRY, toml_edit::Item::Table(entry));
    }
    let entry_json = json!({ "command": launch.command, "args": launch.args });
    Ok((doc.to_string(), entry_json, existed))
}

fn toml_uninstall(text: &str) -> Result<(String, bool, bool)> {
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| anyhow!("existing config.toml is not valid TOML ({e}); not modified"))?;
    let removed = doc
        .get_mut("mcp_servers")
        .and_then(|s| s.as_table_mut())
        .is_some_and(|s| s.remove(ENTRY).is_some());
    if doc
        .get("mcp_servers")
        .and_then(|s| s.as_table())
        .is_some_and(|s| s.is_empty())
    {
        doc.remove("mcp_servers");
    }
    let now_empty = doc.is_empty() && doc.to_string().trim().is_empty();
    Ok((doc.to_string(), removed, now_empty))
}

// --------------------------------------------------------------- files ----

fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| anyhow!("{} is not UTF-8; not modified", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!("cannot read {}: {e}", path.display())),
    }
}

/// Writes via a sibling temp file and rename so readers never see a partial
/// file and a failure leaves the original untouched.
fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    if let Ok(meta) = std::fs::metadata(path)
        && meta.permissions().readonly()
    {
        bail!("{} is read-only; not modified", path.display());
    }
    let tmp = path.with_extension(format!("rekora-tmp-{}", std::process::id()));
    std::fs::write(&tmp, content).with_context(|| format!("writing {}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!("replacing {}: {e}", path.display()));
    }
    Ok(())
}

fn load_state(project: &Project) -> Result<State> {
    let path = project.data_dir().join("adapters.json");
    match read_optional(&path)? {
        Some(text) => serde_json::from_str(&text)
            .map_err(|e| anyhow!("{} is corrupt ({e}); fix or delete it", path.display())),
        None => Ok(State::default()),
    }
}

fn save_state(project: &Project, state: &State) -> Result<()> {
    let text = serde_json::to_string_pretty(state)?;
    write_atomic(&project.data_dir().join("adapters.json"), &text)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn time_stamp() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{nanos}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch() -> Launch {
        Launch {
            command: r"C:\Program Files\Rekora ñ\rekora.exe".into(),
            args: vec!["mcp".into(), "serve".into(), "--stdio".into()],
        }
    }

    #[test]
    fn json_preserves_unrelated_and_is_idempotent() {
        let before = r#"{
  "mcpServers": {
    "github": { "command": "gh-mcp", "args": ["--x"] },
    "db": { "command": "db-mcp", "env": { "A": "1" } }
  },
  "other": true
}
"#;
        let (after, _, existed) = json_install(Some(before), &launch()).unwrap();
        assert!(!existed);
        let doc: Value = serde_json::from_str(&after).unwrap();
        let orig: Value = serde_json::from_str(before).unwrap();
        assert_eq!(doc["mcpServers"]["github"], orig["mcpServers"]["github"]);
        assert_eq!(doc["mcpServers"]["db"], orig["mcpServers"]["db"]);
        assert_eq!(doc["other"], true);
        assert_eq!(doc["mcpServers"]["rekora"]["command"], launch().command);
        let (again, _, existed) = json_install(Some(&after), &launch()).unwrap();
        assert!(existed);
        assert_eq!(again, after, "second install changed bytes");
        let (removed, did, empty) = json_uninstall(&after).unwrap();
        assert!(did && !empty);
        assert_eq!(
            serde_json::from_str::<Value>(&removed).unwrap(),
            serde_json::from_str::<Value>(before).unwrap()
        );
    }

    #[test]
    fn toml_preserves_comments_and_tables() {
        let before = "# my settings\nmodel = \"x\" # inline\n\n[mcp_servers.docs]\ncommand = \"docs-mcp\"\nargs = [\"a b\"]\n\n[profiles.fast]\nmodel = \"y\"\n";
        let (after, _, existed) = toml_install(Some(before), &launch()).unwrap();
        assert!(!existed);
        assert!(after.starts_with("# my settings\nmodel = \"x\" # inline\n"));
        assert!(after.contains("[mcp_servers.docs]\ncommand = \"docs-mcp\"\nargs = [\"a b\"]"));
        assert!(after.contains("[profiles.fast]\nmodel = \"y\""));
        let parsed: toml_edit::DocumentMut = after.parse().unwrap();
        assert_eq!(
            parsed["mcp_servers"]["rekora"]["command"].as_str(),
            Some(launch().command.as_str())
        );
        let (again, _, _) = toml_install(Some(&after), &launch()).unwrap();
        assert_eq!(again, after);
        let (removed, did, empty) = toml_uninstall(&after).unwrap();
        assert!(did && !empty);
        assert_eq!(removed, before);
    }

    #[test]
    fn malformed_inputs_are_refused() {
        assert!(json_install(Some("{ not json"), &launch()).is_err());
        assert!(json_install(Some("[1,2]"), &launch()).is_err());
        assert!(json_install(Some(r#"{"mcpServers": 3}"#), &launch()).is_err());
        assert!(toml_install(Some("mcp_servers = 3"), &launch()).is_err());
        assert!(toml_install(Some("[[[bad"), &launch()).is_err());
    }

    #[test]
    fn toml_created_file_uninstalls_to_empty() {
        let (after, _, _) = toml_install(None, &launch()).unwrap();
        let (_, did, empty) = toml_uninstall(&after).unwrap();
        assert!(did && empty, "{after}");
    }
}
