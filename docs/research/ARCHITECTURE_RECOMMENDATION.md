# Architecture recommendation (agent efficiency)

Status: **proposal awaiting architectural approval. Nothing here is implemented.** Evidence and
answers are in [AGENT_EFFICIENCY_RESEARCH.md](AGENT_EFFICIENCY_RESEARCH.md) and the files it links.
This version incorporates the independent review recorded in [DECISION_LOG.md](DECISION_LOG.md) (H10–H17).

## 1. Thesis

Rekora cannot make model requests cheaper directly: it does not own them, and the hosts already manage
prompt caching, compaction, tool search and subagents. The failures Rekora is positioned to reduce in
long coding sessions are (a) state lost or misreported around a cut-off, (b) task constraints and
decisions dropped when the host compacts, and (c) not knowing what a session cost. Rekora already holds
the durable per-project state for (a) and (b) and can read the host files that answer (c).

So the smallest useful architecture is **not** a new efficiency engine. It is narrow extensions of what
exists, each gated by a measurement:

1. Make cut-off capture correct (record `StopFailure`; session-scoped, fresh turns; no false briefings).
2. Make host compaction less lossy by pointing the summary at Rekora and re-injecting the active task
   *as data* after compaction.
3. Build the measurement harness before any of the above is accepted.
4. Later, and only if measured as useful: atomic claims with leases, overlap warnings, a usage ledger.

## 2. Constraints respected

Rust; existing SQLite store and migrations; crate direction cli → code → store → core; local-first;
MCP and CLI as the only surfaces; existing hooks and adapters (dry-run, backup, reversible, `--confirm`);
existing task and handoff semantics (versioned, optimistic concurrency, agent text is data and is never
promoted to instructions or to `verified`).

**Not introduced:** new crates, a new database, a daemon, an orchestrator, a proxy, a proprietary-model
dependency, an LLM call on any lifecycle event, a semantic cache.

## 3. Placement by crate

| Concern | Crate | Why there |
|---|---|---|
| Turn freshness/retention rules, lease arithmetic, provider-neutral usage record, dated price table, overlap rules | `rekora-core` | Pure domain rules, no I/O |
| Turn rows, lease columns, `usage_event` table, queries (each checked with `EXPLAIN QUERY PLAN` at 10k+ rows, rule 10) | `rekora-store` | Persistence |
| Per-host hook event tables, new hook handlers, host-file parsers (Claude transcript, Codex rollout, OpenCode DB read-only), `doctor` drift report | `rekora-cli` (`hook.rs`, `adapters.rs`, `main.rs`) | Host formats are adapter concerns |
| `git diff --name-only` per worktree | `rekora-cli` (`project.rs`) | Git is already invoked there as a plain subprocess |

No change to `rekora-code`.

## 4. Capability classes

| Class | Items | Guarantee |
|---|---|---|
| **Core** (any MCP host) | tasks, handoffs, context packs; later leases and usage queries | Durable, versioned, tested against real SQLite |
| **Platform-specific adapter** | `StopFailure` and `PreCompact` (Claude only), `SessionStart` by `source` (Claude, Codex), usage parsers | Only what each host documents; nothing elsewhere |
| **Advisory integration** | overlap warnings, duplicate-task suggestions, budget and loop warnings | Informs the agent; never blocks |
| **Enforced inside Rekora today** | `task_update` with `expected_version` rejects stale writes | Real, tested. **Not** task ownership: `handoff_resume` claims with no version check (last writer wins, `mcp.rs` `handoff_resume`, `main.rs` handoff resume) |
| **Enforceable inside Rekora after H1** | one owner per task: claim succeeds only if unowned, lease expired, or same owner, at the read version | Needs H1 (migration) |
| **Checkpoint-enforceable (opt-in only)** | deny next tool (`PreToolUse`), refuse next prompt (`UserPromptSubmit`), `continue:false` | Between model calls; overshoot of in-flight calls possible; never called a hard budget |
| **Not enforceable by Rekora** | hard spend caps, per-response token caps, cancelling in-flight requests, model choice | Needs the request path (provider limits, proxy). Documented, not built |
| **Optional daemon capabilities** | none | See section 7 |

## 5. Roadmap

Costs are rough engineering estimates for this codebase. "Benefit required" is what the benchmark must
show before the item is accepted, not a prediction.

### Essential (the MVP)

