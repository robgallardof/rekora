# Agent hosts: MCP configuration, rules, hooks, subagents and display surfaces

Research date: 2026-11-24, from each host's official documentation (evidence classes and NOT VERIFIED
marks inline). Covers the hosts beyond Claude Code, Codex, OpenCode, DeepSeek Harness and Antigravity,
which are in [PLATFORM_CAPABILITY_MATRIX.md](PLATFORM_CAPABILITY_MATRIX.md) and
[MODEL_DELEGATION.md](MODEL_DELEGATION.md). No host below was run (**NOT EXECUTED**).

Adapters implemented from this research (project scope, dry-run unless `--confirm`, entry launched with
`--project <root>` because most hosts do not document the server's working directory, JSONC refused):
`cursor` (`.cursor/mcp.json`), `kiro` (`.kiro/settings/mcp.json`), `roo` (`.roo/mcp.json`),
`factory` (`.factory/mcp.json`), `gemini` (`.gemini/settings.json`), `qwen` (`.qwen/settings.json`).
GitHub Copilot CLI and Grok CLI read the project `.mcp.json` written by `rekora agent install claude`.
Every other host gets manual instructions (README, "Agents without Rekora hooks").

## Long-tail coding-agent hosts: MCP, rules, hooks, subagents, display (research, 2026-11-24)

Scope: hosts other than Claude Code, Codex CLI, OpenCode, DeepSeek Harness and Antigravity.
Method: official docs fetched on 2026-11-24 with WebFetch and WebSearch. Nothing was installed or
run. No host integration was tested: **every row is NOT EXECUTED.** "NOT VERIFIED" means the fact
was not found on an official page I fetched. In those cases the row gives secondary or
third-party evidence, or nothing.

Rekora note: `rekora mcp serve --stdio` resolves the project from `--project` or the process cwd
(`crates/rekora-cli/src/main.rs:381,449`, `mcp.rs:14`). Most hosts do **not document** the cwd
for stdio servers, so adapters should write an explicit `--project <abs path>`. Where the host
supports `${workspaceFolder}`, the adapter can use that instead.

### Summary table

| Host | MCP config path(s) | Format + key (stdio) | cwd behaviour | Rules file(s) | Hooks | Subagents / model control | Display surface | Evidence |
|---|---|---|---|---|---|---|---|---|
| Cursor (editor + CLI) | project `.cursor/mcp.json`; user `~/.cursor/mcp.json` | JSON, `mcpServers.<n>.{type:"stdio",command,args,env,envFile}` | `cwd` key not documented; `${workspaceFolder}` = folder containing `.cursor/mcp.json` | `.cursor/rules/*.mdc`, `AGENTS.md` (nested), user/team rules in UI | Yes: `.cursor/hooks.json`, `~/.cursor/hooks.json` (`version:1`); `sessionStart`/`postToolUse` can return `additional_context` | `.cursor/agents/*.md` (+ `.claude/agents`, `.grok/agents`), frontmatter `model: inherit` or model id | None documented for third parties (NOT VERIFIED) | Official, undated |
| Windsurf, now **Devin Desktop** (Cognition) | Cascade/Devin: project `.devin/mcp_config.json`, `.devin/mcp_config.local.json`; user `~/.config/devin/mcp_config.json` (Win `%AppData%/devin/mcp_config.json`). Legacy editor discovery: `~/.codeium/windsurf/mcp_config.json` | JSON, `mcpServers.<n>.{command,args,env}`; `${env:VAR}` | Not documented | `AGENTS.md`/`AGENTS.local.md`/`AGENT.md` (root to cwd), `.devin/rules/*.md`, `~/.devin/rules`, legacy `.windsurf/rules`, `.windsurfrules` | `.devin/hooks.v1.json` (project/user); `PreToolUse` shown; "add context" stated but not specified | Shares harness with Devin CLI; NOT VERIFIED | NOT VERIFIED | Official, undated; MCP path moved in Devin CLI v3000.3 |
| Gemini CLI | project `.gemini/settings.json`; user `~/.gemini/settings.json`; system `/etc/gemini-cli/settings.json` | JSON, `mcpServers.<n>.{command,args,env,cwd,timeout,trust}` | `cwd` key exists; default not documented | `GEMINI.md` (global `~/.gemini/GEMINI.md`, workspace, JIT); `context.fileName` can add `AGENTS.md` | `settings.json` `hooks`: SessionStart, SessionEnd, BeforeAgent, AfterAgent, BeforeModel, AfterModel, BeforeToolSelection, BeforeTool, AfterTool, PreCompress, Notification; SessionStart injects context | `.gemini/agents/*.md`, `~/.gemini/agents/*.md`, `model:` (default `inherit`); `/model` | NOT VERIFIED | Official, MCP page updated 2026-10-06. **Replaced by Antigravity CLI for unpaid/Google One users on 2026-06-18** |
| VS Code (generic + Copilot agent mode) | workspace `.vscode/mcp.json`; user-profile `mcp.json`; Copilot-portable `.mcp.json` (workspace root) and `~/.copilot/mcp-config.json` | JSON: editor files use `servers.<n>.{type:"stdio",command,args,env,envFile,cwd}`; portable files use `mcpServers` (no `envFile`) | **Documented: defaults to the workspace folder**; `${workspaceFolder}` supported | `.github/copilot-instructions.md`, `.github/instructions/*.instructions.md`, `AGENTS.md` (via Copilot) | Copilot hooks (see Copilot CLI); VS Code-side NOT VERIFIED | Custom agents `.github/agents`; model picker | Status bar via a VS Code extension API (a real third-party surface) | Official, 2026-11-18. JSONC comments NOT VERIFIED on page (VS Code opens it in JSONC mode) |
| GitHub Copilot CLI | user `~/.copilot/mcp-config.json` (`$COPILOT_HOME`); project `.mcp.json` then `.github/mcp.json` (walks cwd up to repo root; `.mcp.json` wins) | JSON, `mcpServers.<n>.{type:"local",command,args,env,tools:["*"]}`; bare map also accepted; does **not** read `.vscode/mcp.json` | `cwd` documented only in the VS Code reference for the portable format | `.github/copilot-instructions.md`, `.github/instructions/**`, `AGENTS.md` | `.github/hooks/*.json`, `~/.copilot/hooks/*.json`, inline `hooks` in `.github/copilot/settings.json`/`~/.copilot/settings.json`; `version:1`; `additionalContext` on sessionStart, postToolUse, postToolUseFailure, subagentStart, notification | Agent profiles `~/.copilot/agents`, `.github/agents`; built-ins Explore/Task/General/Code review/Research/Rubber duck; `--agent=` | `statusLine` command in `~/.copilot/settings.json`: **NOT VERIFIED** (community blogs only, experimental) | Official, undated |
| Cline | IDE: file opened from the Configure tab (globalStorage); CLI `~/.cline/data/settings/cline_mcp_settings.json` (`CLINE_MCP_SETTINGS_PATH`) | JSON, `mcpServers.<n>.{command,args,env,disabled,autoApprove}` | Not documented | `.clinerules/` or `.cline/rules/`, global `~/Documents/Cline/Rules`, `AGENTS.md`, `~/.agents/AGENTS.md`, auto-detects `.cursorrules`/`.windsurfrules` | Moved to "SDK Plugins" page; NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | Official, undated. No project-scope MCP file documented |
| Roo Code | global `mcp_settings.json` (UI, no OS path given); project `.roo/mcp.json` (wins over global) | JSON, `mcpServers.<n>.{command,args,cwd,env,alwaysAllow,disabled,timeout,watchPaths,disabledTools}`; `${env:VAR}` in args only | **Documented: `cwd` defaults to the first workspace folder** | `.roo/rules` (NOT VERIFIED on fetched page) | NOT VERIFIED | Modes; NOT VERIFIED | NOT VERIFIED | Official, last updated 2026-05-15; docs moved to roocodeinc.github.io |
| Kilo Code | global `~/.config/kilo/kilo.jsonc`/`kilo.json`; project `kilo.jsonc`, `.kilo/kilo.jsonc` (+ `.json`) | **JSONC**, `mcp.<n>.{type:"local",command:[...],environment,enabled,timeout}` (OpenCode-style); legacy `mcpServers` example still on page | Not documented | `AGENTS.md` (fallback `AGENT.md`), subtree AGENTS.md, also reads `.claude/` and `.agents/` | NOT VERIFIED | Custom modes/subagents, `agent.<name>.prompt`; model NOT VERIFIED | NOT VERIFIED | Official, undated. `.kilocode/mcp.json` no longer documented |
| Kiro (AWS) | workspace `.kiro/settings/mcp.json`; user `~/.kiro/settings/mcp.json`; agent JSON `mcpServers` has the highest priority | JSON, `mcpServers.<n>.{command,args,env,disabled,autoApprove,disabledTools}` | `cwd` not documented | `.kiro/steering/*.md`, `~/.kiro/steering/`, `AGENTS.md` (root, subdirs, always included) | `.kiro/hooks/<id>.json` (`version:"v1"`, `hooks[]`; triggers PostFileSave, PreToolUse, PostToolUse, Stop...; action `command` (runs in project root, JSON on stdin) or `agent` (injects prompt)); global `~/.kiro/hooks/` (CLI V3) | Custom agents (agent JSON `resources`, `mcpServers`); model NOT VERIFIED | NOT VERIFIED | Official: MCP page 2026-11-24, hooks 2026-11-15, steering 2026-10-21 |
| Zed | settings file (`zed: open settings file`); repo docs: `~/.config/zed/settings.json`, Win `%AppData%\Zed\settings.json`; project `.zed/settings.json` NOT VERIFIED | JSON(C), `context_servers.<n>.{command,args,env}` | Not documented | First match of `.rules`, `.cursorrules`, `.windsurfrules`, `.clinerules`, `.github/copilot-instructions.md`, `AGENT.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` (repo docs); now "Instructions/Skills", personal `AGENTS.md` | NOT VERIFIED | Agent profiles; NOT VERIFIED | NOT VERIFIED | Official MCP page (v0.224.0, undated). Settings are commented JSONC in practice: **NOT VERIFIED** |
| Qwen Code | project `.qwen/settings.json`; user `~/.qwen/settings.json` | JSON, `mcpServers.<n>.{command,args,env,cwd,timeout,trust}` | `cwd` key exists; default not documented | `QWEN.md` (mentioned); `AGENTS.md` NOT VERIFIED | `settings.json` hooks, 23 events incl. SessionStart, UserPromptSubmit, PreCompact, Stop, SubagentStart; `hookSpecificOutput.additionalContext`; stdout reaches context on SessionStart/UserPromptSubmit/UserPromptExpansion | `.qwen/agents/`, `~/.qwen/agents/`; `model: inherit / fast / id / authType:modelId` | "Status Line" page exists (not read): NOT VERIFIED | Official: MCP 2026-10-19, subagents 2026-11-13 |
| Amp (Sourcegraph/Amp Inc.) | user `~/.config/amp/settings.json(c)` (Win `%USERPROFILE%\.config\amp\...`); workspace nearest `.amp/settings.json(c)` | **JSONC allowed**, flat dotted key `"amp.mcpServers".<n>.{command,args,env}` | Not documented; workspace servers need `amp mcp approve` | `AGENTS.md` (cwd and parents to $HOME, subtrees), `~/.config/amp/AGENTS.md`, fallback `AGENT.md`/`CLAUDE.md` | Not on fetched pages (NOT VERIFIED) | Task, Read Thread built in; Oracle/Librarian/Finder as plugins with editable `MODEL`; subagents inherit thread model | NOT VERIFIED | Official, undated |
| Goose (Block / AAIF) | user only: `~/.config/goose/config.yaml`; Win `%APPDATA%\Block\goose\config\config.yaml` | **YAML**, `extensions.<n>.{type:stdio,name,enabled,cmd,args,env_keys,envs,timeout}` | Not documented | `AGENTS.md`, then `.goosehints` (`CONTEXT_FILE_NAMES`), global `~/.config/goose/.goosehints` | NOT VERIFIED | Subagents, recipes; per-subagent model NOT VERIFIED | NOT VERIFIED | Official mirror goose-docs.ai, undated (block.github.io pages 404) |
| Crush (Charm) | **`crushrc` (Bash with builtins)**: `./.crushrc`, `./crushrc`, `~/.config/crush/crushrc` (Win `%USERPROFILE%\.config\crush\crushrc`); JSON `crush.json` **deprecated** | `mcp add <n> --command .. --args .. --env K V`; JSON `{"type":"stdio","command","args"}` | Not documented | `AGENTS.md` (default init name), `~/.config/crush/CRUSH.md`, `~/.config/AGENTS.md` | "Preliminary" (`docs/hooks`, not read) | Model picker `ctrl+l`, `model add` | NOT VERIFIED | Official README |
| Aider | **No native MCP** (third-party 2026 reviews; issue #4506): NOT VERIFIED officially | n/a | n/a | `CONVENTIONS.md` via `--read`, or `.aider.conf.yml` `read:` | No | `--model`, `--editor-model` (NOT VERIFIED here) | No | Official conventions page; MCP status from secondary sources |
| Continue | project `.continue/mcpServers/*.yaml` (one file per block); also accepts **JSON files copied from Claude Desktop/Cursor/Cline** in that folder | YAML block `name,version,schema:v1,mcpServers:[{name,type:stdio,command,args,env}]` | `cwd` not documented | `.continue/rules` (NOT VERIFIED) | NOT VERIFIED | Per-role models in config (NOT VERIFIED) | NOT VERIFIED | Official, undated |
| JetBrains Junie | project `.junie/mcp/mcp.json`; user `~/.junie/mcp/mcp.json` (IDE settings page writes here); CLI `--mcp-location` | JSON, `mcpServers.<n>.{command,args,env}` or url | NOT VERIFIED | `.junie/AGENTS.md` (exclusive if present), root `AGENTS.md`, legacy `.junie/guidelines.md`; CLI global `~/.junie/AGENTS.md` | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | Official search snippets (junie.jetbrains.com); page not fully read |
| JetBrains AI Assistant | IDE settings: Tools > AI Assistant > MCP; JSON import; level Global or Project | JSON import `mcpServers.<n>.{command,args}`; **"working directory" field in dialog** | Explicit working-dir field | `.aiassistant/rules` NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | IDE plugin widgets (NOT VERIFIED) | Official, AI Assistant 2026.3 Help. Storage file not documented: UI only |
| Warp | global `~/.warp/.mcp.json`; project `{repo}/.warp/.mcp.json` (CLI page says `~/.warp_cli/.mcp.json`) | JSON, `mcpServers.<n>.{command,args,env,working_directory}` | `working_directory` optional; docs advise setting it | `AGENTS.md` or `WARP.md`; global `~/.agents/AGENTS.md` | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | docs.warp.dev search snippets only (fetch 404): **NOT VERIFIED** |
| Trae (ByteDance) | project `.trae/mcp.json` (third-party), UI Settings > MCP | JSON `mcpServers` (third-party) | NOT VERIFIED | `.trae/rules/project_rules.md` vs `.trae/project_rules.md` (sources disagree) | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | **NOT VERIFIED** (official page truncated) |
| Factory Droid CLI | user `~/.factory/mcp.json`; folder (ancestor) and project `.factory/mcp.json` | JSON, `mcpServers.<n>.{type:"stdio"(optional),command,args,env,disabled}`; `${NAME}` in env | Not documented | `AGENTS.md` (nav link only; NOT VERIFIED) | `~/.factory/` and `.factory/` `hooks.json`, `settings.json`/`settings.local.json` `hooks`; PreToolUse, PostToolUse, UserPromptSubmit, Notification, Stop, SubagentStop, PreCompact, SessionStart, SessionEnd; SessionStart/UserPromptSubmit `additionalContext` | Custom droids (`.factory/droids`) NOT VERIFIED | NOT VERIFIED | Official (docs.factory.com), undated |
| Augment (IDE + Auggie CLI) | IDE: Settings panel, Easy MCP, "Import from JSON"; Auggie CLI `~/.augment/settings.json` | JSON, `mcpServers.<n>.{command,args,env}` | Not documented | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | Official, undated; Auggie 1.x prerelease |
| Claude Desktop | user only: macOS `~/Library/Application Support/Claude/claude_desktop_config.json`, Win `%APPDATA%\Claude\claude_desktop_config.json`, Linux `~/.config/Claude/claude_desktop_config.json` | JSON, `mcpServers.<n>.{command,args,env}` | **No workspace; docs say paths must be absolute** | n/a (Projects UI) | No | No | No | Official modelcontextprotocol.io |
| Grok CLI (xAI, beta) | `~/.grok/config.toml` (Win `%USERPROFILE%\.grok\config.toml`), project `.grok/config.toml`; **also reads `.mcp.json`, `.cursor/mcp.json`, `~/.claude.json`** (lower priority) | TOML `[mcp_servers.<n>] command,args,env,startup_timeout_sec,tool_timeout_sec` | Not documented | NOT VERIFIED | NOT VERIFIED | Reads `.grok/agents` (per Cursor docs) NOT VERIFIED | NOT VERIFIED | Official docs.x.ai, undated. Ignore community `superagent-ai/grok-cli` `.grok/settings.json` |
| Kimi CLI (Moonshot) | user `~/.kimi/mcp.json`; `--mcp-config-file` | JSON, `mcpServers.<n>.{command,args,env}` | Not documented | NOT VERIFIED | "Hooks (Beta)" page exists, not read | "Agents and Subagents" page exists, not read | NOT VERIFIED | Official, undated |
| Mistral Vibe | user `config.toml` (`~/.vibe/config.toml`, NOT VERIFIED officially), project `.vibe/config.toml` (third-party) | TOML `[[mcp_servers]] name,transport="stdio",command,args,env` (array of tables) | Not documented | AGENTS.md NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | Official MCP page, undated |
| OpenHands CLI | `~/.openhands/mcp.json` | JSON `mcpServers.<n>.{command,args,env}` | Not documented | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | Official. **"No longer actively maintained"** |
| Plandex | NOT VERIFIED (not researched in time) | | | | | | | NOT VERIFIED |

### Per-host notes (things that matter to an adapter)

- **Cursor.** The cleanest target: `.cursor/mcp.json` holds only `mcpServers`, and `${workspaceFolder}`
  is documented, so `args: ["mcp","serve","--stdio","--project","${workspaceFolder}"]` works. The docs
  don't say whether comments are allowed, and one docs example contains a `//` comment. The adapter
  should parse strictly and refuse on failure. Hooks give `sessionStart` `additional_context`, which
  could carry a Rekora context or resume hook later. Subagents take a `model:` frontmatter key.
- **Windsurf → Devin Desktop.** `docs.windsurf.com` now redirects to `docs.devin.ai/desktop`, so the
  product has been folded into Devin. The current MCP files are `.devin/mcp_config.json` (project),
  `.devin/mcp_config.local.json` and `~/.config/devin/mcp_config.json`. Interpolation is
  `${env:VAR}` only and there is no workspace variable. The old `~/.codeium/windsurf/mcp_config.json`
  is now described as editor discovery only. The path just changed (CLI v3000.3), and some
  translated pages still show `config.json`. **Volatile: give manual instructions for now.**
- **Gemini CLI.** `settings.json` holds many unrelated keys, but `mcpServers` is a plain object.
  Docs say Gemini CLI was replaced by Antigravity CLI for unpaid/Google One users on 2026-06-18, so
  the audience is shrinking. Add `AGENTS.md` via `context.fileName`, which is also an adapter edit;
  only do it with `--confirm`.
- **VS Code / Copilot.** There are two shapes. Editor files (`.vscode/mcp.json`, profile `mcp.json`)
  use `servers` plus `type:"stdio"`. Portable Copilot files (`.mcp.json`, `~/.copilot/mcp-config.json`)
  use `mcpServers`. VS Code is the only host that **documents cwd = workspace folder**. Copilot CLI
  reads project `.mcp.json`/`.github/mcp.json` (with `type:"local"`) and explicitly does not read
  `.vscode/mcp.json`. A Rekora `.mcp.json` entry already written for Claude Code is therefore also
  read by Copilot CLI and Grok CLI. Check that `type` is omitted or accepted by each host.
  `.vscode/mcp.json` is edited by users in VS Code's JSONC mode, so expect comments and trailing
  commas.
- **Copilot CLI hooks** match the Claude Code model closely: `sessionStart.additionalContext` and
  `subagentStart.additionalContext`. The status line is community-documented only and experimental.
- **Cline.** There is no project-scope MCP file. The IDE file lives in VS Code globalStorage, which
  is undocumented in the fetched page. Give manual instructions.
- **Roo Code.** Project `.roo/mcp.json` with `mcpServers`, and `cwd` defaults to the first workspace
  folder (documented). Safe.
- **Kilo Code.** Now OpenCode-derived: JSONC `kilo.jsonc` with `mcp.<n>.{type:"local",command:[...]}`.
  The existing OpenCode adapter logic applies, but treat it as JSONC (comments). Manual, or reuse
  the OpenCode adapter if it already preserves JSONC.
- **Kiro.** `.kiro/settings/mcp.json` is a dedicated JSON file with `mcpServers`, and the docs were
  updated today. Steering reads `AGENTS.md`. Hooks are file-based JSON. A `command` action runs in
  the project root, while an `agent` action injects a prompt. Safe.
- **Zed.** `context_servers` lives inside the full user `settings.json`, which users keep with
  comments. A strict-JSON rewrite would destroy them. Manual only.
- **Qwen Code.** A Gemini fork with `.qwen/settings.json` `mcpServers`, plus `cwd`. It has a rich
  Claude-style hook set (23 events, `additionalContext`) and subagents with `model:`. Safe, with the
  same caveat as Gemini that `settings.json` is shared with other keys.
- **Amp.** Uses a dotted flat key `"amp.mcpServers"` and accepts `.jsonc`. Workspace servers need
  `amp mcp approve`. Manual (or an `amp mcp add` command line, if one exists: NOT VERIFIED).
- **Goose.** User-level YAML only, with keys `cmd`/`envs` (not `command`/`env`). Manual, or
  `goose configure`.
- **Crush.** Config is now a Bash `crushrc` and JSON is deprecated. The `crushrc` files are
  executable code, so never auto-edit them. Manual: `mcp add rekora --command rekora --args ...`.
- **Aider.** No native MCP. Rules only: suggest `read: [AGENTS.md]` in `.aider.conf.yml` (manual).
- **Continue.** The safest possible target. Rekora can own a whole file,
  `.continue/mcpServers/rekora.yaml`, so removing the entry means deleting that file, and no merge
  is needed. Continue also ingests a Claude-style JSON file in that folder.
- **JetBrains.** Junie: `.junie/mcp/mcp.json` with `mcpServers` (official snippets, page not fully
  read). It is likely safe but should be re-verified before an adapter is built. AI Assistant is
  UI-only (project or global level, with an explicit working-directory field), so give manual
  instructions.
- **Warp.** Paths came only from search snippets (the fetch returned 404) and differ between the
  app and the CLI. Manual, NOT VERIFIED.
- **Trae.** No official page could be read. Manual, NOT VERIFIED.
- **Factory Droid.** `.factory/mcp.json` with `mcpServers` (`type` optional), at user, ancestor-folder
  and project levels. It has Claude-style hooks with SessionStart `additionalContext` (stdout on
  exit 0 also counts). Safe.
- **Augment.** UI import plus Auggie's `~/.augment/settings.json`, which is a shared settings file
  and a prerelease CLI. Manual.
- **Claude Desktop.** User-only file and no workspace concept. The adapter must write an absolute
  `--project`, which pins one project per entry, so it is a poor fit for a per-project memory.
  Manual, or one named entry per project.
- **Grok CLI.** TOML config, but it also reads project `.mcp.json` and `.cursor/mcp.json`, so it is
  covered for free by the Claude Code/Copilot `.mcp.json` and Cursor adapters. There is no separate
  adapter need.
- **Kimi CLI.** User `~/.kimi/mcp.json` with `mcpServers`. A user-wide file needs an absolute
  `--project`, so manual or a user-wide warning.
- **Mistral Vibe.** TOML array-of-tables `[[mcp_servers]]`. It can reuse Codex's TOML editing
  approach, but the paths are not officially verified. Manual.
- **OpenHands CLI.** Unmaintained: skip.

### Recommendation: automatic adapter now vs manual only

**Safe for an automatic (`--confirm`, backup, own-entry) adapter now.** These are stable, documented,
dedicated JSON files with an `mcpServers`-like shape and no comments expected. All of them are
project scope.
1. **Cursor** `.cursor/mcp.json` (`mcpServers`, `type:"stdio"`, `${workspaceFolder}`).
2. **Copilot CLI / portable Copilot** `.mcp.json` (`mcpServers`, `type:"local"`). This is the same
   file Claude Code uses, and Grok CLI also reads it.
3. **Kiro** `.kiro/settings/mcp.json`.
4. **Roo Code** `.roo/mcp.json` (cwd defaults to workspace).
5. **Factory Droid** `.factory/mcp.json`.
6. **Continue** `.continue/mcpServers/rekora.yaml`, a file Rekora owns outright (best reversibility).
7. **Qwen Code** `.qwen/settings.json` and **Gemini CLI** `.gemini/settings.json`. Both are safe if
   the adapter merges only `mcpServers.rekora` and refuses non-strict JSON. Gemini's audience is
   shrinking.

**Conditional (strict-parse guard; refuse and fall back to manual if comments are found).**
VS Code `.vscode/mcp.json` (`servers`, JSONC-prone) and Junie `.junie/mcp/mcp.json` (re-verify docs).

**Manual instructions only.** Devin Desktop/Windsurf (path just moved), Zed (JSONC settings), Amp
(JSONC plus approval), Kilo Code (JSONC, OpenCode shape), Goose (YAML, user-only), Crush (Bash
`crushrc`), Mistral Vibe and Grok TOML, Cline (no project file), Augment, JetBrains AI Assistant
(UI), Warp and Trae (unverified), Claude Desktop and Kimi (user-only, need an absolute project
path), Aider (no MCP), OpenHands (unmaintained).

**Always pass `--project`**, because cwd is documented only for VS Code and Roo. Use
`${workspaceFolder}` where it is supported (Cursor, VS Code). Elsewhere, use the absolute project
path the adapter resolves, so behaviour doesn't depend on whatever cwd the host chooses.

**Display surface for a live cost estimate.** The documented third-party surfaces are a VS Code
extension status-bar item (works in VS Code, Cursor, Windsurf and Kiro forks, NOT VERIFIED for
forks) and a Copilot CLI `statusLine` command (experimental, community-documented). Qwen has a
"Status Line" page (not read). No other host documented one in the pages fetched. For the rest,
use hook output (`additionalContext` at SessionStart) or an MCP `status` tool result.

### Sources (fetched 2026-11-24)

| Host | URL | Page date |
|---|---|---|
| Cursor MCP | https://cursor.com/docs/context/mcp | none |
| Cursor hooks | https://cursor.com/docs/agent/hooks | none |
| Cursor subagents | https://cursor.com/docs/context/subagents | none |
| Cursor rules | https://cursor.com/docs/context/rules | none |
| Devin Desktop (ex-Windsurf) MCP | https://docs.devin.ai/desktop/cascade/mcp (redirect from docs.windsurf.com) | none |
| Windsurf plugins MCP | https://docs.devin.ai/windsurf/plugins/cascade/mcp.md | none |
| Devin hooks/rules | https://docs.devin.ai/desktop/cascade/hooks | none |
| Devin CLI MCP | https://docs.devin.ai/cli/extensibility/mcp/configuration (search snippet) | none |
| Gemini CLI MCP | https://geminicli.com/docs/tools/mcp-server/ | 2026-10-06 |
| Gemini hooks | https://geminicli.com/docs/hooks/ | 2026-04-13 |
| Gemini subagents | https://geminicli.com/docs/core/subagents/ | 2026-06-08 |
| Gemini GEMINI.md | https://geminicli.com/docs/cli/gemini-md/ | 2026-06-18 |
| VS Code MCP guide | https://code.visualstudio.com/docs/copilot/customization/mcp-servers | 2026-11-18 |
| VS Code MCP reference | https://code.visualstudio.com/docs/agents/reference/mcp-configuration | 2026-11-18 |
| Copilot CLI | https://docs.github.com/en/copilot/how-tos/use-copilot-agents/use-copilot-cli | none |
| Copilot CLI MCP | https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers | none |
| Copilot hooks | https://docs.github.com/en/copilot/reference/hooks-configuration | none |
| Copilot statusLine (community) | https://tgrall.github.io/blog/2026/05/02/copilot-cli-customize-statusline | 2026-05-02 |
| Cline MCP | https://docs.cline.bot/mcp/configuring-mcp-servers | none |
| Cline rules | https://docs.cline.bot/customization/cline-rules | none |
| Roo Code MCP | https://roocodeinc.github.io/Roo-Code/features/mcp/using-mcp-in-roo | 2026-05-15 |
| Kilo Code MCP | https://kilo.ai/docs/automate/mcp/using-in-kilo-code | none |
| Kilo AGENTS.md | https://kilo.ai/docs/customize/agents-md | none |
| Kiro MCP | https://kiro.dev/docs/mcp/configuration/ | 2026-11-24 |
| Kiro hooks | https://kiro.dev/docs/hooks/ | 2026-11-15 |
| Kiro steering | https://kiro.dev/docs/steering/ | 2026-10-21 |
| Zed MCP | https://zed.dev/docs/ai/mcp | none (v0.224.0) |
| Zed rules (search) | https://zed.dev/docs/ai/instructions | none |
| Qwen MCP | https://qwenlm.github.io/qwen-code-docs/en/users/features/mcp/ | 2026-10-19 |
| Qwen hooks | https://qwenlm.github.io/qwen-code-docs/en/users/features/hooks/ | none |
| Qwen subagents | https://qwenlm.github.io/qwen-code-docs/en/users/features/sub-agents/ | 2026-11-13 |
| Amp settings | https://ampcode.com/docs/cli/settings | none |
| Amp MCP | https://ampcode.com/docs/customize/mcp | none |
| Amp AGENTS.md | https://ampcode.com/docs/customize/agents-md | none |
| Amp models/subagents | https://ampcode.com/docs/models-and-subagents | none |
| Goose config | https://goose-docs.ai/docs/guides/config-files | none |
| Goose hints | https://goose-docs.ai/docs/guides/context-engineering/using-goosehints | none |
| Crush | https://github.com/charmbracelet/crush | README |
| Aider conventions | https://aider.chat/docs/usage/conventions.html | none |
| Aider MCP (secondary) | https://codemyspec.com/blog/aider-review-2026 | 2026 |
| Continue MCP | https://docs.continue.dev/customize/deep-dives/mcp | none |
| JetBrains AI Assistant MCP | https://www.jetbrains.com/help/ai-assistant/mcp.html | 2026.3 |
| Junie MCP (search) | https://junie.jetbrains.com/docs/junie-cli-mcp-configuration.html | none |
| Warp MCP (search only) | https://docs.warp.dev/agents/capabilities/mcp.md | none |
| Trae (third-party) | https://preline.co/docs/mcp-trae.html | none |
| Factory MCP | https://docs.factory.com/cli/configuration/mcp | none |
| Factory hooks | https://docs.factory.com/cli/configuration/hooks-guide | none |
| Augment IDE MCP | https://docs.augmentcode.com/setup-augment/mcp | none |
| Auggie CLI | https://docs.augmentcode.com/cli/integrations | none |
| Claude Desktop | https://modelcontextprotocol.io/docs/develop/connect-local-servers | none |
| Grok CLI | https://docs.x.ai/build/features/mcp-servers | none |
| Kimi CLI | https://moonshotai.github.io/kimi-cli/en/customization/mcp.html | none |
| Mistral Vibe | https://docs.mistral.ai/vibe/code/cli/mcp-servers | none |
| OpenHands CLI | https://docs.openhands.dev/openhands/usage/cli/mcp-servers | none |
