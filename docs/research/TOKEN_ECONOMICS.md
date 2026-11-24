# Token economics, budgets and governance

Research date: 2026-11-24. Research only: nothing here is implemented. Part of the investigation
summarised in [AGENT_EFFICIENCY_RESEARCH.md](AGENT_EFFICIENCY_RESEARCH.md).

Evidence classes: **PR** peer-reviewed · **PP** preprint · **DOC** official documentation ·
**OSS** open-source code (read at source) · **VR** vendor-reported · **CI** community ·
**HYP** our inference, untested. **NOT VERIFIED** = not confirmed from a primary source.

**Rekora's position, which decides most answers below:** Rekora is a local binary called by the host
through MCP and lifecycle hooks. It never sends or sees model requests. Whatever it measures, it reads
from what hosts persist or emit; whatever it enforces, it enforces at hook boundaries.

---

## 1. What the providers report (ground truth fields, DOC)

| Provider | Uncached input | Cache read | Cache write | Output | Convention |
|---|---|---|---|---|---|
| Anthropic Messages | `input_tokens` (only tokens after the last breakpoint) | `cache_read_input_tokens` | `cache_creation_input_tokens` (split `ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens`) | `output_tokens` | **exclusive**: total input = read + creation + input |
| OpenAI Responses | `input_tokens` − cached | `input_tokens_details.cached_tokens` | `input_tokens_details.cache_write_tokens` (GPT-5.6+) | `output_tokens` | **inclusive**: cached is a subset of input |
| Gemini | `promptTokenCount` − cached | `usageMetadata.cachedContentTokenCount` | — | `candidatesTokenCount` (+ `thoughtsTokenCount`) | **inclusive** |
| DeepSeek | `prompt_cache_miss_tokens` | `prompt_cache_hit_tokens` | — | `completion_tokens` | split |

OpenAI Chat Completions' `usage.prompt_tokens_details.cached_tokens` is long-standing but was not on the
page fetched: **NOT VERIFIED** today. A provider-neutral record therefore needs
`{uncached_input, cache_read, cache_write_short, cache_write_long, output, reasoning}` plus a per-provider
rule for inclusive vs exclusive input.

## 2. What the hosts expose locally (Q6)

| Host | Source | Fields | Status |
|---|---|---|---|
| Claude Code | Transcript JSONL at `transcript_path` (given to every hook) | assistant entries carry `message.usage` with the raw Anthropic fields and `requestId` | **Observed on this machine, undocumented format** ("doesn't document the transcript's internal format", hooks DOC); written asynchronously, may lag the turn |
| Claude Code | Statusline stdin JSON | `cost.total_cost_usd` ("Estimated … may differ from your actual bill"), `context_window.current_usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens}` (last call only), `context_window.used_percentage`, `prompt_cache.{warm, misses, expected_rebuilds,…}`, `rate_limits.{five_hour,seven_day,spend_limit}` | DOC; display surface, last call only |
| Claude Code | OpenTelemetry (`CLAUDE_CODE_ENABLE_TELEMETRY=1`) | metric `claude_code.token.usage` with `type` ∈ `input`/`output`/`cacheRead`/`cacheCreation`; event `claude_code.api_request` with `cache_read_tokens`, `cache_creation_tokens`, `cost_usd`, `request_id`, `model`; hook `prompt_id` = OTel `prompt.id` | DOC; needs a collector and user-set env vars |
| Claude Agent SDK | Result message | `total_cost_usd`, `modelUsage[*].{inputTokens, cacheReadInputTokens, cacheCreationInputTokens, costUSD, costBasis}` | DOC: "client-side estimates, not authoritative billing data" |
| Codex CLI | Rollout `token_count` events (`~/.codex/sessions`, path from CI sources) | `TokenUsage { input_tokens, cached_input_tokens, cache_write_input_tokens, output_tokens, reasoning_output_tokens, total_tokens }`, `last_token_usage` and cumulative `total_token_usage` | OSS (`codex-rs/protocol/src/protocol.rs`); rollout path NOT VERIFIED in official docs; duplicate events reported by CI |
| Codex CLI | `PostModelRequest` hook | `token_usage` ("Token counts for the attempt when available, otherwise `null`"), `status`, `error` | DOC; diagnostic only, cannot block |
| Codex CLI | OpenTelemetry | histogram `turn.token_usage` with `token_type` ∈ `total`/`input`/`cached_input`/`output`/`reasoning_output` | DOC; off by default |
| OpenCode | `~/.local/share/opencode/opencode.db` (`OPENCODE_DB`) | session columns `cost`, `tokens_input` (non-cached), `tokens_output`, `tokens_reasoning`, `tokens_cache_read`, `tokens_cache_write` | OSS (dev branch, migration 2026-05-10); docs advise backup before external inspection → read-only access only |
| DeepSeek Harness | — | — | **NOT VERIFIED**, no primary docs found |

