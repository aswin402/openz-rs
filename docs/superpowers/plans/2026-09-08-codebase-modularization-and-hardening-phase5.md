# Codebase Modularization, Workspace Isolation & Hardening Plan (Phase 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract subagent workspace isolation and worktree lifecycle management out of [`src/tools/subagent/delegate_task.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task.rs) into a dedicated [`src/tools/subagent/workspace.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/workspace.rs) module (slashing `delegate_task.rs` from 1,125 lines down to ~290 lines), extract 430 lines of embedded unit tests from [`src/agent/agent_loop/build.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs) into [`src/agent/agent_loop/build_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build_tests.rs), and release `v0.0.153`.

**Architecture:**
1. **Subagent Workspace Isolation Extraction (P1 Item 1.1 in `recommendedfix.md`)**:
   - Create [`src/tools/subagent/workspace.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/workspace.rs) containing `WorktreeGuard`, `WorktreeCleanupPolicy`, worktree registry, quota enforcement, stale resource cleanup, workspace creation (`create_isolated_workspace`), filtered copying, change sync-back (`sync_changes_back`), simulation teardown messages, and post-run evolution review.
   - Refactor [`src/tools/subagent/delegate_task.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task.rs) to focus purely on `DelegateTaskTool` and `delegate_task_models_to_try`, re-exporting `workspace::*` for backward compatibility.
   - Update [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs) and [`src/tools/subagent/delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs) to import directly from `workspace`.
2. **Prompt Builder Test Decomposition**:
   - Extract 430 lines of unit tests (lines 1098–1527) from [`src/agent/agent_loop/build.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs) into [`src/agent/agent_loop/build_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build_tests.rs).
   - Link cleanly in `build.rs` via `#[cfg(test)] #[path = "build_tests.rs"] mod build_tests;`.
3. **Release Bump & Invariant Verification**:
   - Multi-surface bump to `v0.0.153` across `Cargo.toml`, `onpkg.json`, `README.md`, `CHANGELOG.md`, and `recommendedfix.md`.
   - Verify exact 260 native tools invariant and 0 clippy warnings.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Subagent Workspace Lifecycle into `src/tools/subagent/workspace.rs`
**Files:**
- Create: [`src/tools/subagent/workspace.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/workspace.rs)
- Modify: [`src/tools/subagent/delegate_task.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task.rs)
- Modify: [`src/tools/subagent/delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs)
- Modify: [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs)

- [x] **Step 1:** Create `src/tools/subagent/workspace.rs` with `WorktreeGuard`, `WorktreeCleanupPolicy`, cleanup registry, disk quota helpers, workspace creation/teardown, recursive directory filtering, sync back, and evolution review.
- [x] **Step 2:** Refactor `src/tools/subagent/delegate_task.rs` to re-export `pub use super::workspace::*;` and retain only `DelegateTaskTool` and `delegate_task_models_to_try`.
- [x] **Step 3:** Register `pub mod workspace;` in `src/tools/subagent/mod.rs` and update imports in `delegate_profile.rs`.
- [x] **Step 4:** Run `cargo test -p openz --lib tools::subagent -j 2` and verify all subagent unit tests pass.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2` to verify 0 warnings.
- [x] **Step 6:** Commit: `git commit -am "refactor(subagent): extract workspace isolation and worktree lifecycle into workspace module"`.

---

### Task 2: Extract Embedded Unit Tests from `src/agent/agent_loop/build.rs`
**Files:**
- Create: [`src/agent/agent_loop/build_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build_tests.rs)
- Modify: [`src/agent/agent_loop/build.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs)

- [x] **Step 1:** Extract lines 1098–1527 from `src/agent/agent_loop/build.rs` into `src/agent/agent_loop/build_tests.rs`.
- [x] **Step 2:** Link `build_tests.rs` from `src/agent/agent_loop/build.rs` via `#[cfg(test)] #[path = "build_tests.rs"] mod build_tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::build -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to verify 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent_loop): extract embedded build unit tests into dedicated build_tests module"`.

---

### Task 3: Invariant Verification, Documentation & Release Bump (`v0.0.153`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.153` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.153` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.153 with subagent workspace extraction and build test decomposition"`.
