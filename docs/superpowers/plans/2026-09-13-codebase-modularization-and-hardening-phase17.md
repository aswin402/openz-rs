# Codebase Modularization & Core Providers/Security Test Decomposition (Phase 17)

**Goal:** Extract embedded test suites from [`src/tools/device_inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory.rs) (118 lines), [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs) (115 lines), [`src/providers/openai.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/openai.rs) (108 lines), [`src/providers/mock.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mock.rs) (100 lines), and [`src/agent/source_ledger.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/source_ledger.rs) (94 lines) into dedicated test modules. Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.165`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/tools/device_inventory.rs`
**Files:**
- Create: [`src/tools/device_inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory_tests.rs)
- Modify: [`src/tools/device_inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory.rs)

- [x] **Step 1:** Extract lines 706–823 of `src/tools/device_inventory.rs` into `src/tools/device_inventory_tests.rs`.
- [x] **Step 2:** In `src/tools/device_inventory.rs`, link via `#[cfg(test)] #[path = "device_inventory_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::device_inventory -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/device_inventory_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/core/secrets.rs`
**Files:**
- Create: [`src/core/secrets_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets_tests.rs)
- Modify: [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs)

- [x] **Step 1:** Extract lines 161–275 of `src/core/secrets.rs` into `src/core/secrets_tests.rs`.
- [x] **Step 2:** In `src/core/secrets.rs`, link via `#[cfg(test)] #[path = "secrets_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib core::secrets -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(core): extract embedded test suite into core/secrets_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/providers/openai.rs`
**Files:**
- Create: [`src/providers/openai_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/openai_tests.rs)
- Modify: [`src/providers/openai.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/openai.rs)

- [x] **Step 1:** Extract lines 781–888 of `src/providers/openai.rs` into `src/providers/openai_tests.rs`.
- [x] **Step 2:** In `src/providers/openai.rs`, link via `#[cfg(test)] #[path = "openai_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib providers::openai -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(providers): extract embedded test suite into providers/openai_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/providers/mock.rs`
**Files:**
- Create: [`src/providers/mock_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mock_tests.rs)
- Modify: [`src/providers/mock.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mock.rs)

- [x] **Step 1:** Extract lines 191–290 of `src/providers/mock.rs` into `src/providers/mock_tests.rs`.
- [x] **Step 2:** In `src/providers/mock.rs`, link via `#[cfg(test)] #[path = "mock_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib providers::mock -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(providers): extract embedded test suite into providers/mock_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/agent/source_ledger.rs`
**Files:**
- Create: [`src/agent/source_ledger_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/source_ledger_tests.rs)
- Modify: [`src/agent/source_ledger.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/source_ledger.rs)

- [x] **Step 1:** Extract lines 211–304 of `src/agent/source_ledger.rs` into `src/agent/source_ledger_tests.rs`.
- [x] **Step 2:** In `src/agent/source_ledger.rs`, link via `#[cfg(test)] #[path = "source_ledger_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::source_ledger -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent/source_ledger_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.165`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.165` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.165` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.165 with decomposed device_inventory, secrets, openai, mock, and source_ledger test suites"`.
