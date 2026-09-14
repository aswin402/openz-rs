# Codebase Modularization & Developer Tools Test Suite Decomposition (Phase 22)

**Goal:** Extract embedded test suites from [`src/tools/ast_grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep.rs) (63 lines), [`src/tools/github.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github.rs) (59 lines), [`src/tools/arguments.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs) (58 lines), [`src/tools/grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep.rs) (53 lines), and [`src/tools/image_generator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator.rs) (47 lines) into dedicated test modules (~280 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.170`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Use hardened `[profile.dev]` and `[profile.test]` profiles (`debug = 0`, `codegen-units = 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/tools/ast_grep.rs`
**Files:**
- Create: [`src/tools/ast_grep_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep_tests.rs)
- Modify: [`src/tools/ast_grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep.rs)

- [x] **Step 1:** Extract lines 384–445 of `src/tools/ast_grep.rs` into `src/tools/ast_grep_tests.rs`.
- [x] **Step 2:** In `src/tools/ast_grep.rs`, link via `#[cfg(test)] #[path = "ast_grep_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::ast_grep -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/ast_grep_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/github.rs`
**Files:**
- Create: [`src/tools/github_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_tests.rs)
- Modify: [`src/tools/github.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github.rs)

- [x] **Step 1:** Extract lines 462–519 of `src/tools/github.rs` into `src/tools/github_tests.rs`.
- [x] **Step 2:** In `src/tools/github.rs`, link via `#[cfg(test)] #[path = "github_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::github -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/github_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/arguments.rs`
**Files:**
- Create: [`src/tools/arguments_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments_tests.rs)
- Modify: [`src/tools/arguments.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs)

- [x] **Step 1:** Extract lines 126–182 of `src/tools/arguments.rs` into `src/tools/arguments_tests.rs`.
- [x] **Step 2:** In `src/tools/arguments.rs`, link via `#[cfg(test)] #[path = "arguments_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::arguments -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/arguments_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/grep.rs`
**Files:**
- Create: [`src/tools/grep_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep_tests.rs)
- Modify: [`src/tools/grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep.rs)

- [x] **Step 1:** Extract lines 293–344 of `src/tools/grep.rs` into `src/tools/grep_tests.rs`.
- [x] **Step 2:** In `src/tools/grep.rs`, link via `#[cfg(test)] #[path = "grep_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::grep -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/grep_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/image_generator.rs`
**Files:**
- Create: [`src/tools/image_generator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator_tests.rs)
- Modify: [`src/tools/image_generator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator.rs)

- [x] **Step 1:** Extract lines 604–649 of `src/tools/image_generator.rs` into `src/tools/image_generator_tests.rs`.
- [x] **Step 2:** In `src/tools/image_generator.rs`, link via `#[cfg(test)] #[path = "image_generator_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::image_generator -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/image_generator_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.170`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.170` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.170` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.170 with decomposed ast_grep, github, arguments, grep, and image_generator test suites"`.
