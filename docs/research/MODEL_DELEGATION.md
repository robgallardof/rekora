# Model delegation, cost estimates and prices

Research date: 2026-11-24. This file backs `rekora-core::effort`, `rekora-core::usage`,
`rekora-cli::agents` and `rekora statusline`. The three reports below were produced by independent
researcher agents from official documentation and papers; each claim carries an evidence class and
**NOT VERIFIED** marks what could not be confirmed. No host was driven by a model to verify delegation
end to end (**NOT EXECUTED**).

## What Rekora implements, and why this way

| Finding | Evidence | What Rekora does |
|---|---|---|
| Switching models turn by turn inside a task rarely pays off (TwinRouterBench, Scrouting, TurnCost; LiteLLM's keyword router barely beat random) and every model switch rebuilds the prompt cache | PP + DOC | Never routes requests. It *suggests* a tier per task (`light`/`standard`/`deep`) |
| Starting cheap and escalating on evidence beats classifying up front (SWE-Router: 0.780 vs 0.627 Route-AUC) | PP | The suggestion starts at `light` and rises only for recorded failures, breadth of files, remaining work; objective words raise it one step at most |
| Every major host lets a subagent definition pin a model, and enforces it | DOC | `rekora agent install claude-agents\|codex-agents\|antigravity-agents` writes `rekora-light`, `rekora-standard`, `rekora-deep` pinned to host models; delegating to a subagent keeps the main session's cache intact |
| Codex lists its models in `~/.codex/models_cache.json`; families Luna < Terra < Sol < Astra by price | OSS (observed) + DOC (prices) | Codex tier agents pick the newest listed Luna / Sol / Astra model |
| Hosts record per-response usage in their own session files | OSS (observed) | The Stop hook reads it, `rekora usage` reports it per task and session, `rekora statusline` shows it in Claude Code |
| Subscriptions are not billed per token | DOC | Every figure is labelled "API est." with the price date |

Tier mapping (pinned in the agent files; users can edit or remove them):

| Tier | Claude Code | Codex | Antigravity |
|---|---|---|---|
| light | `haiku`, effort low | newest `*-luna`, reasoning low | `flash` |
| standard | `sonnet`, effort medium | newest `*-sol`, reasoning medium | `flash` |
| deep | `opus`, effort high | newest `*-astra`, reasoning high | `pro` |

Known limits: long-context price tiers are not modelled (under-estimates those requests), DeepSeek is
priced at peak (over-estimates off-peak), Gemini 4 Carbon/Argon prices double on 2026-12-01 and are not
updated yet, Claude transcripts are an undocumented format. The suggestion is advisory: the main agent
decides whether to delegate.

## Complexity-based model routing and delegation for coding agents

Research date: 2026-11-24. Scope: whether and how Rekora could help coding agents send work to cheaper
models and save the top ("pro") model for the cases that need it. This builds on
`docs/research/TOKEN_ECONOMICS.md` §7 (RouteLLM, FrugalGPT, cache invalidation on model switch). No
repository files were changed.

**Evidence classes:** PR = peer-reviewed; PP = preprint; DOC = official documentation; OSS =
open-source implementation; VR = vendor-reported; COM = community; HYP = hypothesis or own arithmetic.
"NOT VERIFIED" means I could not confirm the claim from a primary source during this session.

---

### 0. Executive summary

1. **The market has settled on four mechanisms.** All four are owned by the host or the gateway, not by a
   memory layer:
   - **(a) Phase split.** A strong planner hands off to a cheaper executor: Claude Code `opusplan`, Cline
     Plan/Act, Aider architect/editor, Antigravity Orchestrator, Factory Spec-mode model.
   - **(b) Escalate on demand.** A cheap executor consults a strong model mid-task: Anthropic advisor tool,
     Amp/OpenRouter "oracle"/advisor.
   - **(c) Delegated subtasks on cheaper models.** Claude Code subagent `model:`, Roo/Kilo
     orchestrator modes, CCR `<CCR-SUBAGENT-MODEL>`.
   - **(d) Per-request automatic routers.** OpenRouter Auto, Cursor Router (Auto), Not Diamond Code,
     LiteLLM complexity/Switchyard, Devin/Windsurf Adaptive, Kilo Auto.
2. **Evidence for (a) and (b) on coding is moderate and positive.** It is vendor-run, but the
   numbers come with setups. Examples: Aider R1+Sonnet scored 64.0 % at $13.29, against o1's 61.7 % at $186.5.
   Anthropic reports Sonnet with an Opus advisor at +2.7 pp on SWE-bench Multilingual and −11.9 % cost.
3. **Evidence for (d), per-request routing inside agentic trajectories, is weak or negative.**
   - TwinRouterBench (PP): on its static SWE-bench slice *every* router had negative cost savings,
     "because one under-routed step fails the whole trajectory".
   - Scrouting (PP): a replay audit found "no learned router we tested exceeded the solve rate of
     always picking the strongest model". Its own cost win came from a scout handoff, not from the router.
   - TurnCost (PP): delegating turns to a cheaper model *raised* cost in half the settings evaluated.
4. **Escalate on failure, not classify up front.** The best agentic results use either a few cheap
   exploratory turns before deciding (SWE-Router, PP) or failure-triggered escalation (Switchyard, the
   Antigravity Orchestrator "return to planner on repeated failure", the advisor "when stuck on a recurring error").
   Classifying from the prompt alone did worse in SWE-Router: Route-AUC 0.627 vs 0.780 for the
   same weak/strong pair.
5. **Deterministic signals exist and are cheap.** LiteLLM ships a 7-dimension keyword/length scorer, but
   it lost to a cost-matched shuffle only marginally (41.5 % vs 39.2 % win rate) in its own benchmark.
   The strongest no-LLM predictors are *trajectory* signals (repeated identical failures, test failures,
   compaction) and *structural* signals (number of files/modules touched, repository scale).
6. **Implication for Rekora (HYP).** Rekora still should not proxy or route requests. It already stores
   the signals that the evidence says matter most: recorded failures, attempts, remaining items, files
   touched and the handoff after a cut-off. It could expose them as a deterministic **difficulty/escalation hint**
   that hosts use only at cache-safe boundaries: subagent model choice, the advisor, or a new session.

---

### 1. Existing tools: mechanism, signals, configuration, maturity

#### 1.1 Claude Code (Anthropic) — DOC

- **`opusplan` alias.** Per the docs: "uses `opus` during plan mode, then switches to `sonnet` for
  execution." The only signal is the user entering plan mode. Configuration: `/model opusplan`, with
  `ANTHROPIC_DEFAULT_OPUS_MODEL` and `ANTHROPIC_DEFAULT_SONNET_MODEL` picking the two models.
  The cache cost is already recorded in TOKEN_ECONOMICS §7: each toggle "starts a fresh cache".
- **Subagent model tiers.** Set the frontmatter `model:` to `sonnet|opus|haiku|fable|<id>|inherit`. Precedence:
  per-invocation parameter > definition `model` > `CLAUDE_CODE_SUBAGENT_MODEL` > parent model.
  `CLAUDE_CODE_SUBAGENT_MODEL_FORCE=1` forces one model on all subagents.
  - The docs list "Control costs by routing tasks to faster, lower-cost models like Haiku" as a subagent
    benefit.
  - Built-in Explore *inherits* the parent model, so it does not use Haiku by default. The docs say to override it
    with a user agent named `Explore` with `model: haiku`.
  - Subagents also take an `effort` field (`low`…`max`).
  - Cache: a subagent starts its own cache, and the parent's cache is unaffected (TOKEN_ECONOMICS §7).
- **Advisor tool (experimental).** The executor model calls a server-side `advisor` tool. The advisor reads
  the *full* transcript and returns guidance, typically 400–700 text tokens (Anthropic blog).
  - Signals: "model-driven rather than rule-based". In practice it consults "before committing to an approach,
    when an error keeps recurring, and before declaring a task done".
  - Configuration: `/advisor opus`, the `advisorModel` setting or `--advisor`. The advisor must rank at or above the
    main model. There is "no setting to cap or force advisor calls" in Claude Code. The API exposes
    `max_uses`.
  - **Cache: toggling the advisor "does not invalidate your main model's prompt cache".** The
    advisor's own read is not cached; each call re-processes the full transcript.
  - This is the only escalation mechanism found that is explicitly cache-neutral for the main loop.
- **Effort levels** are a second, cheaper dial. Effort is "adaptive reasoning … based on task complexity". Current
  defaults are `medium` for Opus/Sonnet/Haiku 5.5. Changing effort is not a model switch. Whether it
  preserves the cache is NOT VERIFIED here, but OpenRouter's Auto Router docs say it *prefers adjusting
  effort* over switching models because a switch rebuilds the cache.

#### 1.2 claude-code-router (musistudio/CCR) — OSS, MIT

- **Scale.** 38.4k GitHub stars, and v3.1.6 is the current version (read from the GitHub page on 2026-11-24).
  The project now also ships a desktop app and supports Codex and other agent profiles.
- **Mechanism.** CCR is a local proxy that Claude Code points at through `ANTHROPIC_BASE_URL`. A `Router` block maps
  request classes to `provider,model`:
  - `default`
  - `background`: Haiku-class requests that Claude Code sends for housekeeping
  - `think`: requests with thinking enabled
  - `longContext` above `longContextThreshold` (default 60 000 tokens per a mirrored `router.ts`)
  - `webSearch`
  - `image`
- **Subagent routing.** A subagent prompt can start with `<CCR-SUBAGENT-MODEL>provider,model</CCR-SUBAGENT-MODEL>`.
  CCR strips the tag and routes the request. `CUSTOM_ROUTER_PATH` loads a JS function that returns a model
  per request. Version 3 adds header/body conditions, rewrites, retries and ordered fallbacks.
- **Signals.** Request shape only: which model was requested, the thinking flag, token count, tools. There is no
  task-complexity model.
- **Evidence.** Key names come from the official docs (now moved to ccrdesk.top) and from mirrors of the
  router source. I could not open the current official routing page (404/redirect), so exact current key
  names are NOT VERIFIED for v3. A known pitfall (COM): `env` values in `settings.json` override what CCR
  injects, which silently bypasses the router.

#### 1.3 Aider architect/editor — OSS, Apache-2.0

The architect model writes a solution description and the editor model turns it into file edits. Configuration:
`--architect --model X --editor-model Y`. The signal is the user's choice; there is no automatic split. Results are in §2.3.

#### 1.4 Cline Plan/Act — DOC, OSS (Apache-2.0)

Turning on "Use different models for Plan and Act" sets a model per mode. Plan is read-only. The signal is the
user's mode toggle. Cline's blog (VR) says Opus 4.1 → Sonnet 4 was the most common cross-mode pairing,
at 25.3 % of cross-mode usage.

#### 1.5 Roo Code and Kilo Code

- **Roo Code: shut down on 2026-05-15 and repository archived** (COM, consistent across several reports, including
  the archived README pointing to Cline and Zoo Code). Features while it was alive (DOC):
  - "Sticky models": each mode remembers its last API configuration profile.
  - A built-in **Orchestrator ("Boomerang") mode** breaks work into subtasks for other modes, each with its own model.
  - The orchestrator itself cannot read or write files or run commands, to avoid "context poisoning".
- **Kilo Code** (OSS, maintained):
  - **Auto Model** maps the *mode* to a tier through the `x-kilocode-mode` header. In the gateway docs,
    plan/architect/orchestrator/debug/ask go to the frontier tier (an Opus model) and code/build go to an efficient tier.
  - Mappings are served from the Kilo API, so they change without a client release. Docs disagree on
    Opus 4.6 vs 4.7.
  - Kilo's own test (VR, 2026-07-27): one task, one run per setup, test written by the vendor. Auto
    cost $1.26, against $2.90 for GPT-5.6 Sol everywhere and $2.47 for Sonnet 5 everywhere. Logic checks were the same
    for all setups (29/30). This is anecdotal: n=1 with no variance.

#### 1.6 Antigravity (Google) — DOC

The **Orchestrator** option pairs a planner with an executor:
- Planner: Gemini 4 Neon (default), Carbon, Argon or Claude Opus 5.5.
- Executor: Gemini 4 Hydrogen (default) or Claude Sonnet 5.5.

Each message starts with a read-only planning phase. "If the executor hits trouble, such as a repeatedly
failing verification step or an unsettled design choice, control returns to the planner." This is a phase
split plus failure-triggered escalation, and Google recommends it as the default. Google also recommends Hydrogen alone for
"upgrades, boilerplate, or documentation". The docs say Orchestrator uses fewer tokens than the planner alone,
but give no numbers (no evaluation published). Forum users report that the model actually answering does not always match the selection
(COM, NOT VERIFIED).

#### 1.7 Amp (Sourcegraph/Amp Inc.) — VR/DOC

- **Rush mode (2025-11-07).** Rush ran on Haiku 4.5 and was "67 % cheaper and 50 % faster than `smart`"
  token by token.
  - On a small task it was 77 % cheaper. On a complex refactor it took about 2× longer and was only 19 % cheaper.
  - Amp's own warning: "It'll be slower and not much cheaper, if it even arrives at a solution at all."
  - This is the clearest vendor statement that weak models on complex agentic tasks lose their price advantage.
- **The dial (2026-07-09).** Amp replaced smart/rush/deep with a user-set **low/medium/high/ultra** dial:
  - low = GLM-5.2
  - medium = GPT-5.6 Sol at medium effort
  - high = Sol at xhigh effort
  - ultra = Fable 5
  - Every mode keeps an "oracle" (a second-opinion model).

  The rationale: "The dial asks one question: how hard is this task?" Amp chose a *human* up-front choice
  plus an oracle, not an automatic router.

#### 1.8 Factory (Droid) — DOC

Spec mode can use a different model and reasoning effort (`specModeModel`, `specModeReasoningEffort`). Custom
droids (subagents) each set their own model. There is a constraint: reasoning-enabled Anthropic models pair only with
Anthropic models, because encrypted reasoning traces must be readable by the continuing model. Mixing models
is therefore constrained not only by cache but also by reasoning-trace compatibility.

#### 1.9 Automatic per-request routers

| Product | Mechanism and signals | Cache handling | Evidence | Class |
|---|---|---|---|---|
| OpenRouter Auto (`openrouter/auto`) | "A fast decision model rates eleven aspects of the request" (task type, task changed, difficulty, speed need, codebase knowledge, more thinking would help). Later turns carry session signals such as repeated errors and failing tests | "Restrained about switching models, because a switch rebuilds the prompt cache"; prefers changing effort | No fee; points to its own router benchmarks; no setup in the docs | DOC/VR |
| Cursor Router (Auto) | "Runs a classifier on each agent request and routes it based on task type and complexity"; Cost/Balance/Intelligence modes | Not documented | "No published evaluation" | DOC |
| Not Diamond Code (2026-08-04) | Local proxy; per-turn prediction of the cost and reward of each model × effort from session state, token counts, complexity, implicit feedback and KV-cache state | Explicitly weighs breaking a warm cache | Claims Opus 4.8 xhigh parity at −39 % cost (Poly-SWE-bench) and −61 % (LongCodeQA); dataset sizes not stated; closed early access | VR |
| LiteLLM `complexity_router` | No-LLM heuristic over 7 weighted dimensions (§3.1) plus optional LLM classifier, keyword rules and Thompson-sampled tiers (v2) | Not documented | Own benchmark (§2.4) | OSS/DOC |
| LiteLLM + NVIDIA NeMo Switchyard (2026-11-06) | Routes "on the agent's tool trajectory rather than the prompt"; failures weighted by severity; "repeated identical failures and context compaction force an escalation"; a passing test after a failure counts as recovery; escalation holds until the trajectory stabilises; LLM judge only for unclear turns | Not mentioned | No numbers | OSS/DOC |
| Devin / Windsurf "Adaptive" | "Simple tasks routed to fast, efficient models, complex tasks to more capable ones"; candidate models not named | Not documented | None | DOC |
| Requesty Auto Routing | Classifies the *first* user message and keeps the route for 60 min (session stickiness) | Implicitly sticky | Vendor blog only; NOT VERIFIED | VR |
| Portkey conditional routing | Rule-based on metadata, parameters or path (for example, a header carrying task type) | User-controlled | No routing-quality claim | DOC/COM |
| Martian | Prompt-based "model mapping" router | — | NOT VERIFIED (only secondary pages found) | COM |

**Task-complexity MCP servers / Claude Code plugins.** "harness-router" is described as an MIT-licensed local
MCP server for routing coding tasks, but its page returned 404 (NOT VERIFIED). An HN "Show HN: smart model
routing directly in Claude, Codex and Cursor" thread exists (COM, not read). No mature, evaluated MCP
complexity classifier was found.

---

### 2. Evidence on complexity-based delegation

#### 2.1 Single-turn routing and cascades (background)

- **RouteLLM** (PR, ICLR 2025; Apache-2.0 code): more than 2× cheaper at about 95 % of GPT-4 quality on MT-Bench,
  but near random on MMLU without in-domain labels. It is a single-turn chat benchmark. See TOKEN_ECONOMICS §7.
- **FrugalGPT** (PR, TMLR 2024): an LLM cascade with a learned scorer; "up to 98 %" cost reduction at GPT-4
  parity. Evaluated on QA/classification with checkable answers.
- **Hybrid LLM** (PR, ICLR 2024; Ding et al., Microsoft): a router predicts query difficulty against a
  tunable quality target. It made "up to 40 % fewer calls to the large model with no drop in response quality". The
  setting is single-turn and edge/cloud; the dataset details were not re-read here.
- **AutoMix** (PR, NeurIPS 2024): few-shot *self-verification* by the small model plus a POMDP router. It reports
  "over 50 %" cost reduction at comparable performance on 5 datasets and 5 models. Its own caveat is that self-verification is
  noisy. That matters for Rekora rule 6: agent text "never self-verifies".

None of these four evaluates multi-step tool-using agents.

#### 2.2 Agentic and coding routing (2026 preprints)

| Paper | Setup | Result | Limitations |
|---|---|---|---|
| **SWE-Router** (PP, arXiv 2607.00053, ICML 2026 DL4Code workshop) | Cheap model runs K∈{1..4} exploratory turns; a Qwen2.5-Coder-7B LoRA value head reads the partial trajectory (failed tests, stack traces, files) and decides to continue or escalate. The strong model *restarts from the original prompt*, because conditioning on the weak trajectory "biased the strong model toward its mistakes". SWE-bench Verified; pairs gpt-5-mini or deepseek-v3.2 → gemini-3-pro (solo: 55.6 % / 62.4 % / 70.4 %; cost $16.6 / $22.6 / $391.3 for 500 instances) | Route-AUC 0.780 with K=2 vs 0.627 prompt-only (K=0) for the deepseek pair: **trajectory beats up-front classification** | Held-out set is only 100 instances; 2 pairs; does not beat baselines on SWE-Smith (distribution shift); no explicit savings figure |
| **TwinRouterBench** (PP, arXiv 2605.18859) | Static track: 970 prefixes from 520 instances (SWE-bench, BFCL, mtRAG, QMSum, PinchBench). Dynamic track: live routing on 100 held-out SWE-bench Verified cases, pool from Opus 4.6 down to deepseek-v3.2 | Dynamic: trained UncommonRoute 75/100 at $0.26/task vs Opus-only 74/100 at $0.55 (−53 %); **rule-based** UncommonRoute 73/100 at **$1.73** (3× *more* expensive). Static: on SWE-bench every router had negative savings, "because one under-routed step fails the whole trajectory" | n=100; one router family; read from v1 HTML (paper is at v5) |
| **Scrouting** (PP, arXiv 2608.04804 v2) | A 7B scout explores the repository and writes a sandbox-verified handoff; a router picks among Opus 4.6, GPT-5.2, Kimi K2.5 and Gemini 3 Flash. SWE-bench Pro Python, 266 tasks | 159/266 at $0.230/solve vs Opus 4.6 solo 158 at $1.274. **But** "always cheapest fixer (Kimi K2.5) + handoff" also reached 159 at $0.227, so the win came from the handoff, not the routing. Replay audit over three SWE benchmarks: "no learned router we tested exceeded the solve rate of always picking the strongest model" | Solo rows count API cost only; calibration n=99 is "directional" |
| **Agent-as-a-Router / ACRouter** (PP, arXiv 2606.22902) | CodeRouterBench, about 10K tasks with verified scores from 8 LLMs; routing as a Context→Action→Feedback loop with memory | Adding per-task-dimension performance statistics to an LLM router gave a +15.3 % relative gain over a heuristic router with the same priors | "Living" report; no absolute regret on the abstract page |
| **TurnCost** (PP, arXiv 2610.15093, 2026-10-10) | 600 public trajectories, 2 scaffolds, 2 models | Turn count explains 84 % of log-cost variance; 98.6 % of input tokens are served from cache; **delegating turns to a cheaper model raised cost in half the settings**, close to the closed-form break-even | Shares an author with Prefix Stability (not independent); models and pricing not on the abstract page |

**Reading (HYP).** Inside a trajectory the economics differ from single-turn routing. The expensive model's
cached prefix costs about 0.1× per input token, and switching rebuilds the whole prefix at the write price. A
failed cheap step can fail the whole task. Routing pays off at *task* granularity: pick the model for a whole
task or subtask, or restart it, which is what SWE-Router and Scrouting do. It does not pay off as a per-turn
switch on a warm cache.

#### 2.3 Measured model mixing in coding agents

- **Aider architect/editor (VR, 2024-09-26; aider's code-editing benchmark).**
  - Solo diff-format scores: o1-preview 79.7 %, Sonnet 3.5 77.4 %, GPT-4o 71.4 %.
  - o1-preview + deepseek (whole) scored 85.0 %, o1-preview + Sonnet 82.7 %, Sonnet + Sonnet 80.5 %, GPT-4o + GPT-4o 75.2 %.
  - Pairing a model *with itself* gained about 3 pp, so part of the gain is the two-pass structure, not the model mix.
  - The post itself says the top setup is "quite slow, so probably not practical for interactive use".
- **Aider R1 + Sonnet (VR, 2025-01-24; polyglot, 225 Exercism exercises).**
  - Architect/editor: R1+Sonnet 64.0 % at $13.29; o1 61.7 % at $186.5 ("14X less cost").
  - Solo: R1 56.9 % at $5.42; Sonnet 51.6 % at $14.41.
- **Aider leaderboard (VR, read 2026-11-24).** o3 (high) + gpt-4.1 scored 78.2 % at $17.55, against o3 (high) solo at 81.3 %
  for $21.23. That is −3.1 pp for −17 % cost, so mixing *lost* quality there.
- **Anthropic advisor (VR, 2026-04-09).**
  - SWE-bench Multilingual (300 problems × 5 trials, 9 languages): Sonnet 4.6 + Opus 4.6 advisor scored **+2.7 pp** at
    **−11.9 %** cost per task, compared with Sonnet solo.
  - BrowseComp (1,266 problems, 1 attempt each, not coding): Haiku 4.5 + Opus advisor scored 41.2 %, Haiku solo 19.7 %,
    still "29 %" below Sonnet solo, at 85 % less cost.
  - The setups differ between arms (thinking on or off, prompts), so this is not a clean ablation.
- **Anthropic multi-agent research (VR, 2025).** An Opus 4 lead with Sonnet 4 subagents beat single-agent Opus 4 by
  90.2 % on an internal *research* eval (not coding). Token usage explained 80 % of variance. Multi-agent
  setups use about 15× the tokens of chat.
- **Anthropic Haiku 4.5 launch (DOC/VR, 2025-10-15).** "Sonnet 4.5 can break down a complex problem into multi-step
  plans, then orchestrate a team of multiple Haiku 4.5s". Haiku 4.5 scored 73.3 % on SWE-bench Verified (50 trials) at
  $1/$5 per million tokens.

#### 2.4 Heuristic vs learned routing: LiteLLM's own benchmark (VR/OSS)

The benchmark used 300 prompts (120 WildChat, 90 MATH-500/MMLU-Pro, 90 BigCodeBench) and a three-model Gemini ladder.
The judge was a pairwise model from another family.

| Strategy | Win rate vs flagship | Savings |
|---|---|---|
| Semantic Auto Router, threshold 0.3 | 48.2 % | 47.5 % |
| All cheap | 43.7 % | 97.4 % |
| complexity_router, fitted | 41.5 % | 58.3 % |
| complexity_router, default | 39.8 % | 78.0 % |
| Shuffled control (cost-matched) | 39.2 % | 59.4 % |

The keyword heuristic was barely better than random at matched cost, and "all cheap" beat it on
quality. Caveats: the judge disagreed with itself on 32 % of order-reversed pairs, code was not executed, and
the prompts were single-turn.

#### 2.5 Difficulty prediction from task text

- **SWE-bench Verified** (DOC, OpenAI): three annotators labelled each task's time-to-fix (<15 min, 15 min–1 h, 1–4 h,
  >4 h). There are 196 "easy" (<15 min) tasks and 45 that take more than 1 h. Note this is human time, not agent difficulty.
- **Al-Haque & Johnson, ESEM 2026** (PR): static features give AUC 0.863 for agent success, driven by
  **patch fragmentation and repository scale**. Prompt linguistics matter mainly in the mid band.
  Caveat: "task patch" features appear to come from the *reference* fix, which is not available before solving.
  Their pre-hoc value is NOT VERIFIED.
- **Agent Psychometrics** (PP/workshop, arXiv 2604.00594): IRT with features from issue, repository, solution and
  tests; predicts task-level performance for unseen benchmarks and scaffolds. The abstract gives no
  issue-text-only vs with-solution comparison.

So difficulty is predictable from structure. But the best features (patch spread) only become observable
*during* work. That again favours deciding after a short exploration over deciding from the prompt alone.

---

### 3. Deterministic complexity signals (no LLM call)

#### 3.1 Published rubrics

- **LiteLLM complexity_router** (OSS):
  - Seven dimensions with default weights: tokenCount 0.10 (thresholds 15/400 tokens), codePresence 0.30,
    reasoningMarkers 0.25 ("step by step", "analyze"), technicalTerms 0.25 ("architecture", "distributed",
    "encryption", plus custom keywords), simpleIndicators 0.05, multiStepPatterns 0.03, questionComplexity 0.02.
  - Tier boundaries are 0.15/0.35/0.60. Two or more reasoning markers send the request straight to REASONING.
  - Weak evidence (§2.4).
- **Switchyard** (OSS): trajectory signals. Severity-weighted failures in a rolling window. Repeated
  *identical* failure or a context compaction forces an escalation; a test pass after a failure counts as recovery.
  Escalation stays sticky until the trajectory stabilises.
- **OpenHands StuckDetector** (OSS, see TOKEN_ECONOMICS §8): the same failure patterns
  (`action_error=3`, `alternating_pattern=6`) can double as escalation triggers.
- **Vendor task-type guidance** (DOC/VR), consistent across Amp, Antigravity and Anthropic:
  - **Cheap:** simple bugs with known files, small UI changes, minor features, upgrades, boilerplate,
    docs, unit tests, exploration/search.
  - **Strong:** new end-to-end features, bugs "without a clear diagnosis", architecture
    refactors, multi-repository migrations, system design, completion review.

#### 3.2 Candidate signals Rekora already has or could compute (HYP; untested)

| Signal | Source in Rekora | Direction | Evidence basis |
|---|---|---|---|
| Recorded failed attempts on the same task / repeated identical error hash | task_update history, hook-side failure log | escalate | Switchyard, advisor, Antigravity, SWE-Router (failed tests in the prefix) |
| Test failures after an edit; pass after a failure → de-escalate | PostToolUse outcomes | escalate / recover | Switchyard |
| Context compaction or usage-limit cut-off mid-task | compaction/handoff events | escalate (resume on a capable model) | Switchyard ("compaction forces an escalation") |
| Files and modules touched (distinct crates or directories) | code index + diff | more = harder | ESEM 2026 (patch fragmentation, repository scale) |
| Remaining checklist items and handoff size | tasks/handoffs | many open items = keep the planner | HYP |
| Task-type keywords (refactor, architecture, migration, security, concurrency, "why", "design" vs rename, docs, test, typo, bump) | task title/description | strong / cheap | LiteLLM technicalTerms; vendor guidance; weak as a sole signal |
| Explicit file paths named in the task | task text | cheap is fine | Amp rush guidance ("mention the files") |
| Diff size of the planned change | git | larger = harder | HYP (correlate of fragmentation) |

**Design rule (HYP).** Treat these as a *hint* attached to `context`/`handoff_resume` output, for
example `difficulty: low|medium|high, escalate: true, reasons: [...]`. Do not make them a router. The host or user
applies the hint only at boundaries where switching does not break a warm cache: choosing a subagent's
`model:`, enabling the advisor, or starting a new session after a handoff. Calibrate the hint against recorded task
outcomes before trusting it, and log any decision it drove (rule 2: never claim it helped unless measured).

---

### 4. Risks and best practice

1. **Quality regressions compound across steps.**
   - TwinRouterBench: one under-routed step fails the whole trajectory.
   - Amp: weak model on a complex refactor = 2× slower, only 19 % cheaper, "if it even arrives at a solution".
   - Aider: o3+gpt-4.1 was 3.1 pp below o3 solo.
   - Weak models also produce plausible but unused code; Kilo's own test found a transaction path that "was never called".
2. **Cache invalidation.**
   - Every main-loop model switch re-reads the full context at write price. Claude Code docs: "each model has its own cache";
     `opusplan` toggles start a fresh cache.
   - TurnCost found 98.6 % of input tokens are cache hits, so a switch forfeits most of the economy. Delegation raised cost in half of its settings.
   - Mitigations: switch only at task boundaries; use subagents (separate cache); use the advisor (main cache
     preserved); prefer effort changes over model changes (OpenRouter).
3. **Reasoning-trace compatibility.** Factory: encrypted reasoning cannot cross providers mid-conversation.
4. **Escalation loops and thrash.**
   - Switchyard holds an escalation until the trajectory stabilises, and Requesty keeps a route sticky for 60 min. Both choices are hysteresis.
   - The advisor has no cap in Claude Code (`max_uses` exists in the API).
   - Rekora should cap escalations per task, and de-escalate only after a verified pass, never on agent self-report (rule 6).
5. **Weak-trajectory contamination.** SWE-Router restarts the strong model from the original prompt because
   continuing the weak model's trajectory biased it toward the weak model's mistakes. A Rekora handoff passed to a stronger model
   should therefore carry *verified facts and failures*, not the weak model's reasoning. This is consistent with Scrouting,
   where only sandbox-verified handoff claims are kept.
6. **Start cheap and escalate vs classify up front.**
   - The evidence favours **start cheap, observe briefly, escalate on concrete failure signals**:
     SWE-Router (K=2–3 turns beats K=0), Switchyard, the advisor, the Antigravity planner fallback.
   - The cost is K wasted cheap turns on hard tasks. That is acceptable when cheap turns cost 1/17 to 1/24 of strong ones (SWE-Router costs).
   - Up-front classification is reasonable only for clearly typed work: docs, rename, tests, search on cheap; design and
     migration on strong. Even then it works best as a human or mode choice (Amp dial, Kilo per-mode), not a learned router.
   - For whole-task routing, "always strongest" remains a hard baseline to beat on solve rate (Scrouting audit).
7. **Opacity and drift.** Router mappings change server-side (Kilo, Cursor, OpenRouter's default policy now
   "costs more per task than the previous generation"). Users report that the selected model differs from the one serving
   (Antigravity, COM). Any Rekora evidence should record the model and effort that actually ran, from host-reported usage.

---

### 5. Recommendation for Rekora (HYP)

- **Do not build or bundle a request router.** The position in TOKEN_ECONOMICS §7 stands.
- **Do provide a deterministic "difficulty and escalation hint"** computed from Rekora-owned data: failures,
  attempts, cut-offs, files and modules touched, open items, task type. Surface it in `context`, `handoff_resume` and
  `task_list`. It must be explainable (`reasons`) and carry no instructions (rule 6).
- **Document host recipes that are cache-safe:** subagents with `model: haiku/sonnet` for search, tests and docs;
  `/advisor opus` for Sonnet-main sessions; `opusplan`; and a "resume on a stronger model" note when a handoff
  records repeated failures.
- **Measure before claiming.** Record per-task model, effort, cost and outcome (host usage fields), and report hint
  calibration as `NOT EXECUTED` until it has been run on real tasks.

---

### Sources

| # | Source | Class | Date / read |
|---|---|---|---|
| 1 | Claude Code model config — https://code.claude.com/docs/en/model-config | DOC | read 2026-11-24 |
| 2 | Claude Code advisor — https://code.claude.com/docs/en/advisor | DOC | read 2026-11-24 |
| 3 | Claude Code subagents — https://code.claude.com/docs/en/sub-agents | DOC | read 2026-11-24 |
| 4 | Anthropic, "The advisor strategy" — https://claude.com/blog/the-advisor-strategy | VR | 2026-04-09 |
| 5 | Claude API advisor tool — https://platform.claude.com/docs/en/agents-and-tools/tool-use/advisor-tool | DOC | search 2026-11-24 |
| 6 | Anthropic, Claude Haiku 4.5 — https://www.anthropic.com/news/claude-haiku-4-5 | DOC/VR | 2025-10-15 |
| 7 | Anthropic, multi-agent research system — https://www.anthropic.com/engineering/multi-agent-research-system | VR | 2025 |
| 8 | claude-code-router — https://github.com/musistudio/claude-code-router (MIT, 38.4k stars, v3.1.6) | OSS | read 2026-11-24 |
| 9 | CCR routing docs (moved to ccrdesk.top; key names via mirrors) | OSS/COM | partly NOT VERIFIED |
| 10 | Aider architect — https://aider.chat/2024/09/26/architect.html | VR | 2024-09-26 |
| 11 | Aider R1+Sonnet — https://aider.chat/2025/01/24/r1-sonnet.html | VR | 2025-01-24 |
| 12 | Aider leaderboards — https://aider.chat/docs/leaderboards/ | VR | read 2026-11-24 |
| 13 | Cline Plan & Act — https://docs.cline.bot/core-workflows/plan-and-act ; Cline blog usage patterns | DOC/VR | read 2026-11-24 |
| 14 | Roo Code boomerang docs — https://docs.roocode.com/features/boomerang-tasks ; shutdown reports (kilo.ai, morphllm, thenewstack) | DOC/COM | shutdown 2026-05-15 |
| 15 | Kilo Auto Model — https://kilo.ai/docs/code-with-ai/agents/auto-model ; https://blog.kilo.ai/p/auto-model-vs-picking-your-own | DOC/VR | 2026-07-27 |
| 16 | Antigravity models — https://antigravity.google/docs/models | DOC | read 2026-11-24 |
| 17 | Amp rush mode — https://ampcode.com/news/rush-mode ; the dial — https://ampcode.com/news/the-dial | VR | 2025-11-07; 2026-07-09 |
| 18 | Factory mixed models — https://docs.factory.ai/cli/configuration/mixed-models | DOC | search 2026-11-24 |
| 19 | OpenRouter Auto Router — https://openrouter.ai/docs/guides/routing/routers/auto-router | DOC/VR | read 2026-11-24 |
| 20 | Cursor Router — https://cursor.com/docs/cursor-router | DOC | read 2026-11-24 |
| 21 | Not Diamond Code — https://www.notdiamond.ai/blog/not-diamond-code-intelligent-model-routing-for-coding-agents | VR | 2026-08-04 |
| 22 | LiteLLM auto routing — https://docs.litellm.ai/docs/proxy/auto_routing ; benchmark — https://docs.litellm.ai/docs/proxy/auto_routing_benchmark | OSS/DOC/VR | read 2026-11-24 |
| 23 | LiteLLM Switchyard — https://docs.litellm.ai/blog/switchyard-tool-aware-routing | OSS/DOC | 2026-11-06 |
| 24 | Devin/Windsurf Adaptive — https://docs.devin.ai/cli/adaptive.md ; https://docs.windsurf.com/windsurf/adaptive.md | DOC | search only |
| 25 | Requesty / Portkey / Martian comparison pages | VR/COM | NOT VERIFIED |
| 26 | RouteLLM, arXiv 2406.18665 | PR (ICLR 2025) | — |
| 27 | FrugalGPT, arXiv 2305.05176 | PR (TMLR 2024) | — |
| 28 | Hybrid LLM, arXiv 2404.14618 | PR (ICLR 2024) | — |
| 29 | AutoMix, arXiv 2310.12963 | PR (NeurIPS 2024) | — |
| 30 | SWE-Router, arXiv 2607.00053 | PP (ICML 2026 workshop) | 2026-06-30 |
| 31 | TwinRouterBench, arXiv 2605.18859 | PP | v1 read; v5 2026-10-24 |
| 32 | Scrouting, arXiv 2608.04804 v2 | PP | 2026-11-18 |
| 33 | Agent-as-a-Router, arXiv 2606.22902 | PP | 2026-06-26 |
| 34 | TurnCost, arXiv 2610.15093 | PP | 2026-10-10 |
| 35 | SWE-bench Verified — https://openai.com/index/introducing-swe-bench-verified/ | DOC | 2024 |
| 36 | Al-Haque & Johnson, ESEM 2026 — https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ESEM.2026.3 | PR | 2026 |
| 37 | Agent Psychometrics, arXiv 2604.00594 | PP (ICLR 2026 workshop) | 2026-04 |
| 38 | harness-router (MCP) — https://fstubner.github.io/harness-router/ | OSS | 404, NOT VERIFIED |

## Research: routing delegated work to cheaper models, per host

Status: research only. No repository files were changed. Accessed 2026-11-24.

Evidence classes: **DOC** = official documentation read today. **DOC-partial** = official page read, but the fetch was cut short or summarised, so details may be incomplete. **COMMUNITY** = forum, GitHub issue or third-party write-up. **NOT VERIFIED** = claimed somewhere, but not confirmed in an official source today.

Method caveat: pages were read with an automated fetcher that summarises long pages. Field names and paths below are given only where the fetched text quoted them. Anything inferred is marked as such.

### 0. Core finding

Rekora does not send model requests. Each host picks the model. A third-party tool can still affect the choice in four ways, from most to least reliable:

1. **Host-enforced config** (user confirms, adapter writes it). Examples: a subagent definition file with a `model:` field, a per-role config layer, or an environment variable/setting read by the host. The host applies the model on every run, whatever the model "decides".
2. **Hook rewriting of the delegation call.** A pre-tool hook changes the `model` argument of the spawn/subagent tool call, or a model hook overrides the request. Where this exists it is enforced, but it is fragile across host versions. It is confirmed only for Gemini CLI `BeforeModel`. For Claude Code PreToolUse `updatedInput` on the Agent tool it is plausible but NOT VERIFIED.
3. **Delegation instructions written as data.** Rekora's MCP response or hook context recommends something like "delegate X to subagent `rekora-cheap`". The model may ignore it. This works only if the named subagent exists through mechanism 1.
4. **Free text such as "use a cheaper model".** Purely advisory. Nothing changes unless the model calls a tool that takes a `model` parameter.

The robust design follows from this ranking. Rekora (adapter, dry-run, `--confirm`) installs a small set of **tiered agent definitions** per host (for example `*-fast`, `*-standard`, `*-deep`), each pinned to a model or tier through the host's own field. Rekora's task classification (MCP `context`/`task_update`) then only has to *name* the tier. The host enforces the model.

Cache note, which holds for every host whose docs cover it: each model has its own prompt cache. Switching the **main** conversation's model mid-session forces an uncached re-read. Delegating to a **subagent** on another model does not hurt the parent's cache, because the subagent has its own context. So route by delegation, never by switching the main model mid-task.

### 1. Google Antigravity (App, CLI `agy`, IDE, SDK)

#### Models (DOC: /docs/models/)
The model table lists these plans:
- **Gemini 4 Neon**: Pro/Ultra/Enterprise. It is the default for new conversations.
- **Gemini 4 Carbon**: Pro/Ultra/Enterprise.
- **Gemini 4 Argon**: Pro/Ultra/Enterprise. It has a removal date.
- **Gemini 4 Hydrogen**: all plans.
- **Orchestrator**: Pro/Ultra/Enterprise, limited.
- **Gemini 3.8 Flash**: all plans. It has a removal date.
- **Gemini 3.7 Flash, 3.6 Flash and 3.1 Pro**: Enterprise only.
- **Claude Sonnet 5.5 (thinking) and Claude Opus 5.5 (thinking)**: Pro (non-trial), Ultra and Enterprise.
- **Claude Sonnet 4.6 and Opus 4.6 (thinking)**: Free/Plus/Pro. They have a removal date.
- **GPT-OSS-120b**: Free/Plus/Pro/Ultra. It has a removal date.

Several models say "Will be removed on November 30, 2026", six days after this research. Image generation uses Nano Banana 2 and is "not customizable".

**Orchestrator** is the host's own cost-tiering feature. A planner (Neon by default; also Carbon, Argon or Claude Opus 5.5) works with an executor (**Gemini 4 Hydrogen** by default, or Claude Sonnet 5.5). You set it under **Settings > Models > Orchestrator Settings** ("Orchestrator Model", "Executor Model"). In the CLI it runs as `agy --model "Orchestrator (Neon)"`. Enterprise identifier: `gemini-pro-orchestrator`. "Effort isn't adjustable for Orchestrator." Changelog v2.35.0 (2026-11-23): default Orchestrator/Executor "now start at High thinking effort".

**Selection scope.** The model is chosen per conversation in the selector and "persists between messages within a conversation". A change mid-run takes effect after the current turn.

**Thinking levels.** The App offers Low/Medium (default)/High/Extra. The CLI takes `low|medium|high|xhigh`, either through a suffix (`--model gemini-4-neon-xhigh`) or with `--effort`.

**Quotas.** The selector shows "Weekly Limit Remaining" and "Five Hour Limit Remaining", tracked separately for Gemini and for Claude/GPT.

#### Listing models (DOC: /docs/cli/reference/)
- `agy models` lists models and `agy agents` lists agents.
- `--model`, `--effort` and `--agent` apply to one session.
- `-p/--print` with `--output-format text|json|stream-json` runs headless.
- `/model <model> <prompt>` runs one prompt on another model without changing the default.
- Settings live in `~/.gemini/antigravity-cli/settings.json`. Keys: `model` (label saved by `/model`), `modelProvider` (`"gemini"`, used with `GEMINI_API_KEY`) and `customModelsConfig.customModels[].modelName/label`.
- There is no per-agent model key in settings.json (DOC: /docs/settings/).

A tool can detect available models by running `agy models`. The exact output format is NOT VERIFIED. The hook input field `modelName` (see below) reports the model actually in use.

#### Subagents: per-subagent model is supported (DOC: /docs/subagents/)
- Definitions are Markdown with YAML frontmatter, at `.agents/agents/<name>.md` or `.agents/agents/<name>/agent.md` (workspace), `~/.gemini/config/agents/<name>.md` (global), or `plugins/<plugin>/agents/`. A session can also create transient ones with the `define_subagent` tool.
- Fields:
  - `name` and `description` are required. The planner reads `description` to decide when to delegate.
  - `tools` (string[]).
  - `mainAgent` (default `true`) and `subagent` (default `true`).
  - **`model`**: default `inherit`. Doc text: "Model tier used when invoked (`inherit`, `flash`, or `pro`)." Only tiers are allowed, not exact versions.
  - `commandExecutionPolicy` (`off|auto|eager|sandbox`).
  - `mcpServers` (object[]).
  - `skills` and `plugins`.
- The parent invokes a subagent through the `invoke_subagent` tool. In the CLI you use `@name` or the `/agents` panel.
- Limits:
  - A subagent does **not** inherit the parent's conversation history.
  - It **does** inherit command prefixes, file scopes and sandbox settings.
  - Nesting is capped at 10 levels.
  - Known issue: a misspelled tool name in `tools` can hang the subagent.
- Inheritance of rules, skills and MCP is not stated on /docs/subagents/ or /docs/customization-discovery/. Rules discovery is workspace-wide ("loads every rule it finds"), so a subagent probably sees `AGENTS.md`/`GEMINI.md`, but this is NOT VERIFIED. To be safe, a Rekora subagent should list `mcpServers` (Rekora) explicitly.
- **Dynamic Workflows** (DOC: /docs/dynamic-workflows/, v2.32.0) are Python scripts the agent writes. They use `agent(prompt, schema, role, type_name, workspace, model)`, with `model` = `inherit` (default), `flash_lite`, `flash` or `pro`. "If a tier isn't available to you, the agent fails and lists the valid choices". That failure message is a usable probe for available tiers. Run a script with `/run-workflow`.
- **SDK** (`types.SubagentConfig`): only `name/description/system_instructions/tools` are shown. No per-subagent model was found (DOC-partial).
- **Teamwork** (`/teamwork`, paid plans) has these roles: Sentinel, Project Orchestrator, Explorers (read-only), Workers, Critic, Challenger, Auditor and Success Auditor. The workspace defaults to `~/teamwork_projects/{PROJECT_NAME}`, which holds request/plan/progress artifacts. The docs do **not** say which model each role uses or whether it is configurable. Treat role models as host-controlled. `/swarm` (all plans) and `/boost` (paid) give no model detail either.

#### Rules, skills, MCP and hooks
- **Rules** (DOC: /docs/rules/):
  - Global: `~/.gemini/AGENTS.md`, `~/.gemini/GEMINI.md`, `~/.gemini/config/{AGENTS,GEMINI}.md` and `~/.gemini/config/rules/*.md`.
  - Workspace: `<dir>/AGENTS.md`, `<dir>/GEMINI.md` and `<dir>/.agents/rules/*.md` (legacy `.agent/rules`). Only immediate children are scanned.
  - Frontmatter: `trigger` is one of `always_on|model_decision|glob|manual`; also `description` and `globs`.
  - Limits: a single file is truncated past 24,000 bytes. All `always_on` and global rules share a "20,000-token aggregate rules budget"; overflow is demoted to pointers. The CLI doesn't support `manual` rule mentions.
- **Skills**: `.agents/skills/<folder>/SKILL.md` and `~/.gemini/config/skills/`. The CLI also reads `~/.gemini/antigravity-cli/skills/` and `~/.gemini/skills/`. Frontmatter has `name` (optional) and `description` (required). There is **no model field**.
- **MCP** (DOC: /docs/mcp/):
  - Global: `~/.gemini/config/mcp_config.json`. Workspace: `<workspace>/.agents/mcp_config.json`.
  - Format: root `mcpServers`. Each server has `command`, `args`, `env` and `cwd` (stdio), or `serverUrl`/`url` (Streamable HTTP only; "`httpUrl` field aren't supported").
  - Optional fields: `headers`, `disabled`, `enabledTools`, `disabledTools`, `timeoutSeconds`.
  - Default timeouts are 300 s to connect and 180 s per call. Tool schemas are lazy-loaded by default.
- **Hooks** (DOC: /docs/hooks/):
  - Files: `.agents/hooks.json` and `~/.gemini/config/hooks.json`, plus plugin `hooks.json`. The CLI ignores hooks placed in `settings.json`.
  - Events: `SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`. Type is `command` only; default timeout 30 s.
  - Input includes `modelName` (e.g. `gemini-3.6-flash-medium`).
  - Outputs: `injectSteps` (PreInvocation), `decision` and `overwrite` (PreToolUse, shallow merge into `toolCall.args`), and `terminationBehavior`.
  - No hook output changes the model. PreToolUse `overwrite` on the `invoke_subagent` call could in theory swap the subagent *name* (and so its tier). This is NOT VERIFIED.
  - v2.33.0 fixed a bug where agent, rule and hooks.json files saved **with a UTF-8 BOM were ignored**. Rekora adapters should write files without a BOM on Windows.

#### Display surface (DOC: /docs/cli/statusline/)
- Configure with `statusLine: {"type":"command","command":"…"}` in `~/.gemini/antigravity-cli/settings.json`. Other keys: `padding`, `enabled`, `stack_with_default`.
- The command runs through `sh -c`, or `cmd /c` on Windows.
- stdin JSON includes:
  - `model` {`id`, `display_name`, `effort`}.
  - `cost` {`total_usd` (includes subagents), `subagent_usd`, `estimated`}. These are list-price estimates.
  - `context_window` (input/output totals, `context_window_size`, used and remaining percent).
- Throttle: at most once per 300 ms, 5 s kill. The script is disabled after 30 consecutive failures.
- The App and IDE have no documented status-bar API for plugins. Plugins (`plugin.json` or `.antigravity-plugin/plugin.json`) bundle skills, MCP, agents, rules and hooks only.
- Whether the Antigravity IDE is a VS Code fork with Open VSX extensions is NOT VERIFIED today; the docs fetched do not say.

#### Known problems
Official CLI troubleshooting covers only four topics: PATH (`%LOCALAPPDATA%\agy\bin` on Windows), keyring, clipboard over SSH and the updater lock (`AGY_CLI_DISABLE_AUTO_UPDATE`). The official changelog adds: v2.30.0 now names a missing MCP command; v2.32.0 shows MCP startup errors on plugin cards; v2.31.1 fixed Enterprise sign-in on Windows; v2.33.0 fixed the BOM bug.

Community reports (COMMUNITY):
- MCP tool calls hang on Windows/WSL2.
- Antigravity 2.0 CLI MCP fails with "context deadline exceeded" on Windows.
- Regression where a server worked on 2.3.1 but failed on 2.5.x.
- macOS GUI launch loses `PATH`, causing "executable file not found in $PATH". The Dynamic Workflows doc confirms the same env-var issue for Dock launches.
- `mcp_config.json` rejects a `type` field.
- Guides disagree on the path (`~/.gemini/antigravity/mcp_config.json` vs `~/.gemini/config/mcp_config.json`). The current official path is `~/.gemini/config/`.

Implications for Rekora: use absolute `command` paths, set `cwd` explicitly, avoid a `type` key, and write without a BOM.

### 2. Claude Code

#### Models and selection (DOC: code.claude.com/docs/en/model-config)
- Aliases: `default` (clears the override), `best` (= opus), `fable`, `sonnet`, `opus`, `haiku`, `sonnet[1m]`, `opus[1m]`, `opusplan` (Opus in plan mode, Sonnet for execution) and `opusplan[1m]`. Full IDs also work (e.g. `claude-opus-5-5`).
- Priority order:
  1. `/model`
  2. `claude --model`
  3. `ANTHROPIC_MODEL`
  4. settings `model`
  5. `ANTHROPIC_DEFAULT_MODEL`
- Alias remapping: `ANTHROPIC_DEFAULT_OPUS_MODEL`, `_SONNET_MODEL`, `_HAIKU_MODEL` and `_FABLE_MODEL`.
- Restriction (managed settings): `availableModels` (family, version prefix or full ID), plus `enforceAvailableModels`, `deniedModels` and `availableModelsMatch` (v2.1.283+).
- Effort levels: `low|medium|high|xhigh|max` on 5.x and Fable models. Resolution order:
  1. `CLAUDE_CODE_EFFORT_LEVEL`
  2. A level picked in the session (`/effort`, the `/model` picker)
  3. `--effort`
  4. settings `effortLevel` or per-model `modelSettings` (neither accepts `max`)
  5. The model's default
- Skill and subagent frontmatter can both set `effort`. Skills/commands also accept `model` (DOC: prompt-caching page).
- Listing models: the `/model` picker is the documented way. Outside the host, the Anthropic API `GET /v1/models` lists models for an API key, but that does not reflect subscription entitlements or `availableModels`.

#### Subagents: strongest control of any host (DOC: /sub-agents)
- Locations: managed settings, the `--agents` flag, `.claude/agents/`, `~/.claude/agents/` and plugin `agents/`. Higher priority wins on name clashes.
- Frontmatter:
  - `model`: `sonnet|opus|haiku|fable`, a full ID, or `inherit`.
  - `effort`, `tools`, `disallowedTools`, `permissionMode`.
  - `mcpServers`: ignored for plugin agents.
  - `hooks`, `skills` (preloaded, up to 32), `memory` (`user|project|local`).
  - `isolation: worktree`, `background`, `maxTurns`.
  - `omitClaudeMd`, `initialPrompt`, `experimental` (e.g. `cacheTtl`).
- **Model resolution**, first match wins:
  1. The per-invocation Agent tool `model` parameter
  2. The frontmatter `model`
  3. `CLAUDE_CODE_SUBAGENT_MODEL`
  4. The parent's model
- Setting `CLAUDE_CODE_SUBAGENT_MODEL` together with `CLAUDE_CODE_SUBAGENT_MODEL_FORCE=1` overrides both of the first two. Forks keep the parent model.
- The Agent tool input is `prompt`, `description`, `subagent_type` and `model` (an optional alias). The PostToolUse `tool_response.resolvedModel` reports the model actually used. This is a verification signal Rekora can record.
- Inheritance:
  - A non-fork subagent loads the CLAUDE.md hierarchy. Built-in Explore, Plan and web-fetch skip it.
  - MCP tools are inherited (filtered by `tools`).
  - Skills are not preloaded, but the Skill tool can still load them.
  - Auto memory is not loaded.
- Built-in Explore runs on the model the `opus` alias resolves to for subscription/Console accounts. To make Explore cheap, define a project agent named `Explore` with `model: haiku` (documented).
- Concurrency defaults to 20 (`CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`); nesting depth to 3 (`CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`).

#### Hooks (DOC: /hooks)
- 33 events, including `SessionStart`, `SubagentStart`, `SubagentStop`, `PreToolUse`, `PostToolUse`, `PreCompact`/`PostCompact`, `PreModelSwitch`/`PostModelSwitch`, `StopFailure` and `SessionEnd`.
- Locations: user, project and local settings; managed settings; plugin `hooks/hooks.json`; skill and subagent frontmatter.
- Types: `command|http|mcp_tool|prompt|agent`.
- Windows: `shell: "powershell"`. Exec form needs a real `.exe`, not a `.cmd` shim.
- Model visibility:
  - Only SessionStart may receive `model`, and not always.
  - Model-switch hooks get `from_model`/`to_model`. `PreModelSwitch` can block a switch.
  - There is no `$CLAUDE_MODEL` env var.
- PreToolUse `updatedInput` "Replaces the entire input object". Applied to the `Agent` tool it would rewrite `model`. The docs do not state this explicitly: **NOT VERIFIED**, and it needs a real-process test.

#### Display surface (DOC: /statusline)
- Configure `statusLine` {`type: "command"`, `command`, `padding`, `refreshInterval` (minimum 1 s)}.
- stdin JSON includes `model.id`/`display_name`, `cost.total_cost_usd`, `context_window.*` (with `current_usage.cache_*` tokens), `prompt_cache` {`hit_ratio`, `misses`, `warm`, `ttl`, …}, `effort.level`, `rate_limits.five_hour/seven_day/spend_limit`, `agent.name`, `worktree` and `session_id`.
- Windows: the command runs through Git Bash, or PowerShell if Git Bash is absent. Use forward slashes in paths.
- Plugins ship agents, skills, hooks, monitors and MCP. Enabling plugin agents, skills and hooks keeps the cache (DOC: prompt-caching).

#### Cache (DOC: /prompt-caching)
- "Each model has its own cache". A model switch causes an uncached turn.
- `opusplan` toggles count as switches.
- A skill `model` that differs from the session model makes that turn a switch, unless it uses `context: fork`.
- A subagent has its own cache with a default TTL of 5 min (`subagentPromptCacheTtl` or `CLAUDE_CODE_SUBAGENT_PROMPT_CACHE_TTL`). The parent's cache is unaffected.

### 3. OpenAI Codex CLI (DOC: learn.chatgpt.com, redirected from developers.openai.com/codex)
- **Models**:
  - Recommended: GPT-6.2 Astra, GPT-6.2 Sol, gpt-6-astra, gpt-6.1-sol, gpt-6.1-terra ("fast, efficient") and gpt-6-luna ("most efficient").
  - Select with `/model` (interactive), `--model`/`-m` (including `codex exec -m gpt-6.2-sol "…"`) or `model = "…"` in config.toml.
  - Config keys: `model_provider`, `model_reasoning_effort` (`low|medium|high|xhigh|max|ultra`, as offered by the model), `plan_mode_reasoning_effort` and `model_catalog_json` (a local catalog loaded at startup).
- **Profiles**: `$CODEX_HOME/<name>.config.toml`, selected with `--profile`. A project `.codex/config.toml` ignores `profile`/`profiles` and loads only for trusted projects.
- **Listing models**: no official list command. `/model` shows the picker. COMMUNITY: a cache at `~/.codex/models_cache.json`, layered as bundled catalog → remote catalog → `model_catalog_json`. `codex --list-models` is NOT VERIFIED.
- **Subagents** (DOC: /agent-configuration/subagents):
  - Built-in agents: `default`, `worker` and `explorer`.
  - Custom agent TOML files go in `~/.codex/agents/` or `.codex/agents/`. Required fields: `name`, `description`, `developer_instructions`. Optional: **`model`**, `model_reasoning_effort`, `model_reasoning_summary`, `model_verbosity`, and `skills.config` to disable skills.
  - Config-level role entries `[agents.<name>]` take `description` and `config_file` (a TOML layer).
  - Global defaults: `agents.default_subagent_model` and `agents.default_subagent_reasoning_effort`. An explicit spawn value takes precedence; otherwise the subagent inherits the parent.
  - Concurrency: `agents.max_concurrent_threads_per_session` (legacy `agents.max_threads`). Feature flag `features.multi_agent` is on by default.
  - Subagents inherit the sandbox, approvals, MCP servers and skills. `sandbox_mode`/`mcp_servers` in an agent file "don't override the parent".
  - The spawn tool name (`spawn_agent`) and worktree isolation are not on the page: NOT VERIFIED.
- **Hooks** (DOC: /docs/hooks; on by default):
  - Events: PreToolUse, PermissionRequest, PostToolUse, PreCompact, PostCompact, UserPromptSubmit, SubagentStop, Stop, PreModelRequest, PostModelRequest, Interrupt, SessionStart, SubagentStart, SessionEnd.
  - Common input includes `model` ("Active model slug").
  - SubagentStart `additionalContext` "is added as extra developer context for the subagent". This is the clean way to hand a subagent Rekora context.
  - PreModelRequest/PostModelRequest (`model`, `turn_id`, `agent_id`, `agent_type`, `token_usage`) are "diagnostic only". No hook changes the model.
  - Files: `~/.codex/hooks.json` (Windows `%USERPROFILE%\.codex\hooks.json`), `.codex/hooks.json`, inline `[hooks]`, plugin `hooks/hooks.json`, and managed `requirements.toml`.
  - Windows: `commandWindows`/`command_windows`.
- **Display**:
  - `tui.status_line` is an ordered array of **built-in** item IDs only, with no custom command. GitHub #20140 was closed as a duplicate of the open #17827 (COMMUNITY).
  - `notify` is a command receiving a JSON payload (schema not on the page).
  - The PostModelRequest hook's `token_usage` is the only programmatic cost feed.

### 4. OpenCode (DOC: opencode.ai/docs, now V2; V1 at /v1/docs)
- **Default model**: `model` in `opencode.json[c]`, using the ID format `provider/model#variant` (variants carry `settings.reasoningEffort`). `/models` changes the current session only. The list comes from models.dev, provider integrations and config.
- **Listing**: `opencode models [provider]` and `--refresh` (COMMUNITY mirror; not on the official V2 CLI page). There is also an official Console model list API.
- **Agents**:
  - Modes: `primary|subagent|all`. Built-ins: `build`, `plan`, `general` and `explore`.
  - Files: `.opencode/agents/<name>.md` and `~/.config/opencode/agents/`, or an `agents` key in config (V2; V1 used `agent`).
  - Fields: `description`, `mode`, **`model`**, `system`, `permissions`, `steps`, `hidden`, `disabled`, `request`.
  - "A subagent uses its configured model, or inherits the parent session's model". Delegation goes through the `subagent` tool.
  - Whether a subagent inherits AGENTS.md or MCP is not stated: NOT VERIFIED.
- **Plugins**:
  - JS/TS files in `.opencode/plugins/` or `~/.config/opencode/plugins/`.
  - V1 events: `tool.execute.before/after`, `session.idle`, `session.compacted`, `experimental.session.compacting`, `shell.env`, `tui.toast.show`, `tui.prompt.append`, `tui.command.execute`.
  - A model-changing hook (`chat.params`) is NOT VERIFIED.
  - TUI-only plugins are configured in `cli.json`; toast is the documented display surface.
  - Plugins run in-process, so they can compute a cost estimate. A persistent status line is NOT VERIFIED.

### 5. Gemini CLI (DOC: geminicli.com)
- Note: the docs say Gemini CLI was "replaced by Antigravity CLI on June 18th, 2026" for unpaid and Google One users, and "has not been optimized for Gemini 4 Argon". Treat it as a legacy host.
- **Models**: `/model` dialog with Auto (Gemini 3: `gemini-3-pro-preview` plus `gemini-3-flash-preview`), Auto (2.5) or Manual, and `--model` at startup. Auto selects "based on the complexity of the task". This is the host's own complexity routing. No list command is documented.
- **Subagents**:
  - Files: `.gemini/agents/*.md` and `~/.gemini/agents/*.md`.
  - Frontmatter: `name`, `description`, `kind` (`local|remote`), `tools` (supports `mcp_*` wildcards), `mcpServers`, **`model`** (default `inherit`), `temperature`, `max_turns`, `timeout_mins`.
  - Built-ins: `codebase_investigator`, `cli_help`, `generalist`, `browser_agent` (off by default). Override them via `agents.overrides.<name>` (example `model: gemini-3-flash-preview`).
  - Subagents cannot call subagents. They are on by default (`experimental.enableAgents`).
- **Hooks**:
  - Configured in `settings.json` `hooks` (project, user, system, extensions).
  - Events: SessionStart, SessionEnd, BeforeAgent, AfterAgent, BeforeModel, AfterModel, BeforeToolSelection, BeforeTool, AfterTool, PreCompress, Notification.
  - **BeforeModel `hookSpecificOutput.llm_request` "overrides parts of the outgoing request (for example, changing models…)"**. This is the only *documented* hook-level model override found.
  - Windows is not covered in the docs.
- **Extensions** bundle prompts, MCP, commands, themes, hooks, sub-agents and skills. No status-line API was found: NOT VERIFIED.

### 6. DeepSeek Harness (`dsh`; DOC-partial: GitHub README and docs/*.md)
- Developer preview with an explicit warning: "THERE WILL BE COMPATIBILITY-BREAKING CHANGES". The architecture is "everything-is-a-plugin" on Cordis: the model adapter, tool registry and loop are all plugins.
- **Config**:
  - `$DSH_HOME/profiles/<profile>/cordis.patch.yml`, plus `$DSH_HOME/cordis.patch.yml` and `--patch` overlays.
  - Credentials: `$DSH_HOME/.credentials.yaml`. Inspect the result with `dsh --profile web --dump-config`.
  - Profiles: `web`, `headless`, `sdk`, `sdk-minimal`, `acp`.
- **Models**:
  - Providers and routes take keys such as `api` (`openai-completions|openai-responses|anthropic-messages|google-generative-ai`), `baseURL`, `models`, `modelOverrides`, `reasoning` and `reasoningEfforts`.
  - "Fetch available models" calls `GET /models`.
  - A session keeps the model recorded in its log. Changes apply on the next request.
  - A per-subagent model was **not found**: NOT VERIFIED.
- **Subagents**: providers range "from a fresh child agent to a delegated turn in another product". Delegating to Claude Code/Codex as subagents, and the bridges that run their `hooks.json`, are reported by press (COMMUNITY). They are not in the docs read.
- **Hooks**: native interception uses `agent/*` and `tools/*` event listeners (`agent/turn-stopping`).
- **Display**: Web UI at `127.0.0.1:3080`. Plugins can register a `ConversationNodeDefinition` and a renderer, so a Rekora plugin could render cost in the UI (DOC, architecture). The Trajectory view shows tokens per step (COMMUNITY).
- MCP, AGENTS.md inheritance and Windows support are not documented in the pages read.

### 7. Cursor (brief; DOC: cursor.com/docs)
- **Auto** has "three modes: Cost, Balance, and Intelligence". Routing is described under Cursor Router. Max Mode exists only on legacy request plans.
- **Subagents**:
  - Files: `.cursor/agents/` (also reads `.claude/agents/` and `.grok/agents/`) and `~/.cursor/agents/`.
  - Fields: `model` (default `inherit`, or an ID such as `composer-2`), `readonly`, `is_background`.
  - The built-in Explore "Uses a faster model".
  - Subagents inherit MCP tools. Parallel Task calls run at the same time.
- Because Cursor reads `.claude/agents/`, a Rekora `.claude/agents/*.md` file would also be loaded by Cursor. Model IDs differ between the two, so a pinned `model: haiku` may not resolve in Cursor: NOT VERIFIED. Prefer `inherit`, or write host-specific files.

### 8. Per-host summary table

| Host | Model listing | Per-subagent / per-task model | Config file | Enforcement | Cache implication |
|---|---|---|---|---|---|
| Antigravity | `agy models`; hook `modelName`; workflow tier error lists valid tiers | Subagent `model: inherit\|flash\|pro`; workflow `agent(model=flash_lite\|flash\|pro)`; Orchestrator planner/executor (UI); Teamwork roles: undocumented | `.agents/agents/*.md`, `~/.gemini/config/agents/`, `~/.gemini/antigravity-cli/settings.json` | Enforced (tier only, not version); host maps tier to model | Not documented; assume per-model cache. Delegate, don't switch |
| Claude Code | `/model` picker; API `/v1/models` (API key only); PostToolUse `resolvedModel` | Frontmatter `model` (alias/ID/`inherit`) + `effort`; Agent tool `model` param; `CLAUDE_CODE_SUBAGENT_MODEL[_FORCE]`; skill `model` | `.claude/agents/*.md`, settings.json, env | Enforced; `availableModels` can veto silently (check `resolvedModel`) | Per-model cache (DOC); subagent has own cache, parent intact |
| Codex CLI | `/model`; `model_catalog_json`; `models_cache.json` (COMMUNITY) | Agent TOML `model`, `model_reasoning_effort`; `agents.default_subagent_model`; spawn-time model wins | `.codex/agents/*.toml`, `~/.codex/config.toml`, `[agents.<name>].config_file` | Enforced | Not documented; assume per-model |
| OpenCode | `/models`; `opencode models` (COMMUNITY) | Agent `model: provider/model#variant` | `.opencode/agents/*.md`, `opencode.jsonc` `agents` | Enforced | Provider-dependent; NOT VERIFIED |
| Gemini CLI (legacy) | `/model` dialog | Subagent `model` (default `inherit`); `agents.overrides.<builtin>.model`; BeforeModel hook override | `.gemini/agents/*.md`, `.gemini/settings.json` | Enforced (file) / enforced (hook) | NOT VERIFIED |
| DeepSeek Harness | UI "Fetch available models" (`GET /models`) | Not found | `$DSH_HOME/profiles/<p>/cordis.patch.yml` | Session-level only (found) | NOT VERIFIED |
| Cursor | UI picker | Subagent `model` (default `inherit`); Auto Cost/Balance/Intelligence | `.cursor/agents/*.md` (+ reads `.claude/agents/`) | Enforced | NOT VERIFIED |

#### Hooks, subagent context and display surfaces

| Host | Hook config / events relevant to Rekora | How a subagent gets Rekora knowledge | Live-cost display surface (input) | Runs a command? |
|---|---|---|---|---|
| Antigravity | `.agents/hooks.json`; SessionStart, Pre/PostInvocation (`injectSteps`), Pre/PostToolUse, Stop; Windows: BOM fix v2.33 | Subagent frontmatter `mcpServers` + `skills`; rules inheritance NOT VERIFIED; no history inherited | CLI `statusLine` (`model`, `cost.total_usd/subagent_usd`, `context_window`) | Yes (`sh -c` / `cmd /c`) |
| Claude Code | settings `hooks`; SessionStart/SubagentStart `additionalContext`; PreToolUse `updatedInput` | CLAUDE.md loaded (except Explore/Plan); MCP inherited; `skills` preload; `mcpServers` | `statusLine` (cost, context, `prompt_cache`, `rate_limits`, `agent.name`) | Yes, + `refreshInterval` |
| Codex | `hooks.json`; SessionStart, **SubagentStart `additionalContext`**, Pre/PostModelRequest (`token_usage`); `commandWindows` | MCP + skills inherited from parent; SubagentStart context | `tui.status_line` built-in items only; `notify` command | Status line: no; notify/hooks: yes |
| OpenCode | in-process JS/TS plugins (`tool.execute.*`, `session.*`) | agent `system` + project instructions/skills added (scope for subagents NOT VERIFIED) | `tui.toast.show`; TUI plugins via `cli.json` | In-process code |
| Gemini CLI | settings `hooks`; BeforeAgent `additionalContext`; BeforeModel override | subagent `mcpServers`; tools inherited | none found | — |
| DeepSeek Harness | Cordis `agent/*`, `tools/*` events | plugin-level (NOT VERIFIED) | Web UI node renderer plugin | In-process plugin |

### 9. Ranked mechanisms for Rekora

1. **Tiered agent files written by the adapter.** Antigravity `model: flash|pro`; Claude `model: haiku|sonnet|opus` + `effort`; Codex agent TOML `model` + `model_reasoning_effort`; OpenCode `model`; Gemini CLI `model`; Cursor `model`. Each file includes the Rekora MCP server and instructions to call `context`. This is enforced by the host, dry-run by default and reversible. Codex user-wide files must say so in the plan (AGENTS.md rule 7).
2. **Host-native defaults the user sets once.** Claude `CLAUDE_CODE_SUBAGENT_MODEL` (with `_FORCE=1` it overrides everything, which also defeats per-tier routing); Codex `agents.default_subagent_model`; Antigravity Orchestrator executor. Enforced, but a single tier only.
3. **Hook rewrite of the spawn call.** Gemini CLI BeforeModel is DOC-confirmed. Claude PreToolUse `updatedInput` on `Agent` and Antigravity `overwrite` on `invoke_subagent` are NOT VERIFIED. Codex is impossible (model hooks are diagnostic only). Enforced but brittle: it needs a real-process test per host version.
4. **Context injection that names a tier**: SessionStart/SubagentStart `additionalContext`, Antigravity `injectSteps`, MCP `context` response. Advisory. The model chooses which agent to call.
5. **Prose advice** ("use a cheaper model"). Advisory and the least reliable. Never present it as a guarantee.

**Verification loop.** Record the model that actually ran: Claude `resolvedModel` (PostToolUse on Agent), Codex `model` in PostModelRequest/SubagentStart, Antigravity `modelName` in hook input, statusline `model.id`. Store it with the task in Rekora, so routing claims are measured rather than assumed (AGENTS.md rule 2).

**Pitfalls.**
- Tier-to-model mappings drift. Several Antigravity models are removed on 2026-11-30.
- `availableModels`/plan entitlements can silently substitute another model.
- Cursor reads `.claude/agents/` with different model IDs.
- The Antigravity BOM bug affects files written on Windows.
- Switching the main session model breaks its cache, so route through subagents only.

### 10. Sources

| URL | Date | Evidence |
|---|---|---|
| https://antigravity.google/docs/models | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/subagents/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/teamwork/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/dynamic-workflows/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/rules/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/skills/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/mcp/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/hooks/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/settings/ ; /docs/agent-settings/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/cli/reference/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/cli/statusline/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/cli/troubleshooting/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/customization-discovery/ ; /docs/plugins/ ; /docs/faq/ | accessed 2026-11-24 | DOC |
| https://antigravity.google/docs/sdk/subagents/ | accessed 2026-11-24 | DOC-partial |
| https://antigravity.google/docs/changelog/ (v2.28.1–v2.35.1, first 100k chars only) | 2026-10-28 to 2026-11-24 | DOC-partial |
| https://discuss.ai.google.dev/t/no-mcp-servers-are-working-in-antigravity/143312 | n.d. | COMMUNITY |
| https://discuss.ai.google.dev/t/antigravity-2-0-cli-mcp-servers-fail-to-initialize-context-deadline-exceeded/169571 | n.d. | COMMUNITY |
| https://discuss.ai.google.dev/t/antigravity-2-5-x-roblox-studio-mcp-fails-to-load-but-works-correctly-on-version-2-3-1/177624 | n.d. | COMMUNITY |
| https://discuss.ai.google.dev/t/mcp-config-json-wont-accept-type-field/120831 | n.d. | COMMUNITY |
| https://discuss.ai.google.dev/t/bug-mcp-servers-crash-with-executable-file-not-found-in-path-when-antigravity-is-launched-via-macos-gui/138495 | n.d. | COMMUNITY |
| https://github.com/modelcontextprotocol/servers/issues/3278 | n.d. | COMMUNITY |
| https://code.claude.com/docs/en/model-config | accessed 2026-11-24 | DOC-partial (first 100k chars) |
| https://code.claude.com/docs/en/sub-agents | accessed 2026-11-24 | DOC-partial |
| https://code.claude.com/docs/en/hooks | accessed 2026-11-24 | DOC-partial (200k of 294k chars) |
| https://code.claude.com/docs/en/statusline | accessed 2026-11-24 | DOC |
| https://code.claude.com/docs/en/prompt-caching | accessed 2026-11-24 | DOC |
| https://learn.chatgpt.com/docs/config-file/config-reference | accessed 2026-11-24 | DOC |
| https://learn.chatgpt.com/docs/agent-configuration/subagents | accessed 2026-11-24 | DOC |
| https://learn.chatgpt.com/docs/hooks | accessed 2026-11-24 | DOC |
| https://learn.chatgpt.com/docs/models.md | accessed 2026-11-24 | DOC |
| https://github.com/openai/codex/issues/20140 (dup of #17827) | 2026-04-29 | COMMUNITY |
| https://developer.aliyun.com/article/1745814 (models_cache.json) | 2026 | COMMUNITY |
| https://opencode.ai/docs/agents/ ; /docs/models ; /docs/plugins/ ; /v1/docs/plugins/ ; /docs/cli/ | accessed 2026-11-24 | DOC |
| https://mintlify.com/anomalyco/opencode/cli/models | n.d. | COMMUNITY (mirror) |
| https://geminicli.com/docs/core/subagents/ ; /docs/cli/model/ ; /docs/hooks/ ; /docs/hooks/reference/ ; /docs/extensions/ | accessed 2026-11-24 | DOC |
| https://raw.githubusercontent.com/deepseek-ai/deepseek-harness/HEAD/README.md ; docs/architecture.md ; docs/user/guide/index.md ; docs/user/guide/providers.md ; apps/cli/README.md | accessed 2026-11-24 | DOC-partial |
| https://thenewstack.io/deepseek-harness-open-source-plugins/ ; https://betterstack.com/community/guides/ai/deepseek-harness-plugin/ | 2026 | COMMUNITY |
| https://cursor.com/docs/context/subagents ; https://cursor.com/docs/models | accessed 2026-11-24 | DOC |

NOT EXECUTED: no host was run. No hook, statusline, `agy models`, `opencode models` or `codex /model` output was observed, and no `updatedInput`/`overwrite` model rewrite was tested.

## Token prices for Rekora cost estimates (research, 2026-11-24)

Scope: current list prices per million tokens (MTok, USD, standard/global tier, non-batch) for models coding
agents use, the usage-to-cost formula per provider, and how Rekora should store a dated price table.
All pages below were fetched on **2026-11-24**. Anything not stated on an official page is marked
**NOT VERIFIED**. No repository files were changed.

### 1. Sources (official, accessed 2026-11-24)

| Id | URL | Used for |
|---|---|---|
| A1 | https://platform.claude.com/docs/en/about-claude/pricing | Claude prices, cache multipliers, Haiku 5.5 tier, data residency |
| A2 | https://platform.claude.com/docs/en/about-claude/models/overview | Claude model IDs, retirement status |
| A3 | https://platform.claude.com/docs/en/build-with-claude/prompt-caching | Claude usage fields |
| A4 | https://code.claude.com/docs/en/model-config | Claude Code alias mapping |
| A5 | https://code.claude.com/docs/en/costs | `/usage`, "Estimated cost", subscriber framing, `modelPricing` |
| A6 | https://code.claude.com/docs/en/statusline | `cost.total_cost_usd`, `rate_limits.*` |
| O1 | https://developers.openai.com/api/docs/pricing | OpenAI prices incl. 30m/12h cache writes, >272K tier |
| O2 | https://developers.openai.com/api/docs/models | Astra/Sol/Terra/Luna descriptions |
| O3 | https://developers.openai.com/api/docs/guides/prompt-caching | OpenAI cache billing + usage fields |
| O4 | https://learn.chatgpt.com/docs/pricing (redirect target of developers.openai.com/codex/pricing) | Codex on ChatGPT plans vs API key |
| G1 | https://aistudio.google.com/docs/pricing (redirect target of ai.google.dev/gemini-api/docs/pricing) | Gemini prices |
| G2 | https://aistudio.google.com/docs/models | Gemini model roles |
| G3 | https://aistudio.google.com/docs/caching | Implicit caching |
| G4 | https://aistudio.google.com/api-reference/generate-content | `usageMetadata` fields |
| D1 | https://api-docs.deepseek.com/quick_start/pricing | DeepSeek prices, peak/off-peak |
| D2 | https://api-docs.deepseek.com/guides/kv_cache | DeepSeek cache usage fields |
| D3 | https://api-docs.deepseek.com/news/news260424 | `deepseek-chat`/`deepseek-reasoner` retirement |

Machine-readable sources (section 6) were downloaded the same day: LiteLLM
`model_prices_and_context_window.json` (5.19 MB, 7,165 keys), `https://models.dev/api.json` (6.9 MB),
`https://openrouter.ai/api/v1/models` (0.9 MB, no auth needed).

### 2. Price table (USD per MTok, standard tier)

"Cache write" = price per token written to cache, as a *total* rate (not a surcharge on top of input).

#### 2.1 Anthropic (A1, A2)

| Model id | Input | Cache read | Cache write 5m | Cache write 1h | Output | Notes |
|---|---|---|---|---|---|---|
| claude-fable-5-2 | 8.00 | 0.20 | 10.00 | 16.00 | 40.00 | read = 0.025x |
| claude-mythos-5-2 | 8.00 | 0.20 | 10.00 | 16.00 | 40.00 | limited availability |
| claude-fable-5-1 | 10.00 | 0.25 | 12.50 | 20.00 | 50.00 | read = 0.025x |
| claude-mythos-5-1 | 10.00 | 0.25 | 12.50 | 20.00 | 50.00 | limited |
| claude-fable-5 | 10.00 | 1.00 | 12.50 | 20.00 | 50.00 | |
| claude-mythos-5 | 10.00 | 1.00 | 12.50 | 20.00 | 50.00 | limited |
| claude-opus-5-5 | 4.00 | 0.20 | 5.00 | 8.00 | 20.00 | read = 0.05x |
| claude-opus-5 | 5.00 | 0.50 | 6.25 | 10.00 | 25.00 | |
| claude-opus-4-8 / 4-7 / 4-6 / 4-5 | 5.00 | 0.50 | 6.25 | 10.00 | 25.00 | |
| claude-opus-4-1, claude-opus-4 | 15.00 | 1.50 | 18.75 | 30.00 | 75.00 | retired on first-party API |
| claude-sonnet-5-5 | 2.00 | 0.10 | 2.50 | 4.00 | 10.00 | read = 0.05x |
| claude-sonnet-5 | 2.00 | 0.20 | 2.50 | 4.00 | 10.00 | intro price made permanent (no Sept 2026 increase) |
| claude-sonnet-4-6 / 4-5 | 3.00 | 0.30 | 3.75 | 6.00 | 15.00 | |
| claude-sonnet-4 | 3.00 | 0.30 | 3.75 | 6.00 | 15.00 | retired on first-party API |
| claude-haiku-5-5 (prompt ≤100K) | 0.10 | 0.01 | 0.125 | 0.20 | 0.50 | |
| claude-haiku-5-5 (prompt >100K) | 0.50 | 0.05 | 0.625 | 1.00 | 2.50 | whole request repriced |
| claude-haiku-4-5 | 1.00 | 0.10 | 1.25 | 2.00 | 5.00 | |

Modifiers (A1): no long-context surcharge on Claude 4.6+ except Haiku 5.5 (1M at standard price).
Haiku 5.5 threshold counts *all* input incl. cache reads/writes, per request. `inference_geo: "us"`
multiplies every category by 1.1 (4.6+). Fast mode: Fable/Mythos 5.2 $16/$80, Opus 5.5 $8/$40,
Opus 5 / 4.8 $10/$50; cache multipliers apply on top. Web search $10 / 1,000 searches. Thinking tokens
are billed as output (A5). Claude 4.7+ tokenizer yields ~30% more tokens than earlier models for the same
text — relevant when comparing token counts across models, not for pricing.

**Claude Code alias mapping (A4, Anthropic API):** `default` → Opus 5.5 (Pro/Max/Ultra/Team/Enterprise/API;
Sonnet 4.5 on Microsoft Foundry); `best` = `opus` → `claude-opus-5-5`; `sonnet` → `claude-sonnet-5-5`;
`haiku` → `claude-haiku-5-5`; `fable` → `claude-fable-5-2` (Fable 5 in Claude apps gateway sessions);
`opusplan` = opus in plan mode, sonnet otherwise; `[1m]` suffix selects 1M context (already default for
Fable, Sonnet 5+, Haiku 5.5, Opus 4.7+). Overrides: `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_{OPUS,SONNET,
HAIKU,FABLE}_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`. Other providers map aliases to older models (Foundry:
Opus 4.6 / Sonnet 4.5 / Haiku 4.5). Rekora should price by the **resolved model id reported in the
transcript/usage**, never by alias.

#### 2.2 OpenAI (O1)

Short context = ≤272K input tokens; long = >272K (whole request). Writes: "30m" is the default lifetime,
"12h" is opt-in (`prompt_cache_options.ttl = "12h"`, Responses API).

| Model id | Input | Cached input | Write 30m | Write 12h | Output | Long: in / cached / out / w30m / w12h |
|---|---|---|---|---|---|---|
| gpt-6.2-astra | 8.00 | 0.40 | 10.00 | 20.00 | 40.00 | 16 / 0.80 / 60 / 20 / 40 |
| gpt-6.2-sol | 2.00 | 0.10 | 2.50 | 5.00 | 10.00 | 4 / 0.20 / 15 / 5 / 10 |
| gpt-6.1-sol | 2.00 | 0.10 | 2.50 | 5.00 | 10.00 | 4 / 0.20 / 15 / 5 / 10 |
| gpt-6.1-terra | 1.00 | 0.05 | 1.25 | 2.50 | 5.00 | 2 / 0.10 / 7.50 / 2.50 / 5 |
| gpt-6-astra | 10.00 | 1.00 | 12.50 | 25.00 | 50.00 | 20 / 2 / 75 / 25 / 50 |
| gpt-6-sol | 2.00 | 0.20 | 2.50 | 5.00 | 10.00 | 4 / 0.40 / 15 / 5 / 10 |
| gpt-6-luna | 0.10 | 0.01 | 0.125 | 0.25 | 0.50 | 0.20 / 0.02 / 0.75 / 0.25 / 0.50 |
| gpt-5.6-sol | 4.00 | 0.40 | 5.00 | 10.00 | 20.00 | 8 / 0.80 / 30 / 10 / 20 |
| gpt-5.6-terra | 2.00 | 0.20 | 2.50 | 5.00 | 12.00 | 4 / 0.40 / 18 / 5 / 10 |
| gpt-5.6-luna | 0.20 | 0.02 | 0.25 | 0.50 | 1.20 | 0.40 / 0.04 / 1.80 / 0.50 / 1.00 |
| gpt-5.5 | 5.00 | 0.50 | — | — | 30.00 | 10 / 1 / 45 |
| gpt-5.4 | 2.50 | 0.25 | — | — | 15.00 | 5 / 0.50 / 22.50 |
| gpt-5.4-mini | 0.75 | 0.075 | — | — | 4.50 | |
| gpt-5.3-codex | 1.75 | 0.175 | — | — | 14.00 | only Codex-named model listed |
| gpt-5.2 | 1.75 | 0.175 | — | — | 14.00 | |
| gpt-5 / gpt-5.1 | 1.25 | 0.125 | — | — | 10.00 | |

"—" = no write price listed on O1 (pre-5.6 models: NOT VERIFIED whether writes are free; historically they
were billed as ordinary input). Other tiers: "Fast" (renamed from Priority on 2026-07-30), Ultrafast for
GPT-6.2 Astra/Sol (3x Standard through 2027-02-28, 6x from 2027-03-01), Flex (beta wind-down), Batch;
regional/FedRAMP +10% on some models. Reasoning tokens are billed as output (reasoning field on O1/O3
not separately priced; NOT VERIFIED as an explicit statement, but consistent with Codex logs where
`total_tokens = input_tokens + output_tokens` and `reasoning_output_tokens` is a subset of output).

**Sol / Astra / Terra / Luna (O2).** OpenAI does not explain the names; it describes roles:
Astra — "Our most capable model for your hardest projects"; Sol — "Our go-to model for everyday work,
from coding to powering agents at scale"; Terra — "A fast, efficient model for visual reasoning, precise
computer use, and clear, natural writing"; Luna — "Our most efficient model for focused, high-volume
tasks". Cost ladder 6.x: Astra $8/$40 > Sol $2/$10 > Terra $1/$5 > Luna $0.10/$0.50. All four 6.x flagships:
1.05M context, 128K max output. The version prefix (6, 6.1, 6.2, 5.6) is the generation; the suffix is the
tier. Note that within a tier the price changes by generation (gpt-6-sol cache read 0.20 vs gpt-6.2-sol 0.10),
so the table must be keyed by full id.

**ChatGPT-plan Codex (O4).** Signed in with ChatGPT, Codex draws from the plan allowance, not per-token
billing: "ChatGPT Work and Codex share pricing, credits, and usage limits"; Plus/Business have a 5-hour
allowance, "Pro plans currently have no five-hour limit", "Weekly limits may also apply". Credits extend
usage and are priced in credits per MTok. "API-key sessions use separate API pricing, not your ChatGPT
allowance." Local Codex rollouts on this machine (`~/.codex/sessions/.../*.jsonl`) carry
`total_token_usage {input_tokens, cached_input_tokens, cache_write_input_tokens, output_tokens,
reasoning_output_tokens, total_tokens}` and a `rate_limits` object (`primary`, `secondary`, `credits`,
`plan_type`) — observed on a free-plan session with `primary/secondary = null`; the populated
percentage shape is NOT VERIFIED here.

#### 2.3 Google Gemini (G1, G2, G3)

| Model id | Input | Cache read | Cache write | Output (incl. thinking) | Notes |
|---|---|---|---|---|---|
| gemini-4-carbon | 2.00 → **4.00 from 2026-12-01** | 0.10 → 0.20 | n/a (implicit only) | 10.00 → 20.00 | "Our strongest reasoning model" |
| gemini-4-argon | 2.00 → **4.00 from 2026-12-01** | 0.10 → 0.20 | n/a | 10.00 → 20.00 | efficient frontier reasoning |
| gemini-4-neon | 3.00 | 0.15 | n/a | 15.00 | "Frontier-level intelligence" |
| gemini-4-hydrogen | 1.50 | 0.075 | n/a | 7.50 | everyday coding/agentic (Flash-like) |
| gemini-3.8-flash | 0.75 → 1.50 from 2027-01-01 | 0.075 → 0.15 | storage $0.50 → $1.00 /MTok/h | 3.75 → 7.50 | previous-gen Flash |
| gemini-3.6-flash | 0.75 → 1.50 from 2027-01-01 | 0.075 → 0.15 | storage $0.50 → $1.00 | 3.75 → 7.50 | |
| gemini-3.5-flash-lite | 0.30 | 0.03 | storage $1.00 | 2.50 | |
| gemini-3.1-flash-lite | 0.25 (audio 0.50) | 0.025 | storage $1.00 | 1.50 | |
| gemini-3.1-pro-preview ≤200K | 2.00 | 0.20 | storage $4.50 | 12.00 | preview |
| gemini-3.1-pro-preview >200K | 4.00 | 0.40 | storage $4.50 | 18.00 | |
| gemini-3-flash-preview (legacy) | 0.50 | 0.05 | storage $1.00 | 3.00 | |

Gemini has no per-token "cache write" price: explicit caching is charged as normal input plus hourly
storage; Gemini 4 is "Implicit caching only (explicit caching is not supported)". Implicit caching is on by
default for 2.5+; G3 gives no discount percentage — the discount is implied by the cache price (Gemini 4 =
0.05x input; 3.x = 0.1x). Minimum cacheable prefix 4,096 tokens (Gemini 4, 3.8/3.6 Flash, 3.1 Pro).
The Pro/Flash/Flash-Lite mapping of Gemini 4 codenames is NOT VERIFIED (Google does not label them).
Two scheduled price increases fall within weeks of today, which is the strongest argument for
`effective_from` dates in Rekora's table.

#### 2.4 DeepSeek (D1, D3)

| Model id | Version | Cache hit (peak / off-peak) | Cache miss (peak / off-peak) | Output (peak / off-peak) |
|---|---|---|---|---|
| deepseek-flash | DeepSeek-V4.1-Flash-1017 | 0.006 / 0.003 | 0.30 / 0.15 | 1.20 / 0.60 |
| deepseek-v4-pro | DeepSeek-V4-Pro-0813 | 0.044 / 0.022 | 1.32 / 0.66 | 3.96 / 1.98 |

Peak = 01:00–04:00 and 06:00–10:00 UTC, Mon–Fri, excluding Chinese public holidays; all other hours are
off-peak at 50%. Legacy names `deepseek-v4-flash`, `deepseek-v4-flash-vision-exp` are billed at Flash
price. `deepseek-chat` / `deepseek-reasoner` were retired after 2026-07-24 15:59 UTC (D3). No cache-write
charge is documented (NOT VERIFIED; LiteLLM records 0). Context 1M. DeepSeek pricing therefore needs the
**request timestamp**, not just the token counts.

### 3. Cost formula per provider

Let `P_in, P_cr, P_cw5m, P_cw1h, P_out` be per-token prices for the tier that applies to the request.

**Anthropic — exclusive input counting (A3).** `input_tokens` is only the uncached tail;
`total_input = input_tokens + cache_read_input_tokens + cache_creation_input_tokens`, and
`cache_creation_input_tokens = ephemeral_5m_input_tokens + ephemeral_1h_input_tokens`.

```
cost = input_tokens*P_in + cache_read*P_cr + eph_5m*P_cw5m + eph_1h*P_cw1h + output_tokens*P_out
```
Thinking is inside `output_tokens`. For Haiku 5.5, choose tier by `total_input > 100_000` per request.
Multiply everything by 1.1 when `inference_geo = "us"`; fast mode uses its own base rates. If only the
aggregate `cache_creation_input_tokens` is known, assume 5m (the default) and flag the estimate.

**OpenAI — inclusive input counting (O3).** `usage.input_tokens` is the total;
`input_tokens_details.cached_tokens` (reads, excludes writes) and `cache_write_tokens`
(= `ttl_30m_tokens + ttl_12h_tokens`) are subsets.

```
ordinary = input_tokens - cached_tokens - cache_write_tokens
cost = ordinary*P_in + cached*P_cr + w30m*P_cw30m + w12h*P_cw12h + output_tokens*P_out
```
`output_tokens` includes reasoning (`output_tokens_details.reasoning_tokens` / Codex
`reasoning_output_tokens` are informational — do not add again). Long tier when `input_tokens > 272_000`.
Codex rollout files use `input_tokens` (inclusive), `cached_input_tokens`, `cache_write_input_tokens`
(no TTL split — assume 30m), `output_tokens`, `reasoning_output_tokens`.

**Gemini — inclusive (G4).** `promptTokenCount` "includes the number of tokens in the cached content";
`cachedContentTokenCount` is the cached part; `thoughtsTokenCount` is reported separately from
`candidatesTokenCount` (`totalTokenCount = prompt + toolUsePrompt + thoughts + candidates`).

```
cost = (promptTokenCount + toolUsePromptTokenCount - cachedContentTokenCount)*P_in
     + cachedContentTokenCount*P_cr + (candidatesTokenCount + thoughtsTokenCount)*P_out
```
Tool-use prompt tokens billed as input: NOT VERIFIED. Explicit-cache storage (per hour) is not derivable
from per-request usage; omit and say so. Long tier on 3.1 Pro when prompt > 200K.

**DeepSeek — split counting (D2).** `prompt_cache_hit_tokens` and `prompt_cache_miss_tokens` partition the
input (that they sum to `prompt_tokens` is NOT VERIFIED on D2, but implied by the definitions).

```
cost = hit*P_hit(t) + miss*P_miss(t) + completion_tokens*P_out(t)    # t = request time → peak/off-peak
```

**Generic normalised record for Rekora** (what a host adapter should produce before pricing):
`{provider, model_id, ts, uncached_input, cache_read, cache_write_5m, cache_write_1h, cache_write_30m,
cache_write_12h, output (incl. reasoning), reasoning (informational), service_tier, geo}`. Convert
inclusive providers to exclusive once in the adapter so the pricing code has a single formula.

### 4. Subscriptions: what hosts show, and how to frame estimates

- **Claude Code (A5, A6).** `/usage` shows a Session block with "Estimated cost", computed "locally from
  token counts at list price"; for Pro/Max/Ultra "the session cost figure isn't relevant for billing
  purposes. Subscribers see plan usage bars". Team/Enterprise seats use "a rolling five-hour window and
  a weekly window". Status-line JSON exposes `cost.total_cost_usd` and, only for Pro/Max/Ultra (or a
  gateway with spend limits), `rate_limits.five_hour.used_percentage`, `rate_limits.seven_day.used_percentage`
  (0–100) with `resets_at` (Unix seconds), and `rate_limits.spend_limit.*`. Usage credits beyond the plan
  are dollar-billed; Fable may bill to usage credits. Orgs can set `modelPricing` so the figure uses
  contracted rates.
