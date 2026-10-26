# Host adapters and compatibility

```bash
rekora agent list                         # hosts, versions, config state
rekora agent install claude               # dry run: shows the planned change
rekora agent install claude --confirm     # writes .mcp.json (backup first)
rekora agent install codex --confirm      # writes $CODEX_HOME/config.toml (user-wide!)
rekora agent uninstall codex --confirm    # removes only the `rekora` entry
```

| Host | Config | Scope | Status | Evidence (2026-10-26, Windows 11) |
|---|---|---|---|---|
| Claude Code | `<checkout>/.mcp.json` → `mcpServers.rekora` | project | **Tested on real host** 2.1.310 | `claude mcp get rekora` reads it (pending approval until you approve it in Claude Code); `claude -p … --mcp-config .mcp.json` called `search` and returned the right id (`r20_real_claude_code_calls_rekora`) |
| Codex CLI | `$CODEX_HOME/config.toml` (default `~/.codex`) → `[mcp_servers.rekora]` | **user** | **Tested on real host** 0.164.0 | `codex mcp list` shows it; `codex exec` called `search` and returned the right id (`r20_real_codex_calls_rekora`, config passed with `-c`, user file untouched) |
| Any other MCP client (Cursor, Gemini CLI, OpenCode, Antigravity, Hermes …) | Configure `rekora mcp serve --stdio` manually | — | Contract-tested only (real JSON-RPC client in tests) | — |

Not implemented: lifecycle hooks (auto-capture, session start injection) for any host. Hosts differ in hook
contracts and MCP alone does not provide them; agents call `context`/`handoff_resume` explicitly instead.

## How the server finds the project

The host launches `rekora mcp serve --stdio` (no project argument). On each call the server discovers the
project from its working directory — both tested hosts start servers in the session directory. If you launch it
elsewhere, pass `--project <path>` in the host config.

## Approval behaviour

Tools declare MCP annotations. Read tools (`search`, `context`, `symbols`, `status`, `task_list`) are
`readOnlyHint: true`; `remember`, `task_update`, `handoff_*` are `readOnlyHint: false, destructiveHint: false`.
Codex `exec` with approval policy `never` refused unannotated tools and accepts read-only ones.

## Safety guarantees (tested)

Dry run by default · refuses malformed or read-only files without changing a byte · keeps unrelated servers
(JSON key order; TOML comments and formatting via `toml_edit`) · backup under `.rekora/backups/` · aborts if the
file changed during planning · atomic replace · second install is a byte-for-byte no-op · uninstall restores the
original content, and deletes the file only if Rekora created it.
