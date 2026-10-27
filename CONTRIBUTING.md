# Contributing to Rekora

Thanks for helping. Read [AGENTS.md](AGENTS.md) first — the same rules apply to humans and AI agents.

## Setup

```bash
git clone https://github.com/robgallardof/rekora
cd rekora
cargo test --workspace
```

Requirements: stable Rust (see `rust-toolchain.toml`), a C compiler (SQLite and tree-sitter grammars are
compiled from source), and `git` on PATH for the integration tests.

## Pull requests

1. One focused change per PR, with a user scenario in the description.
2. Tests against real dependencies (temporary SQLite files, real Git repositories, the real binary and MCP
   child processes). Do not mock the component under test.
3. `//!` docs on every module, `///` on every public item, comments that explain *why*.
4. Before pushing:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

5. Update `docs/STATUS.md`, `CHANGELOG.md` and any affected doc. Never describe unimplemented behaviour as
   available, and never report a test or benchmark you did not run.

Benchmark numbers in PRs must include the command, machine (CPU, RAM, OS), build profile and commit.

## Good first issues

Look for the `good first issue` label. Typical starters: a new language in `rekora-code` (with fixtures and
tests), extra redaction patterns (with false-positive tests), doc fixes verified against `--help`.

## Licence

By contributing you agree that your contributions are licensed under the MIT licence of this repository.

## Commit language

All new Git commit subjects and bodies must be written in English, including merge
and squash messages. Keep the repository's existing message conventions.
This applies to all contributors and coding agents, and only to new commits.
Do not rewrite existing history to translate commit messages.
