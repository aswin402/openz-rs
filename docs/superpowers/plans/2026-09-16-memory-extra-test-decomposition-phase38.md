# Phase 38: Memory Extra Subsystem Test Suite Modularization & Tests Monolith Deletion Implementation Plan

## Overview
Decompose OpenZ's single largest remaining monolithic test file, [`src/tools/memory_extra/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/tests.rs) (1,460 lines, 36 tests), into component-local sibling test modules colocated with their target memory subsystems:
- `working_tests.rs` (4 tests)
- `episodic_tests.rs` (3 tests)
- `search_tests.rs` (3 tests)
- `coordinator_tests.rs` (5 tests)
- `codebase_tests.rs` (4 tests)
- `facts_tests.rs` (17 tests)

Completely delete `src/tools/memory_extra/tests.rs`, harden SQLite test locks to eliminate database contention, verify the 260 registered native tools invariant and 0 clippy warnings, and bump OpenZ to `v0.0.186`.

---

## File Structure & Responsibility Map

| Module | Source File | Sibling Test Suite | Tests | Description |
|---|---|---|---|---|
| Working Memory | [`src/tools/memory_extra/working.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/working.rs) | [`src/tools/memory_extra/working_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/working_tests.rs) | 4 | Set, get, expire, promote, evict working memory slots |
| Episodic Memory | [`src/tools/memory_extra/episodic.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/episodic.rs) | [`src/tools/memory_extra/episodic_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/episodic_tests.rs) | 3 | Execution episodes, reflections, tool performance metrics with `test_lock` |
| Search Memory | [`src/tools/memory_extra/search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/search.rs) | [`src/tools/memory_extra/search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/search_tests.rs) | 3 | Shared team memory, FTS5 text search, hybrid semantic search |
| Memory Coordinator | [`src/tools/memory_extra/coordinator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/coordinator.rs) | [`src/tools/memory_extra/coordinator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/coordinator_tests.rs) | 5 | Cross-tier memory recall, conflict resolution, semantic slot arbitration |
| Codebase Memory | [`src/tools/memory_extra/codebase.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/codebase.rs) | [`src/tools/memory_extra/codebase_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/codebase_tests.rs) | 4 | Codebase indexing, context compression, memory statistics counting |
| Semantic Facts | [`src/tools/memory_extra/facts.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/facts.rs) | [`src/tools/memory_extra/facts_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/facts_tests.rs) | 17 | Fact extraction, history queries, invalidation, forget purging, proactive recall |
| Memory Extra Root | [`src/tools/memory_extra/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/mod.rs) | — | — | Clean up `mod tests;` reference, host shared `TestEnvLock` if needed |
| Legacy Monolith | [`src/tools/memory_extra/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/tests.rs) | — | — | Deleted completely |

---

## Tasks

### Task 1: Extract Working Memory Unit Tests into `working_tests.rs`
- **Goal:** Extract 4 tests for working memory (`test_set_get_working_memory`, `test_get_working_memory_expired`, `test_promote_working_memory`, `test_evict_expired_working_memory`) from `tests.rs` into `src/tools/memory_extra/working_tests.rs`.
- **Files:**
  - Create: `src/tools/memory_extra/working_tests.rs`
  - Modify: `src/tools/memory_extra/working.rs` (add `#[cfg(test)] mod tests;`)
  - Remove extracted tests from `src/tools/memory_extra/tests.rs`
- **Verification:** `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::memory_extra::working -j 1`.
- **Commit:** `refactor(memory_extra): extract working memory unit tests into dedicated sibling module`.

### Task 2: Extract Episodic and Search Unit Tests into `episodic_tests.rs` & `search_tests.rs`
- **Goal:** Extract 3 tests for episodic memory into `episodic_tests.rs` (adding `test_lock().lock().await` to eliminate SQLite database locks) and 3 tests for search memory into `search_tests.rs`.
- **Files:**
  - Create: `src/tools/memory_extra/episodic_tests.rs`, `src/tools/memory_extra/search_tests.rs`
  - Modify: `src/tools/memory_extra/episodic.rs` (add `#[cfg(test)] mod tests;`), `src/tools/memory_extra/search.rs` (add `#[cfg(test)] mod tests;`)
  - Remove extracted tests from `src/tools/memory_extra/tests.rs`
