# Codebase Modularization & Core Engine Test Decomposition (Phase 12)

**Goal:** Extract embedded test suites from [`src/config/schema.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs) (383 lines), [`src/config/loader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs) (358 lines), [`src/sop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/sop/mod.rs) (266 lines), [`src/tools/filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs) (247 lines), and [`src/agent/agent_loop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/mod.rs) (237 lines) into dedicated test modules. Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.160`.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/config/schema.rs`
**Files:**
- Create: [`src/config/schema_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema_tests.rs)
- Modify: [`src/config/schema.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs)

- [x] **Step 1:** Extract lines 1112–1495 of `src/config/schema.rs` into `src/config/schema_tests.rs`.
- [x] **Step 2:** In `src/config/schema.rs`, link via `#[cfg(test)] #[path = "schema_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib config::schema -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(config): extract embedded test suites into config/schema_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/config/loader.rs`
**Files:**
- Create: [`src/config/loader_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader_tests.rs)
- Modify: [`src/config/loader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs)

- [x] **Step 1:** Extract lines 570–927 of `src/config/loader.rs` into `src/config/loader_tests.rs`.
- [x] **Step 2:** In `src/config/loader.rs`, link via `#[cfg(test)] #[path = "loader_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib config::loader -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(config): extract embedded test suite into config/loader_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/sop/mod.rs`
**Files:**
- Create: [`src/sop/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/sop/tests.rs)
- Modify: [`src/sop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/sop/mod.rs)

- [x] **Step 1:** Extract lines 411–676 of `src/sop/mod.rs` into `src/sop/tests.rs`.
- [x] **Step 2:** In `src/sop/mod.rs`, link via `#[cfg(test)] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib sop -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(sop): extract embedded test suite into sop/tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/filesystem.rs`
**Files:**
- Create: [`src/tools/filesystem_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem_tests.rs)
- Modify: [`src/tools/filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs)

- [x] **Step 1:** Extract lines 537–783 of `src/tools/filesystem.rs` into `src/tools/filesystem_tests.rs`.
- [x] **Step 2:** In `src/tools/filesystem.rs`, link via `#[cfg(test)] #[path = "filesystem_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::filesystem -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/filesystem_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/agent/agent_loop/mod.rs`
**Files:**
- Create: [`src/agent/agent_loop/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tests.rs)
- Modify: [`src/agent/agent_loop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/mod.rs)

- [x] **Step 1:** Extract lines 685–921 of `src/agent/agent_loop/mod.rs` into `src/agent/agent_loop/tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/mod.rs`, link via `#[cfg(test)] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent_loop): extract embedded test suite into agent_loop/tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.160`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.160` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.160` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.160 with decomposed config, sop, filesystem, and agent loop test suites"`.
