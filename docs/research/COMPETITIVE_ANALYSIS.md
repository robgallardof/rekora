# Competitive and architectural comparison (agent efficiency)

Research date: 2026-11-24. Extends [COMPETITIVE_MATRIX.md](COMPETITIVE_MATRIX.md) (memory, 2026-10-26)
to the efficiency, governance and coordination capabilities examined in
[AGENT_EFFICIENCY_RESEARCH.md](AGENT_EFFICIENCY_RESEARCH.md). No system below was installed or run:
every cell comes from the project's own documentation or source (**NOT EXECUTED**). "n.d." means not
documented in the sources reviewed.

Evidence classes: **PR** peer-reviewed · **PP** preprint · **DOC** documentation · **OSS** source ·
**VR** vendor-reported · **CI** community.

## 1. Matrix

| System | Implementation approach | Storage model | Context lifecycle | Checkpoint strategy | Token measurement | Cost enforcement | Caching | Concurrency | Failure recovery | Extensibility | Integration requirements | Resource consumption | Main limitations |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Rekora** (this repo, v0.1.0) | Local memory + tasks/handoffs for coding agents; MCP + CLI + 2 lifecycle hooks | SQLite + FTS5 per project (`.rekora/`), WAL, versioned migrations | Token-budgeted context packs on demand; SessionStart briefing | Versioned tasks (agent-driven `task_update`); last turn per host on `Stop` | **None** (conservative byte estimate for its own packs only) | None | Content-hash (BLAKE3) incremental code index; no answer cache | Optimistic versions on updates (claims last-writer-wins); `BEGIN IMMEDIATE`; 12-process test | Briefing + `handoff_resume` (failing turn on `StopFailure` not recorded) | Adapters per host | One binary; hooks/MCP registered per host | ~22 ms process start, hooks 140 ms p50 (git-bound) | No usage data; no compaction integration; turns not session-scoped |
| **Claude Code (native)** | Agent host | JSONL transcripts; auto memory dir; team task files | Auto/manual compaction, tool search, deferred tools, context editing | `/resume`, `/rewind` (host-local) | Statusline, OTel, transcripts (estimates) | Gateway spend limits (`rate_limits.spend_limit`); SDK `max_budget_usd` | Prompt caching managed by host | Agent teams (file-locked task claims), worktrees | Resume per host; teammates not restored | 33 hook events, plugins, MCP | — | — | Host-local state; no cross-host continuity; no verified provenance |
| **Codex CLI (native)** | Agent host | Rollout JSONL, `~/.codex` | Auto compaction (`model_auto_compact_token_limit`) | `codex resume` | `token_count` events, `PostModelRequest`, OTel | n.d. | Provider prompt caching | Subagents | Resume per host | 14 hook events, MCP, plugins | — | — | Compaction not steerable by hooks |
| **LiteLLM proxy** | Request-path gateway | Postgres (+ Redis counters) | n.a. | n.a. | Per request from provider usage + cost map | **Hard**: key/user/team/session budgets, reservation by default | Optional response/semantic caching | Reservation leases; Redis for multi-instance | Fails open without DB unless configured | Plugins, callbacks | Every host must point at the proxy (`ANTHROPIC_BASE_URL`…) | A server + DB | Must be in the request path; can break `cache_control` if misconfigured |
| **Letta / MemFS** | Stateful agent server; Letta Code memory as git-backed Markdown | Server DB + vectors; MemFS git repo | `system/` always loaded; sleep-time reflection | Git commit per memory change | n.d. | n.d. | n.d. | Memory subagents in git worktrees | Git history | Custom tools | Letta API / local backend | LLM calls for reflection/defrag | LLM write path; conflict handling unspecified |
| **Mem0** | LLM extract → LLM ADD/UPDATE/DELETE/NOOP | Vector store (+ Neo4j) | Top-k memories per query | n.d. | Reports ~7k tokens/conversation | n.d. | n.d. | n.d. | n.d. | Pluggable providers | LLM + embeddings + vector DB | 2 LLM phases per write | Own paper: full context beats it on accuracy (72.9 vs 66.9 %) |
| **Graphiti / Zep** | LLM-built bi-temporal KG | Neo4j / FalkorDB / Neptune | Hybrid graph search | Edges invalidated, not deleted | n.d. | n.d. | n.d. | Async ingestion | n.d. | Entity types | Graph DB + LLM + embeddings | Heavy | Ingestion delay; self-evaluated |
| **Kindex** (v0.54.2) | Local knowledge graph + hooks for many hosts | SQLite + FTS5 (+ sqlite-vec) | 5 token tiers; quarantined pre-compact candidates | n.d. | Tier budgets | n.d. | n.d. | Advisory locks, collab threads | n.d. | Many host adapters | Python | Light | License now BSL 1.1 per README |
| **claude-mem** | Hooks → localhost worker → LLM compression | SQLite (+ Chroma) | SessionStart injection | n.d. | n.d. | n.d. | n.d. | Single worker queue | Queue in SQLite | Multi-provider | Node worker on a port | LLM call per observation batch | AGPL; LLM write path; Claude-centric |
| **Basic Memory** | Markdown knowledge graph + MCP | Markdown files + SQLite FTS5 index | `build_context` traversal | Files (git-able) | n.d. | n.d. | Index | WAL + watcher | Re-index from files | Obsidian-compatible | Python MCP server | Light | Not code-aware |
| **Serena** | LSP code tools + memory files | `.serena/memories/*.md` | On-demand reads | Files | n.d. | `max_answer_chars` caps | n.d. | n.d. | Re-onboard | Language servers | Python + LSPs | LSP processes | Memories unindexed, unverified |
| **MCP Agent Mail** | Mail-style coordination server | Git artifacts + SQLite FTS5 | Inboxes, threads | Git commits | n.d. | n.d. | n.d. | **Advisory TTL file leases; conflicts returned with the grant** | Lease expiry | Beads integration | HTTP MCP server; optional pre-commit guard | ~10 MB/project (author) | Advisory only |
| **Beads** | Git-backed issue tracker for agents | Dolt (versioned SQL) | `bd ready` queue; compaction of closed issues | Dolt commits | n.d. | n.d. | n.d. | **Atomic `--claim`**, hash ids | Dolt history | Formulas, refs | `bd` CLI + Dolt | Light | Embedded mode single writer |
| **LangGraph** | Framework persistence | Checkpointer (SQLite/Postgres) + store | Thread state | **Checkpoint per super-step, pending writes kept** | n.d. | n.d. | n.d. | Per thread | Resume from checkpoint (node restarts) | Pluggable savers | Python/JS app | Unbounded growth unless pruned | Framework lock-in |
| **OpenHands** | Agent platform + SDK | Append-only event files + base state | LLM summarising condenser; stuck detector | Event log | Token-aware condenser | n.d. | n.d. | n.d. | Resume from events; in-flight action undocumented | SDK | Runtime/sandbox | LLM summarisation calls | Lossy summaries; no verified memory |

