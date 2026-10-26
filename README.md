# Rekora

Local-first, verifiable memory for AI coding agents. *(Español: [README.es.md](README.es.md))*

> **Status: early development (Stage B vertical slice).** Not released, not published. Everything
> below was run on Windows 11; Linux/macOS run in CI but have not been checked by hand yet. See
> [docs/STATUS.md](docs/STATUS.md) for exactly what is and isn't implemented.

Agents forget decisions between sessions, and a second agent starts from zero. Rekora keeps a small,
per-project store of decisions, constraints, facts and known issues — each with its sources, status
and history — and hands agents only what is relevant to the task, within a token budget, or says
`no_evidence` when it has nothing.

- One executable, offline, no model, no daemon, no telemetry.
- One SQLite database per project in `.rekora/` (git-ignored automatically).
- Memories cite files; if a cited file changes, results are flagged **possibly stale**.
- Agent-written memories stay `candidate` until a human verifies them.
- CLI and an MCP stdio server.

## Build

Requires stable Rust (declared MSRV 1.88, not yet verified in CI). SQLite is bundled; a C compiler is needed for the first build.

```bash
cargo build --release
# binary: target/release/rekora(.exe)
```

## Quickstart

```bash
cd your-repo
rekora init
rekora memory add --type decision --source docs/adr/0001.md:3 --verified "The local store uses SQLite in WAL mode"
rekora memory search "sqlite store" --explain
rekora context --task "work on the storage layer" --budget 1024
rekora doctor
```

Example `search --explain` output:

```text
- [decision | verified] Local store uses SQLite in WAL mode (src: docs/adr.md:2) #01a17977-81cf
    score 1.200 = relevance 1.000 (coverage 1.00 [sqlite store mode], bm25 rel 1.00) x kind 1.20 x status 1.00 x freshness 1.00 x branch 1.00
    source docs/adr.md:2: Fresh
```

Every command accepts `--format json` for automation. Errors in JSON mode are
`{"error": {"code": "...", "message": "..."}}` with stable codes (`not_initialized`, `source_outside_root`,
`version_conflict`, ...). Exit codes: `0` ok (a `no_evidence` search is ok), `1` error, `2` usage,
`3` `doctor` found a critical problem.

### Commands (implemented)

| Command | What it does |
|---|---|
| `init` | Create `.rekora/` at the Git top-level (or current dir). Idempotent. |
| `status` | Scope id, schema version, branch, counts. |
| `memory add TEXT [--type K] [--source PATH[:LINE]]... [--verified] [--origin human\|agent]` | Store a memory. Secrets are redacted; identical retries are deduplicated. |
| `memory search QUERY [--explain] [--limit N]` | Ranked current memories, or `no_evidence`. |
| `memory show ID` | Memory, source freshness and audit history. |
| `memory verify\|dispute ID [--expected-version N]` | Change status (optimistic concurrency). |
| `memory supersede OLD --by NEW` | Retire OLD from search, keep its history. |
| `memory delete ID --confirm` | Permanently delete content; the audit log keeps only the fact. |
| `context --task TEXT [--budget N]` | Budgeted briefing for an agent. |
| `doctor` | Read-only health checks. |
| `mcp serve --stdio` | MCP server with tools `search`, `context`, `remember`, `status`. |

## Using it from an agent (MCP)

Run the server from inside the project (or pass `--project PATH`):

```bash
rekora mcp serve --stdio
```

Register it with any MCP client that launches stdio servers. **Not yet tested inside a real Claude
Code, Codex or other host** — it is tested with a real MCP JSON-RPC client over stdio (protocol
`2025-11-25`). Host adapters (`rekora agent install ...`) are planned, not implemented.

`remember` over MCP always stores `candidate` memories with origin `agent`. Verifying, disputing,
superseding and deleting are CLI-only (human) actions.

## Limitations (today)

- Keyword retrieval only (SQLite FTS5); no synonyms or embeddings.
- Project scope only; no user-level or team scope, no import/export yet.
- No handoffs/tasks, no Git/worktree-aware indexing, no host adapters yet.
- Secret redaction is pattern-based and will miss unusual formats.
- Token counts are conservative estimates (UTF-8 bytes / 3), not a real tokenizer.
- No benchmarks have been run; no performance claims are made.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Tests use real SQLite files, real Git repositories and a real MCP child process. See [AGENTS.md](AGENTS.md)
for contribution rules and [docs/adr](docs/adr) for decisions. The original specification lives in
[planning/](planning/).

## License

Intended: MIT OR Apache-2.0 (license files to be added before the first public release).
