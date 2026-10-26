# Competitive matrix

Checked on 2026-10-26 from each project's own README (linked). Rekora's column only lists what is
implemented and tested in this repository; see [docs/STATUS.md](../STATUS.md). No head-to-head
benchmark against any of these projects has been run yet.

| Capability | [Kindex](https://github.com/wandercom/kindex) (v0.49.0) | [Hermes Agent](https://github.com/NousResearch/hermes-agent) | [Mem0](https://github.com/mem0ai/mem0) | [Graphiti](https://github.com/getzep/graphiti) | **Rekora** (this repo) |
|---|---|---|---|---|---|
| What it is | Local knowledge graph + CLI shared by coding agents | Self-improving general agent (terminal + chat gateways) | Memory layer library/server/cloud for AI apps | Temporal context-graph framework | Local memory + handoffs for coding agents |
| Runtime | Python ≥3.10 (pip/uv) | Python + Node | Python / TypeScript, Docker server, cloud | Python ≥3.10 | One Rust executable (SQLite bundled) |
| Storage | SQLite (`~/.kindex`) + git-tracked `.kin/` | Not detailed in README (MEMORY.md/USER.md import mentioned) | Vector store (Qdrant mentioned) | Neo4j / FalkorDB / Neptune (required) | SQLite per project in `.rekora/` (git-ignored) |
| LLM needed to write memory | No (optional features) | Yes (agent-curated, LLM summaries) | **Yes** ("requires an LLM to function") | **Yes** (LLM + embeddings) | **No** |
| Retrieval | FTS5 BM25 + graph traversal (RRF), optional vectors | FTS5 session search + LLM summarisation | Embeddings + entity linking | Graph + hybrid search | FTS5 BM25 (porter) + coverage floor + explained score |
| Abstains when nothing matches | Grounding floor, shadow mode by default | Not stated | Not stated | Not stated | **Yes**, `no_evidence` (specificity 1.0 on our 10 unanswerable questions) |
| Provenance / staleness | Content digests, asserted vs observed time, `kin verify` | Not stated | Not stated | Episodes as provenance, bi-temporal invalidation | BLAKE3 source hashes → `possibly_stale`; audit log |
| Agent text trusted? | Auto-capture goes to a review queue | Agent curates its own memory | LLM extracts memories | LLM extracts facts | Agent text is `candidate`; only CLI (human) verifies |
| MCP | `kin-mcp`, "50+ native tools", remote HTTP mode | MCP client (connects to servers) | Not in README (agent skills instead) | Reference MCP server (no auth, per README) | 9 tools over stdio, read-only annotations |
| Host integrations | Claude Code plugin + hooks, Codex, Gemini CLI, Antigravity, OpenCode, Cursor | It *is* the agent | LangGraph, CrewAI, agent skills | Via MCP | Claude Code (`.mcp.json`), Codex (`config.toml`); no hooks |
| Task handoff between agents | Coordination features listed | n/a | n/a | n/a | Versioned tasks + `rekora.handoff/1` document |
| Branch / worktree awareness | Not stated | n/a | n/a | n/a | Shared store across worktrees, per-branch code index, `branch_mismatch` |
| Code index | Incremental code analysis | n/a | n/a | n/a | tree-sitter declarations for Rust, Python, JS, TS/TSX, C# |
| Context budget | 5 tiers (~4000 → ~100 tokens) | n/a | n/a | n/a | Exact budget, whole-item packing, conservative estimator |
| Skills / procedural learning | — | Autonomous skill creation (agentskills.io) | Agent skills | — | Not implemented (deliberately) |
| License | MIT | MIT | Apache-2.0 | Apache-2.0 | MIT |

## Honest reading

- **Kindex already covers most of Rekora's surface** — and more (graph, hooks for many hosts, remote mode,
  tiers). Rekora is not unique in having memory, provenance, MCP or hooks.
- What Rekora tests as a *difference* is narrower: one dependency-free executable; no LLM in any write path;
  agent text can never self-verify; strict `no_evidence` abstention; per-project physical isolation;
  task handoffs as a versioned document; worktree-aware storage. Whether those matter more than
  Kindex's breadth is an open question for users, not something this table proves.
- Mem0's published scores (LoCoMo 92.5, LongMemEval 94.4) are self-reported, on its managed platform,
  for conversational memory — a different task from code-project continuity. They are not comparable to
  our relevance suite.

## Reuse instead of reimplementing

- MCP: official `rmcp` SDK rather than a hand-written JSON-RPC layer.
- Parsing: tree-sitter grammars rather than custom parsers.
- Search: SQLite FTS5 (BM25, porter) rather than a custom index.
- Config editing: `toml_edit` (format-preserving) rather than regex edits.
- Ideas borrowed conceptually (no code copied): Kindex's asserted/observed split and review queue;
  Graphiti's invalidate-don't-delete history.

## Next comparison to run

Same 40-memory labelled corpus loaded into Kindex (FTS-only mode, no embeddings) on the same machine,
measuring P@1, R@5, MRR, no-answer specificity and context tokens. Not executed yet.
