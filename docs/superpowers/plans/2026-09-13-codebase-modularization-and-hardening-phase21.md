# Codebase Modularization & Channels/Ratatui/Template Test Decomposition (Phase 21)

**Goal:** Extract embedded test suites from [`src/channels/whatsapp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs) (73 lines), [`src/channels/notifications.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/notifications.rs) (73 lines), [`src/channels/cli/input.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/input.rs) (65 lines), [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs) (62 lines), and [`src/tools/template_compiler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler.rs) (62 lines) into dedicated test modules (~335 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.169`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Use hardened `[profile.dev]` and `[profile.test]` profiles (`debug = 0`, `codegen-units = 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/channels/whatsapp.rs`
**Files:**
- Create: [`src/channels/whatsapp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp_tests.rs)
- Modify: [`src/channels/whatsapp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs)

- [x] **Step 1:** Extract lines 372–444 of `src/channels/whatsapp.rs` into `src/channels/whatsapp_tests.rs`.
- [x] **Step 2:** In `src/channels/whatsapp.rs`, link via `#[cfg(test)] #[path = "whatsapp_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::whatsapp -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into channels/whatsapp_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/channels/notifications.rs`
**Files:**
- Create: [`src/channels/notifications_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/notifications_tests.rs)
- Modify: [`src/channels/notifications.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/notifications.rs)

- [x] **Step 1:** Extract lines 252–323 of `src/channels/notifications.rs` into `src/channels/notifications_tests.rs`.
- [x] **Step 2:** In `src/channels/notifications.rs`, link via `#[cfg(test)] #[path = "notifications_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::notifications -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into channels/notifications_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/channels/cli/input.rs`
**Files:**
- Create: [`src/channels/cli/input_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/input_tests.rs)
- Modify: [`src/channels/cli/input.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/input.rs)

- [x] **Step 1:** Extract lines 762–826 of `src/channels/cli/input.rs` into `src/channels/cli/input_tests.rs`.
- [x] **Step 2:** In `src/channels/cli/input.rs`, link via `#[cfg(test)] #[path = "input_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::cli::input -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into channels/cli/input_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/channels/ratatui/session.rs`
**Files:**
- Create: [`src/channels/ratatui/session_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session_tests.rs)
- Modify: [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs)

- [x] **Step 1:** Extract lines 156–216 of `src/channels/ratatui/session.rs` into `src/channels/ratatui/session_tests.rs`.
- [x] **Step 2:** In `src/channels/ratatui/session.rs`, link via `#[cfg(test)] #[path = "session_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::ratatui::session -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into channels/ratatui/session_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/template_compiler.rs`
**Files:**
- Create: [`src/tools/template_compiler_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler_tests.rs)
- Modify: [`src/tools/template_compiler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler.rs)

- [x] **Step 1:** Extract lines 217–277 of `src/tools/template_compiler.rs` into `src/tools/template_compiler_tests.rs`.
- [x] **Step 2:** In `src/tools/template_compiler.rs`, link via `#[cfg(test)] #[path = "template_compiler_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::template_compiler -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/template_compiler_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.169`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.169` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.169` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.169 with decomposed whatsapp, notifications, input, session, and template_compiler test suites"`.