Provider admin APIs (Anthropic Usage & Cost API, now "legacy" with a successor; OpenAI Usage/Costs API)
are the authoritative reconciliation source but need organisation-admin keys and lag about 5 minutes
(DOC). Host-side dollar figures are estimates by every host's own documentation.

### Q6 answer — how can cached and uncached tokens be measured?

From Rekora's position, by **reading what hosts already persist**, never by owning requests:

1. Claude Code: parse `message.usage` from the transcript at `transcript_path` when the `Stop`,
   `StopFailure` or `SessionEnd` hook fires; deduplicate by `requestId`/message `id` (parallel tool calls
   share one usage; SDK docs say per-message `output_tokens` can be a placeholder — NOT VERIFIED for
   transcripts). Label the source `host_transcript, observed`.
2. Codex: `last_token_usage` from rollout `token_count` events, or the `PostModelRequest` hook payload.
3. OpenCode: a read-only query of its SQLite database.
4. Optional and opt-in only: an OTel receiver, or provider admin APIs for reconciliation.

Store **tokens as the fact** and compute cost from a dated, sourced price table, recording which table
was used. Never present a host dollar estimate as a bill. Because the Claude transcript format is
undocumented, the parser must tolerate drift and record `NOT PARSED` rather than guess (HYP).

## 3. Can an external service enforce budgets when the host owns the request? (Q7)

**Only at hook boundaries, only between model calls, and never as a hard ceiling.**

| Host / event | Can block? | Mechanism |
|---|---|---|
| Claude `PreToolUse` | yes | exit 2 or `permissionDecision: "deny"` |
| Claude `UserPromptSubmit` | yes | exit 2 or `decision: "block"` |
| Claude `PostToolBatch` | yes | "Stops the agentic loop before the next model call" |
| Claude `Stop` / `SubagentStop` | inverse | blocking **prevents stopping** — the agent keeps going and spends more; never use for budgets |
| Any Claude hook | yes | `"continue": false` + `stopReason` halts after the hook |
| Codex `PreToolUse`, `UserPromptSubmit` | yes | deny / exit 2 |
| Codex `PreModelRequest`, `PostModelRequest` | no | "diagnostic only" |

What hooks cannot do: cancel a streaming request, cap one response's tokens, stop subagent calls already
in flight, or act where the host bypasses hooks. Overshoot is therefore at least one model response for
the main loop plus one per running subagent (HYP; the Claude Agent SDK documents the same overshoot for
its own `max_budget_usd`: the crossing response is included).

A **hard** cap needs something in the request path: a LiteLLM proxy (`ANTHROPIC_BASE_URL`), a Claude apps
gateway spend limit, or provider workspace/organisation spend limits (Anthropic returns 400
`invalid_request_error` for user-set limits; 429 with `enforced_spend_limit_reached` and no
`retry-after` for tier caps — DOC). Proxies carry their own risk: Claude Code docs warn that a gateway
that strips `cache_control` makes "your entire conversation history bill as uncached input on every turn".

**Verdict:** Rekora can offer measurement, advisory warnings and *checkpoint* enforcement. Anything it
ships must be called "advisory budget guard, enforced at hook checkpoints, overshoot possible", never a
hard budget. Users who need a hard cap should be pointed to provider workspace limits or a proxy, which
Rekora must not configure implicitly (AGENTS.md rule 7).

## 4. Rate limits

- Anthropic (DOC): RPM / ITPM / OTPM per model and tier, token bucket. **Cache reads do not count toward
  ITPM** on default limits (input and cache creation do), so caching raises throughput. Headers
  `retry-after` (not on every 429) and `anthropic-ratelimit-*-{limit,remaining,reset}`.
- OpenAI (DOC): "Cached input tokens still count toward tokens-per-minute limits." `x-ratelimit-*`
  headers NOT VERIFIED.
