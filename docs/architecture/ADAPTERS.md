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
| OpenCode 1.x | `<checkout>/opencode.json` → `mcp.rekora` | project | **Real discovery checked** 1.19.4 | `opencode mcp list` connects in an isolated Git checkout; model inference NOT EXECUTED |
| DeepSeek Harness | `<checkout>/.dsh/rekora.cordis.yml` | project | **Real MCP runtime checked** 0.2.1-rc.2 | official runtime discovers 9 tools and executes `status` against temporary SQLite; model inference NOT EXECUTED |
| Any other MCP client (Cursor, Gemini CLI, Antigravity, Hermes …) | Configure `rekora mcp serve --stdio` manually | — | Contract-tested only (real JSON-RPC client in tests) | — |

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

## OpenCode and DeepSeek Harness

```sh
rekora agent install opencode                 # dry run
rekora agent install opencode --confirm
opencode mcp list
rekora agent install deepseek --confirm       # alias: dsh
# The overlay is opt-in, never a user-wide settings edit:
dsh web --patch .dsh/rekora.cordis.yml
rekora agent uninstall opencode --confirm
rekora agent uninstall deepseek --confirm
```

OpenCode uses the **1.x** project schema (`type: local`, command array, `enabled`). Existing
`opencode.jsonc` and 2.x `mcp.servers` configurations are refused rather than overwritten.
DeepSeek uses an explicit overlay for `@deepseek-ai/dsh-mcp-client`; JSON output is valid YAML.
Its `cwd` is pinned to this checkout. The API/model alone is not an MCP host: a DeepSeek model
in OpenCode uses the OpenCode adapter; the official Harness uses the DeepSeek adapter.
No provider key or default model is written. Both adapters preserve unrelated entries, back up
changes, default to dry run and support owned-entry removal.

`python scripts/test_agent_hosts.py` checks real OpenCode discovery and an actual DeepSeek MCP
`status` call, all in temporary directories. Build Rekora first; set `OPENCODE_BIN` and
`AGENT_HOST_MODULES` to an isolated npm installation containing `opencode-ai@1.19.4` and
`@deepseek-ai/dsh@0.2.1-rc.2` (Node 24.9+). CI reproduces this in `agent-hosts.yml`.
No model request is made and provider inference remains **NOT EXECUTED** without credentials.

Contracts checked 2026-10-26:
[OpenCode MCP](https://opencode.ai/docs/en/mcp-servers),
[official Harness MCP](https://github.com/deepseek-ai/deepseek-harness/tree/master/packages/mcp/mcp-client).
