# Codebase Modularization & Interactive Subsystems Test Decomposition (Phase 14)

**Goal:** Extract embedded test suites from [`src/tools/orchestrator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator.rs) (175 lines), [`src/grounding.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/grounding.rs) (171 lines), [`src/channels/ratatui/app.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/app.rs) (165 lines), [`src/tools/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web.rs) (161 lines), and [`src/tools/cron.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron.rs) (153 lines) into dedicated test modules. Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.162`.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` or `-j 1` (`cargo test -p openz --lib <test_name> -j 1`, `cargo clippy -p openz --lib -j 1`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/tools/orchestrator.rs`
**Files:**
- Create: [`src/tools/orchestrator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator_tests.rs)
- Modify: [`src/tools/orchestrator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator.rs)

- [x] **Step 1:** Extract lines 304–475 of `src/tools/orchestrator.rs` into `src/tools/orchestrator_tests.rs`.
- [x] **Step 2:** In `src/tools/orchestrator.rs`, replace embedded test block with `#[cfg(test)] #[path = "orchestrator_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::orchestrator -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(orchestrator): extract embedded test suite into tools/orchestrator_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/grounding.rs`
**Files:**
- Create: [`src/grounding_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/grounding_tests.rs)
- Modify: [`src/grounding.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/grounding.rs)

- [x] **Step 1:** Extract lines 275–443 of `src/grounding.rs` into `src/grounding_tests.rs`.
- [x] **Step 2:** In `src/grounding.rs`, link via `#[cfg(test)] #[path = "grounding_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib grounding -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(grounding): extract embedded test suite into grounding_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/channels/ratatui/app.rs`
**Files:**
- Create: [`src/channels/ratatui/app_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/app_tests.rs)
- Modify: [`src/channels/ratatui/app.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/app.rs)

- [x] **Step 1:** Extract lines 422–584 of `src/channels/ratatui/app.rs` into `src/channels/ratatui/app_tests.rs`.
- [x] **Step 2:** In `src/channels/ratatui/app.rs`, link via `#[cfg(test)] #[path = "app_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::ratatui::app -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(ratatui): extract embedded test suite into channels/ratatui/app_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/web.rs`
**Files:**
- Create: [`src/tools/web_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_tests.rs)
- Modify: [`src/tools/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web.rs)

- [x] **Step 1:** Extract lines 782–940 of `src/tools/web.rs` into `src/tools/web_tests.rs`.
- [x] **Step 2:** In `src/tools/web.rs`, link via `#[cfg(test)] #[path = "web_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::web -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/web_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/cron.rs`
**Files:**
- Create: [`src/tools/cron_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron_tests.rs)
- Modify: [`src/tools/cron.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron.rs)

- [x] **Step 1:** Extract lines 448–598 of `src/tools/cron.rs` into `src/tools/cron_tests.rs`.
- [x] **Step 2:** In `src/tools/cron.rs`, link via `#[cfg(test)] #[path = "cron_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::cron -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 1` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/cron_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.162`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.162` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.162` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 1`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.162 with decomposed orchestrator, grounding, ratatui, web, and cron test suites"`.
