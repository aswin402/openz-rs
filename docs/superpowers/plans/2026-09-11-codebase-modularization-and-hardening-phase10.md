# Codebase Modularization, Subagent Runner Extraction & Test Suite Decomposition (Phase 10)

**Goal:** Modularize [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs) (921 lines) by extracting execution runner logic into [`src/tools/subagent/runner.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/runner.rs), extract 652 lines of tests from [`src/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session.rs) into [`src/session_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session_tests.rs), extract 708 lines of tests from [`src/tools/headroom/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/mod.rs) into [`src/tools/headroom/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/tests.rs), extract 682 lines of tests from [`src/tools/self_management/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/mod.rs) into [`src/tools/self_management/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/tests.rs), and bump to `v0.0.158`.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Subagent Runner & Execution Logic into `src/tools/subagent/runner.rs`
**Files:**
- Create: [`src/tools/subagent/runner.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/runner.rs)
- Modify: [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs)

- [x] **Step 1:** Extract `CancelOnDrop`, `WorkspaceIsolation`, `create_workspace_isolation`, `SubagentRunAttempt`, `execute_subagent_run_attempt`, `SubagentRunOutcome`, `handle_subagent_cancellation`, and related execution helpers (lines 62–920 of `src/tools/subagent/mod.rs`) into `src/tools/subagent/runner.rs`.
- [x] **Step 2:** Declare `pub mod runner;` and `pub use runner::*;` in `src/tools/subagent/mod.rs`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::subagent -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(subagent): extract execution runner and workspace isolation into tools/subagent/runner.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/session.rs`
**Files:**
- Create: [`src/session_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session_tests.rs)
- Modify: [`src/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session.rs)

- [x] **Step 1:** Extract lines 183–835 of `src/session.rs` (`hash_tests`, `lock_tests`, `delete_tests`, `summary_tests`) into `src/session_tests.rs`.
- [x] **Step 2:** In `src/session.rs`, link via `#[cfg(test)] #[path = "session_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib session -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(session): extract embedded test suites into session_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/headroom/mod.rs`
**Files:**
- Create: [`src/tools/headroom/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/tests.rs)
- Modify: [`src/tools/headroom/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/mod.rs)

- [x] **Step 1:** Extract lines 31–739 of `src/tools/headroom/mod.rs` into `src/tools/headroom/tests.rs`.
- [x] **Step 2:** In `src/tools/headroom/mod.rs`, link via `#[cfg(test)] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::headroom -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(headroom): extract embedded test suite into tools/headroom/tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/self_management/mod.rs`
**Files:**
- Create: [`src/tools/self_management/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/tests.rs)
- Modify: [`src/tools/self_management/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/mod.rs)

- [x] **Step 1:** Extract lines 30–711 of `src/tools/self_management/mod.rs` into `src/tools/self_management/tests.rs`.
- [x] **Step 2:** In `src/tools/self_management/mod.rs`, link via `#[cfg(test)] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::self_management -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(self_management): extract embedded test suite into tools/self_management/tests.rs"`.

---

### Task 5: Invariant Verification, Documentation & Release Bump (`v0.0.158`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.158` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.158` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.158 with modularized subagent runner and test suites"`.
