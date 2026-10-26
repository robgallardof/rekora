# Rekora — Blueprint Pack

**This ZIP contains a detailed implementation specification, NOT a completed Rekora program or a GitHub repository.** The contained commands specify interfaces to implement and test, not working binary commands yet.

## Files

1. `REKORA_PROMPT_MAESTRO_ES.md`: prompt completo en español para copiar y ejecutar con un agente de programación.
2. `REKORA_MASTER_PROMPT.md`: detailed English technical annex and standalone agent prompt.
3. `REKORA_IMPLEMENTATION_PLAN.md`: phased engineering milestones, dependencies, first PRs, acceptance and release checklist.
4. `REKORA_TEST_MATRIX.md`: 36 unit/integration/E2E/concurrency/security/performance scenarios with actual Git+SQLite+MCP subprocess fixtures.
5. `REKORA_AGENT_ROLES.md`: specialized engineering subagents, file ownership, review rules and example handoffs.

## Suggested use (any coding agent)

- Create an empty directory and place these four files in `planning/`.
- Send the agent the prompt below, then point it at `planning/REKORA_PROMPT_MAESTRO_ES.md` and the technical English annex:

```text
Implement REKORA from planning/REKORA_PROMPT_MAESTRO_ES.md
and planning/REKORA_MASTER_PROMPT.md.
Use planning/REKORA_IMPLEMENTATION_PLAN.md,
planning/REKORA_TEST_MATRIX.md, and
planning/REKORA_AGENT_ROLES.md as binding specifications.
Start with the smallest working vertical slice. Do not stop at planning.
All Rust code must include meaningful module-level and public API comments.
Run real tests after each increment and report only observed results.
Do not assume external MCPs, proprietary agents, cloud services or paid models
are installed. Do not touch global agent settings without consent.
Keep Jev-style decision models, automatic skills and vector inference
experimental until reproducible benchmarks justify them.
```

- Require a successful `cargo fmt`, `cargo clippy`, `cargo test` and integration subprocess run before accepting the first implementation milestone.
- Only after the code exists and works, publish the GitHub repo named Rekora (subject to availability/license checks).

## Research links to verify at implementation time

- Kindex: https://github.com/wandercom/kindex
- Hermes Agent: https://github.com/NousResearch/hermes-agent
- MCP Rust SDK: https://github.com/modelcontextprotocol/rust-sdk
- MCP specification: https://modelcontextprotocol.io/specification
- MCP July 2026 changes: https://blog.modelcontextprotocol.io/posts/2026-07-28/
- Rust tooling: https://doc.rust-lang.org/cargo/ and https://nexte.st/

Re-check date-sensitive SDKs and capabilities; do not infer universal agent compatibility from MCP alone.
