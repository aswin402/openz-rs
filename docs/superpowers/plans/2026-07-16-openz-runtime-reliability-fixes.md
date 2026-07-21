# OpenZ Runtime Reliability Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the runtime bugs found in v0.0.50/v0.0.51 diagnostics and harden the edge cases so they do not recur silently.

**Architecture:** Keep fixes small and test-backed. Unify codebase indexing storage by making `ast_grep_index_codebase` write the same `code_elements` table consumed by `query_code_graph`; improve rule-based fact extraction for common multi-clause language; migrate legacy timeout config values that hide new defaults; and make unsafe subagent workspace fallback explicit and actionable.

**Tech Stack:** Rust, Tokio, rusqlite, serde_json, regex, existing OpenZ native tool architecture.

## Global Constraints

- Do not commit `recommendedfix.md` or local plan notes unless explicitly requested.
- Avoid broad refactors while fixing runtime bugs.
- Each fix must include a regression test.
- Run targeted tests after each task; run `cargo test --lib -- --test-threads=1` before push.
- Preserve existing tool names and schemas for compatibility.

---

## File Structure

- `src/tools/ast_grep.rs`: Bridge ast-grep structural index output into the memory-extra `code_elements` table, fix malformed archive query text, add tests for parser/bridge helpers.
- `src/tools/memory_extra/codebase.rs`: Add test coverage for `query_code_graph` returning indexed results and optionally broaden query matching to signature/file path.
- `src/tools/memory_extra/facts.rs`: Add extraction support for `X built Y with Z`, passive `Y was built with Z`, and carried-subject created/built clauses.
- `src/tools/memory_extra/mod.rs`: Add integration tests for complex fact extraction and storage.
- `src/config/loader.rs`: Migrate old `toolTimeoutSecs: 120` configs to the new 300s default only when the value is still the historical default, while preserving user-customized non-120 values.
- `src/tools/subagent/delegate_task.rs`: Add explicit unsafe-workspace fallback status metadata/message helper so users understand why isolation was skipped.
- `docs/superpowers/plans/2026-07-16-openz-runtime-reliability-fixes.md`: This plan.

---

### Task 1: Fix code graph query/indexer mismatch

**Files:**
- Modify: `src/tools/ast_grep.rs`
- Modify: `src/tools/memory_extra/codebase.rs`

**Interfaces:**
- Consumes: `graph_memory::with_db()` and `scope_from_args()` to write `code_elements`.
- Produces: `ast_grep_index_codebase` stores rows that `query_code_graph` can read immediately.

- [ ] **Step 1: Write failing regression test**

Add a test that creates a temp Rust file, runs the indexing bridge helper or `IndexCodebaseTool`, then calls `QueryCodeGraphTool` for `MemoryCoordinator`/known symbol and expects a non-empty result.

- [ ] **Step 2: Add ast-grep match parser helper**

Create a helper in `src/tools/ast_grep.rs` that converts a JSON match into `{file, element_type, name, signature, start_line, end_line}`. Include support for function, struct, enum, impl, class, and type patterns.

- [ ] **Step 3: Write rows into `code_elements`**

Inside `AstGrepIndexCodebaseTool::call`, after parsing each match, insert into `code_elements` with scoped `user_id/session_id/agent_id` from `scope_from_args(arguments)`. Use `INSERT OR REPLACE` so re-indexing refreshes stale rows.

- [ ] **Step 4: Fix archive query string bug**