## 2. What Rekora already provides (do not rebuild)

| Capability asked about in the brief | Already in Rekora as | Where |
|---|---|---|
| Durable task state / checkpoints | Versioned tasks with constraints, decisions, files, tests, failures, remaining | `rekora-store/src/tasks.rs`, MCP `task_update` |
| Handoff / recovery document | `rekora.handoff/1`, `handoff_create`, `handoff_resume` (sets the owner with no version check: last writer wins) | `tasks.rs`, `mcp.rs` |
| Automatic cut-off capture | `Stop` hook records last turn; `SessionStart` briefing | `rekora-cli/src/hook.rs` |
| Context budget | Whole-item packing under a conservative estimate, deterministic order | `rekora-core/src/budget.rs`, `rekora-store/src/context.rs` |
| Relevance / freshness | BM25 + coverage floor + `no_evidence`; BLAKE3 source hashes → `possibly_stale`; branch mismatch | `search.rs`, `source.rs` |
| Deterministic caching | Incremental code index keyed by content hash and branch snapshot | `rekora-code`, `code.rs` |
| Concurrency control | Optimistic versions on `task_update` and memory status changes; `BEGIN IMMEDIATE`; tested with 12 OS processes | `process_concurrency.rs` |
| Agent isolation | One database per project; worktrees share a store; branch recorded per memory | `project.rs` |
| Governance of agent text | Agent writes are `candidate`; only the CLI verifies; redaction before persistence | `memory.rs`, `redact.rs` |
| Multi-host integration | MCP adapters for 4 hosts, hook adapters for 2, dry-run/backup/uninstall | `adapters.rs` |

## 3. What would be genuinely new (and whether it is worth it)

| Candidate | Exists elsewhere? | Genuine innovation for Rekora? | Verdict |
|---|---|---|---|
| Constraint-preserving compaction (PreCompact steering + post-compact re-injection from a verified store) | Kindex extracts pre-compact candidates; none of the 15 systems reviewed re-injects a durable task's constraints at the compaction boundary across hosts | **Yes** for session-emergent constraints (CLAUDE.md already survives compaction) | Essential (E2), gated by B3 |
| Cross-host cut-off recovery that catches `StopFailure` and API-error stops | Hosts resume only their own sessions | **Yes** | Essential |
| Provider-neutral usage ledger built from host files (cached vs uncached), per task | OTel collectors, LiteLLM (in-path), host status lines | Partly: the *per task / per handoff* join is new; the measurement itself is not | High-value (H5), gated on B1 |
| Atomic task claims as expiring leases + overlap warnings from real `git diff` across worktrees | Beads (claims), Agent Mail (leases), Claude teams (locks, session-scoped) | Partly: durable, cross-host, git-observed overlap is new. Rekora's claims are last-writer-wins today | High-value (H1, H2) |
| Advisory budget guard with SQLite leases | LiteLLM (hard, in-path) | No — weaker copy of LiteLLM | Optional |
| Trajectory reduction / summarisation / compaction engine | AgentDiet, OpenHands, every host | No, and requires LLM calls or a proxy | Not recommended |
| Semantic cache, model router | GPTCache, LangCache, RouteLLM, OpenRouter | No, and unsafe/irrelevant for a non-proxy | Not recommended |
| Orchestrator, mailbox, agent spawning | Claude teams, Teamwork, Agent Mail | No | Not recommended |