- Hosts surface subscription limits differently: Claude Code statusline `rate_limits.five_hour` /
  `seven_day` (subscribers), Codex `rate_limits` inside `token_count` events (OSS).

Rate limits are the reason sessions get cut off; that is the scenario Rekora's handoff hooks exist for.
See PLATFORM_CAPABILITY_MATRIX.md: on Claude Code that cut-off fires `StopFailure`, not `Stop`.

## 5. Concurrent agents sharing a budget (Q9)

**Evidence.** LiteLLM's proxy (DOC + OSS) enables "budget reservation" by default: estimate the maximum
cost, reserve it against the budget, reject before sending if the reservation would exceed it, then
replace the reservation with the actual cost. Its docs state that without reservation "concurrent
requests [can] exceed a configured budget because each request is evaluated only against spend already
recorded", and without a database `max_budget` fails open. Without Redis each proxy instance enforces
its own counters. Community write-ups (CI) describe the same check-then-record race.

**Answer (HYP).** Reserve → execute → reconcile, with leases that expire, in one SQLite `BEGIN IMMEDIATE`
transaction per acquisition (Rekora's store already serialises writers this way and has a 12-process
contention test). Because Rekora cannot see `max_tokens` before a request, the reservation must be an
*estimate* (for example p95 per-turn cost of the same host and model over a recent window), so the
guard stays advisory. Leases expire so a crashed agent does not hold budget. Hierarchy project → task →
agent mirrors LiteLLM's team → member → key. A budget spanning several projects would need a user-level
store and is out of scope (rule 5: one scope per database).

**But first:** there is no evidence yet that Rekora's users need a shared budget. Measurement (section 2)
must come first; the guard is Optional in the roadmap.

## 6. Tool-result caching and semantic caching (Q13)

**vCache** (Schroeder et al., arXiv 2502.03771, **PR, ICLR 2026**, code and benchmarks released):
correct and incorrect semantic-cache hits have overlapping similarity distributions (mean 0.84 vs 0.85 on
one benchmark); the optimal per-entry threshold ranges 0.71–1.0, so no global threshold is safe; static
thresholds on LM-Arena prompts gave 2.5 % error at 0.99, 4.1 % at 0.98, 5.2 % at 0.97, rising as entries
accumulate. Their learned per-entry thresholds reach <0.5 % error at 57 % hits, under an i.i.d.
assumption. Redis LangCache docs (DOC) default to 0.85 and warn about wrong answers; "Closing the
Operational Gap in Semantic Caching" (PR, EMNLP 2026 Industry) finds the best offline models are often the
worst operationally at a fixed threshold.

**Q13 answer — semantic caching becomes unsafe when:**

1. correctness depends on details embeddings do not separate: paths, identifiers, versions, flags,
   negation;
2. the underlying state changed (a repository moves under the cache; a semantic key carries no version);
3. results are reused across scopes (forbidden outright by rule 5);
4. the cached thing is a side-effecting or non-deterministic action;
5. the cache grows and runs long (static-threshold error rises with volume);
6. inside agent loops, where a false hit becomes an observation and compounds.

For a coding agent almost every useful question falls under 1 or 2. **Only deterministic,
content-addressed caching is defensible**: key = (scope, tool, canonical arguments, content hash of every
input read, tool version). Rekora already does this for its code index (BLAKE3 per file, incremental).
Note that the provider prompt cache already makes *re-sent* context cheap; the remaining saving from a
tool-result cache is avoiding re-reading large unchanged outputs as *new* tokens, which hosts partly do
themselves. Similarity may be used for **retrieval** (suggesting labelled memories), never to
**substitute an answer**.

## 7. Model routing (Q14)

| Source | Class | Claim | Limits |
|---|---|---|---|
| RouteLLM (arXiv 2406.18665) | PR (ICLR 2025) | >2× cheaper; "over 85 % on MT Bench, 45 % on MMLU, 35 % on GSM8K" at ~95 % of GPT-4 quality (LMSYS blog, VR) | Single-turn chat benchmarks; Arena-trained routers **near random on MMLU** without ~1,500 in-domain labels; no agentic evaluation |
| FrugalGPT (arXiv 2305.05176) | PR (TMLR 2024) | Match GPT-4 with up to 98 % cost reduction | QA/classification with checkable answers; per-task scorer; cascade latency and wasted cheap calls |
| Claude Code model config / caching | DOC | "Each model has its own cache. Switching models recomputes the entire request"; each `opusplan` plan-mode toggle "starts a fresh cache"; `PreModelSwitch` hook exists | — |
| Claude Code subagents | DOC | A subagent's first request does not read the parent's cache, and the parent's cache is unaffected | — |
| OpenRouter Auto Router | VR/DOC | No router fee; no published evaluation | Conflicting docs about the backing router |

