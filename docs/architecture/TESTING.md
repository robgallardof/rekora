# Testing

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                        # unit, integration, E2E-local, doctests
cargo test -p rekora-cli --test continuity -- --ignored       # real hosts (spends model tokens)
docker build -t rekora:local .
python scripts/test_docker.py                                # actual stdio + HTTP containers and persistence
rekora benchmark run --suite relevance                        # quality
rekora benchmark run --suite core                             # latency/throughput
```

| Level | Where | Real dependencies |
|---|---|---|
| Unit | `src/` `#[cfg(test)]` in each crate | none needed |
| Integration | `crates/rekora-store/tests/store.rs`, `crates/rekora-code/tests/index.rs` | SQLite files on disk, real Git repos, real tree-sitter |
| E2E-local | `crates/rekora-cli/tests/e2e.rs`, `continuity.rs` | compiled binary, Git, MCP child processes over stdio, OS processes in parallel |
| E2E-host | `continuity.rs` (`#[ignore]`) | installed Claude Code / Codex with credentials |
| HTTP | `crates/rekora-cli/tests/http.rs` | real TCP listener, MCP child, temp SQLite; legacy sessions and modern stateless requests |
| Docker | `scripts/test_docker.py` | real Docker image, Compose, custom port, container recreation and persisted SQLite |

Fixtures in [`fixtures/`](../../fixtures) are synthetic and copied into temp Git repos per test.

## Scenario map

| Scenario | Test(s) |
|---|---|
| S01 / R01 restart | `r01_persist_close_reopen_same_record`, `s01_store_restart_and_retrieve_with_source` |
| S02 / R02 handoff | `s02_handoff_between_two_agents_in_separate_processes`, `s01_s02_cli_memory_visible_over_mcp…` |
| S03 / R03 isolation | `s03_two_scopes_never_leak`, `s03_similar_repos_are_isolated` |
| S04 worktrees | `s04_worktrees_share_store_and_keep_branch_specific_decisions`, `branches_have_separate_snapshots` |
| S05 / R05 / R36 incremental | `s05_r05_r36_incremental_reindex_and_stale_memory` |
| S06 superseded | `s06_superseded_excluded_and_relevant_found_among_noise` |
| S07 / R07 no evidence | `s07_*` (store and CLI) |
| S08 / R08 budget | `r08_context_pack_respects_every_budget`, budget unit tests |
| S09 / R09 injection | `r09_injection_text_is_labelled_untrusted_data`, `r14_r31_export_import_cli_and_untrusted_annotation` |
| S10 / R10 / R32 adapters | `s10_r10_r32_adapters_preserve_unrelated_config_and_roll_back`, `adapter_created_file_is_removed_on_uninstall` |
| S11 / R11 migration | `r11_failed_migration_rolls_back_whole_step` |
| S12 / R12 concurrency | `r12_concurrent_writers_and_readers_lose_nothing` (threads), `r12_independent_processes_write_concurrently_without_loss` (processes) |
| S13 / R13 / R22 hostile paths | `r13_r22_*`, `r22_junction_*` (Windows), `r22_symlink_*` (Unix), `r13_import_rejects_hostile_entries…` |
| S14 / R14 idempotent import | `r14_r17_export_import_idempotent_and_untrusted`, `r14_r31_*` |
| S15 / R15 / R26 conflicts | `s15_stale_version_update_conflicts`, `s15_r26_task_updates_are_versioned…`, `concurrent_task_writers_serialise…` |
| S16 / R16 languages | `s16_r16_symbols_across_languages_with_exact_locations`, `lang::tests::*` |
| S17 / R17 delete | `s17_delete_removes_content_everywhere…`, `delete_requires_confirm_and_is_audited` |
| S18 / R18 doctor | `r33_commands_before_init_fail_with_code` (exit 3), `r24_malformed_or_readonly_config…` |
| S19 / R19 performance | `rekora benchmark run --suite core` (results in PERFORMANCE.md) |
| S20 / R20 real hosts | `r20_real_claude_code_calls_rekora`, `r20_real_codex_calls_rekora` |
| R24 bad host config | `r24_malformed_or_readonly_config_is_never_modified` |
| R25 branch switch mid-scan | `r25_snapshot_change_during_scan_is_refused` |
| R27 score explanation | `score_explanation_reproduces_score` |
| R29 malformed MCP | `r29_bad_mcp_input_is_rejected_and_server_survives` |
| R30 newer schema | `r30_newer_schema_refused_without_writing` |
| R33 before init | `r33_*` |
| R34 relevance | relevance suite |
| R35 protocol | `r35_tools_declare_safety_annotations`; handshake tested with `2025-11-25` only |

Not covered: R21 (secret in conversational events — no event capture exists), R23 (consolidation — not
implemented), R28 (valid-time queries — not implemented), fuzzing campaigns.
