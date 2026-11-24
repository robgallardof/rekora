# Platform capability matrix

Research date: 2026-11-24. Research only. What each agent host lets an external tool like Rekora
observe, block, inject and measure, and what that implies for persistence and recovery.
Part of [AGENT_EFFICIENCY_RESEARCH.md](AGENT_EFFICIENCY_RESEARCH.md).

Evidence classes as in the other files (**DOC** official docs, **OSS** source code, **CI** community,
**HYP** our inference). Event names are quoted from the hosts' docs; anything not confirmed there is
**NOT VERIFIED**. Hosts change monthly: re-check before implementing.

## 1. What Rekora uses today

`crates/rekora-cli/src/adapters.rs` installs exactly two hook events for Claude Code and Codex:
`HOOK_EVENTS = [("SessionStart", "session-start"), ("Stop", "stop")]`. MCP is registered for Claude Code
(project `.mcp.json`, or user scope by the user's own `claude mcp add`), Codex (user `config.toml`),
OpenCode 1.x and DeepSeek Harness (project files). The installed state on the investigating machine
(`rekora doctor`, 2026-11-24): Codex MCP, Claude MCP (user scope), `claude-hooks` and `codex-hooks`
installed.

## 2. Q8 — what each platform can actually intercept

Legend: Y documented yes · N documented no · ? not documented / NOT VERIFIED.

| Platform | Event | Observe | Block | Inject context | Token usage | Notes |
|---|---|---|---|---|---|---|
| Claude Code | `SessionStart` (`startup`/`resume`/`clear`/`compact`/`fork`) | Y | N | Y (`additionalContext`, plain stdout) | partial (`context_tokens`, `prompt_cache_likely_expired` on resume/fork) | Rekora's briefing point; fires again after compaction with `source=compact` |
| Claude Code | `UserPromptSubmit` | Y | Y | Y | N | 30 s timeout |
| Claude Code | `PreToolUse` | Y | Y (`deny`) | Y | N | `updatedInput` rewrites args |
| Claude Code | `PostToolUse` / `PostToolUseFailure` / `PostToolBatch` | Y | feedback; `PostToolBatch` stops the loop | Y | Agent tool only | Natural event-level checkpoint after edits/tests |
| Claude Code | `PreCompact` (`manual`/`auto`) | Y | Y | **Y — stdout becomes instructions for the compaction summary** (≤ 10,000 chars) | N | Unique: steer what survives compaction |
| Claude Code | `PostCompact` | Y (`compact_summary`) | N | N | N | Persist the summary as a checkpoint |
| Claude Code | `Stop` | Y (`last_assistant_message`) | Y (keeps working) | Y | N | **Does not run when the turn ends on an API error** |
| Claude Code | `StopFailure` | Y (`error` ∈ `rate_limit`, `billing_error`, `authentication_failed`, `server_error`, `max_output_tokens`, …) | N | N | N | "Runs instead of Stop when the turn ends due to an API error" — the only hook that sees a usage-limit cut-off |
| Claude Code | `SubagentStart`/`SubagentStop`, `TaskCreated`/`TaskCompleted`, `TeammateIdle` | Y | Y (except Start) | Y (Start/Stop) | N | Native team tasks can be mirrored |
| Claude Code | `SessionEnd` | Y (`reason`) | N | N | N | 1.5 s shared budget |
| Claude Code | Statusline / OpenTelemetry | Y | N | N | **Y** (context %, rate-limit %, tokens, cache) | Only pre-emptive quota signal; display/telemetry, not hooks |
| Codex | `SessionStart` (`startup`/`resume`/`clear`/`compact`/`fork`) | Y | `continue:false` | Y | N | |
| Codex | `UserPromptSubmit` | Y | Y | Y | N | |
| Codex | `PreToolUse` / `PermissionRequest` | Y | Y | Y (PreToolUse) | N | Bash, apply_patch, MCP, local tools; not hosted tools. Docs: "a useful guardrail, not a complete enforcement boundary" |
| Codex | `PostToolUse` | Y | replaces result | Y | N | |
| Codex | `PreCompact` / `PostCompact` | Y (`trigger`) | Y (`continue:false`) | **N — stdout ignored** | N | Cannot steer the summary |
| Codex | `Stop` / `SubagentStop` | Y | Y (`reason` becomes a new prompt) | via reason | N | Behaviour on usage limit NOT VERIFIED; no `StopFailure` equivalent |
| Codex | `PreModelRequest` / `PostModelRequest` | Y | N (diagnostic only) | N | **Y** (`token_usage`, `status`, `error`) | Only error/limit signal in Codex |
| Codex | `Interrupt`, `SessionEnd` | Y | N | N | N | 1 s default timeout (max 3 s); SessionEnd also after 30 min idle |
| OpenCode V2 | `execute.before/after`, session `prompt`/`context`/`compaction`/`retry`, `permission.evaluate`, `event.subscribe` | Y | partial | **Y** (system parts, keyed `<context>` entries, can *replace* the compaction summary) | Y (`generate` tokens/cost) | Needs a JS/TS plugin; Rekora cannot integrate with a JSON hook file here |
| OpenCode V1 | `session.idle`, `session.compacted`, `tool.execute.*`, `experimental.session.compacting` | Y | throw in before-hook (CI) | Y | ? | Version-dependent; one report of the compacting hook silently never running |
| Antigravity | `SessionStart`, `PreInvocation`, `PostInvocation` | Y | Y (Invocation: `terminate`) | Y (`injectSteps`) | N | Config `.agents/hooks.json`, `~/.gemini/config/hooks.json` |
| Antigravity | `PreToolUse` / `PostToolUse` | Y | Y (Pre) | N | N | Rich decision set |
| Antigravity | `Stop` | Y (`terminationReason`, `error`, `fullyIdle`) | Y (`continue`) | Y | N | No compaction hook |
| Antigravity | Teamwork artifacts | Y (files) | N | N | ? | Plan and progress artifacts in a project directory |
| Gemini CLI | `SessionStart`, `BeforeAgent`, `AfterTool`, `BeforeModel`, `AfterModel`, `BeforeTool`, `BeforeToolSelection` | Y | Y (most) | Y | ? | `PreCompress`, `SessionEnd`, `Notification` advisory |
| DeepSeek Harness | Claude Code / Codex hook bridges, MCP client | ? | ? | ? | ? | Developer preview; bridges described only by community sources: **NOT VERIFIED** |

**Cross-cutting conclusions:**

1. Every platform with documented hooks can inject context at session start and observe tool completion (DeepSeek Harness: not verified).
2. Only Claude Code can steer the content of a compaction (and only OpenCode V2 can replace it).
3. Only Claude Code has an explicit API-error stop event (`StopFailure`).
4. **No platform documents a hook on a hard crash** (kill, OOM, power loss). Design must assume none.
5. Token and quota signals live outside the hook channel (statusline, OTel, transcript files), except
   Codex `PostModelRequest`.

## 3. Gaps in Rekora's current hooks (derived)

| Gap | Evidence | Consequence today |
|---|---|---|
| A usage-limit cut-off on Claude Code fires `StopFailure`, not `Stop` | Claude hooks DOC | Earlier turns, tasks and live `git status` still reach the next briefing, but **the failing turn and its error are not recorded**, so the briefing cannot say the session was cut off by a limit. STATUS listed host behaviour on usage limits as NOT VERIFIED; the docs now answer it for Claude. `StopFailure`'s `last_assistant_message` is the API error string and must not overwrite the previous turn's message |
| Turns are not tied to a session, store no worktree root, have no freshness bound, and any uncommitted change (even untracked tool folders) triggers a briefing | Observed 2026-11-24 on this repository (`hook.rs` `briefing`, `tasks.rs` `last_turn`) | The session that started this investigation was briefed with a month-old turn from a Claude session about a **different project** whose `Stop` hook fired with `cwd` inside this checkout; the only trigger was two untracked tool folders. Nothing was unfinished. A "same checkout" check alone would not catch this (the foreign session's `cwd` *was* this checkout); freshness, session id and an untracked-only rule would. The same record is the first observed evidence that a live Claude Code session fires Rekora's `Stop` hook (STATUS: NOT EXECUTED) |
| `SessionStart` `source` is ignored | Claude and Codex DOC (`startup`/`resume`/`clear`/`compact`/`fork`) | After a compaction, the same full briefing is re-injected instead of the active task's constraints |
| No `PreCompact` / `PostCompact` | Claude DOC | One preprint found the first summary dropped ~1/3 of anti-cheating rules on research-replication benchmarks (transfer to coding constraints untested, see CONTEXT_ENGINEERING_FINDINGS.md); Rekora is not consulted at the compaction boundary |
| Per-step progress depends on agent discipline (`task_update`) | MCP instructions | A cut-off mid-turn loses everything since the last voluntary update |
| One shared `HOOK_EVENTS` list for both hosts | `adapters.rs` | Adding Claude-only events (`StopFailure`, steerable `PreCompact`) needs per-host event tables; existing users must re-install the user-wide hook files explicitly |
| Task claims are last-writer-wins | `mcp.rs` / `main.rs` `handoff_resume` call `update_task` with no expected version | A second agent silently takes over a claimed task |
| Hook latency includes 2–3 `git` subprocesses | Measured, section 6 | Fine at session start; too slow to run on every tool call as is |

## 4. Q11 — persistence granularity for long-running coding tasks

Durable-execution systems agree (DBOS, Restate, Temporal — DOC; LangGraph — DOC): **persist at step
boundaries, where a step is the smallest unit with an externally visible side effect, record the step's
result, never checkpoint mid-step.** LangGraph checkpoints each super-step and keeps the pending writes
of siblings that succeeded; OpenHands V1 appends one file per event and overwrites a base state.

For coding agents (HYP, grounded in the above):

| Level | Trigger | Content | Cost |
|---|---|---|---|
| Event | `PostToolUse` on edit/write/apply_patch and test/build commands (matcher-limited) | tool, files, exit status, HEAD | one SQLite insert; must not call `git` |
| Checkpoint | `task_update` by the agent; `PostCompact`; `TaskCompleted`; `SubagentStop` | versioned task state: done, remaining, decisions, tests with results, commit ref | already exists (versioned tasks) |
| Handoff | `Stop`, `StopFailure`, `PreCompact`, `SessionEnd` | `rekora.handoff/1` derived from the two levels above | already exists |

Do **not** persist transcripts or token streams — hosts already keep them (Claude JSONL, Codex
rollouts). Store pointers (`session_id`, `transcript_path`) instead. Code state belongs in git; a
checkpoint references a commit.

**Open question for the benchmark:** is the event level worth it? It costs a process spawn per matched
tool call (~22 ms process start measured, more with a store open) and adds write volume. The
checkpoint + handoff levels plus `git status` at resume may be enough. BENCHMARK_DESIGN.md (B4) measures
recovery correctness with and without it before anything is built.

## 5. Q12 — how a task should recover after a process crash

Modelled on DBOS/Restate replay, adapted to non-deterministic agents (HYP):

1. **Detect** at `SessionStart`: an `in_progress` task whose last update belongs to an earlier session
   with no closing handoff is a crash or cut-off. `StopFailure` (Claude) or a `PostModelRequest` error
   (Codex) can mark it `interrupted` with the error type when they fire; absence of a close marker is
   treated the same way.
2. **Reconstruct from durable facts, not from the model's memory:** last checkpoint, events after it,
   current `git status` and diff against the recorded commit.
3. **Treat the in-flight step as unknown:** re-verify rather than re-execute — re-run tests and builds,
   inspect the tree for half-applied edits, and flag non-idempotent commands (migrations, pushes,
   deploys) as "needs confirmation" in the briefing. Never re-run them automatically.
4. **Inject a resume pack** at session start (Claude/Codex `additionalContext`, Antigravity
   `injectSteps`, OpenCode V2 context entry), labelled as data.
5. **Version, don't overwrite:** the resume is a new task version (Rekora tasks are already versioned;
   LangGraph forks on `update_state`).
6. **Bound auto-continuation** with the host's loop protection; never auto-continue after a crash.

Cross-host resume (Claude Code cut off by quota, continue in Codex) works only because steps 2–4 depend
on Rekora's store and git, not on one host's transcript format. That is Rekora's main differentiator over
native `--resume` / `codex resume`, which are host-local.

## 6. Q15 — embedded or daemon?

Measured on the investigating machine (Windows 11, 64 GB RAM, installed release binary
`rekora 0.1.0`, 30 runs each, 2026-11-24 — **EXECUTED**):

| Command | p50 ms | p95 ms |
|---|---|---|
| `rekora --version` (process start only) | 22.4 | 41.4 |
| `git rev-parse --show-toplevel` | 49.3 | 73.4 |
| `git status --porcelain` | 57.2 | 121.7 |
| `rekora hook session-start` inside a project | 138.3 | 170.5 |
| `rekora hook session-start` outside a project | 98.0 | 135.0 |
| `rekora status --format json` | 161.9 | 202.0 |

The cost of an embedded hook is dominated by the `git` subprocesses it starts, not by Rekora's own
process start (~22 ms). A daemon would remove ~20 ms per call and add a lifecycle (start, stop, crash,
upgrade, port or socket security, stale state) on every machine. SQLite WAL with `BEGIN IMMEDIATE`
already serialises 4 writer + 8 reader OS processes in the existing concurrency test.

**Answer:** embedded only. A daemon gives no measurable advantage at current call rates. Revisit only if
a per-tool-call hook is adopted *and* the benchmark shows its p95 latency hurts the session — and first
try removing `git` calls from that path. The existing optional HTTP MCP mode (Docker) stays what it is:
an optional deployment, not a required service.

## 7. Q16 — integrating with native orchestration instead of duplicating it

Native orchestrators already provide spawning, roles, assignment, messaging, file-ownership rules and
verification gates: Claude Code agent teams (lead, teammates, shared task list in
`~/.claude/tasks/{team}/`, file-locked claims, JSON mailboxes, `TaskCreated`/`TaskCompleted`/`TeammateIdle`
hooks; experimental), Antigravity Teamwork (`/teamwork`: Sentinel, Project Orchestrator, Explorers,
Workers with exclusive file ownership, Critic/Challenger/Auditor gates, plan and progress artifacts),
Codex subagents (`SubagentStart`/`SubagentStop`).

What they do **not** provide, by their own docs: durability across resume (Claude: "`/resume` and
`/rewind` do not restore in-process teammates"; team config is deleted at session end), cross-host
portability, verified provenance, and project-scoped long-term memory. Teamwork's docs do not describe
resume.

**Answer (HYP grounded in DOC):**

1. **System of record, not scheduler.** No spawner, mailbox, assignment logic or A2A transport in Rekora.
2. **Mirror, don't own.** Optionally record native task/subagent events (`TaskCreated`, `TaskCompleted`,
   `SubagentStop`) with the native id as an external reference; never write into native task files.
3. **Fill the resume gap.** On the lead's `SessionStart` with `source=resume`, brief which tasks had open
   work and what was last verified, so the lead can respawn them.
4. **Same read path for every agent.** Teammates load project MCP servers, so `context`/`search` are
   available without per-agent setup; their writes stay `candidate`.
5. **Share identifiers, not runtimes.** One task id in branch names, handoffs and native task titles.
6. **Budget context per role.** A worker gets its task's checkpoint, not the whole pack.

## Sources

| Title | URL | Date | Class |
|---|---|---|---|
| Claude Code hooks reference | https://code.claude.com/docs/en/hooks | accessed 2026-11-24 | DOC |
| Claude Code statusline | https://code.claude.com/docs/en/statusline | accessed 2026-11-24 | DOC |
| Claude Code monitoring (OTel) | https://code.claude.com/docs/en/monitoring-usage | accessed 2026-11-24 | DOC |
| Claude Code agent teams | https://code.claude.com/docs/en/agent-teams | accessed 2026-11-24 | DOC |
| Claude Code worktrees | https://code.claude.com/docs/en/worktrees | accessed 2026-11-24 | DOC |
| Codex hooks | https://learn.chatgpt.com/docs/hooks | accessed 2026-11-24 | DOC |
| Codex config reference | https://learn.chatgpt.com/docs/config-file/config-reference.md | accessed 2026-11-24 | DOC |
| OpenCode plugins | https://opencode.ai/docs/plugins/ | accessed 2026-11-24 | DOC |
| OpenCode server plugins (V2) | https://opencode.ai/docs/build/plugins/server | accessed 2026-11-24 | DOC |
| OpenCode V2 compaction | https://opencode.ai/v2/compaction.md | accessed 2026-11-24 | DOC |
| OpenCode compacting-hook issue | https://github.com/vectorize-io/hindsight/issues/2656 | 2026 | CI |
| Antigravity Teamwork | https://antigravity.google/docs/teamwork/ | accessed 2026-11-24 | DOC |
| Antigravity hooks | https://antigravity.google/docs/hooks.md | accessed 2026-11-24 | DOC |
| Antigravity SDK lifecycle | https://www.antigravity.google/docs/sdk/lifecycle/ | accessed 2026-11-24 | DOC |
| Gemini CLI hooks | https://geminicli.com/docs/hooks/ | accessed 2026-11-24 | DOC |
| DeepSeek Harness | https://github.com/deepseek-ai/deepseek-harness | accessed 2026-11-24 | OSS |
| LangGraph persistence / checkpointers | https://docs.langchain.com/oss/python/langgraph/persistence | accessed 2026-11-24 | DOC |
| OpenHands SDK conversation persistence | https://docs.openhands.dev/sdk/guides/convo-persistence.md | accessed 2026-11-24 | DOC |
| OpenHands Software Agent SDK | https://arxiv.org/abs/2511.03690 | 2025-11 | PP |
| DBOS architecture | https://docs.dbos.dev/architecture | accessed 2026-11-24 | DOC |
| Restate durable execution | https://docs.restate.dev/concepts/durable_execution | accessed 2026-11-24 | DOC |
| MCP specification 2026-07-28 and changelog | https://modelcontextprotocol.io/specification/2026-07-28/changelog | 2026-07-28 | DOC |
| MCP Tasks extension | https://modelcontextprotocol.io/extensions/tasks/overview.md | accessed 2026-11-24 | DOC |
| A2A specification 1.0.0 | https://a2a-protocol.org/latest/specification/ | v1.0.0 | DOC |
