# OpenZ Worktree Quota Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent stale OpenZ subagent worktrees from consuming large disk space under `~/.openz/worktrees`.

**Architecture:** Add a focused cleanup policy in `src/tools/subagent/delegate_task.rs` that prunes `openz_worktree_*` directories by age, count, total byte size, and free-space safety margin. Reuse the existing startup cleanup hook in `src/cli/mod.rs` and the pre-create cleanup call in `create_isolated_workspace()`.

**Tech Stack:** Rust std filesystem APIs, existing OpenZ config path resolver, existing unit test module in `src/tools/subagent/tests.rs`.

## Global Constraints

- Only delete directories named `openz_worktree_*` inside the target worktrees directory.
- Never delete arbitrary user directories.
- Keep cleanup best-effort; failure to delete one directory must not stop app startup.
- Add tests before implementation.

---

### Task 1: Size-Aware Worktree Cleanup

**Files:**
- Modify: `src/tools/subagent/delegate_task.rs`
- Modify: `src/tools/subagent/tests.rs`

**Interfaces:**
- Produces: `pub struct WorktreeCleanupPolicy`
- Produces: `pub fn cleanup_worktrees_dir(worktrees_dir: &Path, policy: WorktreeCleanupPolicy)`
- Produces: `pub fn directory_size_bytes(path: &Path) -> u64`

- [ ] Step 1: Add failing tests for age cleanup and total-size quota cleanup.
- [ ] Step 2: Run `cargo test --lib tools::subagent::tests::test_worktree_cleanup`. Expected: fail because functions/types do not exist.
- [ ] Step 3: Implement policy, directory size walker, and cleanup function.
- [ ] Step 4: Wire `cleanup_stale_resources()` and `create_isolated_workspace()` to use policy-based cleanup.
- [ ] Step 5: Run targeted tests and `cargo check`.
