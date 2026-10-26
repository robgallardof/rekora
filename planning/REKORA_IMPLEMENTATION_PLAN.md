# REKORA — Implementation Roadmap, Milestones and Decisions

This file is a plan only. No implementation/test claims are made here. Each gate is objectively auditable.

## Product outcome

A developer using two different coding agents can resume work without reexplaining context, while Rekora securely retrieves relevant branch-appropriate source-backed memory with bounded token and CPU cost. All default features work offline and require only the Rekora executable.

## Release slices and dependencies

| Slice | Working feature | Prerequisite | Acceptance gate | Deferred |
|---|---|---|---|---|
| 0. Research | Evidence-backed competitor matrix, risky assumptions, ADR-0001, fixtures | None | Sources, specs and benchmark plan captured | All coding except scaffold |
| 1. Local truth | Rust CLI + schema + transactional local storage + source-bound memory | 0 | R01, R03, R07, R11 partial, cross-process restart | Vectors/graphs/agents |
| 2. MCP | `rekora mcp serve --stdio`, validated schemas, real client test | 1 | R02 read workflow, R29, R35 | HTTP/remote sync |
| 3. Retrieval | FTS5, abstention, explanation and context budget | 1,2 | R06,R08,R14,R27,R34 | Model summaries |
| 4. Safety/DX | Scope controls, import/export, audit, docs, doctor | 1-3 | R09,R13,R17,R18,R21,R22,R30,R31,R33 | Hosted SaaS |
| 5. Adapters | Claude/Codex install/dry-run/uninstall and version capabilities | 2,4 | R10,R20 (only if installed),R24,R32 | Unsupported native hooks |
| 6. Handoffs | Portable task state and optimistic concurrency | 1,2,5 | R02,R15,R26 | Distributed queue |
| 7. Git intelligence | Git/worktree and incremental tree-sitter indexing | 3,4 | R04,R05,R16,R25,R36 | Universal call graph |
| 8. Release hardening | Cross-platform CI, docs, packaging, coverage/benchmarks | all | R12,R19, release checklist | Optional AI features |
| 9. Optional research | vectors, graphs, skills, decision models, sync, GUI | data from 1-8 | A/B validated improvement documented in ADR | Never force into core |

Parallelization is allowed only when module contracts are frozen and integration branches are coordinated. Core storage/retrieval boundaries precede adapters. Never implement every slice concurrently on an empty repository.

## Essential architecture decisions to record

1. **Single executable + modular monolith** versus separate daemons: avoid mandatory background services.
2. **SQLite + FTS5** versus standalone search services: prefer portable transactions and one install. SQLite native code is an explicit tradeoff despite Rust-first philosophy.
3. **Versioned source evidence** versus free-text summary only: verification and stale detection have priority over context volume.
4. **Protocol MCP + host adapters** versus one vendor plugin: interoperability is the product; host hooks not guaranteed by MCP.
5. **Scope-first access control** versus filter-after-search: privacy and no cross-project influence.
6. **Candidate vs verified memory** versus trusting agent text: stored LLM output is unverified until evidence supports it.
7. **No mandatory embeddings/LLM** versus semantic-only search: default offline operation and deterministic tests.
8. **Experiment gates for probabilistic models** versus adopting Jev immediately: only add after controlled improvement.
9. **Minimal code index** versus invented universal code comprehension: begin with file/AST facts and explicit unknowns.
10. **No implicit global user config edits**: install is opt-in, reversible and diffable.

## Recommended MVP CLI experience

On first run (the following are designed commands, not existing ones):

```powershell
rekora --version
rekora init --scope project
rekora memory add --type decision --source ./docs/adr/0001.md "Use SQLite for local store"
rekora memory search "storage" --explain
rekora context --task "continue data layer" --budget 1024
rekora agent list
rekora agent install codex --dry-run
rekora doctor
```

CLI outputs should support pretty human text and stable `--format json` for automation. Exit codes and error schemas documented. Project-local cache/memory must be ignored by Git unless exported intentionally.

## Per-pull-request Definition of Done

- The change has a user scenario, risk/impact statement and clearly scoped code.
- The change contains Rustdoc for exported APIs, module comments and *why* comments on nonobvious logic.
- New behavior has deterministic positive, negative, regression, boundary, and if applicable security tests.
- Integration behavior uses actual dependencies rather than an internal mock of the dependency under test.
- Both error handling and cleanup are covered; no untracked file/config modifications.
- `fmt`, `clippy`, unit/integration tests are run with command/result recorded, or marked unexecuted with reason.
- Documentation, feature support matrix and examples reflect actual behavior.
- No unbenchmarked speed/accuracy assertion; any measured numbers include environment and corpus.
- Small diff; no redundant wrappers, frameworks or feature flags without users.

## Quality/performance policy

Default builds must compile and work without downloading model weights or contacting a service. Suggested first targets (not benchmark results): local search warmed p95 <100 ms for 100k short records on stated reference hardware; CLI start p95 <250 ms; zero idle daemon usage because daemon is absent by default. More important than raw speed: no scope leaks, no stale truth returned without warning, predictable `no_evidence`, bounded context and safe updates.

Use synthetic and optionally permissively licensed public code fixtures. For any comparison with Kindex, run actual supported Kindex version on the same platform/hardware with the same dataset and clearly annotate architecture differences. Publish failures and limitations.

## Release checklist

- [ ] Name and package availability reviewed for Rekora (GitHub, crates.io, trademark concerns; never assume).
- [ ] Cargo.lock, MSRV, rustfmt/clippy baseline fixed; SBOM and third-party license review.
- [ ] Default offline, no model required; no outbound calls without config.
- [ ] SQLite migrations, backups, recoverability, storage retention and deletions tested.
- [ ] MCP 2026-07-28 compatibility verified for actual used SDK; older versions handled honestly.
- [ ] Native Windows/Linux/macOS CI green; install/uninstall examples tested.
- [ ] CLI/MCP structured output and JSON schema version documented.
- [ ] Adapter host compatibility matrix with verified versions, limitations and test evidence.
- [ ] Threat model, prompt-injection/secret/cross-scope regression suite green.
- [ ] Published benchmark script, environment, seed, metrics and real results.
- [ ] README English + Spanish, quickstart, examples, troubleshooting, contribution and security docs.
- [ ] Optional features clearly distinguished from shipping features.
- [ ] Release artifacts with checksums; reproducible build notes where supported.

## First three pull requests (recommendation)

**PR 1 — Scaffold + reality check**: Rust workspace, minimal `rekora --version`, docs and ADR-0001, actual temp Git fixture, CI with formatting and tests. No empty feature crates. Observable passing test.

**PR 2 — Source-backed memory storage**: migrations, store transaction API, scope isolation, persist/reopen, CLI init/add/search, unit and on-disk integration tests. R01 and R03 must pass.

**PR 3 — MCP + retrieval**: `rekora mcp serve --stdio`, real child-process client, validated tool contracts, FTS5 + no-evidence threshold. R02 read, R07, R29 and basic conformance must pass.

## Execution report template

```markdown
### Milestone / commit
Implemented:
Not implemented:
Files changed:
Commands run:
Unit: X pass, Y fail, Z skipped (or not run + reason)
Integration: X pass, Y fail, Z skipped (or not run + reason)
E2E local: X pass, Y fail, Z skipped (or not run + reason)
Real host tests: tested host versions or NOT EXECUTED
Security tests:
Performance: setup, p50/p95/p99/RSS OR NOT MEASURED
Docs/comments updated:
Known risks:
Next smallest milestone:
```
