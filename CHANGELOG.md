# Changelog

## Unreleased

- Stage B vertical slice: `rekora init`, `status`, `memory add|search|show|verify|dispute|supersede|delete`,
  `context`, `doctor`, `mcp serve --stdio` (tools `search`, `context`, `remember`, `status`).
- SQLite (bundled) + FTS5 store with versioned migrations, pre-upgrade snapshots and an append-only audit log.
- Source-bound memories with BLAKE3 freshness checks, path-boundary enforcement and secret redaction.
