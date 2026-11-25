# Decision log

Hypotheses and their accept/reject criteria. Status reflects measurements in this repository only.

| # | Hypothesis | Test | Result | Status |
|---|---|---|---|---|
| H1 | Keyword retrieval with a coverage floor can abstain on unanswerable questions without losing answerable ones | Relevance suite: 30 answerable / 10 unanswerable (`rekora benchmark run --suite relevance`) | Specificity 1.0; 0 answerable abstained (after H2) | Accepted for v0.1, needs a larger, independently labelled corpus |
| H2 | English stemming improves recall without hurting abstention | Same suite before/after migration v4 | P@1 0.833 → 0.933, R@5 0.900 → 1.000, MRR 0.867 → 0.967, specificity 1.0 → 1.0 | Accepted (ADR-0003). Risk: corpus written by the same author as the code |
| H3 | Per-project database files give rank isolation by construction | S03/R03 tests inspect raw JSON of searches and packs | No leakage observed | Accepted (ADR-0001) |
| H4 | One query per objective/remaining item beats one concatenated query for handoffs | `s02_handoff_between_two_agents_in_separate_processes` failed with concatenation, passes with multi-query | Constraint memory now included | Accepted |
| H5 | Hosts accept read-only tools without interactive approval | Codex 0.164.0 `exec` before/after annotations | Blocked → completed | Accepted for Codex; Claude Code `-p` uses `--allowedTools` |
| H6 | Embeddings would materially improve recall | Not run | — | Deferred; must beat H2 numbers on a corpus not written by us, offline, behind a feature flag |
| H7 | Lifecycle hooks (auto-capture) are worth their risk | Handoff hooks shipped 2026-10-27 (`SessionStart`, `Stop`; no content auto-capture). First live evidence of a Claude `Stop` firing: a turn recorded 2026-10-29 | Hooks fire live; also produced a false briefing (see H10) | Partly accepted: handoff hooks yes; content auto-capture still deferred (would land as `candidate` only) |
| H8 | Jev-style probabilistic decision model adds value | Not run | — | Rejected for core; out of scope until a baseline exists |
| H9 | Skill generation (Hermes-style) helps coding continuity | Not run | — | Deferred; no automatic execution ever |

## Agent-efficiency investigation (2026-11-24) — pre-registered, NOT EXECUTED

Source: [AGENT_EFFICIENCY_RESEARCH.md](AGENT_EFFICIENCY_RESEARCH.md). Suites in
[BENCHMARK_DESIGN.md](BENCHMARK_DESIGN.md). All rows below are hypotheses registered before any run.