- **Verification:** `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::memory_extra::episodic -j 1` and `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::memory_extra::search -j 1`.
- **Commit:** `refactor(memory_extra): extract episodic and search unit tests into dedicated sibling modules`.

### Task 3: Extract Memory Coordinator Unit Tests into `coordinator_tests.rs`
- **Goal:** Extract 5 coordinator tests (`test_memory_coordinator_write_recall_stats_and_forget`, `test_memory_coordinator_auto_importance_and_exclusive_relation_resolution`, `test_memory_coordinator_resolves_semantic_similarity_conflicts_by_importance`, `test_memory_coordinator_resolves_semantic_slot_conflicts`, `test_memory_coordinator_writes_semantic_ids_and_graph_relations`) into `coordinator_tests.rs`.
- **Files:**
  - Create: `src/tools/memory_extra/coordinator_tests.rs`
  - Modify: `src/tools/memory_extra/coordinator.rs` (add `#[cfg(test)] mod tests;`)
  - Remove extracted tests from `src/tools/memory_extra/tests.rs`
- **Verification:** `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::memory_extra::coordinator -j 1`.
- **Commit:** `refactor(memory_extra): extract coordinator unit tests into dedicated sibling module`.

### Task 4: Extract Codebase Memory Unit Tests into `codebase_tests.rs`
- **Goal:** Extract 4 codebase tests (`test_index_codebase_captures_rust_impl_trait_methods`, `test_memory_stats_uses_coordinator_snapshot`, `test_memory_stats_counts_session_skills_and_working_layers`, `test_compress_context`) into `codebase_tests.rs`.
- **Files:**
  - Create: `src/tools/memory_extra/codebase_tests.rs` (including `TestEnvLock`)
  - Modify: `src/tools/memory_extra/codebase.rs` (add `#[cfg(test)] mod tests;`)
  - Remove extracted tests from `src/tools/memory_extra/tests.rs`
- **Verification:** `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::memory_extra::codebase -j 1`.
- **Commit:** `refactor(memory_extra): extract codebase memory unit tests into dedicated sibling module`.

### Task 5: Extract Facts Unit Tests into `facts_tests.rs` and Delete Monolithic `tests.rs`
- **Goal:** Extract 17 facts and proactive recall tests into `facts_tests.rs`, declare `#[cfg(test)] mod tests;` in `facts.rs`, delete `src/tools/memory_extra/tests.rs`, and remove `#[cfg(test)] mod tests;` from `src/tools/memory_extra/mod.rs`.
- **Files:**
  - Create: `src/tools/memory_extra/facts_tests.rs`
  - Modify: `src/tools/memory_extra/facts.rs` (add `#[cfg(test)] mod tests;`), `src/tools/memory_extra/mod.rs` (remove `mod tests;`)
  - Delete: `src/tools/memory_extra/tests.rs`
- **Verification:** `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::memory_extra -j 1`.
- **Commit:** `refactor(memory_extra): complete test decomposition, extract facts_tests, and delete monolithic tests.rs`.

### Task 6: Invariant & Linter Verification
- **Goal:** Ensure all 36 memory_extra tests pass, verify exact 260 native tools invariant, verify 0 clippy warnings.
- **Commands:**
  - `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::memory_extra -j 1 -- --test-threads=1`
  - `CARGO_INCREMENTAL=0 cargo test -p openz --lib test_native_tool_registration_names -j 1`
  - `cargo clippy -p openz --lib -j 2`

### Task 7: Release Bump (`v0.0.186`), Documentation & Artifact Creation
- **Goal:** Synchronous bump across `Cargo.toml`, `onpkg.json`, `README.md`, add `CHANGELOG.md` entry, add Section 4.50 in `recommendedfix.md`, write Phase 38 artifact, and commit.
- **Commit:** `chore(release): bump openz to v0.0.186 completing memory_extra test suite decomposition`.
