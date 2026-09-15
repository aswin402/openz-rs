# Codebase Modularization & Agent Loop Subsystem Test Suite Decomposition (Phase 28)

**Goal:** Extract embedded unit test suites from [`src/agent/agent_loop/tool_execution.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tool_execution.rs) (45 lines), [`src/agent/agent_loop/save.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/save.rs) (28 lines), [`src/agent/agent_loop/intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent.rs) (40 lines), [`src/agent/agent_loop/streaming.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/streaming.rs) (33 lines), and [`src/agent/marketplace_intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/marketplace_intent.rs) (73 lines) into dedicated test modules (~219 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.176`.

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

### Task 1: Extract Embedded Unit Tests from `src/agent/agent_loop/tool_execution.rs`
**Files:**
- Create: [`src/agent/agent_loop/tool_execution_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tool_execution_tests.rs)
- Modify: [`src/agent/agent_loop/tool_execution.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tool_execution.rs)

- [x] **Step 1:** Extract lines 474–517 of `src/agent/agent_loop/tool_execution.rs` into `src/agent/agent_loop/tool_execution_tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/tool_execution.rs`, link via `#[cfg(test)] #[path = "tool_execution_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::tool_execution -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent_loop/tool_execution_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/agent/agent_loop/save.rs`
**Files:**
- Create: [`src/agent/agent_loop/save_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/save_tests.rs)
- Modify: [`src/agent/agent_loop/save.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/save.rs)

- [x] **Step 1:** Extract lines 722–748 of `src/agent/agent_loop/save.rs` into `src/agent/agent_loop/save_tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/save.rs`, link via `#[cfg(test)] #[path = "save_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::save -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent_loop/save_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/agent/agent_loop/intent.rs`
**Files:**
- Create: [`src/agent/agent_loop/intent_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent_tests.rs)
- Modify: [`src/agent/agent_loop/intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent.rs)

- [x] **Step 1:** Extract lines 184–222 of `src/agent/agent_loop/intent.rs` into `src/agent/agent_loop/intent_tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/intent.rs`, link via `#[cfg(test)] #[path = "intent_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::intent -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent_loop/intent_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/agent/agent_loop/streaming.rs`
**Files:**
- Create: [`src/agent/agent_loop/streaming_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/streaming_tests.rs)
- Modify: [`src/agent/agent_loop/streaming.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/streaming.rs)

- [x] **Step 1:** Extract lines 94–125 of `src/agent/agent_loop/streaming.rs` into `src/agent/agent_loop/streaming_tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/streaming.rs`, link via `#[cfg(test)] #[path = "streaming_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::streaming -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent_loop/streaming_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/agent/marketplace_intent.rs`
**Files:**
- Create: [`src/agent/marketplace_intent_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/marketplace_intent_tests.rs)
- Modify: [`src/agent/marketplace_intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/marketplace_intent.rs)

- [x] **Step 1:** Extract lines 106–177 of `src/agent/marketplace_intent.rs` into `src/agent/marketplace_intent_tests.rs`.
- [x] **Step 2:** In `src/agent/marketplace_intent.rs`, link via `#[cfg(test)] #[path = "marketplace_intent_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::marketplace_intent -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent/marketplace_intent_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.176`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.176` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.176` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.176 with decomposed agent loop test suites"`.
