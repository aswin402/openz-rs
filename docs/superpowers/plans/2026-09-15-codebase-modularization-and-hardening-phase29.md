# Codebase Modularization & Core System Utilities Test Suite Decomposition (Phase 29)

**Goal:** Extract embedded unit test suites from [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs) (37 lines), [`src/core/sqlite.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/sqlite.rs) (32 lines), [`src/core/http.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http.rs) (17 lines), [`src/shutdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/shutdown.rs) (35 lines), and [`src/model_registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/model_registry.rs) (37 lines) into dedicated test modules (~158 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.177`.

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

### Task 1: Extract Embedded Unit Tests from `src/core/process.rs`
**Files:**
- Create: [`src/core/process_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process_tests.rs)
- Modify: [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs)

- [x] **Step 1:** Extract lines 67–102 of `src/core/process.rs` into `src/core/process_tests.rs`.
- [x] **Step 2:** In `src/core/process.rs`, link via `#[cfg(test)] #[path = "process_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib core::process -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(core): extract embedded test suite into core/process_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/core/sqlite.rs`
**Files:**
- Create: [`src/core/sqlite_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/sqlite_tests.rs)
- Modify: [`src/core/sqlite.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/sqlite.rs)

- [x] **Step 1:** Extract lines 30–60 of `src/core/sqlite.rs` into `src/core/sqlite_tests.rs`.
- [x] **Step 2:** In `src/core/sqlite.rs`, link via `#[cfg(test)] #[path = "sqlite_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib core::sqlite -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(core): extract embedded test suite into core/sqlite_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/core/http.rs`
**Files:**
- Create: [`src/core/http_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http_tests.rs)
- Modify: [`src/core/http.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http.rs)

- [x] **Step 1:** Extract lines 37–52 of `src/core/http.rs` into `src/core/http_tests.rs`.
- [x] **Step 2:** In `src/core/http.rs`, link via `#[cfg(test)] #[path = "http_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib core::http -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(core): extract embedded test suite into core/http_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/shutdown.rs`
**Files:**
- Create: [`src/shutdown_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/shutdown_tests.rs)
- Modify: [`src/shutdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/shutdown.rs)

- [x] **Step 1:** Extract lines 256–289 of `src/shutdown.rs` into `src/shutdown_tests.rs`.
- [x] **Step 2:** In `src/shutdown.rs`, link via `#[cfg(test)] #[path = "shutdown_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib shutdown -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(shutdown): extract embedded test suite into shutdown_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/model_registry.rs`
**Files:**
- Create: [`src/model_registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/model_registry_tests.rs)
- Modify: [`src/model_registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/model_registry.rs)

- [x] **Step 1:** Extract lines 179–214 of `src/model_registry.rs` into `src/model_registry_tests.rs`.
- [x] **Step 2:** In `src/model_registry.rs`, link via `#[cfg(test)] #[path = "model_registry_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib model_registry -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(model_registry): extract embedded test suite into model_registry_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.177`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.177` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.177` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.177 with decomposed core utilities and reliability test suites"`.
