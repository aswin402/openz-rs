# Codebase Modularization & Test Suite Decomposition (Phase 11)

**Goal:** Extract embedded test suites from [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs) (578 lines), [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs) (554 lines), [`src/agent/agent_loop/loop_control.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/loop_control.rs) (495 lines), and [`src/providers/resolver.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/resolver.rs) (448 lines) into dedicated test modules. Preserves 100% backward compatibility and exact 260 registered native tools invariant.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/cli/builder.rs`
**Files:**
- Create: [`src/cli/builder_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder_tests.rs)
- Modify: [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs)

- [x] **Step 1:** Extract lines 37–614 of `src/cli/builder.rs` into `src/cli/builder_tests.rs`.
- [x] **Step 2:** In `src/cli/builder.rs`, link via `#[cfg(test)] #[path = "builder_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib cli::builder -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(cli): extract embedded test suite into cli/builder_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/cli/tools.rs`
**Files:**
- Create: [`src/cli/tools_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools_tests.rs)
- Modify: [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs)

- [x] **Step 1:** Extract lines 34–587 of `src/cli/tools.rs` into `src/cli/tools_tests.rs`.
- [x] **Step 2:** In `src/cli/tools.rs`, link via `#[cfg(test)] #[path = "tools_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib cli::tools -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(cli): extract embedded test suite into cli/tools_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/agent/agent_loop/loop_control.rs`
**Files:**
- Create: [`src/agent/agent_loop/loop_control_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/loop_control_tests.rs)
- Modify: [`src/agent/agent_loop/loop_control.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/loop_control.rs)

- [x] **Step 1:** Extract lines 284–778 of `src/agent/agent_loop/loop_control.rs` into `src/agent/agent_loop/loop_control_tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/loop_control.rs`, link via `#[cfg(test)] #[path = "loop_control_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::loop_control -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent_loop): extract embedded test suite into loop_control_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/providers/resolver.rs`
**Files:**
- Create: [`src/providers/resolver_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/resolver_tests.rs)
- Modify: [`src/providers/resolver.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/resolver.rs)

- [x] **Step 1:** Extract lines 302–749 of `src/providers/resolver.rs` into `src/providers/resolver_tests.rs`.
- [x] **Step 2:** In `src/providers/resolver.rs`, link via `#[cfg(test)] #[path = "resolver_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib providers::resolver -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(providers): extract embedded test suite into resolver_tests.rs"`.

---

### Task 5: Invariant Verification, Documentation & Release Bump (`v0.0.159`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.159` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.159` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.159 with modularized cli and loop test suites"`.