| # | Feature | Value | Implementation cost | Operational overhead | Benefit required |
|---|---|---|---|---|---|
| E1 | **Correct cut-off capture.** (1) Per-host hook event tables replace the single `HOOK_EVENTS` list (Claude has `StopFailure` and steerable `PreCompact`; Codex has neither) and only documented events are installed per host. (2) Claude `StopFailure` records the failure *separately* (error type, time, session) without overwriting the previous turn's message — its `last_assistant_message` is the API error string. Only limit-type errors (`rate_limit`, `billing_error`, and similar) mark work as interrupted; `max_output_tokens`, `invalid_request` do not. (3) Turns store `session_id` and the checkout (worktree) root; the briefing ignores turns older than a freshness bound and turns from the current session. (4) Untracked-only changes no longer trigger a briefing by themselves. (5) Retention bound on turn rows (replaces the "one row per host" invariant in `tasks.rs`); legacy `turn:{host}` rows are ignored once past the bound. (6) Upgrade path: `rekora doctor` reports a stale installed hook set; the user re-runs `rekora agent install claude-hooks|codex-hooks --confirm`, which rewrites user-wide `~/.claude/settings.json` / `~/.codex/hooks.json` with backup (rule 7). | The 2026-11-24 false briefing (month-old foreign turn, untracked folders) and the unrecorded failing turn on Claude | Small–medium; turns stay in `meta` or move to a small table (decide in design; a table needs migration v5) | One extra hook process per failed turn | B4: every scripted `StopFailure` and kill case reported as interrupted; zero stale, same-session or foreign turns briefed; untracked-only trees silent |
| E2 | **Compaction-aware briefing.** Claude `PreCompact` prints a **fixed Rekora template** naming only the active task id(s) and telling the summary to keep them and to call `handoff_resume` afterwards; it never echoes task or memory text (task `constraints` are agent-written free text with no sources — rule 6). Claude/Codex `SessionStart` with `source=compact` re-injects the active task's state (objective, constraints, decisions, remaining) through the existing data-labelled briefing, marked "agent-recorded, unverified"; verified memories cited by source are labelled verified. "Active task" = exactly one `in_progress` task bound to this session or, failing a binding, exactly one `in_progress` task on this branch; otherwise only the list of ids is shown. MCP calls do not carry the host session id, so the binding would come from a `PostToolUse` hook matched on Rekora's own `handoff_resume`/`task_update` tools, which sees both `session_id` and the task id (Claude and Codex both match MCP tools by name; to be verified on the real hosts). The `PreCompact` handler must finish well under the 10 s hook timeout because compaction waits for it. | Summaries can drop rules (one PP: ~1/3 of anti-cheating rules on research benchmarks; transfer to coding untested) | Small–medium: two hook paths reusing handoff rendering | Runs only at compaction; appended text | B3 (protocol §7 of BENCHMARK_DESIGN): lower violation rate of session-emergent task constraints than C1 *and* than "constraints in CLAUDE.md", no success loss |
| E3 | **Measurement harness:** B0 (Rekora overhead), B4 (recovery, scripted, no model spend), B3 pilot. | Nothing above can be accepted or rejected without it | Medium: scripts + fixtures | None at runtime | It is the measurement |

### High-value (after the MVP results)

| # | Feature | Value | Cost | Overhead | Benefit required |
|---|---|---|---|---|---|
| H1 | **Atomic claims with leases.** Columns `lease_owner`, `lease_expires_at` (migration), not patchable through `TaskPatch`. `handoff_resume --claim` succeeds only if unowned, expired or same owner, at the read version; every `task_update` by the owner renews; expired claims show as stale and are reclaimable. | Makes "one owner per task" true; recovers from crashed owners without permanent locks | Small–medium (migration + store logic) | None | B5: no double ownership across N OS processes; stale claim reclaimable after TTL |
| H2 | **Overlap warnings from real git.** `status`/`handoff_resume` list files changed in other worktrees (`git diff --name-only` vs merge base) that intersect this task's files. Warn, never block. | Detects colliding work without agent cooperation | Small–medium | One `git` call per worktree, only on those commands | B5: overlaps reported for seeded collisions, none for disjoint work |
| H3 | **Duplicate-task suggestion** at task creation: word-shingle Jaccard ≥ 0.8 over *open* tasks (a scan; no `tasks_fts` table unless open tasks exceed a few thousand, then a migration and `EXPLAIN QUERY PLAN`). Never merges. | Catches "two sessions open the same task" | Small | None | Precision ≥ 0.9 on a labelled set |
| H4 | **Keep Rekora cheap and deterministic:** measure tool-definition and instruction tokens (B0); trim if over budget; no timestamps or counters in `context`/briefing headers. | Rekora's overhead is paid in every session | Small | None | B0 thresholds met |
| H5 | **Usage ledger (measurement, not control).** On `Stop`/`StopFailure`, read the new part of the Claude transcript or Codex rollout, normalise to `{uncached, cache_read, cache_write, output, reasoning}`, dedupe by request id, store per session and (if bound) task; `rekora usage` report. Tokens are facts; cost is an estimate from a dated price table. | Lets users and benchmarks see cost per task | Medium: migration (`usage_event`), tolerant parsers (Claude format undocumented → `NOT PARSED`, never guess) | Bounded incremental read at end of turn | B1: per-response token counts match captured API usage exactly after dedupe on ≥ 20 API-key sessions per host; subscription sessions labelled "unreconciled" |

### Optional (only after the Essential results, and only if users ask)

