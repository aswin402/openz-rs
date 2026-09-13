# Codebase Modularization & Diagnostics/Rendering Test Decomposition (Phase 19)

**Goal:** Extract embedded test suites from [`src/cli/doctor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs) (85 lines), [`src/channels/cli/render.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render.rs) (85 lines), [`src/agent/agent_loop/research_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/research_policy.rs) (85 lines), [`src/tools/shared_memory/workflows.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/workflows.rs) (82 lines), and [`src/tools/video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video.rs) (80 lines) into dedicated test modules (~417 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, and bumps release to `v0.0.167`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Use hardened `[profile.dev]` and `[profile.test]` profiles (`debug = 0`, `codegen-units = 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/cli/doctor.rs`
**Files:**
- Create: [`src/cli/doctor_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor_tests.rs)
- Modify: [`src/cli/doctor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs)

- [x] **Step 1:** Extract lines 517–601 of `src/cli/doctor.rs` into `src/cli/doctor_tests.rs`.
- [x] **Step 2:** In `src/cli/doctor.rs`, link via `#[cfg(test)] #[path = "doctor_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib cli::doctor -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(cli): extract embedded test suite into cli/doctor_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/channels/cli/render.rs`
**Files:**
- Create: [`src/channels/cli/render_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render_tests.rs)
- Modify: [`src/channels/cli/render.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render.rs)

- [x] **Step 1:** Extract lines 1124–1208 of `src/channels/cli/render.rs` into `src/channels/cli/render_tests.rs`.
- [x] **Step 2:** In `src/channels/cli/render.rs`, link via `#[cfg(test)] #[path = "render_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::cli::render -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into channels/cli/render_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/agent/agent_loop/research_policy.rs`
**Files:**
- Create: [`src/agent/agent_loop/research_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/research_policy_tests.rs)
- Modify: [`src/agent/agent_loop/research_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_loop/openz/src/agent/agent_loop/research_policy.rs)

- [x] **Step 1:** Extract lines 248–332 of `src/agent/agent_loop/research_policy.rs` into `src/agent/agent_loop/research_policy_tests.rs`.
- [x] **Step 2:** In `src/agent/agent_loop/research_policy.rs`, link via `#[cfg(test)] #[path = "research_policy_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::agent_loop::research_policy -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(agent): extract embedded test suite into agent/agent_loop/research_policy_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/tools/shared_memory/workflows.rs`
**Files:**
- Create: [`src/tools/shared_memory/workflows_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/workflows_tests.rs)
- Modify: [`src/tools/shared_memory/workflows.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/workflows.rs)

- [x] **Step 1:** Extract lines 380–461 of `src/tools/shared_memory/workflows.rs` into `src/tools/shared_memory/workflows_tests.rs`.
- [x] **Step 2:** In `src/tools/shared_memory/workflows.rs`, link via `#[cfg(test)] #[path = "workflows_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::shared_memory::workflows -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/shared_memory/workflows_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/tools/video.rs`
**Files:**
- Create: [`src/tools/video_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video_tests.rs)
- Modify: [`src/tools/video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video.rs)

- [x] **Step 1:** Extract lines 130–209 of `src/tools/video.rs` into `src/tools/video_tests.rs`.
- [x] **Step 2:** In `src/tools/video.rs`, link via `#[cfg(test)] #[path = "video_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::video -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(tools): extract embedded test suite into tools/video_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.167`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.167` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.167` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.167 with decomposed doctor, render, research_policy, workflows, and video test suites"`.
