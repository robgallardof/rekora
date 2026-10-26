# Rekora — milestone status

Format from `planning/REKORA_IMPLEMENTATION_PLAN.md` ("Execution report template").

## Milestone 1 — Stage B vertical slice (PR 1–3 of the plan) — 2026-10-26

### Implemented

- Cargo workspace, three crates (see `docs/adr/0001-architecture.md`), `rekora` binary.
- `rekora init | status | doctor`, `memory add | search | show | verify | dispute | supersede | delete`,
  `context`, `mcp serve --stdio`; `--format json` everywhere with stable error codes.
- SQLite (bundled) + FTS5, WAL, `BEGIN IMMEDIATE`, migrations via `user_version` with per-step
  transactions and `VACUUM INTO` snapshot, refusal of newer schemas, append-only audit log.
- Source-bound memories: root-boundary check after canonicalisation (traversal, symlink, junction),
  exclusion of `.env*`, keys, `.git/`, `.rekora/`, `node_modules/`, `target/`; BLAKE3 freshness.
- Retrieval: EN/ES stopwords, quoted FTS5 terms (no syntax injection), coverage floor and
  `no_evidence`, explained score, branch mismatch flag, deterministic tie-break.
- Context packs: whole-item packing under a conservative token estimate; "data, not instructions"
  header; untrusted-origin and possibly-stale labels.
- Trust: imports never self-verify, agent text needs a source to start verified (CLI), MCP `remember`
  is always `candidate`; secret redaction before persistence.
- MCP (official `rmcp` 3.7.0): tools `search`, `context`, `remember`, `status`; input size limits;
  tool errors as `isError` results.
- Docs: README (EN/ES), AGENTS.md, ADR-0001, CHANGELOG, CI workflow (Windows/Linux/macOS).

### Not implemented (next stages)

- Stage A research deliverables (`docs/research/*`: competitive matrix, user problems) — **not done**.
- Import/export (R14 via import, R17 export), handoffs/tasks (S02 handoff, R15/R26 over MCP),
  host adapters (S10, R10, R20, R24, R32), Git/worktree indexing (S04, S05, S16, R25, R36),
  benchmarks (S19, R19, R34), user-level scope.
- MCP 2026-07-28 (`server/discover`) path not tested (R35).

### Commands run (Windows 11, rustc 1.99.1, cargo 1.99.1, git 2.55.0)

```text
cargo fmt --all -- --check                               -> ok
cargo clippy --workspace --all-targets -- -D warnings    -> ok, 0 warnings
cargo test --workspace                                   -> 52 passed, 0 failed, 0 skipped
cargo build --release                                    -> ok, rekora.exe 7,312,384 bytes
```

| Suite | Pass | Fail | Skipped |
|---|---|---|---|
| Unit (`rekora-core` 20, `rekora-store` 1) | 21 | 0 | 0 |
| Doctests (`rekora-core`) | 4 | 0 | 0 |
| Integration, real SQLite (`rekora-store/tests/store.rs`) | 16 | 0 | 0 |
| E2E-local, real binary + git + MCP child (`rekora-cli/tests/e2e.rs`) | 11 | 0 | 0 |
| E2E-host (real Claude Code / Codex) | — | — | NOT EXECUTED |

`r22_symlink_escape_is_rejected` is `#[cfg(unix)]` and did not compile/run on Windows; its Windows
counterpart `r22_junction_escape_is_rejected` ran and passed. Linux/macOS CI has not run yet (no remote).

### Scenario coverage (observed passing on Windows)

S01, S03, S06, S07, S08 (R08), S09 (store-level), S11 (R11 migration rollback), S12-lite (R12 with
4 writer + 8 reader *threads*, separate connections — not separate OS processes yet), S13 partial
(paths with spaces/Unicode, traversal, junction), S15 (store-level version conflict), S17 (delete),
R01, R27, R29 (subset), R30, R33, acceptance step 7 (tampered source ⇒ possibly stale).

### Security

Tested: scope isolation (raw JSON inspected), traversal/junction/secret-file rejection, redaction
not reaching DB or WAL bytes, FTS syntax injection, malformed MCP input keeps server alive.
Not done: written threat model, cross-process concurrency, fuzzing, network-silence test.

### Performance

NOT MEASURED.

### Known risks

- Coverage-floor abstention is a heuristic; unrelated queries sharing two content words with a
  memory will still match. Needs the labelled corpus (R34) to tune.
- Token estimate is a heuristic, not a tokenizer.
- `rmcp` 3.x is young; protocol 2026-07-28 behaviour untested here.

### Next smallest milestone

Stage C: `export`/`import --dry-run` with idempotent re-import (R14, R17), cross-process concurrency
test (R12 with OS processes), Stage A research docs, threat model, and the first CI run on all
three OSes.
