//! `rekora`: local-first, verifiable memory for AI coding agents.
//!
//! This binary is a thin adapter: argument parsing, project discovery and
//! output formatting live here; all rules live in `rekora-core` and all
//! persistence/retrieval in `rekora-store`.
//!
//! Exit codes: `0` success (including a `no_evidence` search, which is a valid
//! answer), `1` runtime error, `2` usage error (from clap), `3` `doctor` found a
//! critical problem.

mod mcp;
mod project;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rekora_core::{MemoryStatus, NewMemory, SourcePolicy};
use rekora_store::{SearchOptions, SearchOutcome, SearchStatus, Store, StoreError};
use serde_json::json;

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
    /// Project directory (defaults to searching upward from the current directory).
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
    /// Create the project store (`.rekora/`) at the Git top-level or current directory.
    Init {
        /// Scope to initialise. Only `project` exists today.
        #[arg(long, default_value = "project")]
        scope: String,
    },
    /// Show the project scope, schema and memory counts.
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
    /// Check the installation and the project store; never modifies anything.
    Doctor,
    /// Model Context Protocol server.
    #[command(subcommand)]
    Mcp(McpCommand),
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

#[derive(Subcommand)]
enum McpCommand {
    /// Serve MCP over stdin/stdout for the discovered project.
    Serve {
        /// Use the stdio transport (the only transport; flag kept for explicitness).
        #[arg(long, default_value_t = true)]
        stdio: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let format = cli.format;
    match run(cli) {
        Ok(code) => code,
        Err(error) => {
            let code = error
                .downcast_ref::<StoreError>()
                .map(StoreError::code)
                .unwrap_or("error");
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
            let policy = SourcePolicy::new(&project.root)?;
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
        Command::Doctor => doctor(&start, out),
        Command::Mcp(McpCommand::Serve { .. }) => {
            let project = Project::discover(&start)?;
            // Fail fast on a broken store instead of advertising tools that error.
            drop(Store::open(&project.db_path())?);
            mcp::serve(project)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn init(start: &std::path::Path, scope: &str, out: Output) -> Result<ExitCode> {
    if scope != "project" {
        bail!("scope `{scope}` is not supported yet; only `project` is implemented");
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
    out.emit(
        &json!({
            "root": project.root,
            "scope_id": store.scope_id(),
            "schema_version": schema,
            "branch": branch,
            "commit": commit,
            "memories": stats,
        }),
        || {
            let counts = stats
                .by_status
                .iter()
                .map(|(s, n)| format!("{s} {n}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "project: {}\nscope:   {}\nschema:  v{schema}\nbranch:  {}\nmemories: {} ({})",
                project.root.display(),
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

fn memory(
    project: &Project,
    cwd: &std::path::Path,
    cmd: MemoryCommand,
    out: Output,
) -> Result<ExitCode> {
    let mut store = Store::open(&project.db_path())?;
    let policy = SourcePolicy::new(&project.root)?;
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
                    if outcome.deduplicated { "Already stored" } else { "Stored" },
                    m.id,
                    m.kind,
                    m.status,
                    m.content
                );
                if outcome.status_downgraded {
                    text.push_str("\nnote: stored as candidate (agent/import text needs evidence or human confirmation)");
                }
                if !outcome.redactions.is_empty() {
                    text.push_str(&format!("\nnote: redacted {}", outcome.redactions.join(", ")));
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
                "    id {} v{} origin {} created {}\n",
                hit.memory.id, hit.memory.version, hit.memory.origin, hit.memory.created_at
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

fn doctor(start: &std::path::Path, out: Output) -> Result<ExitCode> {
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
                detail: project.root.display().to_string(),
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
            let ignored = project
                .root
                .join(project::DATA_DIR)
                .join(".gitignore")
                .is_file();
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