| # | Hypothesis | Test | Result | Status |
|---|---|---|---|---|
| H10 | Session-scoped, fresh, bounded turns plus recording `StopFailure` remove false and missing cut-off briefings (E1) | B4 scripted kills and `StopFailure` payloads; regression for the 2026-11-24 foreign-turn briefing | 2026-11-24, scripted (real binary, git, SQLite): `s21` (rate_limit cut-off reported with the real last message, own session silent, `max_output_tokens` ignored), `s22` (month-old and v0.1 turns hidden, untracked-only tree silent) pass. Live host kill: NOT EXECUTED | Implemented; scripted part passes. Accept fully after one live host check. Accept: every limit-type failure/kill reported interrupted, `max_output_tokens` not, zero stale/same-session/foreign briefings, untracked-only trees silent |
| H11 | Re-injecting the active task as data after compaction lowers violations of session-emergent constraints (E2) | B3: C1, C1 + CLAUDE.md, C2a (`PreCompact` template), C2b (re-injection), C2; pilot first for power | Mechanism implemented and tested (`s23`); session→task binding not built (branch fallback). B3: NOT EXECUTED | Pre-registered; benefit unproven until B3. Accept: C2 or C2b below both C1 and CLAUDE.md with no success loss; otherwise reject or report descriptively if under-powered |
| H12 | Rekora's own overhead is small enough to keep installed everywhere | B0 | `hook session-start` 138/171 ms p50/p95 (EXECUTED 2026-11-24, Windows). Conservative bytes/3 estimate (test `b0_…`): tool definitions 8,061 bytes ≈ 2,687 est. tokens; worst-case briefing ≈ 2,093 est. tokens. Real tokenizer count: NOT EXECUTED | **Not met** by the conservative estimate (limits 2,000 / 1,500); latency met. Decide after a real token count (bytes/3 over-counts ~25 %). Accept: definitions + instructions < 2,000 tokens, worst briefing < 1,500 tokens, SessionStart p95 < 300 ms |
| H13 | Host-file usage parsing matches per-response API usage (H5 ledger) | B1 with API-key sessions as ground truth | — | Pre-registered. Accept: exact per-response match after dedupe on ≥ 20 sessions per host |
| H14 | Atomic claims with leases prevent double ownership without permanent locks (H1) | B5, N OS processes | Today claims are last-writer-wins (`handoff_resume` passes no expected version) | Pre-registered |
| H15 | A daemon would materially reduce hook latency | Latency breakdown 2026-11-24 (EXECUTED): process start 22 ms p50, `git status` 57 ms, hook 138 ms | Hook cost is `git`-bound | **Rejected** for now: embedded only; revisit only if a per-tool-call hook exceeds 50 ms p95 without `git` |
| H16 | Rekora should compress/summarise context, cache model answers semantically or route models | Literature review (CONTEXT_ENGINEERING_FINDINGS.md, TOKEN_ECONOMICS.md) | Needs request ownership or LLM calls; semantic caching unsafe (vCache, PR); routing rebuilds caches | **Rejected** (not recommended) |

### H17 — independent review of the research drafts

A separate reviewer agent checked the seven drafts against the researcher reports and the code and
returned 17 findings. All were applied before commit:

1. Review claimed but not recorded → this section and H10–H16 added; H7 updated.
2. "Claim on resume" / "one owner per task" overstated → claims are last-writer-wins today; atomic claims
   moved to H1 with a migration.
3. `PreCompact` would have echoed agent-written task text as summary instructions (rule 6) → fixed
   template naming only task ids; re-injection as labelled data via `SessionStart`.
4. "Active task" undefined → binding via `PostToolUse` on Rekora's own tools; ambiguous → ids only.
5. Cut-off defect overstated, fix mismatched → restated (failing turn not recorded); freshness, session id,
   worktree root, untracked-only rule, no overwrite on `StopFailure`, error-type filter.
6. "No migration, no new surface" understated → per-host event tables, explicit re-install of user-wide
   hook files, turn retention, `doctor` drift report added to the MVP.
7. B3 lacked the CLAUDE.md control and confounded three changes → C1 + CLAUDE.md, C2a, C2b added.
8. B1 ground truth unavailable via admin APIs → API-key sessions with captured per-response usage.
9. "1/3 of constraints lost" overgeneralised → scoped to anti-cheating rules in one preprint.
10. Routing evidence contradicted TurnCost → break-even analysis cited; shared authorship noted.
11. Duplicate-task FTS assumed a non-existent table → Jaccard scan over open tasks.
12. Lease fields in agent-patchable JSON → dedicated columns via migration.
13. MVP inconsistent across files → E1 + E2 + E3 (harness) everywhere; usage ledger moved to H5.
14. B2 under-powered and too expensive → MDE, budget cap, C2 on long-horizon only, C7 deferred, pinned
    versions.
15. CHANGELOG replay corpus too small → public pinned repository or SWE-chat; this repo as smoke test.
16. B3 power unknown and moot cache argument → pilot and power analysis; cost reported, not gated.
17. Nits → "none of the 15 systems reviewed"; "every documented platform"; first live `Stop` evidence;
    TurnCost stated as a correlation.
