# Codebase Modularization & Media/System Test Decomposition (Phase 16)

**Goal:** Extract embedded test suites from [`src/tools/svg_animator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator.rs) (138 lines), [`src/tools/openmedia/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod.rs) (136 lines), [`src/agent/style/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/style/mod.rs) (134 lines), [`src/cron/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/mod.rs) (123 lines), and [`src/tools/task_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager.rs) (119 lines) into dedicated test modules. Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.164`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/tools/svg_animator.rs`
**Files:**
- Create: [`src/tools/svg_animator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator_tests.rs)
- Modify: [`src/tools/svg_animator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator.rs)

- [x] **Step 1:** Extract lines 1020–1157 of `src/tools/svg_animator.rs` into `src/tools/svg_animator_tests.rs`.
- [x] **Step 2:** In `src/tools/svg_animator.rs`, link via `#[cfg(test)] #[path = "svg_animator_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::svg_animator -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/svg_animator_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/openmedia/mod.rs`
**Files:**
- Create: [`src/tools/openmedia/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod_tests.rs)
- Modify: [`src/tools/openmedia/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod.rs)

- [x] **Step 1:** Extract lines 763–898 of `src/tools/openmedia/mod.rs` into `src/tools/openmedia/mod_tests.rs`.
- [x] **Step 2:** In `src/tools/openmedia/mod.rs`, link via `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::openmedia -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(openmedia): extract embedded test suite into tools/openmedia/mod_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/agent/style/mod.rs`
**Files:**
- Create: [`src/agent/style/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/style/mod_tests.rs)
- Modify: [`src/agent/style/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/style/mod.rs)

- [x] **Step 1:** Extract lines 701–834 of `src/agent/style/mod.rs` into `src/agent/style/mod_tests.rs`.
- [x] **Step 2:** In `src/agent/style/mod.rs`, link via `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::style -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(style): extract embedded test suite into agent/style/mod_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/cron/mod.rs`
**Files:**
- Create: [`src/cron/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/mod_tests.rs)
- Modify: [`src/cron/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/mod.rs)

- [x] **Step 1:** Extract lines 333–455 of `src/cron/mod.rs` into `src/cron/mod_tests.rs`.
- [x] **Step 2:** In `src/cron/mod.rs`, link via `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib cron -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(cron): extract embedded test suite into cron/mod_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/task_manager.rs`
**Files:**
- Create: [`src/tools/task_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager_tests.rs)
- Modify: [`src/tools/task_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager.rs)

- [x] **Step 1:** Extract lines 348–466 of `src/tools/task_manager.rs` into `src/tools/task_manager_tests.rs`.
- [x] **Step 2:** In `src/tools/task_manager.rs`, link via `#[cfg(test)] #[path = "task_manager_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::task_manager -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/task_manager_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.164`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.164` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.164` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.164 with decomposed svg_animator, openmedia, style, cron, and task_manager test suites"`.