- **Codex / ChatGPT (O4).** Allowance measured in messages/tasks over 5-hour (not for Pro) and weekly
  windows, plus credits; API-key sessions pay API prices.

**Framing.** Rekora should label every dollar figure **"API-equivalent cost (estimate, list price as of
<table date>)"**, never "spent" or "billed". For subscribers it is a *value/efficiency* metric (what this
task would cost on the API; useful to compare tasks, models, cache hit ratio), not a charge. When the host
provides rate-limit percentages, store them alongside (e.g. "this task used ~6% of the 5-hour window") —
that is the number subscribers actually ration. Record the billing mode per session (`api | subscription |
credits | unknown`) and show "unknown price" rather than $0 when a model id is not in the table. Known
gaps to disclose: contracted discounts, batch, fast/priority tier unless reported, data-residency
multiplier unless reported, server-tool fees (web search), Gemini cache storage.

### 5. Recommendation: dated price table

1. **Ship a snapshot in the binary** (e.g. `crates/rekora-core` holds the pure pricing function and types;
   the data file lives as an embedded TOML/JSON in core or store — core stays free of I/O). Each row:
   `provider, model_id, effective_from (date), effective_to?, unit = "usd_per_mtok", input, cache_read,
   cache_write_5m, cache_write_1h, cache_write_30m, cache_write_12h, output, tier_threshold_tokens?,
   tier_prices?, time_of_day_rule? (DeepSeek), source_url, retrieved_on`. Use integer micro-dollars per
   MTok (or decimal strings) to avoid float drift.