| # | Feature | Why optional |
|---|---|---|
| O1 | Advisory budget guard with SQLite leases (warn via `additionalContext`; opt-in `PreToolUse` deny / `UserPromptSubmit` block) | Needs H5; no evidence of demand; weaker than provider limits or LiteLLM; never a hard budget |
| O2 | Mirror native tasks (`TaskCreated`/`TaskCompleted`/`SubagentStop`) as external references | Agent teams are experimental; only if users run them |
| O3 | Event-level `PostToolUse` log (matcher-limited, no `git` in that path) | Only if B4 shows a recovery gain over checkpoint + git status |
| O4 | Advisory loop detector (OpenHands thresholds) | Hosts have continuation caps; repetition also appears in successful runs |
| O5 | More host adapters: OpenCode V2 plugin shim, Antigravity, Gemini CLI | Each needs a real-host test (rule 2); after E1/E2 prove value |
| O6 | OTel ingestion; provider admin-API reconciliation | Needs user-set env vars, collectors or admin keys; aggregates only |

### Not recommended

| Idea | Reason |
|---|---|
| Rekora-owned summariser, trajectory reducer or compaction engine | Needs an LLM call per event or ownership of the request; rewrites the cached prefix; hosts already do it |
| API proxy for compaction, routing or hard budgets | A new product in the request path; can strip `cache_control` and bill everything uncached; LiteLLM exists |
| Semantic answer cache | Unsafe for paths, identifiers, versions, negation and changing repositories (vCache, PR) |
| Model routing | Rekora does not own requests; each switch rebuilds the cache |
| Orchestrator, mailbox, agent spawning, A2A transport | Hosts own this (Claude teams, Teamwork, Codex subagents); MAST and Cognition describe it as the fragile part |
| Mandatory daemon | Measured saving ~20 ms per call; lifecycle and security cost on every machine |
| Embeddings or a vector store by default | Not justified until a non-author corpus shows paraphrase misses (ADR-0003) |
| LLM in the memory write path (Mem0/A-MEM style) | Non-deterministic, costs tokens, conflicts with "agent text never self-verifies" |
| Echoing stored text into compaction instructions | Stored task text is agent-written data; instructions must be fixed templates (rule 6) |
| Auto-editing CLAUDE.md / AGENTS.md | Rule 7; instruction files are the user's |
| Storing full transcripts | Hosts already keep them; store pointers |

## 6. Smallest useful MVP

**E1 + E2 + E3.** Per-host hook event tables; `StopFailure`, `PreCompact` (fixed template) and
`SessionStart` by `source`; session-scoped, fresh, bounded turns; quiet on untracked-only trees;
`doctor` drift report with explicit re-install; compaction re-injection of the active task as data.
Validated by B0, B4 and a B3 pilot. No new MCP tool. Possibly one small migration if turns move out of
`meta`. Before installing new events, verify with the real hosts that an unknown event in their hooks
file is ignored rather than rejected (Codex in particular) — otherwise install only the documented
subset per host version.

## 7. Embedded, daemon or both?

**Embedded only.** Measured (2026-11-24, Windows 11): process start 22 ms p50; the 138 ms
`hook session-start` is dominated by `git` subprocesses, which a daemon would still run. SQLite WAL with
`BEGIN IMMEDIATE` already handles concurrent agents (12-process test). A daemon would add start/stop,
crash, upgrade, socket/port security and stale-state problems on every machine for ~20 ms. The existing
optional HTTP MCP mode remains an opt-in deployment for containers. Revisit only if a per-tool-call hook
is adopted and B0 shows its p95 above 50 ms after removing `git` from that path.

## 8. Evidence required before expanding beyond the MVP

| To expand into | Required evidence |
|---|---|
| H1–H3 coordination | B5 passes; one real multi-worktree session where overlap was reported correctly |
| H5 usage ledger | B1 as defined in H5 (exact match after dedupe on ≥ 20 API-key sessions per host); parser fixture per observed host version |
| O1 budget guard | H5 in use; a documented user request; B2 shows the guard does not reduce success |
| O3 event log | B4 shows a recovery gain over checkpoint + git status |
| Any additional host adapter | Real-host test executed (rule 2), not contract-only |
| Any efficiency claim in the README | B2 or B3 result with intervals, cached/uncached separated, model and host versions pinned |

## 9. Risks

- Undocumented Claude transcript format → tolerant parser, `NOT PARSED` records, one fixture per version.
- Hosts change hook semantics monthly (Codex hooks became default-on; Claude added `StopFailure`) →
  per-host event tables and `doctor` drift reporting.
- Unknown events in a host's hooks file might be rejected (NOT VERIFIED for Codex) → verify on the real
  host before shipping.
- `PreCompact` output is model input → fixed, bounded template only; never stored text.
- Session→task binding may be ambiguous → when ambiguous, inject only ids.
- Leases and clock skew → one machine per project database; cross-machine use out of scope.
