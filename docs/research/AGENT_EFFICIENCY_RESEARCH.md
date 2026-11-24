# Agent efficiency, context engineering and governance — research report

Date: 2026-11-24. **Research only: no source code, dependency, crate or schema was changed.**
Status of every proposal: awaiting architectural approval.

Purpose: find the smallest, most reliable and most cost-effective change that makes existing coding
agents better when they use Rekora — and what Rekora should explicitly not build.

| File | Contents |
|---|---|
| [CONTEXT_ENGINEERING_FINDINGS.md](CONTEXT_ENGINEERING_FINDINGS.md) | Prefix caching, trajectory reduction, CompactionBench/APEC, context rot, tool schemas (Q1–Q5) |
| [TOKEN_ECONOMICS.md](TOKEN_ECONOMICS.md) | Usage fields, budgets, rate limits, reservations, semantic caching, routing, loops (Q6, Q7, Q9, Q13, Q14) |
| [PLATFORM_CAPABILITY_MATRIX.md](PLATFORM_CAPABILITY_MATRIX.md) | What each host can observe/block/inject; persistence, recovery, daemon, orchestration (Q8, Q11, Q12, Q15, Q16) |
| [COMPETITIVE_ANALYSIS.md](COMPETITIVE_ANALYSIS.md) | 15-system comparison matrix; what exists vs what is new (Q10 context) |
| [BENCHMARK_DESIGN.md](BENCHMARK_DESIGN.md) | Suites B0–B5, configurations C0–C7, metrics and controls |
| [ARCHITECTURE_RECOMMENDATION.md](ARCHITECTURE_RECOMMENDATION.md) | Roadmap (Essential / High-value / Optional / Not recommended), MVP, evidence gates |

## 1. Method

- **Architecture audit** (done by the coordinating agent): read README/README.es, AGENTS.md, CLAUDE.md,
  STATUS.md, ADRs, architecture docs, every crate's source, the schema and migrations, MCP tools, CLI
  commands, hooks, adapters, benchmarks and tests; inspected the installed binary, host registrations
  (`rekora doctor`) and this repository's own store; measured hook latency.
- **External research** by four independent researcher agents with disjoint scopes (context engineering;
  token economics and governance; agent runtimes and platforms; memory and multi-agent coordination),
  each producing a structured report with evidence classes and a sources table. Overlap was limited by
  scope; a few sources (MAST, LangGraph, Claude hooks) were consulted by two researchers and agree.
- **Independent critical review** of the consolidated drafts by a separate reviewer agent instructed to
  challenge unsupported claims and unnecessary features. Its 17 findings and what changed are recorded
  in [DECISION_LOG.md](DECISION_LOG.md) (H10–H17); see section 7.
- The Rekora MCP was consulted: this repository's store holds 0 memories and 0 tasks, so it contributed
  only the hook observation in section 2.3.

Evidence classes used throughout: **PR** peer-reviewed · **PP** preprint · **DOC** official
documentation · **OSS** open-source source code · **VR** vendor-reported · **CI** community ·
**HYP** our inference, untested. Preprint and vendor numbers are reported as their authors' claims.
**No external result was reproduced.** The only measurements executed in this investigation are the
latency figures in PLATFORM_CAPABILITY_MATRIX.md §6.

## 2. Inventory of existing capabilities

### 2.1 What Rekora is today (v0.1.0, commit 69f43bb)

