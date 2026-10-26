# ADR-0002 — Host adapters, MCP tool surface and approval policy

- Status: accepted
- Date: 2026-10-26

## Context

Agents need memory and task continuity without the user editing configs by hand, but host configs are
user-owned and sometimes user-wide (Codex). MCP alone does not define lifecycle hooks, and hosts gate tool
calls behind approval policies.

## Decision

1. **Nine tools, not fifty.** `search`, `context`, `remember`, `status`, `task_list`, `task_update`,
   `handoff_create`, `handoff_resume`, `symbols`. Each is bounded in input and output.
2. **Write policy at the protocol boundary.** MCP may add `candidate` memories and edit tasks (the agents'
   own working state, version-checked). Verify, dispute, supersede, delete and adapter installs are CLI-only.
3. **Annotations.** Read tools declare `readOnlyHint: true`; writes declare `destructiveHint: false`. Measured:
   Codex 0.164.0 `exec` refused an unannotated `search` and accepted it once annotated.
4. **Project discovery per call** from the server's working directory, so one host entry works for any
   project and a server outside a project returns `not_initialized` instead of failing to start.
5. **Adapters edit exactly one owned entry** (`rekora`): Claude Code project `.mcp.json`, Codex
   `$CODEX_HOME/config.toml`. Dry run by default; parse-or-refuse; backup; concurrent-change check; atomic
   write; uninstall restores; created files are deleted on uninstall. `toml_edit` preserves comments.
6. **No hooks.** Session-start injection and auto-capture differ per host and would write memory without
   review. Agents call `context` / `handoff_resume` explicitly (server instructions say so).

## Consequences

- Codex installs are user-wide; the dry run says so and `--confirm` is required.
- Claude Code asks the user to approve project servers once — expected and desirable.
- Real-host tests spend tokens and are `#[ignore]`d; they were run on 2026-10-26 and passed.
