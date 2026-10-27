//! `rekora`: local-first, verifiable memory for AI coding agents.
//!
//! This binary is a thin adapter: argument parsing, project discovery and
//! output formatting live here; rules live in `rekora-core`, persistence and
//! retrieval in `rekora-store`, code indexing in `rekora-code`.
//!
//! Exit codes: `0` success (including a `no_evidence` search, which is a valid
//! answer), `1` runtime error, `2` usage error (from clap), `3` `doctor` found a
//! critical problem.

mod adapters;
mod bench;
mod hook;
mod mcp;
mod mcp_http;
mod project;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rekora_core::task::{TaskPatch, TaskState, TaskStatus};
use rekora_core::{MemoryStatus, NewMemory, SourcePolicy};
use rekora_store::{
    ExportFile, MAX_IMPORT_BYTES, SearchOptions, SearchOutcome, SearchStatus, Store, StoreError,
};
use serde_json::json;

use crate::adapters::{Host, Launch};
use crate::project::Project;

#[derive(Parser)]
#[command(
    name = "rekora",
    version,
    about = "Local-first, verifiable memory for AI coding agents"
)]
struct Cli {
    /// Output format.
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Project directory (defaults to searching from the current directory).
    #[arg(long, global = true)]
    project: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Command {
    /// Create the project store (`.rekora/`) at the repository's main worktree or current directory.
    Init {
        /// Scope to initialise. Only `project` exists.
        #[arg(long, default_value = "project")]
        scope: String,
    },
    /// Show the project scope, schema, branch and counts.
    Status,
    /// Add, search, inspect and curate memories.
    #[command(subcommand)]
    Memory(MemoryCommand),
    /// Build a token-budgeted context pack for a task.
    Context {
        /// What you are about to work on.
        #[arg(long)]
        task: String,
        /// Budget in estimated tokens (conservative: UTF-8 bytes / 3).
        #[arg(long, default_value_t = 1024)]
        budget: usize,
        /// Branch to prefer (defaults to the current Git branch).
        #[arg(long)]
        branch: Option<String>,
    },
    /// Durable tasks that survive sessions and move between agents.
    #[command(subcommand)]
    Task(TaskCommand),
    /// Hand a task to another agent, or pick one up.
    #[command(subcommand)]
    Handoff(HandoffCommand),
    /// Write every memory of this project (and tasks, for reference) as JSON.
    Export {
        /// Output file (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Import memories from an export file as untrusted `candidate`s.
    Import {
        /// Export file to read.
        file: PathBuf,
        /// Report what would happen without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Incrementally index the current checkout's source files (syntax-only).
    Index,
    /// Look up declared symbols from the code index.
    #[command(subcommand)]
    Symbols(SymbolsCommand),
    /// Register Rekora with agent hosts (Claude Code, Codex, OpenCode, DeepSeek Harness).
    #[command(subcommand)]
    Agent(AgentCommand),
    /// Check the installation, store and adapters; never modifies anything.
    Doctor,
    /// Host lifecycle hook (see `agent install claude-hooks|codex-hooks`). Reads the
    /// host's JSON payload on stdin and always exits 0 so it cannot break the host.
    Hook {
        /// Lifecycle event.
        #[arg(value_enum)]
        event: hook::HookEvent,
        /// Host reporting the event.
        #[arg(long, default_value = "agent")]
        host: String,
    },
    /// Model Context Protocol server.
    #[command(subcommand)]
    Mcp(McpCommand),
    /// Offline benchmarks in a temporary directory.
    #[command(subcommand)]
    Benchmark(BenchmarkCommand),
}

#[derive(Subcommand)]
enum MemoryCommand {
    /// Record a memory. Starts as `candidate` unless `--verified` is given.
    Add(AddArgs),
    /// Search current memories with ranking explanation.
    Search {
        /// Query text.
        query: String,
        /// Maximum results.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Show per-factor score breakdown.
        #[arg(long)]
        explain: bool,
        /// Branch to prefer (defaults to the current Git branch).
        #[arg(long)]
        branch: Option<String>,
    },
    /// Show one memory with sources, freshness and audit history.
    Show {
        /// Full id or unique prefix (8+ characters).
        id: String,
    },
    /// Mark a memory as verified (human confirmation).
    Verify(StatusArgs),
    /// Mark a memory as disputed (contradicted by evidence).
    Dispute(StatusArgs),
    /// Mark OLD as replaced by NEW; OLD stops appearing in search.
    Supersede {
        /// Memory being replaced.
        old: String,
        /// Replacement memory.
        #[arg(long)]
        by: String,
    },
    /// Permanently delete a memory's content (the audit log keeps only the fact of deletion).
    Delete {
        /// Full id or unique prefix.
        id: String,
        /// Required: deletion cannot be undone.
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Args)]
struct AddArgs {
    /// Memory text.
    content: String,
    /// fact, decision, constraint, preference, known-issue, solution, procedure.
    #[arg(long = "type", default_value = "fact")]
    kind: String,
    /// Evidence as PATH or PATH:LINE inside the project. Repeatable.
    #[arg(long = "source")]
    sources: Vec<String>,
    /// Record as verified (you confirm it is true).
    #[arg(long)]
    verified: bool,
    /// Who is writing: `human` (default) or `agent`. Agents should pass `agent`.
    #[arg(long, default_value = "human")]
    origin: String,
}

#[derive(Args)]
struct StatusArgs {
    /// Full id or unique prefix.
    id: String,
    /// Fail with `version_conflict` unless the memory is still at this version.
    #[arg(long)]
    expected_version: Option<i64>,
}

/// List edits shared by `task create`, `task update` and `handoff create`.
#[derive(Args, Default)]
struct PatchArgs {
    /// open, in_progress, blocked, done.
    #[arg(long)]
    status: Option<String>,
    /// Latest note for whoever picks this up.
    #[arg(long)]
    note: Option<String>,
    /// Who is working on it.
    #[arg(long)]
    owner: Option<String>,
    /// Add an accepted constraint. Repeatable.
    #[arg(long = "constraint")]
    constraints: Vec<String>,
    /// Add a decision. Repeatable.
    #[arg(long = "decision")]
    decisions: Vec<String>,
    /// Add a touched file. Repeatable.
    #[arg(long = "file")]
    files: Vec<String>,
    /// Add an executed test with its result, e.g. "cargo test: pass". Repeatable.
    #[arg(long = "test")]
    tests: Vec<String>,
    /// Add a failure or dead end. Repeatable.
    #[arg(long = "failure")]
    failures: Vec<String>,
    /// Add remaining work. Repeatable.
    #[arg(long = "todo")]
    remaining: Vec<String>,
    /// Move a remaining item (exact text) to done. Repeatable.
    #[arg(long = "complete")]
    complete: Vec<String>,
}

impl PatchArgs {
    fn into_patch(self) -> Result<TaskPatch> {
        Ok(TaskPatch {
            status: self
                .status
                .map(|s| s.parse())
                .transpose()
                .map_err(StoreError::from)?,
            note: self.note,
            owner: self.owner,
            add: TaskState {
                constraints: self.constraints,
                decisions: self.decisions,
                files: self.files,
                tests: self.tests,
                failures: self.failures,
                remaining: self.remaining,
                done: Vec::new(),
            },
            complete: self.complete,
        })
    }
}

// Parsed once per process; variant size is irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum TaskCommand {
    /// Create a task with a slug id (e.g. `auth-refactor`).
    Create {
        /// Task id: 1-64 chars of a-z 0-9 . _ -
        id: String,
        /// What the task is for.
        #[arg(long)]
        objective: String,
        #[command(flatten)]
        patch: PatchArgs,
    },
    /// List tasks (open work first).
    List {
        /// Include done tasks.
        #[arg(long)]
        all: bool,
    },
    /// Show one task.
    Show {
        /// Task id.
        id: String,
    },
    /// Update a task. Pass `--expected-version` to detect concurrent edits.
    Update {
        /// Task id.
        id: String,
        /// Version you read; the update fails if someone changed it since.
        #[arg(long)]
        expected_version: Option<i64>,
        #[command(flatten)]
        patch: PatchArgs,
    },
}

// Parsed once per process; variant size is irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum HandoffCommand {
    /// Record progress on a task and print the handoff document for the next agent.
    Create {
        /// Task id.
        #[arg(long)]
        task: String,
        /// Context budget in estimated tokens.
        #[arg(long, default_value_t = 1024)]
        budget: usize,
        #[command(flatten)]
        patch: PatchArgs,
    },
    /// Print the handoff document for a task (optionally claiming it).
    Resume {
        /// Task id.
        #[arg(long)]
        task: String,
        /// Claim the task: sets owner and status `in_progress`.
        #[arg(long)]
        owner: Option<String>,
        /// Context budget in estimated tokens.
        #[arg(long, default_value_t = 1024)]
        budget: usize,
    },
}

#[derive(Subcommand)]
enum SymbolsCommand {
    /// Find declarations by name (case-insensitive; exact, prefix, then substring).
    Search {
        /// Name or part of it.
        name: String,
        /// Snapshot (branch) to search; default: current branch. `all` searches every branch.
        #[arg(long)]
        branch: Option<String>,
        /// Maximum results.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum AgentCommand {
    /// Show supported hosts, detected versions and install state.
    List,
    /// Add Rekora's MCP server to a host's config (dry-run unless --confirm).
    Install(AgentArgs),
    /// Remove Rekora's entry from a host's config (dry-run unless --confirm).
    Uninstall(AgentArgs),
}

#[derive(Args)]
struct AgentArgs {
    /// claude or codex.
    host: String,
    /// Show the change without writing (the default).
    #[arg(long, conflicts_with = "confirm")]
    dry_run: bool,
    /// Write the change (backs up the original first).
    #[arg(long)]
    confirm: bool,
    /// Command the host should run (default: this executable's absolute path).
    #[arg(long)]
    command: Option<String>,
}

#[derive(Subcommand)]
enum McpCommand {
    /// Serve MCP over stdio (default) or Streamable HTTP. Discovered per call from
    /// `--project` or the working directory.
    Serve {
        /// Explicitly use stdio (the default when --http is absent).
        #[arg(long, conflicts_with = "http")]
        stdio: bool,
        /// Serve Streamable HTTP at /mcp; requires REKORA_MCP_TOKEN.
        #[arg(long)]
        http: bool,
        /// HTTP bind address and port; use 0.0.0.0:8080 inside Docker.
        #[arg(long, requires = "http", default_value = "127.0.0.1:8080")]
        listen: std::net::SocketAddr,
        /// Additional permitted HTTP Host authority (repeatable).
        #[arg(long, requires = "http")]
        allow_host: Vec<String>,
    },
}

#[derive(Subcommand)]
enum BenchmarkCommand {
    /// Run a suite: `core` (latency/throughput) or `relevance` (labelled quality).
    Run {
        /// Suite name.
        #[arg(long, default_value = "core")]
        suite: String,
        /// Corpus sizes for `core`. Repeatable.
        #[arg(long = "size", default_values_t = [1_000usize, 10_000])]
        sizes: Vec<usize>,
        /// Queries per size for `core`.
        #[arg(long, default_value_t = 200)]
        queries: usize,
        /// Seed for the synthetic corpus.
        #[arg(long, default_value_t = 42)]
        seed: u64,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let format = cli.format;
    match run(cli) {
        Ok(code) => code,
        Err(error) => {
            let code = error_code(&error);
            match format {
                Format::Json => println!(
                    "{}",
                    json!({ "error": { "code": code, "message": format!("{error:#}") } })
                ),
                Format::Text => eprintln!("error: {error:#}"),
            }
            ExitCode::FAILURE
        }
    }
}

fn error_code(error: &anyhow::Error) -> &'static str {
    if let Some(e) = error.downcast_ref::<StoreError>() {
        e.code()
    } else if let Some(e) = error.downcast_ref::<rekora_core::CoreError>() {
        e.code()
    } else if let Some(e) = error.downcast_ref::<rekora_code::IndexError>() {
        e.code()
    } else {
        "error"
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let cwd = std::env::current_dir().context("reading current directory")?;
    let start = cli.project.clone().unwrap_or_else(|| cwd.clone());
    let out = Output(cli.format);
    match cli.command {
        Command::Init { scope } => init(&start, &scope, out),
        Command::Status => status(&Project::discover(&start)?, out),
        Command::Memory(cmd) => memory(&Project::discover(&start)?, &cwd, cmd, out),
        Command::Context {
            task,
            budget,
            branch,
        } => {
            let project = Project::discover(&start)?;
            let store = Store::open(&project.db_path())?;
            let policy = SourcePolicy::new(&project.checkout)?;
            let branch = branch.or_else(|| project.git_context().0);
            let pack = store.context(&task, budget, branch, Some(&policy))?;
            out.emit(&pack, || {
                if pack.items.is_empty() {
                    format!(
                        "(empty pack: {})",
                        pack.reason.as_deref().unwrap_or("nothing relevant")
                    )
                } else {
                    format!(
                        "{}\n\n[{} of {} estimated tokens; {} relevant memories omitted]",
                        pack.render(),
                        pack.used_tokens,
                        pack.budget,
                        pack.omitted
                    )
                }
            });
            Ok(ExitCode::SUCCESS)
        }
        Command::Hook { event, host } => {
            match hook::run(event, &host, &start, std::io::stdin().lock()) {
                Ok(text) => print!("{text}"),
                Err(e) => eprintln!("rekora hook: {e:#}"),
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Task(cmd) => task(&Project::discover(&start)?, cmd, out),
        Command::Handoff(cmd) => handoff(&Project::discover(&start)?, cmd, out),
        Command::Export { output } => export(&Project::discover(&start)?, output, out),
        Command::Import { file, dry_run } => {
            import(&Project::discover(&start)?, &file, dry_run, out)
        }
        Command::Index => {
            let project = Project::discover(&start)?;
            let mut store = Store::open(&project.db_path())?;
            let report = rekora_code::index(&mut store, &project.checkout, &|| project.snapshot())?;
            out.emit(&report, || {
                format!(
                    "indexed {} ({}): {} scanned, {} parsed, {} file-only, {} unchanged, {} removed, {} symbols written, {} skipped\nlimits: {}",
                    project.checkout.display(),
                    report.snapshot,
                    report.scanned,
                    report.parsed,
                    report.file_only,
                    report.unchanged,
                    report.removed,
                    report.symbols_written,
                    report.skipped.len(),
                    report.limits
                )
            });
            Ok(ExitCode::SUCCESS)
        }
        Command::Symbols(SymbolsCommand::Search {
            name,
            branch,
            limit,
        }) => {
            let project = Project::discover(&start)?;
            let store = Store::open(&project.db_path())?;
            let snapshot = match branch.as_deref() {
                Some("all") => None,
                Some(b) => Some(b.to_owned()),
                None => Some(project.snapshot()),
            };
            let hits = store.find_symbols(&name, snapshot.as_deref(), limit)?;
            out.emit(&hits, || {
                if hits.is_empty() {
                    return "no symbols found (run `rekora index` first?)".to_owned();
                }
                hits.iter()
                    .map(|h| {
                        format!(
                            "{}:{}  {} {}  [{}]",
                            h.path, h.line, h.kind, h.name, h.snapshot
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            Ok(ExitCode::SUCCESS)
        }
        Command::Agent(cmd) => agent(&start, cmd, out),
        Command::Doctor => doctor(&start, out),
        Command::Mcp(McpCommand::Serve {
            http,
            listen,
            allow_host,
            ..
        }) => {
            if http {
                mcp_http::serve(start, listen, allow_host)?;
            } else {
                mcp::serve(start)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Benchmark(BenchmarkCommand::Run {
            suite,
            sizes,
            queries,
            seed,
        }) => {
            let report = bench::run(&suite, &sizes, queries, seed)?;
            // Benchmarks are always JSON: they are meant to be archived.
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn init(start: &Path, scope: &str, out: Output) -> Result<ExitCode> {
    if scope != "project" {
        bail!("scope `{scope}` is not supported; only `project` is implemented");
    }
    let project = Project::for_init(start)?;
    let (store, created) = Store::init(&project.db_path())?;
    project.protect_from_git()?;
    out.emit(
        &json!({
            "created": created,
            "root": project.root,
            "database": store.path(),
            "scope_id": store.scope_id(),
        }),
        || {
            format!(
                "{} Rekora project at {}\nstore: {}",
                if created {
                    "Initialised"
                } else {
                    "Already initialised:"
                },
                project.root.display(),
                store.path().display()
            )
        },
    );
    Ok(ExitCode::SUCCESS)
}

fn status(project: &Project, out: Output) -> Result<ExitCode> {
    let store = Store::open(&project.db_path())?;
    let stats = store.stats()?;
    let (branch, commit) = project.git_context();
    let schema = store.schema_version()?;
    let tasks = store.tasks(false)?.len();
    out.emit(
        &json!({
            "root": project.root,
            "checkout": project.checkout,
            "scope_id": store.scope_id(),
            "schema_version": schema,
            "branch": branch,
            "commit": commit,
            "memories": stats,
            "open_tasks": tasks,
        }),
        || {
            let counts = stats
                .by_status
                .iter()
                .map(|(s, n)| format!("{s} {n}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "project:    {}\ncheckout:   {}\nscope:      {}\nschema:     v{schema}\nbranch:     {}\nmemories:   {} ({})\nopen tasks: {tasks}",
                project.root.display(),
                project.checkout.display(),
                store.scope_id(),
                branch.as_deref().unwrap_or("-"),
                stats.total,
                if counts.is_empty() {
                    "none".into()
                } else {
                    counts
                }
            )
        },
    );
    Ok(ExitCode::SUCCESS)
}

fn memory(project: &Project, cwd: &Path, cmd: MemoryCommand, out: Output) -> Result<ExitCode> {
    let mut store = Store::open(&project.db_path())?;
    let policy = SourcePolicy::new(&project.checkout)?;
    const ACTOR: &str = "cli";
    match cmd {
        MemoryCommand::Add(args) => {
            let sources = args
                .sources
                .iter()
                .map(|spec| policy.resolve(spec, cwd))
                .collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::from)?;
            let (branch, commit) = project.git_context();
            let memory = NewMemory {
                kind: args.kind.parse().map_err(StoreError::from)?,
                content: args.content,
                origin: args.origin.parse().map_err(StoreError::from)?,
                status: if args.verified {
                    MemoryStatus::Verified
                } else {
                    MemoryStatus::Candidate
                },
                sources,
                branch,
                commit,
            };
            let outcome = store.add(memory.validate().map_err(StoreError::from)?, ACTOR)?;
            out.emit(&outcome, || {
                let m = &outcome.memory;
                let mut text = format!(
                    "{} {} [{} | {}] {}",
                    if outcome.deduplicated {
                        "Already stored"
                    } else {
                        "Stored"
                    },
                    m.id,
                    m.kind,
                    m.status,
                    m.content
                );
                if outcome.status_downgraded {
                    text.push_str("\nnote: stored as candidate (agent/import text needs evidence or human confirmation)");
                }
                if !outcome.redactions.is_empty() {
                    text.push_str(&format!(
                        "\nnote: redacted {}",
                        outcome.redactions.join(", ")
                    ));
                }
                text
            });
        }
        MemoryCommand::Search {
            query,
            limit,
            explain,
            branch,
        } => {
            let options = SearchOptions {
                limit,
                branch: branch.or_else(|| project.git_context().0),
            };
            let outcome = store.search(&query, &options, Some(&policy))?;
            out.emit(&outcome, || render_search(&outcome, explain));
        }
        MemoryCommand::Show { id } => {
            let memory = store.get(&id)?;
            let freshness: Vec<_> = memory
                .sources
                .iter()
                .map(|s| json!({ "source": s.location(), "freshness": policy.freshness(s) }))
                .collect();
            let history = store.history(&memory.id)?;
            let value = json!({ "memory": memory, "sources": freshness, "history": history });
            out.emit(&value, || {
                serde_json::to_string_pretty(&value).unwrap_or_default()
            });
        }
        MemoryCommand::Verify(StatusArgs {
            id,
            expected_version,
        }) => set_status(
            &mut store,
            &id,
            MemoryStatus::Verified,
            expected_version,
            out,
        )?,
        MemoryCommand::Dispute(StatusArgs {
            id,
            expected_version,
        }) => set_status(
            &mut store,
            &id,
            MemoryStatus::Disputed,
            expected_version,
            out,
        )?,
        MemoryCommand::Supersede { old, by } => {
            let record = store.supersede(&old, &by, ACTOR)?;
            out.emit(&record, || format!("{} superseded by {}", record.id, by));
        }
        MemoryCommand::Delete { id, confirm } => {
            if !confirm {
                bail!("refusing to delete without --confirm (deletion is permanent)");
            }
            let full = store.resolve_id(&id)?;
            store.delete(&full, ACTOR)?;
            out.emit(&json!({ "deleted": full }), || format!("Deleted {full}"));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn set_status(
    store: &mut Store,
    id: &str,
    status: MemoryStatus,
    expected_version: Option<i64>,
    out: Output,
) -> Result<()> {
    let record = store.set_status(id, status, expected_version, "cli")?;
    out.emit(&record, || {
        format!(
            "{} is now {} (v{})",
            record.id, record.status, record.version
        )
    });
    Ok(())
}

fn export(project: &Project, output: Option<PathBuf>, out: Output) -> Result<ExitCode> {
    let export = Store::open(&project.db_path())?.export()?;
    let text = serde_json::to_string_pretty(&export)?;
    match output {
        Some(path) => {
            std::fs::write(&path, text + "\n")
                .with_context(|| format!("writing {}", path.display()))?;
            out.emit(
                &json!({ "written": path, "memories": export.memories.len() }),
                || {
                    format!(
                        "Exported {} memories to {}",
                        export.memories.len(),
                        path.display()
                    )
                },
            );
        }
        None => println!("{text}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn import(project: &Project, file: &Path, dry_run: bool, out: Output) -> Result<ExitCode> {
    let size = std::fs::metadata(file)
        .with_context(|| format!("reading {}", file.display()))?
        .len();
    if size > MAX_IMPORT_BYTES {
        return Err(StoreError::InvalidImport(format!(
            "{} is {size} bytes; the limit is {MAX_IMPORT_BYTES}",
            file.display()
        ))
        .into());
    }
    let text =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let parsed: ExportFile = serde_json::from_str(&text)
        .map_err(|e| StoreError::InvalidImport(format!("not a Rekora export: {e}")))?;
    let mut store = Store::open(&project.db_path())?;
    let report = store.import(&parsed, dry_run, "cli")?;
    out.emit(&report, || {
        let mut text = format!(
            "{}{} created, {} duplicates, {} superseded skipped, {} rejected",
            if dry_run { "dry run: " } else { "" },
            report.created,
            report.duplicates,
            report.skipped_superseded,
            report.rejected.len()
        );
        for r in &report.rejected {
            text.push_str(&format!("\n  #{} {}: {}", r.index, r.code, r.reason));
        }
        text
    });
    Ok(ExitCode::SUCCESS)
}

fn render_task(t: &rekora_store::Task) -> String {
    let mut text = format!("{} [{}] v{}  {}\n", t.id, t.status, t.version, t.objective);
    if let Some(owner) = &t.owner {
        text.push_str(&format!("  owner: {owner}\n"));
    }
    if let Some(branch) = &t.branch {
        text.push_str(&format!("  branch: {branch}\n"));
    }
    if let Some(note) = &t.note {
        text.push_str(&format!("  note: {note}\n"));
    }
    for (label, items) in [
        ("constraints", &t.state.constraints),
        ("decisions", &t.state.decisions),
        ("files", &t.state.files),
        ("tests", &t.state.tests),
        ("failures", &t.state.failures),
        ("remaining", &t.state.remaining),
        ("done", &t.state.done),
    ] {
        if !items.is_empty() {
            text.push_str(&format!("  {label}:\n"));
            for item in items {
                text.push_str(&format!("    - {item}\n"));
            }
        }
    }
    text.trim_end().to_owned()
}

fn task(project: &Project, cmd: TaskCommand, out: Output) -> Result<ExitCode> {
    let mut store = Store::open(&project.db_path())?;
    match cmd {
        TaskCommand::Create {
            id,
            objective,
            patch,
        } => {
            let t = store.create_task(
                &id,
                &objective,
                &patch.into_patch()?,
                project.git_context(),
                "cli",
            )?;
            out.emit(&t, || render_task(&t));
        }
        TaskCommand::List { all } => {
            let tasks = store.tasks(all)?;
            out.emit(&tasks, || {
                if tasks.is_empty() {
                    return "no tasks".into();
                }
                tasks
                    .iter()
                    .map(|t| {
                        format!(
                            "{} [{}] v{} {} ({} remaining)",
                            t.id,
                            t.status,
                            t.version,
                            t.objective,
                            t.state.remaining.len()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        TaskCommand::Show { id } => {
            let t = store.task(&id)?;
            out.emit(&t, || render_task(&t));
        }
        TaskCommand::Update {
            id,
            expected_version,
            patch,
        } => {
            let patch = patch.into_patch()?;
            if patch.is_empty() {
                bail!(
                    "nothing to update; pass at least one of --status/--note/--todo/--complete/..."
                );
            }
            let t = store.update_task(&id, &patch, expected_version, None, "cli")?;
            out.emit(&t, || render_task(&t));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn handoff(project: &Project, cmd: HandoffCommand, out: Output) -> Result<ExitCode> {
    let mut store = Store::open(&project.db_path())?;
    let policy = SourcePolicy::new(&project.checkout)?;
    let (task_id, budget) = match cmd {
        HandoffCommand::Create {
            task,
            budget,
            patch,
        } => {
            let patch = patch.into_patch()?;
            // Always refresh the branch/commit binding at handoff time.
            store.update_task(&task, &patch, None, Some(project.git_context()), "cli")?;
            (task, budget)
        }
        HandoffCommand::Resume {
            task,
            owner,
            budget,
        } => {
            if let Some(owner) = owner {
                let claim = TaskPatch {
                    owner: Some(owner),
                    status: Some(TaskStatus::InProgress),
                    ..TaskPatch::default()
                };
                store.update_task(&task, &claim, None, None, "cli")?;
            }
            (task, budget)
        }
    };
    let doc = store.handoff(&task_id, budget, project.git_context().0, Some(&policy))?;
    out.emit(&doc, || {
        format!(
            "{}\n\n{}\n\n{}",
            render_task(&doc.task),
            if doc.context.items.is_empty() {
                "(no relevant memory)".to_owned()
            } else {
                doc.context.render()
            },
            doc.instructions
        )
    });
    Ok(ExitCode::SUCCESS)
}

fn agent(start: &Path, cmd: AgentCommand, out: Output) -> Result<ExitCode> {
    let project = Project::discover(start)?;
    match cmd {
        AgentCommand::List => {
            let rows: Vec<_> = Host::ALL
                .iter()
                .map(|h| {
                    json!({
                        "host": h.id(),
                        "scope": h.scope(),
                        "config": h.config_path(&project).ok(),
                        "detected_version": h.detected_version(),
                        "state": adapters::check(*h, &project).unwrap_or_else(|e| Some(format!("{e:#}"))),
                        "support": h.support(),
                    })
                })
                .collect();
            out.emit(&rows, || {
                rows.iter()
                    .map(|r| {
                        format!(
                            "{} ({} scope) version: {}\n  config:  {}\n  state:   {}\n  support: {}",
                            r["host"].as_str().unwrap_or_default(),
                            r["scope"].as_str().unwrap_or_default(),
                            r["detected_version"].as_str().unwrap_or("not found on PATH"),
                            r["config"].as_str().unwrap_or("-"),
                            r["state"].as_str().unwrap_or("no config file"),
                            r["support"].as_str().unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        AgentCommand::Install(args) => {
            let host: Host = args.host.parse()?;
            let plan = adapters::plan_install(host, &project, &Launch::current(args.command)?)?;
            finish_plan(&plan, &project, args.confirm, out)?;
        }
        AgentCommand::Uninstall(args) => {
            let host: Host = args.host.parse()?;
            let plan = adapters::plan_uninstall(host, &project)?;
            finish_plan(&plan, &project, args.confirm, out)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn finish_plan(plan: &adapters::Plan, project: &Project, confirm: bool, out: Output) -> Result<()> {
    if confirm {
        adapters::apply(plan, project)?;
    }
    let applied = confirm && plan.action != "none";
    out.emit(&json!({ "plan": plan, "applied": applied }), || {
        let mut text = format!(
            "{} {} ({} scope): {} -> {}",
            if confirm { "applied" } else { "dry run" },
            plan.host.id(),
            plan.scope,
            plan.path.display(),
            plan.action
        );
        if let Some(entry) = &plan.entry {
            text.push_str(&format!(
                "\nentry `rekora`: {}",
                serde_json::to_string(entry).unwrap_or_default()
            ));
        }
        if !confirm && plan.action != "none" {
            text.push_str(
                "\nre-run with --confirm to write (the original is backed up under .rekora/backups/)",
            );
        }
        text
    });
    Ok(())
}

fn render_search(outcome: &SearchOutcome, explain: bool) -> String {
    if outcome.status == SearchStatus::NoEvidence {
        return format!(
            "no_evidence: {}",
            outcome
                .reason
                .as_deref()
                .unwrap_or("nothing relevant in this project")
        );
    }
    let mut text = String::new();
    if outcome.status == SearchStatus::LowConfidence {
        text.push_str("low_confidence: results match only part of the query\n");
    }
    for hit in &outcome.hits {
        text.push_str(&rekora_store::render_line(hit));
        text.push('\n');
        if explain {
            let e = &hit.explain;
            text.push_str(&format!(
                "    score {:.3} = relevance {:.3} (coverage {:.2} [{}], bm25 rel {:.2}) x kind {:.2} x status {:.2} x freshness {:.2} x branch {:.2}\n",
                hit.score,
                e.relevance,
                e.coverage,
                e.matched_terms.join(" "),
                e.bm25_relative,
                e.kind_priority,
                e.status_weight,
                e.freshness_factor,
                e.branch_factor
            ));
            for (location, freshness) in &hit.source_freshness {
                text.push_str(&format!("    source {location}: {freshness:?}\n"));
            }
            text.push_str(&format!(
                "    id {} v{} origin {} branch {} created {}\n",
                hit.memory.id,
                hit.memory.version,
                hit.memory.origin,
                hit.memory.branch.as_deref().unwrap_or("-"),
                hit.memory.created_at
            ));
        }
    }
    text.trim_end().to_owned()
}

/// One doctor check result.
struct Check {
    name: &'static str,
    ok: bool,
    critical: bool,
    detail: String,
}

fn doctor(start: &Path, out: Output) -> Result<ExitCode> {
    let mut checks = Vec::new();
    let git = std::process::Command::new("git")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    checks.push(Check {
        name: "git",
        ok: git.is_some(),
        critical: false,
        detail: git
            .unwrap_or_else(|| "git not found on PATH; branch/commit context disabled".into()),
    });
    match Project::discover(start) {
        Err(e) => checks.push(Check {
            name: "project",
            ok: false,
            critical: true,
            detail: format!("{e:#}"),
        }),
        Ok(project) => {
            checks.push(Check {
                name: "project",
                ok: true,
                critical: true,
                detail: format!(
                    "{} (checkout {})",
                    project.root.display(),
                    project.checkout.display()
                ),
            });
            match Store::open(&project.db_path()) {
                Err(e) => checks.push(Check {
                    name: "store",
                    ok: false,
                    critical: true,
                    detail: format!("{e} ({})", e.code()),
                }),
                Ok(store) => {
                    let problems = store.check()?;
                    checks.push(Check {
                        name: "store",
                        ok: problems.is_empty(),
                        critical: true,
                        detail: if problems.is_empty() {
                            format!(
                                "{} schema v{} ({} memories)",
                                store.path().display(),
                                store.schema_version()?,
                                store.stats()?.total
                            )
                        } else {
                            problems.join("; ")
                        },
                    });
                }
            }
            let ignored = project.data_dir().join(".gitignore").is_file();
            checks.push(Check {
                name: "gitignore",
                ok: ignored,
                critical: false,
                detail: if ignored {
                    ".rekora/ is excluded from Git".into()
                } else {
                    ".rekora/.gitignore missing; local memory could be committed (re-run `rekora init`)".into()
                },
            });
            for host in Host::ALL {
                let (ok, detail) = match adapters::check(host, &project) {
                    Ok(None) => (true, "no config file".to_owned()),
                    Ok(Some(state)) => (!state.contains("missing executable"), state),
                    Err(e) => (false, format!("{e:#}")),
                };
                checks.push(Check {
                    name: host.id(),
                    ok,
                    critical: false,
                    detail,
                });
            }
        }
    }
    let critical_failure = checks.iter().any(|c| c.critical && !c.ok);
    let value = json!({
        "ok": !critical_failure,
        "checks": checks.iter().map(|c| json!({
            "name": c.name, "ok": c.ok, "critical": c.critical, "detail": c.detail
        })).collect::<Vec<_>>(),
    });
    out.emit(&value, || {
        checks
            .iter()
            .map(|c| {
                let mark = if c.ok {
                    "ok  "
                } else if c.critical {
                    "FAIL"
                } else {
                    "warn"
                };
                format!("[{mark}] {:<9} {}", c.name, c.detail)
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(if critical_failure {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    })
}

/// Writes either the JSON form of a value or its human rendering.
#[derive(Clone, Copy)]
struct Output(Format);

impl Output {
    fn emit<T: serde::Serialize>(self, value: &T, text: impl FnOnce() -> String) {
        match self.0 {
            Format::Json => match serde_json::to_string_pretty(value) {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("error: serialising output: {e}"),
            },
            Format::Text => println!("{}", text()),
        }
    }
}
