# Codebase Modularization & Workflow Automation Tools Test Suite Decomposition (Phase 27)

**Goal:** Extract embedded unit test suites from [`src/tools/compiler_auto_heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs) (58 lines), [`src/tools/telegram_send.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/telegram_send.rs) (19 lines), [`src/tools/wasm_sandbox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/wasm_sandbox.rs) (18 lines), [`src/tools/watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/watcher.rs) (17 lines), and [`src/tools/sop.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sop.rs) (16 lines) into dedicated test modules (~128 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.175`.

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

### Task 1: Extract Embedded Unit Tests from `src/tools/compiler_auto_heal.rs`
**Files:**
- Create: [`src/tools/compiler_auto_heal_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal_tests.rs)
- Modify: [`src/tools/compiler_auto_heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs)

- [x] **Step 1:** Extract lines 80–136 of `src/tools/compiler_auto_heal.rs` into `src/tools/compiler_auto_heal_tests.rs`.
- [x] **Step 2:** In `src/tools/compiler_auto_heal.rs`, link via `#[cfg(test)] #[path = "compiler_auto_heal_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::compiler_auto_heal -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/compiler_auto_heal_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/telegram_send.rs`
**Files:**
- Create: [`src/tools/telegram_send_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/telegram_send_tests.rs)
- Modify: [`src/tools/telegram_send.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/telegram_send.rs)

- [x] **Step 1:** Extract lines 257–274 of `src/tools/telegram_send.rs` into `src/tools/telegram_send_tests.rs`.
- [x] **Step 2:** In `src/tools/telegram_send.rs`, link via `#[cfg(test)] #[path = "telegram_send_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::telegram_send -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/telegram_send_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/wasm_sandbox.rs`
**Files:**
- Create: [`src/tools/wasm_sandbox_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/wasm_sandbox_tests.rs)
- Modify: [`src/tools/wasm_sandbox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/wasm_sandbox.rs)

- [x] **Step 1:** Extract lines 141–157 of `src/tools/wasm_sandbox.rs` into `src/tools/wasm_sandbox_tests.rs`.
- [x] **Step 2:** In `src/tools/wasm_sandbox.rs`, link via `#[cfg(test)] #[path = "wasm_sandbox_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::wasm_sandbox -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/wasm_sandbox_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/watcher.rs`
**Files:**
- Create: [`src/tools/watcher_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/watcher_tests.rs)
- Modify: [`src/tools/watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/watcher.rs)

- [x] **Step 1:** Extract lines 233–248 of `src/tools/watcher.rs` into `src/tools/watcher_tests.rs`.
- [x] **Step 2:** In `src/tools/watcher.rs`, link via `#[cfg(test)] #[path = "watcher_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::watcher -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/watcher_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/sop.rs`
**Files:**
- Create: [`src/tools/sop_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sop_tests.rs)
- Modify: [`src/tools/sop.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sop.rs)

- [x] **Step 1:** Extract lines 59–73 of `src/tools/sop.rs` into `src/tools/sop_tests.rs`.
- [x] **Step 2:** In `src/tools/sop.rs`, link via `#[cfg(test)] #[path = "sop_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::sop -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/sop_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.175`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.175` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.175` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.175 with decomposed workflow automation test suites"`.