Replace the malformed `format!("Symbol: {} | Language: {} | File: {}", "Symbol: ...", symbol_name, lang, file)` with `format!("Symbol: {symbol_name} | Language: {lang} | File: {file}")`.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test --lib code_graph ast_grep -- --test-threads=1
```

Expected: new and existing code graph tests pass.

---

### Task 2: Improve fact extraction for complex build clauses

**Files:**
- Modify: `src/tools/memory_extra/facts.rs`
- Modify: `src/tools/memory_extra/mod.rs`

**Interfaces:**
- Consumes: existing `extract_facts(text) -> Vec<ExtractedFact>`.
- Produces: facts for `Alice built AppX with Rust`, `AppX was built with Rust`, and chained clauses like `Alice created AppX and built it with Rust` where possible.

- [ ] **Step 1: Add failing extraction tests**

Add assertions for:

```rust
let facts = facts::extract_facts("Alice built AppX with Rust. AppX was built with Rust.");
assert!(triples.contains(&("Alice", "created", "AppX")));
assert!(triples.contains(&("AppX", "built_with", "Rust")));
```

- [ ] **Step 2: Extend regex patterns**

Add direct patterns:
- `({entity}) built ({entity}) with ({entity})` -> two facts: `created` and `built_with`.
- `({entity}) was built with ({entity})` -> `built_with`.

If one clause can produce multiple facts, change `extract_fact_clause` to return `Vec<ExtractedFact>` or add a wrapper that expands multi-fact clauses while preserving carried subject behavior.

- [ ] **Step 3: Preserve dedup and conflict behavior**

Keep the existing `(from, relation, to)` dedup key. Do not create lower-case duplicate entities.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --lib memory_extra::tests::test_extract_facts_handles_multi_word_and_profile_facts memory_extra::tests::test_extract_and_store_facts_handles_richer_patterns -- --test-threads=1
```

Expected: existing and new fact extraction tests pass.

---

### Task 3: Migrate historical 120s tool timeout config safely

**Files:**
- Modify: `src/config/loader.rs`

**Interfaces:**
- Consumes: loaded `Config` plus raw JSON for migration detection.
- Produces: old configs with `toolTimeoutSecs: 120` migrate to 300; custom values like 60/900 are preserved.

- [ ] **Step 1: Add migration tests**

Add two tests:
- Config with `toolTimeoutSecs: 120` loads and saves `toolTimeoutSecs: 300`.
- Config with `toolTimeoutSecs: 60` remains 60.

- [ ] **Step 2: Implement migration**

In `migrate_config`, if `config.agents.defaults.tool_timeout_secs == 120`, set it to `300` and mark modified. This is safe because 120 was the historical default; user custom values are not touched.

- [ ] **Step 3: Verify**

Run:

```bash
cargo test --lib config::loader::tests -- --test-threads=1
```

Expected: config migration tests pass.

---

### Task 4: Harden unsafe subagent workspace fallback

**Files:**
- Modify: `src/tools/subagent/delegate_task.rs`
- Modify: `src/tools/subagent/delegate_profile.rs` if shared call path needs the same message.
- Modify: `src/tools/subagent/tests.rs`

**Interfaces:**
- Consumes: `create_isolated_workspace(parent_dir) -> Result<PathBuf>`.
- Produces: a clear fallback reason in tool output/status when isolation is skipped due unsafe workspace root.

- [ ] **Step 1: Add test for unsafe workspace error text**

Test that home-like non-git roots return an error containing:

```text
unsafe workspace root
cd into a project git repository
```

- [ ] **Step 2: Improve error message**

Change the current message to include exact user action:

```text
Refusing to recursively copy unsafe workspace root '<path>'. cd into a project git repository before launching OpenZ, or set the agent workspace to a safe project directory. Running subagents in active workspace disables isolation.
```

- [ ] **Step 3: Include fallback metadata in subagent result**

When `create_isolated_workspace` fails and active workspace fallback is used, include `workspaceIsolation: "fallback_active_workspace"` and `workspaceIsolationReason` in the JSON output for `delegate_task`/profile tools.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --lib tools::subagent::tests::test_create_isolated_workspace_rejects_home_like_non_git_root -- --test-threads=1
```

Expected: test passes and message contains the stronger guidance.

---

### Task 5: Full verification and push gate

**Files:**
- No code changes unless verification exposes defects.

- [ ] **Step 1: Run full suite**

```bash
cargo test --lib -- --test-threads=1
cargo check
cargo fmt --check
git diff --check
```

Expected: all pass.

- [ ] **Step 2: Commit source-only changes**

Do not stage `recommendedfix.md` unless the user asks.

```bash
git status --short
git add src/tools/ast_grep.rs src/tools/memory_extra/codebase.rs src/tools/memory_extra/facts.rs src/tools/memory_extra/mod.rs src/config/loader.rs src/tools/subagent/delegate_task.rs src/tools/subagent/delegate_profile.rs src/tools/subagent/tests.rs
git commit -m "fix: harden runtime diagnostics regressions"
```

- [ ] **Step 3: Push only after tests pass**

```bash
git push origin main
```
