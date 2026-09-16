# OpenZ — Complete Subagent Test Suite Decomposition (Phase 37)

## Goal
Complete the full modularization of the subagent test architecture by decomposing the remaining 1,128 lines of `src/tools/subagent/tests.rs` into dedicated, component-local sibling test modules:
1. `src/tools/subagent/evaluator_optimizer_tests.rs` (schema validation and iterative feedback evaluation loops)
2. `src/tools/subagent/optimize_profile_tests.rs` (profile configuration validation and fallback limits)
3. `src/tools/subagent/runner_tests.rs` (runner timeout clamping, provider model overriding, prompt formatting, image formatting, cancellation outcome)
4. `src/tools/subagent/delegate_task_tests.rs` (delegation depth limits, model selection cascades, cancellation propagation)
5. `src/tools/subagent/delegate_profile_tests.rs` (profile denial policies and cancellation propagation)
6. `src/tools/subagent/mod_tests.rs` (evolution gating, nesting checks, tool registry verification), and relocate tool filtering test to `allowlist_tests.rs`
7. Completely delete `src/tools/subagent/tests.rs`.

---

## Detailed Task Breakdown

### Task 1: Extract `evaluator_optimizer_tests.rs` and `optimize_profile_tests.rs`
- **Actions:**
  1. Create `src/tools/subagent/evaluator_optimizer_tests.rs` with:
     - `test_validate_schema_success`
     - `test_validate_schema_failure`
     - `test_evaluator_optimizer_loop_success`
     - `test_evaluator_optimizer_rejects_denied_optimizer_profile`
  2. Add `#[cfg(test)] #[path = "evaluator_optimizer_tests.rs"] mod tests;` in `src/tools/subagent/evaluator_optimizer.rs`.
  3. Create `src/tools/subagent/optimize_profile_tests.rs` with:
     - `subagent_settings_validation_rejects_more_than_three_fallbacks`
  4. Add `#[cfg(test)] #[path = "optimize_profile_tests.rs"] mod tests;` in `src/tools/subagent/optimize_profile.rs`.
  5. Remove these tests from `src/tools/subagent/tests.rs`.
  6. Verify: `cargo test -p openz --lib tools::subagent::evaluator_optimizer -j 1` and `cargo test -p openz --lib tools::subagent::optimize_profile -j 1`.

### Task 2: Extract `runner_tests.rs`
- **Actions:**
  1. Create `src/tools/subagent/runner_tests.rs` with:
     - `test_limit_subagent_models_to_try_keeps_primary_plus_two_fallbacks`
     - `test_resolve_subagent_timeout_uses_default_and_clamps`
     - `test_subagent_provider_prefixed_model_overrides_default_provider`
     - `filesystem_write_denied_policy_helper_detects_denial`
     - `test_ensure_markdown_images_wraps_paths_and_urls`
     - `test_build_subagent_prompt_includes_sections_and_schema`
     - `test_run_subagent_attempt_returns_cancelled_when_token_pre_cancelled`
  2. Add `#[cfg(test)] #[path = "runner_tests.rs"] mod tests;` in `src/tools/subagent/runner.rs`.
  3. Remove these tests from `src/tools/subagent/tests.rs`.
  4. Verify: `cargo test -p openz --lib tools::subagent::runner -j 1`.

### Task 3: Extract `delegate_task_tests.rs` and `delegate_profile_tests.rs`
- **Actions:**
  1. Create `src/tools/subagent/delegate_task_tests.rs` with:
     - `delegate_task_models_to_try_uses_override_fallbacks_then_default`
     - `delegate_task_models_to_try_prefers_vision_fallback_for_images`
     - `test_delegate_task_metadata_is_explicit_for_router`
     - `test_delegation_depth_limit`
     - `test_delegate_task_cancels_while_child_run_is_active`
     - `test_delegate_task_cancellation_propagation`
  2. Add `#[cfg(test)] #[path = "delegate_task_tests.rs"] mod tests;` in `src/tools/subagent/delegate_task.rs`.
  3. Create `src/tools/subagent/delegate_profile_tests.rs` with:
     - `test_delegate_profile_rejects_explicitly_denied_profile`
     - `test_delegate_profile_cancels_while_child_run_is_active`
     - `test_delegate_profile_cancellation_propagation`
  4. Add `#[cfg(test)] #[path = "delegate_profile_tests.rs"] mod tests;` in `src/tools/subagent/delegate_profile.rs`.
  5. Remove these tests from `src/tools/subagent/tests.rs`.
  6. Verify: `cargo test -p openz --lib tools::subagent::delegate_task -j 1` and `cargo test -p openz --lib tools::subagent::delegate_profile -j 1`.

### Task 4: Extract `mod_tests.rs`, relocate allowlist test, and remove `tests.rs`
- **Actions:**
  1. Move `test_filter_tools_for_new_default_subagents` from `tests.rs` into `src/tools/subagent/allowlist_tests.rs`.
  2. Create `src/tools/subagent/mod_tests.rs` with root subagent tests:
     - `subagent_allowlisted_tools_exist_in_registry`
     - `evolution_gate_blocks_smoke_test_summary`
     - `evolution_gate_blocks_when_filesystem_writes_denied`
     - `evolution_gate_allows_substantial_reusable_guidance`
     - `skips_evolution_for_short_smoke_test_outputs`
     - `does_not_skip_evolution_for_substantial_new_skill_output`
     - `simple_step_does_not_allow_nested_delegation`
     - `explicit_specialist_step_allows_nested_delegation`
  3. Update `src/tools/subagent/mod.rs` to declare `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
  4. Delete `src/tools/subagent/tests.rs`.
  5. Verify: `cargo test -p openz --lib tools::subagent -j 1`.

### Task 5: Invariant & Linter Verification
- **Actions:**
  1. Run all subagent tests: `cargo test -p openz --lib tools::subagent -j 1`.
  2. Verify 260 registered native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  3. Verify 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.

### Task 6: Final Documentation, Version Sync & Release Bump (`v0.0.185`)
- **Actions:**
  1. Update `Cargo.toml`, `onpkg.json`, and `README.md` to `0.0.185`.
  2. Add release entry in `CHANGELOG.md`.
  3. Update `recommendedfix.md` with Section 4.49.
  4. Verify version sync: `cargo test -p openz --lib version_sync_tests -j 1`.
  5. Create Phase 37 artifact in the brain directory.
  6. Commit: `chore(release): bump openz to v0.0.185 completing full subagent test decomposition`.
