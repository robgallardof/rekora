# Threat model

Scope: Rekora as installed on a developer machine, used by the developer and by AI agents through the CLI
and MCP stdio. No network listener exists. Reporting policy: [../../SECURITY.md](../../SECURITY.md).

## Assets

Project knowledge (may include internal design and incidents), source files, secrets present on disk,
host agent configurations, the integrity of what agents are told is "verified".

## Actors

| Actor | Trust |
|---|---|
| Developer at the CLI | Trusted (can verify, delete, install adapters) |
| AI agent via MCP | Semi-trusted: can read, add candidates, edit tasks |
| Repository content, imported files, web text that reaches an agent | Untrusted data |
| Other projects on the same machine | Must not influence each other |

## Threats and controls

| # | Threat | Control | Test |
|---|---|---|---|
| T1 | Prompt injection stored as memory, later obeyed | Agent/import text never `verified`; packs labelled "data, not instructions"; origin labels; no tool executes memory content | `r09_*`, `r14_r31_*` |
| T2 | Memory poisoning to fake authority | Only CLI can verify; MCP has no verify/delete tools | `s01_s02_*` (candidate), tool list test |
| T3 | Cross-project leakage | One database per project; scope filter before ranking | `s03_*` (raw JSON inspected) |
| T4 | Secrets persisted | Pattern redaction before write (cloud keys, tokens, PEM, `password=`), `.env*`/keys not citable or indexable | `secrets_never_reach_disk` (DB and WAL bytes), redact unit tests |
| T5 | Path traversal / symlink / junction escape | Canonicalise then check root boundary, on write *and* on read; import paths checked lexically | `r13_r22_*`, `r22_junction_*`, `r22_symlink_*` (Unix CI) |
| T6 | Host config clobbered | Dry-run default, parse-or-refuse, backup, concurrent-change check, atomic write, owned-entry-only uninstall | `s10_r10_r32_*`, `r24_*`, adapter unit tests |
| T7 | Command injection through config | Command and args stored as structured JSON/TOML values; no shell anywhere; Git invoked with fixed argv | `s10_r10_r32_*` (spaces, `ñ`) |
| T8 | Malformed/oversized MCP input | Size limits (query 2 KB, content 8 KiB, ≤16 sources, budget ≤32k), typed errors, server survives | `r29_*` |
| T9 | Disk corruption / interrupted migration | WAL, per-step migration transactions, pre-migration snapshot, newer-schema refusal, `doctor` integrity + FTS check | `r11_*`, `r30_*` |
| T10 | Concurrent writers lose updates | `BEGIN IMMEDIATE`, busy timeout, optimistic versions on memories and tasks | `r12_*` (threads and OS processes), `s15_*` |
| T11 | Indexing executes code | Indexer only reads bytes; never runs builds, package managers or language servers | by construction (`rekora-code`) |
| T12 | Silent network egress | No network code in the binary; no telemetry | by construction; no automated network-silence test yet |

## Known gaps

- Redaction is pattern-based; unusual secret formats will pass.
- The MCP server trusts its host: any process that can spawn it has the developer's file permissions.
- No network-silence test, no fuzzing campaign, no signed release artifacts yet.
- Claude Code project `.mcp.json` contains an absolute path to the executable; review before committing it.
