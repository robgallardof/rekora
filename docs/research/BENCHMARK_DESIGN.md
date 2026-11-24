# Benchmark design (agent efficiency)

Status: **design only, NOT EXECUTED.** Nothing in this file has been run. It defines what must be
measured before any feature from [ARCHITECTURE_RECOMMENDATION.md](ARCHITECTURE_RECOMMENDATION.md) is
accepted or expanded. Hypotheses and thresholds are pre-registered in [DECISION_LOG.md](DECISION_LOG.md)
(H10–H17) before any run.

## 1. Principles

1. **Cost is not tokens.** Report uncached input, cache reads, cache writes (short/long TTL), output and
   reasoning separately, then cost from a dated price table. Raw token deltas overstate savings under
   caching (TurnCost, PP: 98.6 % of input served from cache; AgentDiet, PR: 40–60 % fewer tokens became
   21–36 % lower cost).
2. **Quality first.** A cheaper configuration that solves fewer tasks, passes fewer tests or loses
   constraints is a regression. Report success with confidence intervals.
3. **Test what Rekora can actually change.** Rekora does not own model requests. Conditions that need a
   proxy (trajectory reduction, in-flight rewriting) are *external reference baselines* — to size the
   prize — not candidate Rekora features, and run only if justified.
4. **Real dependencies** (AGENTS.md rule 1): real host binaries, git, SQLite and MCP child processes.
   Scripted agents only for the deterministic suites (B0, B4, B5).
5. **Pre-register** hypothesis, metric, minimum detectable effect and accept/reject rule in
   DECISION_LOG.md; **cap the dollar budget** of every paid suite before starting and stop at the cap.

## 2. Suites

| Id | Suite | Model spend | Decides |
|---|---|---|---|
| B0 | Rekora overhead: tool-definition and instruction tokens, briefing size, hook latency | none / negligible | Whether Rekora is cheap enough to leave on everywhere (H4) |
| B1 | Usage-parser accuracy vs per-response API usage | small | Whether host-file measurement is trustworthy (H5) |
| B2 | Live A/B on coding tasks | large, capped | Cost and success effect of configurations |
| B3 | Compaction fidelity | medium, capped | Whether E2 is worth keeping |
| B4 | Crash and cut-off recovery | none (scripted) + one live check per host | E1 acceptance; persistence granularity (O3) |
| B5 | Coordination under contention | none | H1–H3 acceptance |

**MVP order:** B0 → B4 → B3 pilot. B1 before H5. B2 last, and only for questions the cheaper suites
cannot answer.

## 3. B1 — ground truth for usage

Provider admin APIs need organisation-admin keys and return time-bucketed aggregates, not per-session
records; subscription users (the ones cut off by usage limits) have no API usage at all. So:

- **Ground truth** = sessions run with an API key through the Claude Agent SDK / `claude --print` and
  `codex exec`, capturing every response's `usage` (and request id) as returned by the API.
- **Under test** = the parse of the same session's transcript (Claude) or rollout (Codex), deduplicated
  by request id.
- **Accept** if per-response token counts match exactly after dedupe on ≥ 20 sessions per host.
  Subscription sessions: parsed only, labelled "unreconciled".

## 4. B2 — configurations

| # | Configuration | Owned by | Runs on |
|---|---|---|---|
| C0 | **Baseline**: host defaults, Rekora not installed | Host | SWE-bench subset + long-horizon set |
| C1 | **Checkpoint-only**: Rekora MCP + current hooks; agent asked to `task_update` | Rekora (today) | both |
| C2 | **C1 + E1/E2** (cut-off capture, compaction-aware briefing) | Rekora (proposed) | long-horizon set only (SWE-bench tasks rarely compact, so C2 ≈ C1 there) |
| C3 | Trajectory reduction (masking or AgentDiet-style) | External reference via SDK harness or proxy | only if C0–C2 show a gap worth sizing |
| C4 | Host compaction tuned (threshold; Anthropic background compaction via SDK) | Host / reference | same condition as C3 |
| C5 | Tool schema optimisation: host tool search on/off; Rekora descriptions trimmed vs current | Host + Rekora | SWE-bench subset |
| C6 | Deterministic tool-result reuse: Rekora `symbols` (content-hash index) vs plain reads | Rekora (exists) | SWE-bench subset |
| C7 | Combined Rekora-owned options | Rekora | **only after** at least two components show an individual effect |

Note: on SWE-bench the Rekora store starts empty, so C1 there mostly measures Rekora's own overhead
(tool definitions, briefing) — which is what B0 measures more cheaply. C1 is informative mainly on the
long-horizon set, where tasks and memories accumulate during the session.

## 5. Task sets

- **SWE-bench subset:** 40 tasks from SWE-bench Verified, fixed random sample with published instance ids,
  stratified by difficulty. Used for C0/C1/C5/C6.
- **Long-horizon set:** multi-requirement sessions in the CompactionBench style, where compaction actually
  fires. Source: a named public repository with a pinned commit range and a multi-item changelog, or
  SWE-chat sessions; if CompactionBench data is released, use it. **This repository's own history is too
  small** (23 commits, two CHANGELOG sections) and written by the same author — usable only as a smoke
  test.
- **Cut-off set:** 10 scripted tasks for B4.
- No real secrets and no data from other projects in fixtures.

## 6. Metrics