2. **Price by request date**: pick the row whose `effective_from ≤ ts < effective_to`. Necessary now —
   Gemini 4 Carbon/Argon double on 2026-12-01 and 3.8/3.6 Flash on 2027-01-01; OpenAI Ultrafast changes on
   2027-03-01.
3. **Optional, explicit refresh** (`rekora prices refresh --from litellm|models.dev`, never automatic,
   never network by default): download, map fields, show a diff, write to a user-local override file with
   the fetch date; keep the shipped snapshot as fallback. Imported price data is data, validated against
   a schema; reject negative/absurd values.
4. **User overrides** (contracted rates or a multiplier), mirroring Claude Code's `modelPricing`.
5. **Store the computed cost and the table version** with each task estimate so historic numbers do not
   change silently after a refresh; recompute only on request.

### 6. Machine-readable sources

| Source | Format & units | Key fields | License | Cadence (observed) |
|---|---|---|---|---|
| **LiteLLM** `model_prices_and_context_window.json` (github.com/BerriAI/litellm, `main`) | JSON, **USD per token** (floats, e.g. `4e-06`) | `input_cost_per_token`, `output_cost_per_token`, `cache_read_input_token_cost`, `cache_creation_input_token_cost` (5m / OpenAI 30m), `cache_creation_input_token_cost_above_1hr` (Anthropic 1h), `cache_creation_input_token_cost_above_12hr` (OpenAI 12h), tier suffixes `_above_100k_tokens`, `_above_200k_tokens`, `_above_272k_tokens`, mode suffixes `_batches`, `_priority`, `_flex`, `_ultrafast`; `output_cost_per_reasoning_token`; `input_cost_per_token_cache_hit` (DeepSeek); `litellm_provider`, `max_input_tokens`, `deprecation_date`, `source` | MIT (outside `litellm/enterprise/`) | 5 commits to the file on 2026-11-24 alone (GitHub API) — effectively daily |
| **models.dev** `https://models.dev/api.json` (github.com/sst/models.dev; used by OpenCode) | JSON keyed provider → models, **USD per MTok** | `cost.input`, `cost.output`, `cost.cache_read`, `cost.cache_write`, `cost.reasoning`, `cost.input_audio`, `cost.output_audio`, `cost.tiers[{…, tier:{type:"context", size}}]`, legacy `cost.context_over_200k`; per-model `last_updated` | MIT | Community PRs on TOML files; e.g. gpt-6.2-sol `last_updated` 2026-11-17, gemini-4-carbon 2026-11-10 |
| **OpenRouter** `GET /api/v1/models` (no auth in examples) | JSON, **USD per token as decimal strings** | `pricing.prompt`, `completion`, `request`, `image`, `web_search`, `internal_reasoning`, `input_cache_read`, `input_cache_write`, `input_cache_write_1h`, `overrides[{min_prompt_tokens,…}]` | Terms not reviewed — NOT VERIFIED for redistribution | Live |

