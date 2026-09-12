# Codebase Modularization & System Core Test Decomposition (Phase 15)

**Goal:** Extract embedded test suites from [`src/core/inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/inventory.rs) (146 lines), [`src/agent/agent_loop/transcript.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript.rs) (140 lines), [`src/tools/db_inspector.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector.rs) (137 lines), [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs) (133 lines), and [`src/config/watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/watcher.rs) (128 lines) into dedicated test modules. Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.163`.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 1` (`cargo test -p openz --lib <test_name> -j 1`, `cargo clippy -p openz --lib -j 1`) to preserve laptop responsiveness and avoid memory spikes.
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/core/inventory.rs`
**Files:**
- Create: [`src/core/inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/inventory_tests.rs)
- Modify: [`src/core/inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/inventory.rs)

- [x] **Step 1:** Extract lines 615–758 of `src/core/inventory.rs` into `src/core/inventory_tests.rs`.
- [x] **Step 2:** In `src/core/inventory.rs`, link via `#[cfg(test)] #[path = "inventory_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib core::inventory -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(core): extract embedded test suite into core/inventory_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/agent/agent_loop/transcript.rs`
**Files:**
- Create: [`src/agent/agent_loop/transcript_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript_tests.rs)
- Modify: [`src/agent/agent_loop/transcript.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript.rs)

- [x] **Step 1:** Extract lines 183–320 of `src/agent/agent_loop/transcript.rs` into `src/agent/agent_loop/transcript_tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/transcript.rs`, link via `#[cfg(test)] #[path = "transcript_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::transcript -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent/agent_loop/transcript_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/db_inspector.rs`
**Files:**
- Create: [`src/tools/db_inspector_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector_tests.rs)
- Modify: [`src/tools/db_inspector.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector.rs)

- [x] **Step 1:** Extract lines 266–400 of `src/tools/db_inspector.rs` into `src/tools/db_inspector_tests.rs`.
- [x] **Step 2:** In `src/tools/db_inspector.rs`, link via `#[cfg(test)] #[path = "db_inspector_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::db_inspector -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/db_inspector_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/providers/model_prefs.rs`
**Files:**
- Create: [`src/providers/model_prefs_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs_tests.rs)
- Modify: [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs)

- [x] **Step 1:** Extract lines 131–261 of `src/providers/model_prefs.rs` into `src/providers/model_prefs_tests.rs`.
- [x] **Step 2:** In `src/providers/model_prefs.rs`, link via `#[cfg(test)] #[path = "model_prefs_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib providers::model_prefs -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(providers): extract embedded test suite into providers/model_prefs_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/config/watcher.rs`
**Files:**
- Create: [`src/config/watcher_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/watcher_tests.rs)
- Modify: [`src/config/watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/watcher.rs)

- [x] **Step 1:** Extract lines 168–293 of `src/config/watcher.rs` into `src/config/watcher_tests.rs`.
- [x] **Step 2:** In `src/config/watcher.rs`, link via `#[cfg(test)] #[path = "watcher_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib config::watcher -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(config): extract embedded test suite into config/watcher_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.163`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.163` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.163` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 1`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.163 with decomposed inventory, transcript, db_inspector, model_prefs, and watcher test suites"`.
