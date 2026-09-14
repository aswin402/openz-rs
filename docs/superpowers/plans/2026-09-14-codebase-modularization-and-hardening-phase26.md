# Codebase Modularization & Developer Utilities Test Suite Decomposition (Phase 26)

**Goal:** Extract embedded unit test suites from [`src/tools/js_format.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format.rs) (32 lines), [`src/tools/mermaid.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid.rs) (28 lines), [`src/tools/network.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network.rs) (18 lines), [`src/tools/system_info.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info.rs) (14 lines), and [`src/tools/remote.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote.rs) (11 lines) into dedicated test modules (~103 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.174`.

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

### Task 1: Extract Embedded Unit Tests from `src/tools/js_format.rs`
**Files:**
- Create: [`src/tools/js_format_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format_tests.rs)
- Modify: [`src/tools/js_format.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format.rs)

- [x] **Step 1:** Extract lines 82–112 of `src/tools/js_format.rs` into `src/tools/js_format_tests.rs`.
- [x] **Step 2:** In `src/tools/js_format.rs`, link via `#[cfg(test)] #[path = "js_format_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::js_format -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/js_format_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/mermaid.rs`
**Files:**
- Create: [`src/tools/mermaid_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid_tests.rs)
- Modify: [`src/tools/mermaid.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid.rs)

- [x] **Step 1:** Extract lines 65–91 of `src/tools/mermaid.rs` into `src/tools/mermaid_tests.rs`.
- [x] **Step 2:** In `src/tools/mermaid.rs`, link via `#[cfg(test)] #[path = "mermaid_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::mermaid -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/mermaid_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/network.rs`
**Files:**
- Create: [`src/tools/network_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network_tests.rs)
- Modify: [`src/tools/network.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network.rs)

- [x] **Step 1:** Extract lines 140–156 of `src/tools/network.rs` into `src/tools/network_tests.rs`.
- [x] **Step 2:** In `src/tools/network.rs`, link via `#[cfg(test)] #[path = "network_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::network -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/network_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/system_info.rs`
**Files:**
- Create: [`src/tools/system_info_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info_tests.rs)
- Modify: [`src/tools/system_info.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info.rs)

- [x] **Step 1:** Extract lines 102–114 of `src/tools/system_info.rs` into `src/tools/system_info_tests.rs`.
- [x] **Step 2:** In `src/tools/system_info.rs`, link via `#[cfg(test)] #[path = "system_info_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::system_info -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/system_info_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/remote.rs`
**Files:**
- Create: [`src/tools/remote_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote_tests.rs)
- Modify: [`src/tools/remote.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote.rs)

- [x] **Step 1:** Extract lines 85–94 of `src/tools/remote.rs` into `src/tools/remote_tests.rs`.
- [x] **Step 2:** In `src/tools/remote.rs`, link via `#[cfg(test)] #[path = "remote_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::remote -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/remote_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.174`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.174` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.174` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.174 with decomposed developer utilities test suites"`.
