# Codebase Modularization & Multi-Agent Orchestrator and Channels Test Suite Decomposition (Phase 30)

**Goal:** Extract embedded unit test suites from [`src/orchestrator/spec.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/spec.rs) (65 lines), [`src/orchestrator/validation.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/validation.rs) (58 lines), [`src/channels/websocket/auth.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth.rs) (19 lines), [`src/channels/email.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/email.rs) (8 lines), and [`src/channels/cli/device.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device.rs) (11 lines) into dedicated test modules (~161 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.178`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Use hardened `[profile.dev]` and `[profile.test]` profiles (`debug = 0`, `codegen-units = 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.
- Follow Release & Changelog Protocol: explicit Ideas, Inspirations, and Sources in [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md).

---

### Task 1: Extract Embedded Unit Tests from `src/orchestrator/spec.rs`
**Files:**
- Create: [`src/orchestrator/spec_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/spec_tests.rs)
- Modify: [`src/orchestrator/spec.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/spec.rs)

- [ ] **Step 1:** Extract lines 122–185 of `src/orchestrator/spec.rs` into `src/orchestrator/spec_tests.rs`.
- [ ] **Step 2:** In `src/orchestrator/spec.rs`, link via `#[cfg(test)] #[path = "spec_tests.rs"] mod tests;`.
- [ ] **Step 3:** Run `cargo test -p openz --lib orchestrator::spec -j 1` and verify all tests pass.
- [ ] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [ ] **Step 5:** Commit: `git commit -am "refactor(orchestrator): extract embedded test suite into orchestrator/spec_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/orchestrator/validation.rs`
**Files:**
- Create: [`src/orchestrator/validation_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/validation_tests.rs)
- Modify: [`src/orchestrator/validation.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/validation.rs)

- [ ] **Step 1:** Extract lines 80–136 of `src/orchestrator/validation.rs` into `src/orchestrator/validation_tests.rs`.
- [ ] **Step 2:** In `src/orchestrator/validation.rs`, link via `#[cfg(test)] #[path = "validation_tests.rs"] mod tests;`.
- [ ] **Step 3:** Run `cargo test -p openz --lib orchestrator::validation -j 1` and verify all tests pass.
- [ ] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [ ] **Step 5:** Commit: `git commit -am "refactor(orchestrator): extract embedded test suite into orchestrator/validation_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/channels/websocket/auth.rs`
**Files:**
- Create: [`src/channels/websocket/auth_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth_tests.rs)
- Modify: [`src/channels/websocket/auth.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth.rs)

- [ ] **Step 1:** Extract lines 103–120 of `src/channels/websocket/auth.rs` into `src/channels/websocket/auth_tests.rs`.
- [ ] **Step 2:** In `src/channels/websocket/auth.rs`, link via `#[cfg(test)] #[path = "auth_tests.rs"] mod tests;`.
- [ ] **Step 3:** Run `cargo test -p openz --lib channels::websocket::auth -j 1` and verify all tests pass.
- [ ] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [ ] **Step 5:** Commit: `git commit -am "refactor(websocket): extract embedded test suite into websocket/auth_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/channels/email.rs`
**Files:**
- Create: [`src/channels/email_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/email_tests.rs)
- Modify: [`src/channels/email.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/email.rs)

- [ ] **Step 1:** Extract lines 331–337 of `src/channels/email.rs` into `src/channels/email_tests.rs`.
- [ ] **Step 2:** In `src/channels/email.rs`, link via `#[cfg(test)] #[path = "email_tests.rs"] mod tests;`.
- [ ] **Step 3:** Run `cargo test -p openz --lib channels::email -j 1` and verify all tests pass.
- [ ] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [ ] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into channels/email_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/channels/cli/device.rs`
**Files:**
- Create: [`src/channels/cli/device_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device_tests.rs)
- Modify: [`src/channels/cli/device.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device.rs)

- [ ] **Step 1:** Extract lines 244–253 of `src/channels/cli/device.rs` into `src/channels/cli/device_tests.rs`.
- [ ] **Step 2:** In `src/channels/cli/device.rs`, link via `#[cfg(test)] #[path = "device_tests.rs"] mod tests;`.
- [ ] **Step 3:** Run `cargo test -p openz --lib channels::cli::device -j 1` and verify all tests pass.
- [ ] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [ ] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into cli/device_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.178`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [ ] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [ ] **Step 2:** Bump version to `0.0.178` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [ ] **Step 3:** Add release notes for `v0.0.178` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [ ] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [ ] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [ ] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.178 with decomposed orchestrator and channel test suites"`.
