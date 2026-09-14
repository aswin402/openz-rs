# Codebase Modularization & Browser Subsystem Test Suite Decomposition (Phase 23)

**Goal:** Extract embedded test suites from [`src/tools/browser/status.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status.rs) (73 lines), [`src/tools/browser/gsd.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd.rs) (69 lines), [`src/tools/browser/firefox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox.rs) (61 lines), [`src/tools/browser/broker.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/broker.rs) (38 lines), [`src/tools/browser/common.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common.rs) (16 lines), and [`src/tools/browser/obscura.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura.rs) (12 lines) into dedicated test modules (~269 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.171`.

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

### Task 1: Extract Embedded Unit Tests from `src/tools/browser/status.rs`
**Files:**
- Create: [`src/tools/browser/status_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status_tests.rs)
- Modify: [`src/tools/browser/status.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status.rs)

- [x] **Step 1:** Extract lines 318–390 of `src/tools/browser/status.rs` into `src/tools/browser/status_tests.rs`.
- [x] **Step 2:** In `src/tools/browser/status.rs`, link via `#[cfg(test)] #[path = "status_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::browser::status -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(browser): extract embedded test suite into browser/status_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/tools/browser/gsd.rs`
**Files:**
- Create: [`src/tools/browser/gsd_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd_tests.rs)
- Modify: [`src/tools/browser/gsd.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd.rs)

- [x] **Step 1:** Extract lines 292–360 of `src/tools/browser/gsd.rs` into `src/tools/browser/gsd_tests.rs`.
- [x] **Step 2:** In `src/tools/browser/gsd.rs`, link via `#[cfg(test)] #[path = "gsd_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::browser::gsd -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(browser): extract embedded test suite into browser/gsd_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/browser/firefox.rs`
**Files:**
- Create: [`src/tools/browser/firefox_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox_tests.rs)
- Modify: [`src/tools/browser/firefox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox.rs)

- [x] **Step 1:** Extract lines 421–481 of `src/tools/browser/firefox.rs` into `src/tools/browser/firefox_tests.rs`.
- [x] **Step 2:** In `src/tools/browser/firefox.rs`, link via `#[cfg(test)] #[path = "firefox_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::browser::firefox -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(browser): extract embedded test suite into browser/firefox_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/browser/broker.rs`
**Files:**
- Create: [`src/tools/browser/broker_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/broker_tests.rs)
- Modify: [`src/tools/browser/broker.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/broker.rs)

- [x] **Step 1:** Extract lines 205–242 of `src/tools/browser/broker.rs` into `src/tools/browser/broker_tests.rs`.
- [x] **Step 2:** In `src/tools/browser/broker.rs`, link via `#[cfg(test)] #[path = "broker_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::browser::broker -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(browser): extract embedded test suite into browser/broker_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/browser/common.rs` and `src/tools/browser/obscura.rs`
**Files:**
- Create: [`src/tools/browser/common_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common_tests.rs)
- Modify: [`src/tools/browser/common.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common.rs)
- Create: [`src/tools/browser/obscura_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura_tests.rs)
- Modify: [`src/tools/browser/obscura.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura.rs)

- [x] **Step 1:** Extract lines 179–194 of `src/tools/browser/common.rs` into `src/tools/browser/common_tests.rs`.
- [x] **Step 2:** In `src/tools/browser/common.rs`, link via `#[cfg(test)] #[path = "common_tests.rs"] mod tests;`.
- [x] **Step 3:** Extract lines 326–337 of `src/tools/browser/obscura.rs` into `src/tools/browser/obscura_tests.rs`.
- [x] **Step 4:** In `src/tools/browser/obscura.rs`, link via `#[cfg(test)] #[path = "obscura_tests.rs"] mod tests;`.
- [x] **Step 5:** Run `cargo test -p openz --lib tools::browser -j 1` and verify all tests pass.
- [x] **Step 6:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 7:** Commit: `git commit -am "refactor(browser): extract embedded test suites into browser/common_tests.rs and obscura_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.171`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.171` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.171` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.171 with decomposed browser subsystem test suites"`.