Spot check vs official pages (2026-11-24): LiteLLM and models.dev match A1/O1 for claude-opus-5-5,
claude-sonnet-5-5, claude-haiku-5-5 (both tiers), claude-fable-5-2, gpt-6.2-sol/astra, gpt-6.1-terra,
gpt-6-luna, gpt-5.6-sol (LiteLLM `max_input_tokens` 922000 for 6.x vs 1.05M context on O2 — context
includes output). LiteLLM stores DeepSeek **peak** prices only (no off-peak rule); models.dev stores
DeepSeek **off-peak** prices (0.66/1.98/0.022) — they disagree, both lose the time rule. Neither has the
Gemini 2026-12-01 increase yet (both show Carbon $2/$10). OpenRouter prices are OpenRouter's resale/
routing prices, not provider list prices (deepseek-v4-pro $1.28/$2.56; gemini-4-carbon shows an
`input_cache_write` of $0.1875 that Google does not list), so do not use it as a list-price source.

**Licensing notes.** LiteLLM (MIT) and models.dev (MIT) can be vendored or transformed in a snapshot if the
MIT notice is included (e.g. `THIRD_PARTY_NOTICES` with copyright line). Prices themselves are facts, but
copying the files is cleanest under MIT with attribution. Avoid shipping OpenRouter data. Prefer models.dev
for the shipped mapping (per-MTok units, explicit tiers, `last_updated`) and LiteLLM as a cross-check /
refresh source (richer TTL and tier fields: 1h vs 12h writes). Both lack time-dated history and time-of-day
rules, so Rekora's own table must add `effective_from` and the DeepSeek peak rule, curated from the
official URLs above.

### 7. Open items (NOT VERIFIED)

- Whether pre-5.6 OpenAI models charge cache writes (no write column on O1).
- Gemini: whether `toolUsePromptTokenCount` is billed as input; Gemini 4 tier (Pro/Flash) labels.
- DeepSeek: cache-write charge (none documented); `prompt_tokens = hit + miss`.
- Codex `rate_limits.primary/secondary` populated shape on paid plans (only a free-plan null sample seen).
- OpenRouter data terms for redistribution.
