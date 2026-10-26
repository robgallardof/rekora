# Troubleshooting

Run `rekora doctor` first; it only reads. Exit code `3` means a critical problem (no project / broken store).

| Symptom | Cause | Fix |
|---|---|---|
| `not_initialized` | No `.rekora/rekora.db` at the repo's main worktree or any parent | `rekora init` in the repository |
| `schema_too_new` | Database written by a newer Rekora | Upgrade `rekora`; the file was not modified |
| `source_outside_root` | Cited path resolves outside the checkout (incl. symlink/junction) | Cite a file inside the project |
| `source_excluded` | `.env*`, keys, `.git/`, `.rekora/`, `node_modules/`, `target/` | Cite documentation or code instead |
| `version_conflict` | Someone changed the memory/task after you read it | Re-read (`task show`) and retry with the new version |
| `snapshot_changed` | Branch/commit changed during `rekora index` (twice) | Re-run when the checkout is stable |
| Search says `no_evidence` but you know it exists | Fewer query words match than the coverage floor needs | Use the words of the memory, fewer words, or `--explain` on a broader query |
| Result marked `POSSIBLY STALE` | A cited file changed or disappeared since recording | Check the source; `memory verify` or `supersede` it |
| Claude Code shows "Pending approval" | `.mcp.json` servers need one-time approval | Start `claude` in the project and approve `rekora` |
| Codex refuses tool calls in `exec` | Approval policy | Read tools are annotated read-only; write tools need your approval policy to allow them |
| `doctor` warns `entry points to missing executable` | Rekora moved/rebuilt elsewhere | `rekora agent install <host> --confirm` again |
| Config "not valid JSON/TOML; not modified" | Existing host config is broken | Fix it by hand; Rekora never rewrites a file it cannot parse |
| Restore a host config | — | Originals are in `.rekora/backups/<host>-<timestamp>.bak` |
| Restore after a migration | — | `.rekora/rekora.db.v<N>.bak` is the pre-upgrade copy (stop all `rekora` processes first) |