**Q14 answer.** Routing saves money on benchmark-like, single-turn distributions with in-domain training
data. In a long coding session every model switch re-reads the whole context uncached (a 150K-token
context re-read at 1.25× write instead of 0.05–0.1× read costs roughly 10–25× that turn's cached input —
arithmetic, HYP) and quality errors compound across steps. The only coding-agent analysis found is TurnCost's break-even derivation for delegating turns to cheaper models (PP, arXiv 2610.15093; it shares an author with the Prefix Stability poster, so the two are not independent evidence); no controlled coding-agent routing evaluation was found.
Routing is cache-friendly only at session/task boundaries and for self-contained subagents, both of which
the hosts already offer. **Rekora does not own requests and should not route.** It can at most record
per-model usage and cache rebuilds as evidence.

## 8. Retry and loop detection

- **OpenHands StuckDetector** (OSS, `openhands-sdk/.../stuck_detector.py`, read 2026-11-24): scans the last
  20 events; thresholds `action_observation=4`, `action_error=3`, `monologue=3`, `alternating_pattern=6`,
  plus repeated context-window errors. Portable to a hook-side detector over `PreToolUse`/`PostToolUse`
  inputs (hash of tool + canonical input + outcome).
- **LangGraph** (OSS): `DEFAULT_RECURSION_LIMIT` now 10007 on main (historically 25) — effectively no
  practical loop guard by default.
- **Claude Agent SDK** (DOC): `max_budget_usd` → `error_max_budget_usd`, crossing response included.
- **Codex**: no documented turn limit found (NOT VERIFIED).
- **MAST** (arXiv 2503.13657, PR NeurIPS 2025 D&B): 14 failure modes over 1,600+ multi-agent traces
  (κ = 0.88), including step repetition and unawareness of termination conditions; per-mode percentages
  NOT VERIFIED against the paper's table. A 2026 paper reports repetition also appears in *successful*
  traces, so repetition alone is not a failure signal.
- No rigorous measurement of the cost of runaway loops was found.

A Rekora loop detector would be advisory (inject a warning), opt-in, and only worth building if the
benchmark in BENCHMARK_DESIGN.md shows loops are frequent in real sessions. Hosts already have
continuation caps (Claude Code: `stop_hook_active` and an 8-continuation cap on Stop-hook blocks).

## 9. Summary for the roadmap

| Capability | Can Rekora do it? | Enforceable? | Recommendation |
|---|---|---|---|
| Measure cached / uncached / output tokens per session | Yes, from host files | — | **High-value (H5)**, gated on B1; benchmarks capture API usage directly in the meantime |
| Dated price table, cost labelled as estimate | Yes | — | With H5 |
| Budget warnings | Yes (`additionalContext`, stderr) | Advisory | Optional, after measurement shows need |
| Budget stop at checkpoints | Yes (`PreToolUse` deny, `UserPromptSubmit` block, `continue:false`) | Checkpoint-enforced, overshoot possible | Optional, opt-in only |
| Hard budget cap | No | — | Not recommended inside Rekora; document provider limits / proxy instead |
| Shared budget across agents | Yes (SQLite leases) | Advisory | Optional |
| Semantic answer cache | Technically | — | **Not recommended** |
| Deterministic content-hash cache | Already for the code index | — | Keep; no new general cache |
| Model routing | No (does not own requests) | — | **Not recommended** |
| Loop detection | Yes (hook-side) | Advisory | Optional, evidence-gated |

## Sources

