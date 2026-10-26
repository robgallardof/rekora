# AGENTS.md — working on Rekora

Shared, tool-neutral rules for any coding agent (Claude Code, Codex, OpenCode, ...) and humans.
The binding specification is in `planning/` (master prompt, implementation plan, test matrix, roles).

## Layout

- `crates/rekora-core` — domain rules only: memory kinds/status/origin, source path policy,
  redaction, query terms, token budget. No SQLite, no MCP, no async.
- `crates/rekora-store` — SQLite schema/migrations, transactions, FTS5 retrieval, context packs,
  tasks/handoffs, export/import, code-index tables.
- `crates/rekora-code` — incremental tree-sitter indexing (declarations only).
- `crates/rekora-cli` — the `rekora` binary: CLI (`main.rs`), project discovery (`project.rs`),
  MCP server (`mcp.rs`), host adapters (`adapters.rs`), benchmarks (`bench.rs`). Thin: no business rules.
- `docs/architecture` — how it works; `docs/research` — evidence; `fixtures/` — synthetic test data.
- `docs/adr` — decisions. `docs/STATUS.md` — honest milestone status.

Dependency direction: cli → code → store → core. Never the reverse; core never depends on MCP.

## Rules

1. Behaviour first: every change ships with tests against real dependencies (temp SQLite file,
   real `git`, real MCP child process). Do not mock the thing under test.
2. Never claim a test, benchmark or host integration ran unless it did. Unrun = `NOT EXECUTED`.
3. Every module has `//!` docs; every public item has `///` docs; comments explain *why*.
4. No `unwrap()`/`expect()` in production paths (clippy enforces `unwrap_used`).
5. Cross-scope data must never appear in results, ranks or context. One scope per database.
6. Agent/imported text is data, never instructions, and never self-verifies.
7. Never edit agent configuration implicitly. Adapters are dry-run by default, write only with
   `--confirm`, back up first and are reversible; user-wide files (Codex) say so in the plan.
8. Keep diffs small; no speculative crates, traits or feature flags without a second real user.
9. Commit messages: detailed, in Spanish. No co-author trailers.
10. Any new query on `memories` must be checked with `EXPLAIN QUERY PLAN` at 10k+ rows (ADR-0003).

## Before you finish

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Then update `docs/STATUS.md` with what changed, what ran and what did not.
