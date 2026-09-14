# Codebase Modularization & Core Execution Tools Test Suite Decomposition (Phase 25)

**Goal:** Extract embedded test suites from [`src/tools/mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp.rs) (65 lines), [`src/tools/clipboard.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard.rs) (47 lines), [`src/tools/cargo_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager.rs) (40 lines), [`src/tools/crawl.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl.rs) (38 lines), and [`src/tools/open.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open.rs) (27 lines) into dedicated test modules (~217 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.173`.

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

### Task 1: Extract Embedded Unit Tests from `src/tools/mcp.rs`
**Files:**
- Create: [`src/tools/mcp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_tests.rs)
- Modify: [`src/tools/mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp.rs)

- [x] **Step 1:** Extract lines 925–989 of `src/tools/mcp.rs` into `src/tools/mcp_tests.rs`.
- [x] **Step 2:** In `src/tools/mcp.rs`, link via `#[cfg(test)] #[path = "mcp_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::mcp -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/mcp_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/clipboard.rs`
**Files:**
- Create: [`src/tools/clipboard_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard_tests.rs)
- Modify: [`src/tools/clipboard.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard.rs)

- [x] **Step 1:** Extract lines 74–120 of `src/tools/clipboard.rs` into `src/tools/clipboard_tests.rs`.
- [x] **Step 2:** In `src/tools/clipboard.rs`, link via `#[cfg(test)] #[path = "clipboard_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::clipboard -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/clipboard_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/cargo_manager.rs`
**Files:**
- Create: [`src/tools/cargo_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager_tests.rs)
- Modify: [`src/tools/cargo_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager.rs)

- [x] **Step 1:** Extract lines 314–353 of `src/tools/cargo_manager.rs` into `src/tools/cargo_manager_tests.rs`.
- [x] **Step 2:** In `src/tools/cargo_manager.rs`, link via `#[cfg(test)] #[path = "cargo_manager_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::cargo_manager -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/cargo_manager_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/crawl.rs`
**Files:**
- Create: [`src/tools/crawl_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl_tests.rs)
- Modify: [`src/tools/crawl.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl.rs)

- [x] **Step 1:** Extract lines 274–311 of `src/tools/crawl.rs` into `src/tools/crawl_tests.rs`.
- [x] **Step 2:** In `src/tools/crawl.rs`, link via `#[cfg(test)] #[path = "crawl_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::crawl -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/crawl_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/open.rs`
**Files:**
- Create: [`src/tools/open_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open_tests.rs)
- Modify: [`src/tools/open.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open.rs)

- [x] **Step 1:** Extract lines 126–152 of `src/tools/open.rs` into `src/tools/open_tests.rs`.
- [x] **Step 2:** In `src/tools/open.rs`, link via `#[cfg(test)] #[path = "open_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::open -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/open_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.173`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.173` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.173` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.173 with decomposed core execution tools test suites"`.
