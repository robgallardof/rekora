# Context engineering findings

Research date: 2026-11-24. Research only: nothing here is implemented. Part of the agent-efficiency
investigation summarised in [AGENT_EFFICIENCY_RESEARCH.md](AGENT_EFFICIENCY_RESEARCH.md).

Evidence classes: **PR** peer-reviewed · **PP** preprint · **DOC** official technical documentation ·
**OSS** open-source implementation · **VR** vendor-reported · **CI** community implementation ·
**HYP** experimental hypothesis (our inference, untested). **NOT VERIFIED** = could not be confirmed
from a primary source during the investigation. No number in this file was reproduced by us.

A caveat that applies to almost every result below: compaction and trajectory-reduction studies use
a handful of benchmarks (SWE-bench Verified, Multi-SWE-bench Flash, Terminal-Bench, a few hundred
tasks at most), 1–5 models and usually one run per configuration. Differences of 1–3 solve-rate
points are inside run-to-run noise in most of these setups.

---

## 1. The single most important fact: caching is an exact prefix match

All four providers that matter to Rekora's hosts cache prompts by exact prefix (DOC, accessed
2026-11-24; prices change often, re-check before relying on them):

| Provider | Read price vs base input | Write price | Minimum prefix | Usage fields |
|---|---|---|---|---|
| Anthropic | 0.1× (0.05× Opus/Sonnet 5.5; 0.025× some newer models) | 1.25× (5 min TTL), 2× (1 h TTL) | 512–4,096 tokens by model; shorter prompts silently uncached | `input_tokens` (after last breakpoint), `cache_creation_input_tokens`, `cache_read_input_tokens` |
| OpenAI | 0.1× (0.05× some newest) | free on older models; 1.25× (30 min) / 2.5× (12 h) on GPT-5.6+ | 1,024 tokens | `input_tokens_details.cached_tokens`, `cache_write_tokens` |
| Google Gemini | discount not stated on current page (**NOT VERIFIED**) | explicit cache has storage charge | 2,048–4,096 tokens | `usageMetadata.cachedContentTokenCount` |
| DeepSeek | ≈2–3 % of miss price (current price page) | none listed | best effort, disk based | `prompt_cache_hit_tokens`, `prompt_cache_miss_tokens` |

Any rewrite at position *p* (a summary, masking an old tool output, clearing, a tool-set change, a
timestamp in the system prompt, non-deterministic JSON key order) makes everything after *p* uncached
on the next call and, on providers with a write premium, pays the premium again. OpenAI's guide
explicitly lists "compaction or truncation" and tool or reasoning-effort changes as cache breakers.
Claude Code's own team describes cache hit rate as a production SLO (VR, "Prompt caching is
everything", 2026-04-30) and Manus calls KV-cache hit rate "the single most important metric"
(VR, 2025-07-18).

**Consequence for every efficiency idea in this investigation:** token counts are the wrong unit.
The right unit is *uncached input + cache writes × premium + cache reads × discount + output*.

## 2. Trajectory reduction and observation masking

### AgentDiet — PR (FSE 2026), artifacts released

Xiao et al., "Reducing Cost of LLM Agents with Trajectory Reduction", arXiv 2509.23586,
Proc. ACM Softw. Eng. 3(FSE) 2026. A separate reflection LLM (GPT-5 mini) rewrites one *recent* step
(`s−2`) after each agent step, removing **useless** (caches, verbose logs), **redundant** (file content
echoed in edit calls) and **expired** (search results superseded by opening the file) content, only
when it saves more than θ = 500 tokens.

