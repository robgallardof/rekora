# Data model

Schema version 4 (`PRAGMA user_version`). All timestamps are UTC RFC 3339; formatting for humans happens
at the edges. All tables are `STRICT`.

## Memory

| Field | Meaning |
|---|---|
| `id` | UUIDv7 (time-ordered); CLI accepts a unique prefix of 8+ chars |
| `scope_id` | UUIDv7 assigned at `init`; one per database |
| `kind` | `fact`, `decision`, `constraint`, `preference`, `known_issue`, `solution`, `procedure` |
| `status` | `candidate`, `verified`, `disputed`, `superseded` |
| `origin` | `human`, `agent`, `import` (trust tier) |
| `content` | Redacted text, ≤ 8 KiB |
| `fingerprint` | BLAKE3 of kind + content + sorted source locations; unique per scope → idempotent writes |
| `branch`, `commit_sha` | Git context when recorded |
| `version` | Incremented on every state change (optimistic concurrency) |
| `superseded_by` | Replacement memory id |

`sources(memory_id, ord, path, line, content_hash)`: root-relative `/` paths, BLAKE3 of the file when cited.

### Creation rules (enforced in `rekora-core`)

| Origin | Requested `verified` | Stored as |
|---|---|---|
| human | yes | verified |
| agent | yes, with ≥1 source | verified (CLI only; MCP `remember` always stores candidate) |
| agent | yes, no source | candidate (downgrade reported) |
| import | anything | candidate (or disputed if it was disputed) |
| any | `superseded` / `disputed` | candidate |

Transitions after creation: `verify`, `dispute` (CLI), `supersede` (CLI), `delete` (CLI, `--confirm`).
A superseded memory cannot change status again.

## Task

| Field | Meaning |
|---|---|
| `id` | Slug `[a-z0-9][a-z0-9._-]{0,63}`, unique per scope |
| `objective`, `status` (`open`, `in_progress`, `blocked`, `done`), `note`, `owner` | |
| `state` | JSON: `constraints`, `decisions`, `files`, `tests`, `failures`, `remaining`, `done` |
| `branch`, `commit_sha` | Refreshed on handoff |
| `version` | Optimistic concurrency |

Edits are patches: append items (deduplicated), complete remaining items by exact text, set status/note/owner.

## Handoff document (`rekora.handoff/1`)

```json
{
  "schema": "rekora.handoff/1",
  "task": { "...": "Task as above" },
  "context": { "status": "found", "budget": 1024, "used_tokens": 97, "items": [ { "id": "...", "line": "- [constraint | verified] ..." } ] },
  "instructions": "Continue this task. Respect constraints ..."
}
```

## Events (append-only)

`events(seq, memory_id, action, actor, at, detail)`. Subjects are memory ids or `task:<id>`. Actions:
`create`, `status`, `supersede`, `delete`, `import`, `task_create`, `task_update`. Deleting a memory removes its
content everywhere; the `delete` event keeps only id, kind, actor and time.

## Code index

`code_files(scope_id, snapshot, path, hash, language, size, indexed_at)` and
`code_symbols(scope_id, snapshot, path, name, kind, line)`. `snapshot` = branch, `detached@<sha12>` or `worktree`.

## Export (`rekora.export/1`)

`{format, scope_id, exported_at, memories: [...], tasks: [...]}`. Import ignores tasks (see `transfer.rs`).
