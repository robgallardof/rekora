# Rekora — milestone status

## English commit policy — 2026-10-27

- New commit subjects and bodies, including merge and squash messages, must be in English.
  `AGENTS.md`, `CONTRIBUTING.md` and the Claude entry point agree; existing history is preserved.
- EXECUTED: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace -j 2` (PASS). The first parallel test build exited while compiling
  `blake3`; the bounded retry passed without source changes.
- NOT EXECUTED for this documentation-only change: Docker, credentialed host/model probes or benchmarks.

Format from `planning/REKORA_IMPLEMENTATION_PLAN.md` ("Execution report template").

## Header-matched closing signature — 2026-10-26

- The RG closing banner now shares the cover's full-width canvas, gradient palette,
  rounded frame, large logo and text hierarchy. The existing headers are unchanged.
- Footers remain static; no loading traces or new motion were introduced.
- EXECUTED: local SVG/reference audit and light/dark visual inspection.
- EXECUTED: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (existing opt-in host/model tests remain ignored).
- NOT EXECUTED for this presentation-only update: provider inference, Docker smoke and benchmarks.

## README reading rhythm and motion correction — 2026-10-26

- Removed progress-like animated overlays from icons, separators, diagrams, terminals and signatures.
- Native Markdown headings and feature prose now wrap at the reader's font size; redundant divider images are gone.
- The compact RG signature stays centered and static. Only the cover background has a slow continuous opacity loop;
  labels and geometry do not move, and reduced motion disables the loop.
- EXECUTED: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (existing opt-in host/model tests remain ignored).
- EXECUTED: local references/SVG audit and browser layout checks; browser verified the background still running
  after 20 seconds and no text animations, plus reduced-motion suppression.
- NOT EXECUTED for this presentation correction: provider inference, Docker smoke and benchmarks.

## README layout, owner signature and Archify — 2026-10-26

- Reordered the English and Spanish READMEs from purpose and quickstart to advanced reference.
  OpenCode / DeepSeek setup now belongs to the MCP section, before the final footer.
- Added the actual RG owner logo to the centered final signature and exported bilingual,
  light/dark getting-started diagrams with Archify 3.2.0; sources and licenses are included.
- EXECUTED: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (existing opt-in host/model tests remain ignored).
- EXECUTED: local README image/link/anchor and SVG checks; Archify validation, delivery and browser checks.
- NOT EXECUTED for this documentation change: provider inference, Docker smoke and benchmarks.

## README SVG motion — 2026-10-26

- Added bounded, self-contained decorative CSS motion to locally owned README SVGs.
  Copy, commands and measurements are unchanged; text/data do not move.
- EXECUTED: XML parsing and visual/CSS checks; new motion respects reduced motion.
- EXECUTED: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (existing opt-in host tests remain ignored).
- NOT EXECUTED for this presentation change: model/provider calls, benchmarks and Docker smoke.

## OpenCode / DeepSeek Harness adapters — 2026-10-26

- Added opt-in project adapters: `rekora agent install opencode|deepseek` (alias `dsh`),
  dry-run default, backup, idempotence, unrelated-entry preservation and owned-entry removal.
- OpenCode 1.x schema only; JSONC / 2.x configurations are refused. The official DeepSeek
  MCP overlay pins the checkout working directory and writes no provider credentials.
- EXECUTED: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo test --workspace` (including real Git/temp SQLite/process adapter tests).
- EXECUTED on Windows: `python scripts/test_agent_hosts.py` with OpenCode 1.19.4 (real MCP
  connection) and DeepSeek Harness 0.2.1-rc.2 runtime (9 discovered tools, successful `status`
  against temporary SQLite). Added the same pinned probe to GitHub Actions.
- NOT EXECUTED: live provider/model inference (no DeepSeek API key), OpenCode 2.x,
  performance benchmarks and new Docker smoke for this adapter-only change.
- CI execution after pushing is recorded separately; the workflow's existence is not a passing result.

## Release v0.1.0 — 2026-10-26

- Tagged `v0.1.0` on main after CI passed on Windows, Linux and macOS (including the Docker and
  agent-host jobs). The tag triggers `.github/workflows/release.yml`, which builds and checksums the
  archives into a **draft** release; publishing the draft is a manual step on GitHub.

## Docker follow-up and pending release edits — 2026-10-26

