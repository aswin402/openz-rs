# OpenZ — Subagent Subsystem Test Suite Decomposition (Phase 36)

## Goal
Decompose the monolithic `src/tools/subagent/tests.rs` (1,743 lines) into dedicated sibling test modules for each core subagent component:
1. `src/tools/subagent/workspace_tests.rs` (workspace isolation, worktree cleanup, quotas, home fallback)
2. `src/tools/subagent/lifecycle_tests.rs` (status labels, cancellation, timeout classification & JSON format)
3. `src/tools/subagent/schema_retry_tests.rs` (schema validation retries, fenced JSON extraction, failure handling)
4. `src/tools/subagent/cancellation_token_tests.rs` (CLI cancellation signal observation and token sharing)
5. `src/tools/subagent/parallel_research_tests.rs` (parallel fan-out, partial responses, flush deadlines)

This modularizes subagent tests into component-local suites, preserves private visibility, reduces `subagent/tests.rs` by ~650 lines, and bumps OpenZ to release `v0.0.184`.

---

## Target Subsystems & Metrics

| Component | Source File | Extracted Sibling Test File | Est. Lines |
|---|---|---|---|
| Workspace & Isolation | `src/tools/subagent/workspace.rs` | `src/tools/subagent/workspace_tests.rs` | ~270 lines |
| Lifecycle & Status | `src/tools/subagent/lifecycle.rs` | `src/tools/subagent/lifecycle_tests.rs` | ~120 lines |
| Schema Retry Loop | `src/tools/subagent/schema_retry.rs` | `src/tools/subagent/schema_retry_tests.rs` | ~180 lines |
| Cancellation Token | `src/tools/subagent/cancellation_token.rs` | `src/tools/subagent/cancellation_token_tests.rs` | ~25 lines |
| Parallel Research | `src/tools/subagent/parallel_research.rs` | `src/tools/subagent/parallel_research_tests.rs` | ~55 lines |

---

## Detailed Task Breakdown

### Task 1: Extract `workspace_tests.rs`
- **Actions:**
  1. Create `src/tools/subagent/workspace_tests.rs` with workspace tests (`test_worktree_guard_unregisters_on_drop`, `test_registered_worktree_cleanup_handles_forced_shutdown_path`, `test_worktree_cleanup_removes_old_openz_worktrees`, `test_worktree_cleanup_enforces_total_size_quota_oldest_first`, `test_scratch_workspace_teardown_message_skips_branch_commit_wording`, `test_create_isolated_workspace_uses_scratch_for_home_like_non_git_root`, `test_fallback_copy_skips_heavy_user_cache_dirs`, `attach_workspace_fields_merges_isolation_outcome`).
  2. Add `#[cfg(test)] #[path = "workspace_tests.rs"] mod tests;` in `src/tools/subagent/workspace.rs`.
  3. Remove these tests from `src/tools/subagent/tests.rs`.
  4. Verify: `cargo test -p openz --lib tools::subagent::workspace -j 1`.

### Task 2: Extract `lifecycle_tests.rs`
- **Actions:**
  1. Create `src/tools/subagent/lifecycle_tests.rs` with lifecycle status tests (`test_lifecycle_status_labels_are_stable_for_tui`, `test_compact_lifecycle_line_for_cancellation_is_stable`, `test_lifecycle_classifies_timeout_without_user_cancel`, `test_lifecycle_classifies_timeout_duration_seconds`, `test_lifecycle_timeout_status_json_includes_duration`, `test_compact_lifecycle_line_includes_timeout_duration`, `test_lifecycle_classifies_user_cancel_from_token`).
  2. Add `#[cfg(test)] #[path = "lifecycle_tests.rs"] mod tests;` in `src/tools/subagent/lifecycle.rs`.
  3. Remove these tests from `src/tools/subagent/tests.rs`.
  4. Verify: `cargo test -p openz --lib tools::subagent::lifecycle -j 1`.

### Task 3: Extract `schema_retry_tests.rs`
- **Actions:**
  1. Create `src/tools/subagent/schema_retry_tests.rs` with schema retry tests (`test_schema_retry_accepts_valid_fenced_json`, `test_schema_retry_retries_invalid_json_before_limit`, `test_schema_retry_errors_invalid_json_at_limit`, `test_schema_retry_retries_schema_mismatch_before_limit`, `test_validate_schema_success`, `test_validate_schema_failure`, `schema_retry_loop_accepts_fenced_json_and_cleans_content`, `schema_retry_loop_retries_then_accepts`, `schema_retry_loop_errors_after_attempt_limit`).
  2. Add `#[cfg(test)] #[path = "schema_retry_tests.rs"] mod tests;` in `src/tools/subagent/schema_retry.rs`.
  3. Remove these tests from `src/tools/subagent/tests.rs`.
  4. Verify: `cargo test -p openz --lib tools::subagent::schema_retry -j 1`.

### Task 4: Extract `cancellation_token_tests.rs` and `parallel_research_tests.rs`
- **Actions:**
  1. Create `src/tools/subagent/cancellation_token_tests.rs` with `test_cancellation_token_observes_cli_cancel_signal`.
  2. Add `#[cfg(test)] #[path = "cancellation_token_tests.rs"] mod tests;` in `src/tools/subagent/cancellation_token.rs`.
  3. Create `src/tools/subagent/parallel_research_tests.rs` with `test_parallel_research_partial_response_shape`, `test_parallel_research_flush_deadline_precedes_child_timeout`, `test_parallel_research_metadata_is_explicit_for_router`.
  4. Add `#[cfg(test)] #[path = "parallel_research_tests.rs"] mod tests;` in `src/tools/subagent/parallel_research.rs`.
  5. Remove these tests from `src/tools/subagent/tests.rs`.
  6. Verify: `cargo test -p openz --lib tools::subagent::cancellation_token -j 1` and `cargo test -p openz --lib tools::subagent::parallel_research -j 1`.

### Task 5: Invariant & Linter Verification
- **Actions:**
  1. Run all subagent tests: `cargo test -p openz --lib tools::subagent -j 1`.
  2. Verify 260 registered native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  3. Verify 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.

### Task 6: Final Documentation, Version Sync & Release Bump (`v0.0.184`)
- **Actions:**
  1. Update `Cargo.toml`, `onpkg.json`, and `README.md` to `0.0.184`.
  2. Add release entry in `CHANGELOG.md`.
  3. Update `recommendedfix.md` with Section 4.48.
  4. Verify version sync: `cargo test -p openz --lib version_sync_tests -j 1`.
  5. Create Phase 36 artifact in the brain directory.
  6. Commit: `chore(release): bump openz to v0.0.184 with subagent subsystem test suite modularization`.
