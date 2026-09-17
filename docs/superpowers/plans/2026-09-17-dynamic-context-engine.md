# OpenZ Adaptive Dynamic Context Engine Implementation Plan

**Plan Document:** `docs/superpowers/plans/2026-09-17-dynamic-context-engine.md`  
**Spec Reference:** `docs/superpowers/specs/2026-09-17-dynamic-context-engine-design.md`  
**Date:** September 17, 2026

---

## Plan Overview

This plan decomposes the implementation of the Adaptive Dynamic Context Engine into 4 distinct, test-driven tasks:
1. **Task 1: Configuration Schema & Dynamic Context Registry (`src/providers/context_registry.rs`)**:
   - Implement `AgentDefaults` ratio fields.
   - Implement `DynamicContextRegistry` with 4-tier model context resolution, dynamic models.json loader, and proportional budgeting algorithms.
   - Write comprehensive unit tests in `src/providers/context_registry_tests.rs`.
2. **Task 2: Prompt Budget & Agent Loop Build Phase Integration (`src/agent/agent_loop/build.rs`)**:
   - Replace the hardcoded 64k character clamp with `DynamicContextRegistry::resolve_prompt_budget_chars`.
   - Update prompt assembly to scale dynamically with the model's true window.
   - Unit tests for proportional prompt budgeting.
3. **Task 3: Tool Output Budgeting & Structure-Preserving Compactor (`src/agent/agent_loop/transcript.rs` & `src/agent/context_compactor.rs`)**:
   - Update `transcript.rs` to compute dynamic tool output limit via `DynamicContextRegistry::resolve_tool_output_limit_chars`.
   - Improve `context_compactor.rs` to preserve JSON array elements and structure instead of dropping them.
   - Unit tests for tool output limits and non-destructive compaction.
4. **Task 4: Token-Pressure Compaction & Hermes Pre-Compression Memory Hook (`src/agent/agent_loop/compact.rs` & `render.rs`)**:
   - Implement token estimation across session turns.
   - Integrate `should_compact_context` with token-pressure gating.
   - Add Hermes pre-compression hook to save decisions/facts before history pruning.
   - Unify `src/channels/cli/render.rs` status bar with `DynamicContextRegistry`.
   - SemVer bump to `v0.0.190`, CHANGELOG, and verification.

---

## Global Invariants & Constraints
- **Resource Capping**: All `cargo test` and `cargo check` calls MUST use `-j 1`. All `cargo build` calls MUST use `-j 2`.
- **Tool Count Invariant**: Exactly 260 registered native tools must be preserved.
- **Zero Warnings**: Maintain 0 clippy and compiler warnings.
- **Test-Driven Development**: Tests written and verified for each phase.

---

## Tasks Breakdown

### Task 1: Configuration Schema & Dynamic Context Registry
- Add `prompt_budget_ratio`, `tool_output_ratio`, `compaction_threshold_ratio`, `keep_recent_ratio` to `AgentDefaults` in `src/config/schema.rs`.
- Create `src/providers/context_registry.rs` with:
  - `ContextRegistry` struct & static instance.
  - Pattern matching for major LLM families.
  - File loader for `~/.openz/models.json` / `$OPENZ_CONFIG_DIR/models.json`.
  - Methods: `resolve_context_window()`, `resolve_prompt_budget_chars()`, `resolve_tool_output_limit_chars()`, `estimate_tokens_from_chars()`, `should_compact()`.
- Add module exports in `src/providers/mod.rs`.
- Create `src/providers/context_registry_tests.rs`.
- Verify tests pass with `cargo test -p openz --lib providers::context_registry_tests -j 1`.

### Task 2: Proportional Prompt Budgeting Integration
- In `src/agent/agent_loop/build.rs`:
  - Deprecate old static `resolve_prompt_budget` or rewrite it to delegate to `DynamicContextRegistry::resolve_prompt_budget_chars`.
  - Remove `computed.clamp(8000, 64000)`.
  - Use `DynamicContextRegistry::resolve_prompt_budget_chars(&config.agents.defaults.model, config)`.
  - Ensure custom `prompt_budget_limit` is still respected if explicitly provided.
- Verify existing and new build tests pass with `cargo test -p openz --lib agent::agent_loop -j 1`.

### Task 3: Proportional Tool Limits & Structural Compactor Hardening
- In `src/agent/agent_loop/transcript.rs`:
  - Calculate `limit` dynamically from `DynamicContextRegistry::resolve_tool_output_limit_chars(&config.agents.defaults.model, config)`.
- In `src/agent/context_compactor.rs`:
  - Refactor `compress_json` to keep head + tail array items with clear omission counts rather than throwing away all elements after index 0.
  - Update `compress_logs` to respect dynamic budget instead of rigid 2,000 char clamp.
- Add tests in `src/agent/context_compactor_tests.rs`.
- Verify with `cargo test -p openz --lib agent::context_compactor_tests -j 1`.

### Task 4: Token-Pressure Compaction, Hermes Hook, and CLI Unification
- In `src/agent/agent_loop/compact.rs`:
  - Add token pressure check: estimate tokens across conversation history + prompt.
  - Trigger compaction when token count exceeds `compaction_threshold_ratio * context_window` OR when `messages.len() > max_messages`.
  - Implement `pre_compress_memory_consolidation` (Hermes hook) extracting important facts to pinned/cross-session memory before truncation.
- In `src/channels/cli/render.rs`:
  - Replace private hardcoded model table in `render.rs:758-796` with `DynamicContextRegistry::resolve_context_window`.
- Bump version to `0.0.190` across `Cargo.toml`, `Cargo.lock`, `onpkg.json`, `README.md`, `CHANGELOG.md`.
- Full verification: `cargo test --lib -- test_native_tool_registration_names -j 1`, `cargo clippy -p openz -j 1`.
