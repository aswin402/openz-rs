# Codebase Modularization & Core Subsystems Test Decomposition (Phase 13)

**Goal:** Extract embedded test suites from [`src/agent/activity.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/activity.rs) (229 lines), [`src/cron/scheduler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/scheduler.rs) (222 lines), [`src/tools/resource_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy.rs) (199 lines), [`src/tools/searchxyz/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web.rs) (229 lines), and [`src/tools/web_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search.rs) (195 lines) into dedicated test modules. Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.161`.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/agent/activity.rs`
**Files:**
- Create: [`src/agent/activity_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/activity_tests.rs)
- Modify: [`src/agent/activity.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/activity.rs)

- [x] **Step 1:** Extract lines 478–706 of `src/agent/activity.rs` into `src/agent/activity_tests.rs`.
- [x] **Step 2:** In `src/agent/activity.rs`, link via `#[cfg(test)] #[path = "activity_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::activity -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent/activity_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/cron/scheduler.rs`
**Files:**
- Create: [`src/cron/scheduler_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/scheduler_tests.rs)
- Modify: [`src/cron/scheduler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/scheduler.rs)

- [x] **Step 1:** Extract lines 402–623 of `src/cron/scheduler.rs` into `src/cron/scheduler_tests.rs`.
- [x] **Step 2:** In `src/cron/scheduler.rs`, link via `#[cfg(test)] #[path = "scheduler_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib cron::scheduler -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(cron): extract embedded test suite into cron/scheduler_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/tools/resource_policy.rs`
**Files:**
- Create: [`src/tools/resource_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy_tests.rs)
- Modify: [`src/tools/resource_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy.rs)

- [x] **Step 1:** Extract lines 209–407 of `src/tools/resource_policy.rs` into `src/tools/resource_policy_tests.rs`.
- [x] **Step 2:** In `src/tools/resource_policy.rs`, link via `#[cfg(test)] #[path = "resource_policy_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::resource_policy -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/resource_policy_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/searchxyz/web.rs`
**Files:**
- Create: [`src/tools/searchxyz/web_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web_tests.rs)
- Modify: [`src/tools/searchxyz/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web.rs)

- [x] **Step 1:** Extract lines 1214–1442 of `src/tools/searchxyz/web.rs` into `src/tools/searchxyz/web_tests.rs`.
- [x] **Step 2:** In `src/tools/searchxyz/web.rs`, link via `#[cfg(test)] #[path = "web_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::searchxyz::web -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(searchxyz): extract embedded test suite into tools/searchxyz/web_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/web_search.rs`
**Files:**
- Create: [`src/tools/web_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search_tests.rs)
- Modify: [`src/tools/web_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search.rs)

- [x] **Step 1:** Extract lines 773–967 of `src/tools/web_search.rs` into `src/tools/web_search_tests.rs`.
- [x] **Step 2:** In `src/tools/web_search.rs`, link via `#[cfg(test)] #[path = "web_search_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::web_search -j 2` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/web_search_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.161`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.161` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.161` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.161 with decomposed activity, scheduler, resource policy, and search test suites"`.