- Re-executed `python scripts/test_docker.py`: passed for real stdio and
  authenticated HTTP, a custom published port, Compose health, database survival
  after container recreation and `doctor` on temporary data.
- Reviewed the pending release changes: build Intel macOS on `macos-latest`
  using the explicit Intel target, and keep the changelog honestly marked
  unreleased. No release tag or publication is implied by this commit.
- Required Rust checks remain passing; the release/tag-triggered workflow itself
  was **NOT EXECUTED** in this local follow-up.

## README SVG presentation — 2026-10-26

- Replaced decorative emoji in English/Spanish READMEs with eight original local
  line icons from [readme-builder](https://github.com/robgallardof/readme-builder).
  Preserved existing banners, numbered section assets, dividers, prose and examples.
- Cleaned Mermaid labels and repaired navigation fragments containing invisible
  variation selectors. Heading aliases preserve the former emoji-based link targets.
- Builder audit of all five README Markdown files: no decorative emoji, broken
  local references or unresolved local anchors found. Repeat migration: no changes.
- Required checks executed: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`:
  passed; 91 tests passed, 0 failed, 3 ignored (same helper/opt-in hosts as below).
- Live-host tests, Docker smoke and performance benchmarks were **NOT EXECUTED**
  for this presentation-only change; earlier execution records remain below.
- Other uncommitted release-workflow/changelog changes were preserved separately.

## Docker and HTTP MCP — 2026-10-26

### Implemented

- Multi-stage Dockerfile, non-root runtime, restricted build context, Git and
  healthcheck dependencies, and included MIT license. Compose supports persistent
  project mounts, custom host ports/interfaces, runtime UID:GID, restart policy,
  read-only root filesystem and dropped privileges.
- Optional `mcp serve --http --listen IP:PORT` with `/mcp` and `/healthz`.
  Stdio remains the default. HTTP requires `REKORA_MCP_TOKEN` (32+ printable
  ASCII characters), enforces Host validation and rejects browser Origin headers;
  `--allow-host` adds explicit deployment hostnames. Request body limit: 1 MiB.
- Uses rmcp's Streamable HTTP transport, including legacy sessions and modern
  stateless calls, with graceful SIGINT/SIGTERM shutdown. No global agent
  configuration is changed by this work.
- Added real HTTP integration tests, Docker/Compose smoke script and a Linux
  Docker CI job. Updated deployment instructions, both READMEs and threat model.

### Executed

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 91 passed, 0 failed, 3 ignored (two opt-in real-host
  tests and the subprocess helper, as described below).
- `cargo test -p rekora-cli --test continuity r20_real_ -- --ignored --nocapture`:
  both installed real hosts passed after adding HTTP dependencies.
- `docker build -t rekora:local .`: passed, Linux image built on Docker Desktop.
- `python scripts/test_docker.py`: passed against real Linux containers on
  Docker Desktop: stdio handshake, authenticated HTTP remember, invalid-token
  refusal, custom published port, Compose health, persisted data after container
  recreation, and `doctor`. Temporary containers and project were removed.
- Native HTTP tests passed for legacy sessions, modern `2026-07-28` stateless
  search, candidate trust, CLI-visible persistence, missing/weak credentials,
  transport conflicts, custom allowed Host, and hostile Host/Origin rejection.

### Not executed / limits

- New GitHub Actions Docker job, macOS runtime and Linux host UID mapping:
  **NOT EXECUTED** locally. Linux container build/runtime were executed.
- Remote TLS reverse proxy, browser/OAuth clients, multiple CPU architectures,
  HTTP load benchmarks and image registry publication: **NOT EXECUTED**.
- HTTP serves one mounted project with one shared bearer credential; there is
  no multi-user or multi-project routing. Health is liveness, not store integrity.
  Deployment details and persistent mount requirements: [DOCKER.md](DOCKER.md).

## Final host verification — 2026-10-26

- Re-executed both real-host tests with the installed Claude Code and Codex:
  `cargo test -p rekora-cli --test continuity r20_real_ -- --ignored --nocapture`
  — 2 passed, 0 failed, 0 ignored. Both returned the seeded memory ID.
- Hardened these tests to fail explicitly when a host is missing or exits with
  an error, rather than treating a missing executable as a successful test.
- After the change: `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` passed;
  `cargo test --workspace` passed (89 passed, 0 failed, 3 ignored). The two
  host tests were then executed separately and passed; the remaining ignored
  helper is exercised by the 12-process concurrency test.
- This closing verification did not re-run benchmarks, Linux/macOS execution,
  or release publication: **NOT EXECUTED** in this checkpoint.

## Milestone 2 — Stages C–F (v0.1 feature-complete) — 2026-10-26

### Implemented since Milestone 1

- **Continuity (Stage D):** durable versioned tasks, `rekora.handoff/1` documents, CLI `task …` / `handoff …`,
  MCP `task_list`, `task_update`, `handoff_create`, `handoff_resume`; optimistic concurrency everywhere.
- **Host adapters:** `agent list|install|uninstall` for Claude Code (project `.mcp.json`) and Codex
  (user `config.toml`), dry-run by default, backup, atomic write, owned-entry-only uninstall.
- **Export/import (Stage C):** `rekora.export/1`, `--dry-run`, idempotent, imported text untrusted.
- **Code + Git (Stage E):** `rekora-code` crate (tree-sitter declarations for Rust, Python, JS, TS/TSX, C#),
  per-branch snapshots, incremental re-index, snapshot-change refusal; linked worktrees share one store and
  other-branch memories are flagged.
- **Retrieval:** porter stemming (schema v4); id-index fix (search 100k: ~14 s → 131 ms p50); per-candidate
  coverage checks; handoff context searched per objective/remaining/constraint.
- **MCP:** 9 tools with `readOnlyHint`/`destructiveHint`; per-call project discovery; protocol
  `2025-11-25` (initialize) and `2026-07-28` (`server/discover`, stateless) both tested.
- **Hardening (Stage F):** `benchmark run --suite core|relevance`, threat model, architecture docs, research
  notes, ADR-0002/0003, contributing/security/conduct policies, release workflow (draft releases + SHA256SUMS).
- **Contributed test (another tool in this checkout):** `crates/rekora-store/tests/process_concurrency.rs` —
  4 writer + 8 reader OS processes contend on one versioned record; asserts version 101, contiguous audit log,
  25 changes per writer and integrity. Reviewed and kept.

### Commands run (Windows 11, rustc 1.99.1, git 2.55.0)

```text
cargo fmt --all -- --check                                    -> ok
cargo clippy --workspace --all-targets -- -D warnings         -> ok
cargo test --workspace                                        -> 89 passed, 0 failed, 3 ignored
cargo test -p rekora-cli --test continuity -- --ignored       -> 2 passed (real Claude Code 2.1.310, Codex 0.164.0)
rekora benchmark run --suite relevance                        -> P@1 0.933, R@5 1.000, MRR 0.967, specificity 1.0
rekora benchmark run --suite core --size 1000 --size 10000 --size 100000 --queries 300
                                                              -> see docs/architecture/PERFORMANCE.md
```

The 3 ignored tests are the two real-host tests (spend tokens; run separately above) and `process_worker`,
the child entry point that the R12 process test spawns 12 times.

CI (GitHub Actions, Windows/Linux/macOS) passed for commit `8f4aaf1`; later commits are checked by CI on push.

### Scenario coverage

All S01–S20 have automated tests except where noted; map in
[architecture/TESTING.md](architecture/TESTING.md). Real hosts (S20/R20): **executed and passing** on this
machine. Not covered: R21 (no conversational capture exists), R23 (no consolidation), R28 (no valid-time queries).

### Performance

Measured; see [architecture/PERFORMANCE.md](architecture/PERFORMANCE.md). The plan's target of warm search
p95 < 100 ms at 100k memories is **not met** on the synthetic worst-case corpus (182 ms); met at 10k (29 ms).

### Not implemented (deliberately deferred, see research/DECISION_LOG.md)

Embeddings/vector search, graph relations, lifecycle hooks/auto-capture, skill generation, probabilistic
decision models, sync, GUI, user/team scopes, memory consolidation, Spanish stemming, Kindex head-to-head.

### Known risks

- Relevance corpus written by the same author as the ranking code (risk of overfitting).
- 100k-memory latency above target; FTS5 candidate scoring dominates.
- Redaction is pattern-based. Codex adapter edits a user-wide file (only with `--confirm`).

### Next smallest milestone

Independent relevance corpus + Kindex FTS-only comparison; FTS5 candidate-generation optimisation for 100k;
first tagged release (`v0.1.0`) through the release workflow after reviewing the draft artifacts.

## Milestone 1 (historical) — Stage B vertical slice (PR 1–3 of the plan) — 2026-10-26

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