| Area | Capability | Implementation |
|---|---|---|
| Storage | One SQLite database per project scope, WAL, `BEGIN IMMEDIATE`, migrations v1–v4 via `user_version` with `VACUUM INTO` snapshots, refusal of newer schemas, append-only audit events | `rekora-store/src/schema.rs`, `lib.rs` |
| Memory | Kinds (fact, decision, constraint, preference, known issue, solution, procedure), status (candidate/verified/disputed/superseded), origin; fingerprint dedupe (exact); redaction before persistence | `rekora-core/src/memory.rs`, `redact.rs` |
| Provenance & freshness | Source paths bounded to the project root; BLAKE3 hashes → `possibly_stale`; branch recorded, `branch_mismatch` flag | `source.rs`, `search.rs` |
| Retrieval | FTS5 BM25 with porter stemming, EN/ES stopwords, coverage floor and `no_evidence`, explained score, deterministic tie-break | `search.rs`, `query.rs` |
| Context budgeting | Whole-item packing under a conservative estimate (bytes/3), "data, not instructions" header, multi-query packs for handoffs | `budget.rs`, `context.rs` |
| Tasks & handoffs | Versioned tasks (constraints, decisions, files, tests, failures, remaining), optimistic concurrency on `task_update`, `rekora.handoff/1` documents; `handoff_resume` sets the owner without a version check (last writer wins) | `tasks.rs`, `task.rs` |
| Lifecycle hooks | `rekora hook session-start` (briefing: ≤ 5 unfinished tasks, last turn, ≤ 20 uncommitted paths) and `rekora hook stop` (last turn per host in `meta`); never fail the host | `hook.rs` |
| MCP | 9 tools (`search`, `context`, `remember`, `status`, `task_list`, `task_update`, `handoff_create`, `handoff_resume`, `symbols`) with read-only/destructive annotations; stdio and optional authenticated HTTP; protocol 2025-11-25 and 2026-07-28 tested | `mcp.rs`, `mcp_http.rs` |
| Adapters | Claude Code (`.mcp.json`), Codex (`config.toml`, user-wide), OpenCode 1.x, DeepSeek Harness, Claude and Codex hooks (user-wide); dry-run, backup, byte-identical re-install, owned-entry uninstall | `adapters.rs` |
| Code index | tree-sitter declarations (Rust, Python, JS, TS/TSX, C#), per-branch snapshots, incremental by content hash | `rekora-code`, `code.rs` |
| Transfer | `rekora.export/1`, idempotent import as untrusted candidates | `transfer.rs` |
| Benchmarks | `benchmark run --suite core` (latency at 1k–100k) and `--suite relevance` (P@1 0.933, R@5 1.0, MRR 0.967, specificity 1.0 on a 40-item author-written corpus) | `bench.rs` |
| Tests | 91 passing at last STATUS (unit, real SQLite, 12-process concurrency, E2E with real binary/git/MCP child, HTTP, real Claude Code and Codex hosts opt-in) | `crates/*/tests` |

### 2.2 Capabilities the brief asks about that already exist under other names

- *Checkpoints* → versioned tasks + `task_update`. *Recovery* → `handoff_resume` + SessionStart briefing.
- *Context freshness and relevance* → `possibly_stale`, `branch_mismatch`, coverage floor, `no_evidence`.
- *Token budgeting* → context packs (for Rekora's own output only).
- *Deterministic tool-result caching* → incremental, content-hashed code index and `symbols`.
- *Agent task isolation* → one store per project shared by worktrees, branch-tagged records.
- *Governance of agent output* → candidates never self-verify; only the CLI verifies.

### 2.3 What does not exist

No measurement of tokens or cost; no budget of any kind; no compaction integration; no task leases or
overlap detection; no loop detection; hooks only for `SessionStart` and `Stop`.

Two defects found during the audit (both explained in PLATFORM_CAPABILITY_MATRIX.md §3):

1. **Claude Code fires `StopFailure`, not `Stop`, when a turn ends on an API error such as `rate_limit`
   or `billing_error`** (DOC). Previous turns, tasks and live `git status` still reach the next briefing,
   but the failing turn and its error are not recorded, so the briefing cannot say the session was cut
   off by a limit.
2. **Turns are not session-scoped and have no freshness bound.** This investigation's own session was
   briefed with a turn recorded on 2026-10-29 by a Claude session working on a different project, whose
   `Stop` hook fired with its `cwd` inside this checkout. The briefing was triggered only by two untracked
   tool folders; nothing was unfinished. (The same record is also the first observed evidence that a live
   Claude Code session fires Rekora's `Stop` hook, which STATUS.md listed as NOT EXECUTED.)
3. **Task ownership is not enforced.** `handoff_resume` (MCP and CLI) claims with no expected version,
   so a second agent silently takes over a claimed task.

## 3. Answers to the critical questions

| # | Question | Answer (short) | Evidence strength | Detail |
|---|---|---|---|---|
| 1 | Is compression cheaper under prefix caching? | Usually cheaper in total, by far less than the token cut (AgentDiet: −40–60 % tokens → −21–36 % cost); can be more expensive when it rewrites the prefix often (+24.9 % measured in one case). Under caching, turn count explains 84 % of log-cost variance in one preprint (TurnCost, PP). | One PR + several PP; mechanism DOC | Context §7 |
| 2 | When to compact? | At task boundaries, while the cache is warm, in batches with a minimum saving, before quality degrades; thresholds are model-specific (50–90 % seen in production). A host decision. | DOC + PP | Context §7 |
| 3 | Async progressive compaction? | Plausibly better cost (append-only part) and latency (async part); one preprint, 8 tasks × 5 trials, no code. Host/provider capability, not Rekora's. | PP | Context §3 |
| 4 | What can be removed? | Old acted-upon tool outputs, duplicates, superseded results, verbose logs — ideally restorably. Never objectives, constraints, process rules, decisions, artifact trail, breadcrumbs. | PR + PP + VR | Context §7 |
| 5 | Reduce tool schemas / repeated instructions? | Deferred tool loading, stable tool set, short purposeful descriptions, stable instructions in the cached prefix, dynamic state appended; re-inject constraints after compaction from a durable source. | DOC + VR + PP | Context §6–7 |
| 6 | Measure cached/uncached tokens? | Read host files: Claude transcript `message.usage` (undocumented format), Codex `token_count`/`PostModelRequest`, OpenCode SQLite; admin APIs for reconciliation. Store tokens; cost is an estimate. | DOC + OSS + observed | Tokens §2 |
| 7 | Enforce budgets without owning the request? | Only at hook checkpoints between model calls, with overshoot; a hard cap needs the request path (provider limits, proxy). Never block `Stop` for budgets. | DOC | Tokens §3 |
| 8 | What can each platform intercept? | All: session start injection + tool observation. Only Claude: compaction steering and API-error stop. None: hard-crash hook. Usage data mostly outside hooks. | DOC | Platform §2 |
| 9 | Concurrent budget sharing? | Reserve → execute → reconcile with expiring leases in one transaction (LiteLLM pattern); estimates make it advisory. Not needed until measurement shows demand. | DOC + OSS + HYP | Tokens §5 |
| 10 | Duplicate work without blocking? | Hard-block only double claims of the same task (needs H1: today claims are last-writer-wins); expiring leases; overlap from real `git diff` across worktrees as warnings; duplicate-task suggestions; no file locks, no LLM judgement. | OSS (Beads, Agent Mail) + DOC + HYP | Architecture H1–H3 |
| 11 | Persistence granularity? | Step boundaries: checkpoint per verified step, handoff per session end/compaction/failure; event-level log only if B4 proves it helps. Pointers, not transcripts; code state in git. | DOC (durable execution) + HYP | Platform §4 |
| 12 | Recovery after crash? | Detect unclosed tasks at session start; rebuild from checkpoint + git; re-verify, never re-run, the in-flight step; inject a resume pack; new task version. | DOC + HYP | Platform §5 |
| 13 | When is semantic caching unsafe? | Paths/identifiers/versions/negation, changing repos, cross-scope, side effects, long-running caches, agent loops (vCache: correct and wrong hits have near-identical similarity). Only content-hash caching is safe. | PR | Tokens §6 |
| 14 | Model routing benefits/overheads? | Real on benchmark-like single-turn tasks with in-domain data; in coding sessions each switch rebuilds the cache and errors compound; the only coding-agent analysis found is a break-even derivation in TurnCost (PP, shares an author with the Prefix Stability poster). Not Rekora's to do. | PR + DOC | Tokens §7 |
| 15 | Daemon vs embedded? | Embedded. Measured: process start 22 ms p50; hook latency is `git`-bound (138 ms p50); a daemon saves ~20 ms and adds lifecycle risk. | **EXECUTED** (local) | Platform §6 |
| 16 | Integrate with native Teamwork? | Be the durable, host-neutral ledger beneath native orchestration: no spawning, mailboxes or A2A; mirror native ids read-only; fill the documented resume gap; same MCP read path for every agent; writes stay candidates. | DOC + HYP | Platform §7 |

## 4. Implementation roadmap (summary)

Full table with value, cost, overhead and required benefit: ARCHITECTURE_RECOMMENDATION.md §5.

- **Essential (MVP):** E1 correct cut-off capture (per-host hook tables, `StopFailure`, session-scoped
  fresh bounded turns, quiet on untracked-only trees, `doctor` drift report); E2 compaction-aware
  briefing (fixed `PreCompact` template, `SessionStart source=compact` re-injection of the active task as
  data); E3 measurement harness (B0, B4, B3 pilot).
- **High-value:** H1 atomic claims with leases; H2 overlap warnings from git across worktrees; H3
  duplicate-task suggestions; H4 keep Rekora's own tool definitions and output cheap and deterministic;
  H5 usage ledger from host files (measurement only, gated on B1).
- **Optional:** advisory budget guard; native task mirroring; event-level tool log; loop detector; more
  host adapters (OpenCode V2, Antigravity, Gemini CLI); OTel / admin-API ingestion.
- **Not recommended:** summariser or trajectory reducer; proxy; semantic cache; model routing;
  orchestrator/mailbox/A2A; mandatory daemon; default embeddings; LLM write path; auto-editing
  instruction files; storing transcripts.

## 5. Final decision gate

**Should the functionality live inside Rekora?**
Partly. Cut-off capture, compaction-aware briefings, task claims and (later) usage measurement per task
belong in Rekora because they depend on its durable per-project state. Compression, summarisation,
caching of model calls, routing, hard budgets and orchestration do not: they need the request path or
duplicate what hosts already do.

**Which capabilities already exist?**
Durable versioned tasks and handoffs, cut-off briefing (incomplete), budgeted context packs, freshness
and abstention, content-hash code index, optimistic concurrency across processes, per-project isolation
shared by worktrees, candidate-only agent writes, adapters for four hosts. See §2.

**Which capabilities must be implemented?**
E1, E2 and the harness E3 (the MVP). H1–H5 follow only if the MVP measurements justify them; everything else waits for evidence.

**Which external projects should be reused or studied?**
Study: Beads (atomic claims), MCP Agent Mail (advisory TTL leases), OpenHands StuckDetector, LangGraph
checkpoint semantics, DBOS/Restate recovery rules, APEC's append-only design, AgentDiet's removal
categories, the Compaction-Friendly constraint-survival method. Reuse externally rather than rebuild:
LiteLLM or provider workspace limits for hard budgets. Check licences (Kindex BSL 1.1, claude-mem AGPL)
before borrowing anything beyond ideas.

**Which ideas are not worth implementing?**
A Rekora summariser/compactor, a proxy, a semantic cache, model routing, an orchestrator or mailbox,
A2A, a mandatory daemon, default embeddings, LLM-written memories, automatic edits of CLAUDE.md/AGENTS.md.

**Which controls can be enforced reliably?**
Today, inside Rekora's store: rejection of stale `task_update` writes (`expected_version`). After H1:
one owner per task (claim only if unowned, expired or same owner, at the read version) — today a claim
is last-writer-wins. At host checkpoints (opt-in, with overshoot): denying the next tool call, refusing
the next prompt, `continue:false`. Nothing else — in particular no hard spend cap.

**Embedded, daemon or both?**
Embedded. The optional HTTP mode remains for containers; no daemon is required or proposed.

**Smallest useful MVP?**
E1 + E2 + E3: per-host hook event tables; `StopFailure`, fixed-template `PreCompact` and `SessionStart`
by source; session-scoped, fresh, bounded turns; no briefing for untracked-only trees; `doctor` drift
report with explicit re-install of the user-wide hook files; re-injection of the active task as data
after compaction; the B0/B4 harness and a B3 pilot. No new MCP tool; at most one small migration.

**What evidence is required before expanding it?**
B4: every scripted cut-off reported as interrupted, zero stale, same-session or foreign briefings. B3:
after a pilot sizes the sample, a lower violation rate for session-emergent task constraints than both
the no-integration baseline and "constraints in CLAUDE.md", with no success loss. B1 (before H5 ships):
per-response tokens match captured API usage exactly after dedupe. Any README efficiency claim needs a
B2/B3 result with intervals, cached and uncached separated, and pinned model and host versions.

## 6. Limitations of this investigation

- No external benchmark or system was run; all external numbers are as reported by their sources.
- Several 2026 sources were read at abstract level only (marked in each file). CompactionBench has no
  released code. Hosts change their hook surfaces monthly; event lists are as documented on 2026-11-24.
- DeepSeek Harness integration details, Codex default compaction threshold, Codex behaviour on usage
  limits, and Gemini cache discounts are NOT VERIFIED.
- The relevance corpus behind Rekora's own retrieval numbers was written by the same author as the
  ranking code (known risk, STATUS.md).

## 7. Independent review

An independent reviewer agent read all seven drafts against the four research reports and the codebase
and returned 17 findings (8 must-fix, 8 should-fix, 1 group of nits). All were applied. The main
corrections: task claims are not atomic today (last writer wins); `PreCompact` must use a fixed template
because task text is agent-written; the cut-off defect was restated precisely; the MVP now includes the
harness and the per-host hook tables; the usage ledger moved to High-value behind a realistic B1; B3 gained
a "constraints in CLAUDE.md" control and a pilot; B2 is sized and capped. The pre-registered hypotheses
and the review record are in [DECISION_LOG.md](DECISION_LOG.md) (H10–H17).
