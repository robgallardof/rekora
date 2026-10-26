# ADR-0001 — Single binary, SQLite + FTS5, one scope per database

- Status: accepted (Stage B)
- Date: 2026-10-26

## Context

Rekora must give different coding agents durable, verifiable project memory, offline, with one install
and no background service (planning/REKORA_MASTER_PROMPT.md §1, §4). The first slice needs storage,
keyword retrieval with abstention, a CLI and an MCP stdio server.

## Decision

1. **One executable, three crates.** `rekora-core` (rules, no I/O-heavy deps), `rekora-store`
   (SQLite, migrations, retrieval, context packs), `rekora-cli` (binary: CLI + MCP). Retrieval lives in
   the store crate because ranking is computed inside SQLite FTS5; a separate crate would only forward
   calls. We split again when a second retrieval backend exists.
2. **SQLite bundled via `rusqlite`**, WAL, `BEGIN IMMEDIATE` writes, `PRAGMA user_version` migrations
   in per-step transactions, `VACUUM INTO` snapshot before upgrading a non-empty database. This is a
   native C dependency; Rekora is *not* pure Rust and does not claim to be.
3. **One scope per database file** (`<project>/.rekora/rekora.db`). FTS5 BM25 statistics are
   table-wide, so mixing scopes in one table would let denied data influence ranks. Physical
   separation makes R03 hold by construction; queries still filter by `scope_id`.
4. **Deterministic retrieval with abstention.** Query → content terms (EN/ES stopwords dropped) →
   quoted FTS5 OR-match over current memories → coverage floor → explained multiplicative score.
   Nothing above the floor ⇒ `no_evidence`. Scores order results; they are not probabilities.
5. **Trust tiers at creation.** `import` text is always `candidate`; `agent` text is `candidate`
   unless it cites a source (CLI) — and over MCP it is always `candidate`. Verification, dispute,
   supersession and deletion are CLI (human) actions only.
6. **Sources are re-validated on read.** Stored paths are root-relative; freshness re-canonicalises
   and re-checks the root boundary before hashing, because the database is not trusted either.
7. **Official `rmcp` SDK (3.7)**, stdio only. Tested handshake: `initialize` with protocol
   `2025-11-25`. The 2026-07-28 `server/discover` flow is supported by the SDK but not yet covered by
   our tests.

## Rejected alternatives

- Separate daemon or HTTP server: adds an always-on process and an attack surface for no Stage B gain.
- Embeddings / vector DB in the default build: non-deterministic tests, model downloads, offline cost.
- One global database partitioned by scope: breaks rank isolation (see 3).
- Hand-written JSON-RPC: the official SDK tracks protocol revisions we would otherwise chase.

## Consequences

- Cross-project search is impossible today by design; a user-level private scope will be a second
  database, not a shared table.
- Token counts are estimates (UTF-8 bytes / 3, rounded up per item), conservative versus common BPE
  tokenizers; no tokenizer is shipped.
