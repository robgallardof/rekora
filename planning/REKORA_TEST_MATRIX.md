# REKORA — Executable QA & Real-World Test Matrix

**Status:** specification, not executed tests. The implementing agent must turn these cases into automated tests and publish truthful execution reports.

## Test policy

- `UNIT`: pure logic with deterministic inputs; no DB needed.
- `INTEGRATION`: real SQLite DB on disk, real Git repositories/worktrees, actual parser/FTS5, filesystem and MCP stdio subprocess. No fake storage or fake protocol in these tests.
- `E2E-LOCAL`: real compiled CLI and spawned MCP servers interacting over stdin/stdout after process restart; no LLM required.
- `E2E-HOST`: actual supported, user-authorized installed CLI/IDE agent process. Optional in CI; report `not executed` when unavailable.
- `PROPERTY/FUZZ`: randomized valid and invalid inputs asserting invariants, version compatibility and security boundaries.
- `SECURITY`: inputs intentionally hostile or secret-bearing; tests must ensure non-disclosure and no execution.
- `PERFORMANCE`: reproducible labeled corpus/fixtures with disclosure of CPU/RAM/OS/commit, warm/cold settings and statistic distribution.

All fixtures synthetic but realistic, built from actual source files and Git history. Tests must preserve temporary output on failure where safe; redact any secrets. No test may require paid cloud access by default.

### Required scenarios

| ID | Level | Precondition/fixture | Action | Assertions / acceptance |
|---|---|---|---|---|
| R01 | Integration/E2E | New temp git repo, empty DB | `rekora init`; create source-backed decision; close process; reopen and search | Same ID/content/source; no network or LLM; DB WAL valid |
| R02 | Integration/E2E | Stored pending task, process exited | Start MCP server in fresh child process; `handoff.resume` from distinct client | Task goal, decisions, branch, remaining actions, version and provenance intact |
| R03 | Security/Integration | Repo A+B each contain `AuthService`, conflicting facts | Search under scope A and B separately | Zero unauthorized content in results, ranking and context; negative asserts inspect raw JSON |
| R04 | Integration | Two Git worktrees with distinct commits and conflicting decisions | Query same concept per worktree | Correct branch facts, conflict explanation and no silent merge |
| R05 | Integration | Parser fixture with known symbol references | Rename/remove symbol; incrementally reindex | Only affected sources rescanned; dependent memory flagged possibly stale; historical source retained |
| R06 | Unit/Integration | Relevant verified, duplicate, superseded, disputed and irrelevant records | Search ranked results | Verified current result ranks appropriately; duplicates bounded; superseded excluded from current truth |
| R07 | Integration | Corpus with no matching topic | Search nonsense/unrelated topic | `no_evidence` or explicit low-confidence, not arbitrary nearest neighbor |
| R08 | Property/Integration | Candidate context exceeds 256 tokens | Request budgets 0, 1, 256, 1024 | Never exceeds measured budget; deterministic; no truncation that changes critical meaning |
| R09 | Security | README with prompt injection and exfiltration text | Ingest, retrieve, export; inspect disk/logs | Treated as untrusted source; never made instruction; no `.env` reads or network activity |
| R10 | Integration | Existing Claude/Codex config with 2 unrelated MCPs and hooks | Adapter dry-run/install/install again/uninstall | Valid config; identical unrelated entries; idempotent; approved backup; rollback works |
| R11 | Durability | SQLite migrations + seeded events | Inject crash/failure at controlled transaction points; reopen | Committed-or-rolled-back; consistent schema; recoverable snapshots, no partial migration |
| R12 | Concurrency | Disk DB, multiple independent OS processes | >=8 readers, >=4 writers, many updates | No lost update; monotonic versions; valid event log; no corruption |
| R13 | Property/Security | Windows/unix paths, Unicode, CRLF, oversized input | Use CLI/MCP import and path resolution | Typed bounded errors; no traversal, symlink or junction breakout; no uncontrolled panic |
| R14 | Integration | Export from a real repo, repeated import | Apply same import 2+ times | Idempotency; same effective state and no uncontrolled duplicate records |
| R15 | Concurrency | Task at version N; two writers read N | Both attempt conflicting state change | One wins or serialized; other gets version conflict with current state |
| R16 | Integration | C# + TS/Vue + Rust + Python synthetic compilable samples | Index files and search known symbols | Exact path/symbol/ref for supported grammars; unsupported semantics explicitly marked unknown |
| R17 | Security/E2E | Scoped private memory | Export, delete, search after delete, audit | Export only authorized; deleted excluded; declared retention behavior observable |
| R18 | E2E | Broken config, permission-denied SQLite dir, outdated adapter | Run `rekora doctor` | Actionable diagnostics, no implicit destructive changes, nonzero exit on critical failure |
| R19 | Performance | 1k, 10k, 100k realistic memory records | Warm/cold search/index/startup runs | Machine, corpus, commit, p50/p95/p99/RSS and relevance recorded; no claimed numbers without runs |
| R20 | E2E-HOST | Actual supported Claude/Codex install, opt-in permission | Install integration, run bounded hosted workflow | Real session retrieval works; host version recorded; otherwise `not executed` |
| R21 | Security | `.env`, token string in log, SSH private key fixture, hidden files | Index repo and store conversational event | Configured exclusions honored, sensitive text not persisted, visible false-positive policy |
| R22 | Security | Repo A path traversal and Windows junction into repo B | Try search/index outside grant | Forbidden, even after canonicalization; no data across scope |
| R23 | Integration | 10k records and matching IDs/relations | Compact/consolidate with preview, review/undo | No irreversible auto-merge; restoration retains sources and valid history |
| R24 | E2E | Existing host settings in read-only or malformed config | Adapter install | Meaningful error; original remains byte-for-byte unchanged; no partial mutation |
| R25 | Integration | Background scanner sees branch switch during scan | Switch branch mid-index | Scan restart or snapshot check avoids mixed-branch result |
| R26 | Integration | Concurrent task handoffs across processes | Create/resume/update repeated | Task never loses constraints; idempotent retry behavior preserved |
| R27 | Unit | Scores include conflicting recency/source authority | Generate controlled cases | Score explanation matches actual ranking, no inappropriate global normalization |
| R28 | Unit/Integration | UTC midnight/daylight-transition fixtures | Supersede/time travel queries | Correct valid-time vs observed-time handling; timezone only display |
| R29 | Property/Fuzz | Malformed JSON-RPC, deep JSON, huge tool args | Run real MCP tool handler | Validation, explicit size limits, stable server availability; no secret stack trace |
| R30 | Integration | Incompatible DB schema / interrupted update | Open with old/new binary simulation | Clear version error, safe snapshot, no accidental downgrade write |
| R31 | Security | Untrusted imported memory says to overwrite `AGENTS.md` or execute shell | Query through MCP | Output has untrusted annotation; no write/command generated from memory alone |
| R32 | Integration | Agent config path contains spaces/non-ASCII on Windows | Install + run command | Correct path quoting, no script injection, reversible uninstall |
| R33 | Regression | Empty DB and no project initialized | Run search/context/install errors | Useful error messages and codes, no panic or confusing success status |
| R34 | Relevance | 30 labeled questions with known answer and 10 unanswerable | Run repeatable retrieval eval | Precision/recall, MRR/nDCG, no-answer behavior and token use reported |
| R35 | Compatibility | MCP client supports old vs newest revision | Negotiate and call standard tools | Current-compatible behavior; proper rejection/compat mode on unsupported version |
| R36 | Performance | Re-index same repo twice, then change one file | Measure file reads/hashes/parse counts | Unchanged files not reparsed, performance recorded |

