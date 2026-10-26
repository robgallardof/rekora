# Changelog

## Unreleased

### Added
- Docker image and Compose service with persistent project mounts, custom host ports,
  non-root runtime and healthcheck; real Docker smoke test.
- Optional authenticated Streamable HTTP MCP (`--http --listen ADDRESS:PORT`),
  Host allowlist, browser Origin rejection and bounded HTTP request bodies.
- Durable tasks and versioned handoff documents (`rekora.handoff/1`): `task create|list|show|update`,
  `handoff create|resume`; MCP `task_list`, `task_update`, `handoff_create`, `handoff_resume`.
- Export/import (`rekora.export/1`) with `--dry-run`, idempotent re-import and untrusted-origin handling.
- `rekora-code`: incremental tree-sitter indexing for Rust, Python, JavaScript, TypeScript/TSX and C#;
  `index`, `symbols search`, MCP `symbols`.
- Worktree-aware store (linked worktrees share the main worktree's `.rekora/`), branch mismatch flags.
- Host adapters for Claude Code and Codex: `agent list|install|uninstall` (dry-run by default).
- MCP tool annotations (`readOnlyHint`, `destructiveHint`).
- `benchmark run --suite core|relevance` with a hand-labelled relevance corpus.
- Docs: architecture set, threat model, research notes, ADR-0002/0003, contributing/security policies,
  release workflow with checksums (draft releases).

### Changed
- FTS5 tokenizer is now `porter unicode61 remove_diacritics 2` (schema v4, automatic migration with snapshot).

### Fixed
- Id lookups scanned the whole table because the planner chose the `(scope_id, status)` index; search was
  ~1.3 s at 10k and ~14 s at 100k memories. Now uses the id index (see docs/architecture/PERFORMANCE.md).
- Handoff context searched one concatenated query, which raised the coverage floor and dropped relevant
  constraints; it now searches objective, remaining items and constraints separately.

## 0.1.0 (unreleased first slice)

- `rekora init`, `status`, `memory add|search|show|verify|dispute|supersede|delete`, `context`, `doctor`,
  `mcp serve --stdio` (tools `search`, `context`, `remember`, `status`).
- SQLite (bundled) + FTS5 store with versioned migrations, pre-upgrade snapshots and an append-only audit log.
- Source-bound memories with BLAKE3 freshness checks, path-boundary enforcement and secret redaction.
