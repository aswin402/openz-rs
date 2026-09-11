# Codebase Modularization, Subagent Decomposition & Test Extraction (Phase 9)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Modularize [`src/subagents/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/mod.rs) (1,088 lines) into dedicated domain submodules (`defaults.rs`, `health.rs`, `interactive.rs`, `tests.rs`), extract 766 lines of embedded tests from [`src/tools/shared_memory/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory) (`knowledge_tests.rs`, `auto_capture_tests.rs`), and extract 112 lines of embedded tests from [`src/agent/skills.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills.rs) into [`src/agent/skills_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills_tests.rs). Slashes code bloat while preserving 100% backward compatibility via re-exports.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Decompose `src/subagents/mod.rs` into Domain Submodules
**Files:**
- Create: [`src/subagents/defaults.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/defaults.rs)
- Create: [`src/subagents/health.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/health.rs)
- Create: [`src/subagents/interactive.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/interactive.rs)
- Create: [`src/subagents/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/tests.rs)
- Modify: [`src/subagents/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/mod.rs)

- [x] **Step 1:** Extract `DEFAULT_SUBAGENT_NAMES`, `is_default_subagent`, and `default_profiles()` into `src/subagents/defaults.rs`.
- [x] **Step 2:** Extract `MAX_SUBAGENT_FALLBACKS`, `SubagentHealthRecord`, `SubagentHealthRegistry`, `record_subagent_success`, and `record_subagent_failure` into `src/subagents/health.rs`.
- [x] **Step 3:** Extract interactive CLI wizard menus (`run_subagent_manager`, `manage_menu`, `create_menu`, `generate_profile_with_ai`, `prompt_choose_model`) into `src/subagents/interactive.rs`.
- [x] **Step 4:** Extract unit tests into `src/subagents/tests.rs`.
- [x] **Step 5:** Streamline `src/subagents/mod.rs` to declare submodules, retain `SubagentProfile` and cached loaders, and re-export all items.
- [x] **Step 6:** Run `cargo test -p openz --lib subagents -j 2` and verify all tests pass.
- [x] **Step 7:** Commit: `git commit -am "refactor(subagents): decompose monolithic subagents/mod.rs into domain submodules"`.

---

### Task 2: Extract Shared Memory Unit Tests
**Files:**
- Create: [`src/tools/shared_memory/knowledge_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/knowledge_tests.rs)
- Create: [`src/tools/shared_memory/auto_capture_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/auto_capture_tests.rs)
- Modify: [`src/tools/shared_memory/knowledge.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/knowledge.rs)
- Modify: [`src/tools/shared_memory/auto_capture.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/auto_capture.rs)

- [x] **Step 1:** Extract lines 985–1350 of `knowledge.rs` into `src/tools/shared_memory/knowledge_tests.rs`.
- [x] **Step 2:** Link `knowledge_tests.rs` in `knowledge.rs` via `#[cfg(test)] #[path = "knowledge_tests.rs"] mod tests;`.
- [x] **Step 3:** Extract lines 813–1214 of `auto_capture.rs` into `src/tools/shared_memory/auto_capture_tests.rs`.
- [x] **Step 4:** Link `auto_capture_tests.rs` in `auto_capture.rs` via `#[cfg(test)] #[path = "auto_capture_tests.rs"] mod tests;`.
- [x] **Step 5:** Run `cargo test -p openz --lib tools::shared_memory -j 2` and verify all tests pass.
- [x] **Step 6:** Commit: `git commit -am "refactor(shared_memory): extract unit tests from knowledge and auto_capture into dedicated test modules"`.

---

### Task 3: Extract Skills Unit Tests
**Files:**
- Create: [`src/agent/skills_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills_tests.rs)
- Modify: [`src/agent/skills.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills.rs)

- [x] **Step 1:** Extract lines 992–1103 of `skills.rs` into `src/agent/skills_tests.rs`.
- [x] **Step 2:** Link `skills_tests.rs` in `skills.rs` via `#[cfg(test)] #[path = "skills_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib agent::skills -j 2` and verify all tests pass.
- [x] **Step 4:** Commit: `git commit -am "refactor(skills): extract embedded unit tests into agent/skills_tests.rs"`.

---

### Task 4: Invariant Verification, Documentation & Release Bump (`v0.0.157`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.157` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.157` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.157 with modularized subagents and test suites"`.