| Title | URL | Date | Class |
|---|---|---|---|
| LiteLLM budgets and rate limits | https://docs.litellm.ai/docs/proxy/users | accessed 2026-11-24 | DOC |
| LiteLLM model cost map | https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json | main 2026-11-24 | OSS |
| Claude Code statusline | https://code.claude.com/docs/en/statusline | accessed 2026-11-24 | DOC |
| Claude Code monitoring (OTel) | https://code.claude.com/docs/en/monitoring-usage | accessed 2026-11-24 | DOC |
| Claude Code hooks reference | https://code.claude.com/docs/en/hooks | accessed 2026-11-24 | DOC |
| Claude Code prompt caching | https://code.claude.com/docs/en/prompt-caching | accessed 2026-11-24 | DOC |
| Claude Code model configuration | https://code.claude.com/docs/en/model-config | accessed 2026-11-24 | DOC |
| Agent SDK cost tracking | https://code.claude.com/docs/en/agent-sdk/cost-tracking | accessed 2026-11-24 | DOC |
| Claude API prompt caching | https://platform.claude.com/docs/en/build-with-claude/prompt-caching | accessed 2026-11-24 | DOC |
| Claude API rate limits | https://platform.claude.com/docs/en/api/rate-limits | accessed 2026-11-24 | DOC |
| Claude API usage and cost API | https://platform.claude.com/docs/en/manage-claude/usage-cost-api | accessed 2026-11-24 | DOC |
| OpenAI prompt caching | https://developers.openai.com/api/docs/guides/prompt-caching | accessed 2026-11-24 | DOC |
| Gemini generateContent (UsageMetadata) | https://aistudio.google.com/api-reference/generate-content | accessed 2026-11-24 | DOC |
| DeepSeek context caching | https://api-docs.deepseek.com/guides/kv_cache | accessed 2026-11-24 | DOC |
| Codex hooks | https://learn.chatgpt.com/docs/hooks | accessed 2026-11-24 | DOC |
| Codex advanced config (OTel) | https://learn.chatgpt.com/docs/config-file/config-advanced | accessed 2026-11-24 | DOC |
| Codex `protocol.rs` (TokenUsage) | https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs | main 2026-11-24 | OSS |
| Codex session file schema (community) | https://lin-guanguo.github.io/llm-memory-research/agent-cli/codex-session-file-schema/ | 2025-12-17 | CI |
| OpenCode troubleshooting (DB path) | https://opencode.ai/docs/troubleshooting/ | accessed 2026-11-24 | DOC |
| OpenCode session usage source | https://github.com/anomalyco/opencode/blob/dev/packages/core/src/session/usage.ts | dev 2026-11-24 | OSS |
| vCache | https://arxiv.org/abs/2502.03771 | 2025-02 (v5 2026-02) | PR (ICLR 2026) |
| Closing the Operational Gap in Semantic Caching | https://arxiv.org/abs/2606.19719 | 2026-06 | PR (EMNLP 2026 Industry) |
| MeanCache | https://arxiv.org/abs/2403.02694 | 2024 | PP |
| Redis LangCache concepts | https://redis.io/docs/latest/develop/ai/context-engine/langcache/concepts/ | accessed 2026-11-24 | DOC |
| RouteLLM | https://arxiv.org/abs/2406.18665 | 2024-06 (v4 2025-02) | PR (ICLR 2025) |
| RouteLLM blog | https://lmsys.org/blog/2024-07-01-routellm/ | 2024-07-01 | VR |
| FrugalGPT | https://arxiv.org/abs/2305.05176 | 2023-05; TMLR 2024 | PR |
| OpenRouter Auto Router | https://openrouter.ai/docs/guides/routing/routers/auto-router | accessed 2026-11-24 | DOC/VR |
| MAST | https://arxiv.org/abs/2503.13657 | 2025-03 (v3 2025-10) | PR (NeurIPS 2025 D&B) |
| OpenHands StuckDetector | https://github.com/OpenHands/software-agent-sdk/blob/main/openhands-sdk/openhands/sdk/conversation/stuck_detector.py | main 2026-11-24 | OSS |
| LangGraph recursion limit | https://github.com/langchain-ai/langgraph/blob/main/libs/langgraph/langgraph/_internal/_config.py | main 2026-11-24 | OSS |
| Budget-check race condition (community) | https://dev.to/amavashev/your-ai-agent-budget-check-has-a-race-condition-33ei | n/d | CI |

Not verified: OpenAI Chat Completions cached-token field on today's page; OpenAI usage/cost API fields
and rate-limit headers; DeepSeek cache prices from the pricing page; Codex rollout path in official docs;
Codex turn limits; MAST per-mode percentages; DeepSeek Harness usage storage.