## Production-shaped fixtures

Create under `fixtures/` and use in tests (synthetic test data; no copied user projects):

**C# service:** .NET Web API/sample service with `OrderService`, `OrderLine`, `BusinessScope`, `IOrderRepository`, async update method, concurrency token, simple tests. Add two feature branches with different concurrency decisions. Git commits show refactor. Build with dotnet when available, but indexing test itself must not require dotnet installed.

**TypeScript frontend:** small Vite/Vue or Next-like fixture, `OrderList`, `OrderEditor`, shared API contract and property rename. Explicitly verify source locations. No mandatory npm install for indexing.

**Rust:** small Cargo project with `trait Storage`, `struct Memory`, two modules, renamed public API, unit test and Git history.

**Python:** package with import graph, function/class rename and dynamic behavior marked `unknown` rather than overclaimed.

**Hostile inputs:** crafted prompt-injection Markdown, symlink/junction escapes, path traversal, invalid UTF-8 where supported, large UTF-8, secret-like dummy strings, unsupported source extensions, deeply nested JSON, conflicting times.

**Memory benchmark corpus:** seeded realistic task decisions, duplicates, contradicted and superseded knowledge, irrelevant records and label file with expected matches + no-answer questions. Include generation script, seed, corpus license and independent quality-review checks. Label correctness explicitly; don't auto-evaluate solely by matching generator strings.

## Test structure for each scenario

Each test case must record:

```text
id:
level:
fixture:
operating_systems:
preconditions:
steps:
expected:
assertions:
cleanup:
repro_command:
result: NOT_RUN | PASS | FAIL | SKIPPED
skip_reason: required if skipped
artifacts: stderr/stdout, structured report, captured test DB path if safe
```

Avoid arbitrary sleeps: use explicit completion events or bounded retries. Resource cleanup must work on Windows even when SQLite processes hold file handles. Run tool subprocesses under deterministic timeouts and kill child process trees on failure.

## Coverage and quality gates

- All critical trust, scope and migration code must have positive, negative and edge-case tests, not only percentage coverage.
- Coverage aspiration: >=85% line coverage on critical domain/retrieval/storage modules, but not an excuse for superficial tests; document any exceptions.
- `cargo fmt`, `clippy -D warnings`, unit/integration/E2E-local, doctests and relevant cargo security/license checks must pass on supported CI runners.
- Linux/Windows/macOS native CI required for release. Experimental features tested separately and not required by default.
- Real host tests report the exact tested versions, never an all-agents blanket claim.
- Every confirmed issue or regression gains a reproducible test before marking resolved.
- Benchmarks must have stable baselines and explicit variance, reproducible machine/environment specs and comparison limitations.

## Acceptance walkthrough (minimum public MVP)

On a clean Windows PowerShell and on a Linux/macOS terminal:

1. Install/build Rekora using documented commands, no Python or Node required.
2. `rekora init --scope project` inside a Git fixture.
3. Add a source-bound memory through CLI and retrieve it with `--explain`.
4. Shut down; start `rekora mcp serve --stdio` from a real test client and retrieve that memory.
5. Execute a second-client handoff with branch binding.
6. Search another repo with a similarly named symbol; observe isolation.
7. Tamper with original source; observe staleness warning rather than silently returning unchanged truth.
8. Install a tested host adapter via `--dry-run`, then explicit `--confirm`; uninstall without damaging unrelated settings.
9. Execute the project's entire supported core test suite and security checks; keep results.
10. Run an offline benchmark with a published test corpus and show machine + commit + distributions.

The implementing team may release earlier milestones with a **truthful status**. This acceptance walkthrough defines completed public MVP, not promises before features are implemented.