## 4. Reuse or study

- **Study, borrow ideas (no code):** Beads (atomic claim, `ready` = open with no open blockers,
  `discovered-from` provenance); MCP Agent Mail (advisory TTL leases on repo-relative globs, conflicts
  returned with the grant, opt-in pre-commit guard); OpenHands StuckDetector thresholds; LangGraph's
  super-step + pending-writes semantics; DBOS/Restate "persist step result, re-verify in-flight step";
  APEC's append-only summary idea as the shape of Rekora's versioned handoffs; Graphiti's
  invalidate-don't-delete (Rekora's `supersede` already does this).
- **Reuse as external tools, not dependencies:** LiteLLM proxy or provider workspace limits for anyone
  who needs a hard budget — document, do not configure.
- **Check licenses before borrowing anything concrete:** Kindex's README now says BSL 1.1; claude-mem is
  AGPL-3.0.
- **Borrow benchmark categories, not leaderboards:** LongMemEval/MemoryAgentBench categories (knowledge
  update, abstention, conflict resolution) and the Compaction-Friendly constraint-survival measurement.
  Do not quote LoCoMo: cross-vendor numbers are disputed and not comparable.

## 5. Evidence about memory itself

- No peer-reviewed study shows that persistent memory improves coding-agent task success. SWE-Bench-CL
  (PP) proposes a protocol but reports no results; the codebase-memory preprint reports 83 % vs 92 %
  answer quality at ~10× fewer tokens (author-evaluated).
- Mem0's own paper shows full context beating every memory system on accuracy; its gains are latency
  and tokens.
- Implication: Rekora must not claim efficiency or success gains it has not measured. The benchmark in
  BENCHMARK_DESIGN.md is the prerequisite for any such claim.

Sources for every row: see the source tables in the other research files, plus the existing
[COMPETITIVE_MATRIX.md](COMPETITIVE_MATRIX.md). Additional sources used only here:

| Title | URL | Date | Class |
|---|---|---|---|
| Letta Context Repositories (MemFS) | https://www.letta.com/blog/context-repositories | 2026-02-12 | VR |
| Letta Code MemFS docs | https://docs.letta.com/letta-code/memfs | accessed 2026-11-24 | DOC |
| Letta: Is a filesystem all you need? | https://www.letta.com/blog/benchmarking-ai-agent-memory | 2025-08-12 | VR |
| Mem0 paper | https://arxiv.org/abs/2504.19413 | 2025-04-28 | PP (vendor-authored) |
| Zep: Is Mem0 really SOTA? | https://www.getzep.com/blog/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory/ | 2025-05-06 | VR |
| Zep / Graphiti paper | https://arxiv.org/abs/2501.13956 | 2025-01-20 | PP (vendor-authored) |
| Kindex | https://github.com/wandercom/kindex | accessed 2026-11-24 | OSS |
| claude-mem | https://docs.claude-mem.ai/architecture/overview | accessed 2026-11-24 | OSS |
| Basic Memory | https://github.com/basicmachines-co/basic-memory | accessed 2026-11-24 | OSS |
| Serena memories | https://oraios.github.io/serena/02-usage/045_memories.html | accessed 2026-11-24 | DOC |
| MCP Agent Mail | https://github.com/Dicklesworthstone/mcp_agent_mail | accessed 2026-11-24 | OSS |
| Beads | https://github.com/steveyegge/beads | accessed 2026-11-24 | OSS |
| codebase-memory | https://arxiv.org/abs/2603.27277 | 2026-03-28 | PP |
| SWE-Bench-CL | https://arxiv.org/abs/2507.00014 | 2025-06 | PP |
| LongMemEval | https://arxiv.org/abs/2410.10813 | ICLR 2025 | PR |
| MemoryAgentBench | https://arxiv.org/abs/2507.05257 | ICLR 2026 | PR |
| Anthropic multi-agent research system | https://www.anthropic.com/engineering/multi-agent-research-system | 2025-06-13 | VR |
| Cognition: Don't build multi-agents | https://cognition.com/blog/dont-build-multi-agents | 2025-06-12 | VR |
| MetaGPT | https://arxiv.org/abs/2308.00352 | ICLR 2024 | PR |
