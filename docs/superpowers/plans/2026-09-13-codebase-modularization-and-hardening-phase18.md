# Codebase Modularization & Core Tools Test Suite Decomposition (Phase 18)

**Goal:** Extract embedded test suites from [`src/tools/outline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline.rs) (93 lines), [`src/tools/mcp_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager.rs) (89 lines), [`src/tools/shell.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs) (87 lines), [`src/tools/semantic_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search.rs) (85 lines), and [`src/tools/git_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager.rs) (85 lines) into dedicated test modules (~439 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.166`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/tools/outline.rs`
**Files:**
- Create: [`src/tools/outline_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline_tests.rs)
- Modify: [`src/tools/outline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline.rs)

- [x] **Step 1:** Extract lines 289–381 of `src/tools/outline.rs` into `src/tools/outline_tests.rs`.
- [x] **Step 2:** In `src/tools/outline.rs`, link via `#[cfg(test)] #[path = "outline_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::outline -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/outline_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/mcp_manager.rs`
**Files:**
- Create: [`src/tools/mcp_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager_tests.rs)
- Modify: [`src/tools/mcp_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager.rs)

- [x] **Step 1:** Extract lines 188–276 of `src/tools/mcp_manager.rs` into `src/tools/mcp_manager_tests.rs`.
- [x] **Step 2:** In `src/tools/mcp_manager.rs`, link via `#[cfg(test)] #[path = "mcp_manager_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::mcp_manager -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/mcp_manager_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/shell.rs`
**Files:**
- Create: [`src/tools/shell_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell_tests.rs)
- Modify: [`src/tools/shell.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs)

- [x] **Step 1:** Extract lines 901–987 of `src/tools/shell.rs` into `src/tools/shell_tests.rs`.
- [x] **Step 2:** In `src/tools/shell.rs`, link via `#[cfg(test)] #[path = "shell_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::shell -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/shell_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/semantic_search.rs`
**Files:**
- Create: [`src/tools/semantic_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search_tests.rs)
- Modify: [`src/tools/semantic_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search.rs)

- [x] **Step 1:** Extract lines 345–429 of `src/tools/semantic_search.rs` into `src/tools/semantic_search_tests.rs`.
- [x] **Step 2:** In `src/tools/semantic_search.rs`, link via `#[cfg(test)] #[path = "semantic_search_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::semantic_search -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/semantic_search_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/git_manager.rs`
**Files:**
- Create: [`src/tools/git_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager_tests.rs)
- Modify: [`src/tools/git_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager.rs)

- [x] **Step 1:** Extract lines 123–207 of `src/tools/git_manager.rs` into `src/tools/git_manager_tests.rs`.
- [x] **Step 2:** In `src/tools/git_manager.rs`, link via `#[cfg(test)] #[path = "git_manager_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::git_manager -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/git_manager_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.166`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.166` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.166` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.166 with decomposed outline, mcp_manager, shell, semantic_search, and git_manager test suites"`.