| Item | Value (authors' claims) |
|---|---|
| Setup | Trae Agent; Claude 4 Sonnet and Gemini 2.5 Pro; SWE-bench Verified (200 instances) and Multi-SWE-bench Flash (300 instances, 7 languages); step limits 50/100 |
| Input tokens | −39.9 % to −59.7 % |
| Total cost incl. reflection | −21.1 % to −35.9 % (reflection itself 5.5–11.8 % of original cost) |
| Pass rate | −1.0 to +2.0 points |
| Why cost falls less than tokens | the authors attribute it to cache invalidation; they edit only step `s−2` so the older prefix stays cached |
| Limitations | one agent scaffold; one run per configuration; latency not measured; ~US$2,000 budget; tuning on 100 SWE-bench Verified instances |

### The Complexity Trap — PP (NeurIPS 2025 DL4C workshop), code released

Lindenbauer et al., arXiv 2508.21433. SWE-agent on SWE-bench Verified, n = 500 per configuration, five
model configurations. Masking old observations (window of 10) matched LLM summarisation on solve rate
and both roughly halved cost; masking was cheapest in 4/5 configurations. Summaries made trajectories
about 15 % longer (Qwen3-Coder). Observations were ~84 % of an average turn's tokens. The masking
window had to be retuned for OpenHands (M = 58): **window size is scaffold-dependent**. Only Gemini
costs used real cache pricing; rolling masks change history every turn and themselves break caching
(SWE-agent docs warn about this and offer `polling`).

### TurnCost — PP (arXiv 2610.15093; P. Kumar and R. K. Pandey)

600 public coding-agent trajectories (2 scaffolds × 2 models): **98.6 % of input tokens were served from
the prefix cache**, median observation was only 118–286 tokens, flat pricing overstated loop cost about
7×, and **turn count explained 84 % of the variance of log cost versus 4 % for mean observation size**.
The authors do not show that turns can be cut without hurting success. This is the strongest
evidence that, under caching, *the number of model calls* — not observation size — is the main cost
lever. It shares an author with the "Prefix Stability" poster cited in §4, so the two are not
independent evidence.

### "Benchmarks Are Surprisingly Compaction-Friendly" — PP (arXiv 2610.31778)

OpenCode scaffold, 5 strategies, 3 frontier models. No strategy beat default self-summarisation;
aggressive discarding cost up to 20 points. **The first summary dropped about a third of the
anti-cheating rules, worsening with each compaction; process instructions ("how to work / what to
avoid") were dropped about 6× more often than specifications of what to build.** Benchmarks look
compaction-friendly because the goal is fixed and progress lives on disk.

This is the finding most directly relevant to Rekora: constraints and decisions are exactly what a
lossy host summary loses, and exactly what Rekora stores durably with sources.

### Other work (abstract level only, not reproduced)

| Work | Class | Claim |
|---|---|---|
| ACON (arXiv 2510.00615) | PR (ICML 2026 per listing) | Optimised compression guidelines; −26–54 % peak tokens on AppWorld/OfficeBench; not coding-specific |
| MEM1 (arXiv 2506.15841) | PP | RL-trained constant-size state; needs model training — not applicable to hosted agents |
| SWE-Pruner (arXiv 2601.16746) | PP | 0.6B goal-conditioned line selector; −23–54 % tokens on SWE-bench Verified |
| TokenPilot (arXiv 2606.17016) | PR (EMNLP 2026 Findings per listing) | Ingestion-time filtering + batch eviction schedule to avoid cache invalidation; −56–87 % cost on non-coding benchmarks |
| CliffCompaction (arXiv 2609.26779) | PP | Drop/truncate only, never rephrase, never compact a compaction; up to −50 % cost on Terminal-Bench; ships as an API proxy |
| OpenHands condenser blog | VR | "Up to 2× per-turn cost reduction", 54 % vs 53 % solve on an unspecified subset |

## 3. CompactionBench and APEC

**Found.** Liu, Qiu, Goiri, Zhou, Fonseca, Bianchini, Lai, Choukse (UIUC / Microsoft), "Benchmarking
Long-Horizon Agent Context Compaction with CompactionBench", Preprints.org
doi 10.20944/preprints202611.0780.v1, November 2026. **PP, no code or benchmark release found** — not
reproducible yet. (Unrelated project with a similar name: CompactBench, compactbench.github.io.)

Production analysis of GitHub Copilot agent sessions (June 2026; 13.5 M sessions, 760.5 M LLM calls):

- compaction occurs in **7.8 % of sessions, which consume 44.2 % of all tokens**;
- median compaction takes ~22 % of the turn's time and drops 72.9 % of prompt tokens;
- **median cache hit rate falls from 99.0 % before compaction to 27.0 % on the first call after**;
- trigger thresholds cluster at ~50 %, 65–66 %, 80 % and ~100 % of the window;
- more than ten frontier coding benchmarks barely trigger a single compaction, so most published
  compaction evaluations do not exercise repeated compaction at all.

**APEC = Asynchronous Progressive Compaction** (same paper). The context is
`[system, append-only immutable segment summaries, raw tail]`. At a soft threshold (50 %) the oldest raw
segment is summarised in the background while the agent continues; the summary is *appended* to the
immutable summary prefix so earlier summaries stay cached; a hard threshold (90 %) triggers one
synchronous full compaction that starts a new epoch.

| GPT-5.6 Sol, 8 tasks × 5 trials | Sync 90 % | Sync 50 % | Async 50 % | Prune 90 % | PEC (progressive, blocking) | APEC |
|---|---|---|---|---|---|---|
| Accuracy % | 69.29 | 73.04 | 70.70 | 72.40 | 76.74 | **77.95** |
| Total cost $ | 21.93 | 18.54 | 18.30 | 20.80 | **14.75** | 16.37 |
| Time min | 29.25 | 30.77 | 28.99 | **25.55** | 29.89 | 27.82 |
| Post-compaction cache hit | ~7 % | ~8 % | ~9 % | ~8 % | 33 % | 31 % |

Critical reading: the **append-only progressive** part (PEC) carries the cost win — PEC is the cheapest;
asynchrony mainly buys latency and costs some money (concurrent requests, wasted summaries). Prune-H
(rule-based dedupe of file reads and formatting bloat, no LLM) was fastest. Sample is 8 tasks × 5 trials
on one or two models, and accuracy varies 68–86 % across configuration settings. On GPT-5.4 the cost
difference nearly vanished ($40.64 vs $41.06).

## 4. Practitioner and host evidence

| Source | Class | What matters |
|---|---|---|
| "Don't Break the Cache" (arXiv 2601.06007) | PP | 40 sessions/model/condition; caching saved 38–81 % cost; system-prompt-only caching was most consistent; dynamic tool definitions and pruning old tool calls break the cache |
| "Prefix Stability for Prompt Caching" (NeurIPS 2026 affinity poster) | PP | Full-rate input cost Θ(N²) without caching, Θ(N) with an invariant prefix; a phase-switched system prompt made caching **+24.9 %** more expensive than no caching |
| "Silent and Costly" (arXiv 2610.34817) | PR (ICSE 2027 per listing) | 32 caching mis-integrations in 571 agent projects; most fail silently |
| Claude Code prompt-caching and cost docs | DOC | Compaction invalidates the conversation *and* the CLAUDE.md layer; compaction is cheap while the cache is warm and most expensive on a cold resumed session; compact at natural breaks, not mid-task; `/usage` reports expected rebuilds |
| Claude Code "Prompt caching is everything" | VR | Static → dynamic layout; state updates as appended reminders, not system-prompt edits; never add/remove tools mid-session; compaction request reuses the parent prefix (~1/10 of an uncached call) |
| Anthropic API context management | DOC | `clear_tool_uses` with `clear_at_least` (exists *because* each clear costs a cache write); server-side compaction including background compaction (beta) |
| OpenAI compaction | DOC | `compact_threshold` / `/responses/compact` returning an opaque encrypted item |
| Anthropic "Effective context engineering" | VR | Claude Code compaction keeps decisions, unresolved bugs, details and the 5 most recently read files; structured note-taking outside the window |
| Factory "Compressing context" | VR | Anchored incremental summaries; preserve intent, artifact trail, breadcrumbs; no quantitative evaluation |
| Manus "Context engineering" | VR | Stable prefix, append-only context, deterministic serialisation, restorable compression (drop content, keep path/URL), recite goals at the end |
| Codex `model_auto_compact_token_limit` | DOC (existence) / CI (defaults) | Default threshold and "90 % clamp": **NOT VERIFIED** |

## 5. Context freshness and context rot

- **Lost in the Middle** (Liu et al., TACL 2024, PR): accuracy is highest when relevant information is at
  the start or end of the context.
- **Chroma "Context Rot"** (2025-07-14, VR technical report, code released): performance fell with input
  length in every experiment across 18 models; distractors accelerate the decline; no position effect in
  their NIAH variant, so position effects are task-dependent; length is confounded with difficulty.

Shorter, relevant context improves quality independently of cost. This is the strongest argument for
Rekora's existing model — small, budgeted, retrieval-on-demand packs with source labels — over bulk loading.

## 6. Tool schemas and repeated instructions

| Source | Class | Finding |
|---|---|---|
| Anthropic "Advanced tool use" (2025-11-24) | VR | Tool search + `defer_loading`: a 58-tool setup ~77K → ~8.7K tokens; MCP-eval accuracy 49 → 74 % (Opus 4); little benefit under ~10 tools; internal evals |
| Claude Code MCP docs | DOC | Tool search on by default: only names + server instructions load at start; tool list fixed from the first request; MCP output warning at 10K tokens, default max 25K; outputs >50,000 chars spilled to a file |
| RAG-MCP (arXiv 2505.03275) | PP | Retrieve tool descriptions first; >50 % prompt-token reduction; tool selection 13.6 → 43.1 % on a stress test (tool count NOT VERIFIED) |
| "MCP Tool Descriptions Are Smelly!" (arXiv 2602.14878) | PP | 97 % of 856 tools have description smells; richer descriptions +5.85 pp success but +67 % steps; compact variants kept reliability |
| Anthropic "Code execution with MCP" | VR | 150K → 2K token example is illustrative, not a measurement |
| Community counts | CI, NOT VERIFIED | Nine servers ≈ 38,900 tokens of definitions; JSON Schema dominates |

Rekora's own position: 9 tools, deterministic schemas, fixed server instructions. Under Claude Code's
default tool search only names load, so Rekora's schema cost is already small there; on hosts without
deferred loading it is paid every session. **Rekora's actual tool-definition token cost has not been
measured: NOT EXECUTED** (see BENCHMARK_DESIGN.md, B0).

---

## 7. Answers

### Q1. Is context compression actually cheaper under modern provider prefix caching?

**Usually cheaper in total, by much less than the token reduction suggests, and sometimes more
expensive.** With 0.1× (or lower) cached reads and ~98 % cache hit rates in public coding trajectories
(TurnCost), a 50 % token cut does not give a 50 % cost cut: AgentDiet's 40–60 % token reduction became
21–36 % cost reduction. Every rewrite pays a re-write of the suffix (with premium), the summariser call,
and possible behavioural costs: longer trajectories (+15 %, Complexity Trap) and lost constraints (about a
third of anti-cheating rules on the first summary in one preprint on research-replication benchmarks, Compaction-Friendly; transfer to coding constraints untested). Compression remains worthwhile for long
sessions (cached reads still scale with context × turns; context rot; finite windows — compacting
sessions are 44 % of Copilot's tokens). It loses when it rewrites the prefix often (+24.9 % measured for
a phase-switched prompt). Evidence strength: mechanism DOC; magnitudes from one PR and several PP.

### Q2. When should context be compacted?

At natural task boundaries, while the cache is still warm (Claude Code DOC), in batches that each save a
minimum amount (AgentDiet θ, Anthropic `clear_at_least`, SWE-agent `polling`), and before quality degrades
rather than only at the hard limit (context rot). Production thresholds cluster at 50–90 % and the best
APEC configuration used 50 % soft / 90 % hard; thresholds are model- and window-specific. **For Rekora this
is a host decision** — Rekora does not own the context. What Rekora can do is make compaction *safer*
(constraints re-injected from a durable store) and *observable* (record when it happened and what it
cost).

### Q3. Can asynchronous progressive compaction improve cost and latency?

Plausibly yes, with **one preprint** as evidence (APEC: −25 % cost vs synchronous at 90 %, −9.6 % time vs
synchronous at 50 %, 8 tasks × 5 trials, no code). The append-only design carries the cost benefit,
asynchrony carries the latency benefit. Anthropic now documents background compaction server-side.
This is a host/provider capability. Rekora cannot implement it without becoming a proxy or owning the
model request, which this investigation rejects.

### Q4. Which trajectory elements can be safely removed?

Most to least safe: old raw tool outputs once acted upon (keep the call and the reasoning); redundant
content (duplicate file reads, content echoed in edits); expired content (superseded searches); useless
content (caches, verbose logs beyond the failing part); old thinking blocks. **Must not be removed:**
the objective, user constraints and process rules, decisions, unresolved bugs, the artifact trail (files
changed, tests run and their results), and breadcrumbs (paths, symbols, URLs). Removal is safest when it is
**restorable** — the content lives elsewhere and is referenced by id or path. Under caching, observation
trimming matters mainly in the heavy tail (TurnCost).

### Q5. How can tool schemas and repeated instructions be reduced?

Defer tool definitions (tool search; Claude Code does this by default); keep the tool list stable for a
session; keep descriptions short but with a clear purpose; keep stable instructions once in the cached
prefix and deliver dynamic state as appended messages; move workflow detail into on-demand skills; keep
CLAUDE.md/AGENTS.md short. **Re-inject critical constraints after compaction from a durable source**,
because summaries lose process rules disproportionately (one preprint). Of these, only the last is Rekora's job; the rest
are host or user configuration.

## 8. Implications for Rekora (HYP — untested)

1. **Rekora's value in context engineering is fidelity, not compression.** It should be the restorable
   store that makes host compaction less lossy: after a compaction, re-inject the active task's
   constraints, decisions and next step as labelled data (Claude Code `SessionStart` `source=compact`),
   and use Claude `PreCompact` stdout only for a fixed template that names the task id — never stored
   agent text, which would turn data into instructions (rule 6). Claude Code already re-reads
   CLAUDE.md after compaction, so constraints written there survive; Rekora's added value is for
   constraints that emerge during the session.
2. **Do not build a summariser, a trajectory rewriter or a compaction proxy.** All of them either need an
   LLM call per event, require owning the request (a proxy), or rewrite the prefix the host has cached.
3. **Be cache-neutral.** Keep Rekora's MCP tool list, descriptions and server instructions static;
   keep `context` output deterministic for identical input (no timestamps or counters in the header).
   Tool results are appended content and therefore cache-safe.
4. **Measure with cached and uncached input separated**, or not at all.

## Sources

| Title | URL | Date | Class |
|---|---|---|---|
| Reducing Cost of LLM Agents with Trajectory Reduction (AgentDiet) | https://arxiv.org/abs/2509.23586 | 2025-09; FSE 2026 | PR, artifacts released |
| The Complexity Trap | https://arxiv.org/abs/2508.21433 | 2025-08 (v3 2025-10) | PP, code released |
| CompactionBench / APEC | https://www.microsoft.com/en-us/research/publication/benchmarking-long-horizon-agent-context-compaction-with-compactionbench ; https://dx.doi.org/10.20944/preprints202611.0780.v1 | 2026-11 | PP, no code |
| TurnCost | https://arxiv.org/abs/2610.15093 | 2026-10 | PP |
| Benchmarks Are Surprisingly Compaction-Friendly | https://arxiv.org/abs/2610.31778 | 2026-10 | PP |
| CliffCompaction | https://arxiv.org/abs/2609.26779 | 2026-09 | PP |
| ACON | https://arxiv.org/abs/2510.00615 | 2025-10 | PR (per listing) |
| MEM1 | https://arxiv.org/abs/2506.15841 | 2025-06 | PP |
| SWE-Pruner | https://arxiv.org/abs/2601.16746 | 2026-01 | PP |
| TokenPilot | https://arxiv.org/abs/2606.17016 | 2026-06 | PR (per listing) |
| Sleep-time Compute (Letta) | https://arxiv.org/abs/2504.13171 | 2025-04 | PP |
| Don't Break the Cache | https://arxiv.org/abs/2601.06007 | 2026-01 | PP |
| Prefix Stability for Prompt Caching in LLM Agents | https://neurips.cc/virtual/2026/162685 | 2026 | PP (poster) |
| Silent and Costly | https://arxiv.org/abs/2610.34817 | 2026-10 | PR (per listing) |
| Lost in the Middle | https://arxiv.org/abs/2307.03172 | TACL 2024 | PR |
| Context Rot | https://www.trychroma.com/research/context-rot | 2025-07-14 | VR, code released |
| RAG-MCP | https://arxiv.org/abs/2505.03275 | 2025-05 | PP |
| MCP Tool Descriptions Are Smelly! | https://arxiv.org/abs/2602.14878 | 2026-02 | PP |
| Anthropic prompt caching | https://platform.claude.com/docs/en/build-with-claude/prompt-caching | accessed 2026-11-24 | DOC |
| Anthropic context editing | https://platform.claude.com/docs/en/build-with-claude/context-editing | accessed 2026-11-24 | DOC |
| Anthropic compaction | https://platform.claude.com/docs/en/build-with-claude/compaction | accessed 2026-11-24 | DOC |
| OpenAI prompt caching | https://developers.openai.com/api/docs/guides/prompt-caching | accessed 2026-11-24 | DOC |
| OpenAI compaction | https://developers.openai.com/api/docs/guides/compaction | accessed 2026-11-24 | DOC |
| Gemini caching | https://aistudio.google.com/docs/caching | accessed 2026-11-24 | DOC |
| DeepSeek context caching | https://api-docs.deepseek.com/guides/kv_cache | accessed 2026-11-24 | DOC |
| Claude Code prompt caching | https://code.claude.com/docs/en/prompt-caching | accessed 2026-11-24 | DOC |
| Claude Code costs | https://code.claude.com/docs/en/costs | accessed 2026-11-24 | DOC |
| Claude Code MCP | https://code.claude.com/docs/en/mcp | accessed 2026-11-24 | DOC |
| Prompt caching is everything (Claude Code) | https://claude.dev/blog/lessons-from-building-claude-code-prompt-caching-is-everything/ | 2026-04-30 | VR |
| Effective context engineering for AI agents | https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents | 2025-09-29 | VR |
| Introducing advanced tool use | https://www.anthropic.com/engineering/advanced-tool-use | 2025-11-24 | VR |
| Code execution with MCP | https://www.anthropic.com/engineering/code-execution-with-mcp | 2025-11-04 | VR |
| Context Engineering for AI Agents (Manus) | https://manus.im/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus | 2025-07-18 | VR |
| Compressing context (Factory) | https://factory.com/news/compressing-context | 2025-07-21 | VR |
| OpenHands condenser | https://docs.openhands.dev/sdk/guides/context-condenser | 2025 | OSS / VR |
| SWE-agent history processors | https://swe-agent.com/latest/reference/history_processor_config/ | accessed 2026-11-24 | OSS |
| Codex compaction (community) | https://github.com/openai/codex/issues/4106 | 2026 | CI, NOT VERIFIED |
