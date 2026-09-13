# Codebase Modularization & Tools/Security/Events Test Decomposition (Phase 20)

**Goal:** Extract embedded test suites from [`src/tools/get_logs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs.rs) (80 lines), [`src/tools/manage_whitelist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist.rs) (77 lines), [`src/agent/events.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/events.rs) (76 lines), [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs) (74 lines), and [`src/tools/doc_reader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader.rs) (73 lines) into dedicated test modules (~380 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.168`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Use hardened `[profile.dev]` and `[profile.test]` profiles (`debug = 0`, `codegen-units = 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/tools/get_logs.rs`
**Files:**
- Create: [`src/tools/get_logs_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs_tests.rs)
- Modify: [`src/tools/get_logs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs.rs)

- [x] **Step 1:** Extract lines 159–239 of `src/tools/get_logs.rs` into `src/tools/get_logs_tests.rs`.
- [x] **Step 2:** In `src/tools/get_logs.rs`, link via `#[cfg(test)] #[path = "get_logs_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::get_logs -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/get_logs_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/manage_whitelist.rs`
**Files:**
- Create: [`src/tools/manage_whitelist_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist_tests.rs)
- Modify: [`src/tools/manage_whitelist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist.rs)

- [x] **Step 1:** Extract lines 153–230 of `src/tools/manage_whitelist.rs` into `src/tools/manage_whitelist_tests.rs`.
- [x] **Step 2:** In `src/tools/manage_whitelist.rs`, link via `#[cfg(test)] #[path = "manage_whitelist_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::manage_whitelist -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/manage_whitelist_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/agent/events.rs`
**Files:**
- Create: [`src/agent/events_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/events_tests.rs)
- Modify: [`src/agent/events.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/events.rs)

- [x] **Step 1:** Extract lines 131–207 of `src/agent/events.rs` into `src/agent/events_tests.rs`.
- [x] **Step 2:** In `src/agent/events.rs`, link via `#[cfg(test)] #[path = "events_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::events -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent/events_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/core/heal.rs`
**Files:**
- Create: [`src/core/heal_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal_tests.rs)
- Modify: [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs)

- [x] **Step 1:** Extract lines 394–468 of `src/core/heal.rs` into `src/core/heal_tests.rs`.
- [x] **Step 2:** In `src/core/heal.rs`, link via `#[cfg(test)] #[path = "heal_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib core::heal -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(core): extract embedded test suite into core/heal_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/doc_reader.rs`
**Files:**
- Create: [`src/tools/doc_reader_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader_tests.rs)
- Modify: [`src/tools/doc_reader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader.rs)

- [x] **Step 1:** Extract lines 332–405 of `src/tools/doc_reader.rs` into `src/tools/doc_reader_tests.rs`.
- [x] **Step 2:** In `src/tools/doc_reader.rs`, link via `#[cfg(test)] #[path = "doc_reader_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::doc_reader -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/doc_reader_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.168`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.168` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.168` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.168 with decomposed get_logs, manage_whitelist, events, heal, and doc_reader test suites"`.