| Metric | Definition | Source |
|---|---|---|
| Actual cost | Σ(tokens × dated price) per task, by category | captured API usage (B1 method) |
| Input uncached tokens | Anthropic `input_tokens`; OpenAI `input_tokens − cached_tokens` | API usage |
| Input cached tokens | cache reads; cache writes separately with TTL | API usage |
| Output tokens | reasoning reported separately | API usage |
| Model calls | API requests (dedupe by request id) | API usage |
| Tool calls | by tool, incl. Rekora MCP calls | transcripts |
| Retry rate | API retries + repeated identical tool calls (OpenHands stuck patterns) | transcripts |
| Task success | resolved per benchmark harness | harness |
| Test success | target tests passing; no previously passing test broken | harness |
| Patch quality | diff size, files touched outside the gold patch, blind rubric on a 20 % sample | diff + human |
| Latency | wall time per task; hook p50/p95 | timestamps |
| Recovery correctness | see B4 | B4 assertions |
| Context fidelity | violation rate of seeded constraints after each compaction | B3 |

## 7. Controls and statistics

- **Versions:** pin exact model ids and host versions for the whole campaign; a host update starts a new
  campaign rather than re-running only the baseline.
- **Provider cache effects:** randomise condition order per task; run each condition cold (wait longer
  than the longest TTL, or a per-run nonce at the start of the first user message) unless warm runs are
  the subject; report cache hit rate per run.
- **Difficulty:** stratified sample; paired comparison (same task under every condition); per-stratum
  results.
- **Conditions:** same machine and network, clean worktree per run, fixed tool versions, same
  permission mode, no other agents running.
- **Repetitions and power:** at least 3 runs per task × condition. With 40 tasks × 3 runs, success
  differences of 1–3 points are not resolvable; state the minimum detectable effect before running
  (paired bootstrap, 10,000 resamples, 95 % intervals). A difference whose interval includes zero is
  "no measurable effect".
- **Budget:** estimate the campaign cost from C0 pilot runs; record the cap; stop at the cap and report
  what ran.
- **Blindness:** patch-quality reviewer does not see the condition.

## 8. B3 — compaction fidelity (decisive for E2)

Claude Code re-reads CLAUDE.md after compaction, so constraints written there already survive. Rekora's
possible contribution is for **session-emergent** constraints: rules the user or agent establishes
during the session and records as task constraints.

1. Seed a long-horizon session with 10 constraints introduced *during* the session (process rules such
   as "never edit generated files", and specifications such as "the endpoint returns 404 on unknown
   id"), recorded as Rekora task constraints — what E2 actually re-injects.
2. Force at least 3 compactions (lower the host threshold if needed).
3. After each compaction, measure violations per constraint (tests + a rule checker), separately for
   process rules and specifications.
4. Conditions: **C1** (no compaction integration); **C1 + the same constraints in CLAUDE.md** (the free
   alternative); **C2a** `PreCompact` template only; **C2b** `SessionStart source=compact` re-injection
   only; **C2** both.
5. **Pilot C1 first** to estimate the baseline violation rate, then size the number of sessions so a
   50 % relative reduction has ≥ 80 % power. If the needed size exceeds the budget, report the results
   descriptively instead of accept/reject.
6. **Accept E2** if C2 (or C2b alone, in which case drop `PreCompact`) has a lower violation rate than
   both C1 and "CLAUDE.md", with no loss of task success. Cost is reported, not gated by a fixed
   percentage: after compaction the cache is cold anyway (CompactionBench: 99 → 27 % hit rate), so the
   re-injected tokens are paid at the uncached rate once and then cached; report that cost explicitly.

## 9. B4 — recovery correctness (decisive for E1)

Scripted agent (no model) that performs a known sequence through the real hook binary and MCP server:
edits, test runs, `task_update`, then is killed at random points (between steps, mid-step, during a
hook). Variants: kill with no hook; `StopFailure` invoked with the documented payload (`error` =
`rate_limit`, `billing_error`, `max_output_tokens`); clean `Stop`. A second process then runs
`SessionStart` and `handoff_resume`.

Assertions:
- limit-type failures and kills are reported as interrupted; `max_output_tokens` is not;
- the previous turn's message survives a `StopFailure`;
- last verified step correct; edits after the checkpoint listed;
- **no stale, same-session or foreign turn is briefed** (regression for the 2026-11-24 observation);
- untracked-only trees produce no briefing.

Persistence granularity comparison (for O3): handoff-only vs checkpoint + git status vs event log +
checkpoint. Adopt the event log only if it improves recovery correctness measurably.

One live check per host: real session, host process killed, resumed in the other host.

## 10. B5 — coordination

N OS processes (as in `process_concurrency.rs`) and real git worktrees: concurrent claims of one task
(after H1: exactly one owner), lease expiry and reclaim, stale owner's next write rejected, seeded
overlapping vs disjoint edits for H2, labelled near-duplicate task titles for H3.

## 11. B0 — Rekora's own overhead (run first)

- Tokens of Rekora's 9 tool definitions and server instructions (provider token-count endpoint or the
  host's context view).
- Briefing size, typical and worst case (5 tasks, 20 files).
- Hook latency p50/p95 per event. Baseline measured 2026-11-24 (Windows 11, installed v0.1.0):
  `hook session-start` 138/171 ms, dominated by `git`.
- Thresholds: definitions + instructions < 2,000 tokens; worst-case briefing < 1,500 tokens;
  SessionStart p95 < 300 ms; any per-tool-call hook p95 < 50 ms.

## 12. Reporting

One JSON file per run (environment, versions, condition, task id, metrics, raw usage records) stored
outside the repository; a summary committed under `docs/research/` with every not-run cell marked
**NOT EXECUTED**. A result reports cost, success and fidelity together or it is not reported.
