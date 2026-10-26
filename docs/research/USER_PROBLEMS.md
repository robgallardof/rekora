# User problems

Collected 2026-10-26. **Evidence quality is mixed**: most sources found are vendor blogs and product pages,
not primary user reports; a search for Reddit threads returned none. Each item is labelled. No user
interviews were run.

| # | Problem | Evidence | Quality | Rekora response |
|---|---|---|---|---|
| P1 | Sessions lose decisions and instructions; resumed sessions start fresh | Mirrored issue "[BUG] Severe Session Memory Loss", opened 2025-06-24, closed 2025-08-21 ([claudeissues.com](https://claudeissues.com/issue/2545-bug-severe-session-memory-loss-instructions-and-context-not-retained)); vendor post citing anthropics/claude-code #43696 ([memorylake.ai](https://www.memorylake.ai/blogs/claude-code-forgets-project-context)) | Mirror of a real issue + vendor claim (#43696 not opened by us) | Durable memories and tasks outside the conversation |
| P2 | Compaction drops negative constraints and rejected hypotheses | [fast.io](https://fast.io/resources/claude-code-degradation/) | Vendor article | `failures` and `constraints` lists in tasks; constraints rank highest |
| P3 | Instruction files grow and cost tokens (claims of 1–3k typical, 22k bloated) | [fast.io](https://fast.io/resources/claude-code-degradation/), agentmemory listing on [glama.ai](https://glama.ai/mcp/servers/exr7y451z0) | Vendor numbers, unverified | Budgeted packs; mean 98 tokens on our relevance suite at budget 1024 |
| P4 | Old rules recalled as current; need deprecated/disputed/superseded states | agentmem listing ([glama.ai](https://images.glama.ai/mcp/servers/Thezenmonster/agentmem)), question in [hunted.space](https://webmail.hunted.space/product/agent-memory-dev) | Product pages | `superseded` excluded from search, `disputed` down-weighted, stale-source flag |
| P5 | Memory stores are a persistence vector for prompt injection | [Akto memory injection](https://www.akto.io/mcp-attack-matrix/memory-injection), [practical-devsecops](https://www.practical-devsecops.com/mcp-prompt-injection-attack-vectors/), PoC listing ([glama.ai](https://glama.ai/mcp/servers/bountyyfi/ProjectMemory)), [Trail of Bits on tool-description "line jumping"](https://blog.trailofbits.com/2025/04/21/jumping-the-line-how-mcp-servers-can-attack-you-before-you-ever-use-them/) | Security vendor write-ups + one PoC + Trail of Bits research | Agent/import text never verified; "data, not instructions" header; untrusted-origin labels; no execution |
| P6 | Many memory tools need an LLM and API key just to store a fact | Mem0 README: "requires an LLM to function" ([mem0](https://github.com/mem0ai/mem0)); Graphiti requires LLM + embeddings ([graphiti](https://github.com/getzep/graphiti)) | Primary (project READMEs) | No model in any path |
| P7 | Host approval prompts block memory tools in non-interactive runs | Observed by us: Codex 0.164.0 `exec` refused `rekora/search` ("MCP tool call requires approval, but approval policy is never") until tools declared `readOnlyHint` | Primary (our own run, 2026-10-26) | Read-only annotations on read tools |
| P8 | Keyword search misses inflections ("invoice archiving" vs "Invoices are archived") | Observed by us with Codex 0.164.0 | Primary (our own run) | Porter stemming (migration v4) |

## Not yet investigated

Duplicate memories at scale, cross-repository leaks in other tools, worktree confusion, multi-agent edit
conflicts in the wild. These are design inputs from the specification, not observed user reports.
