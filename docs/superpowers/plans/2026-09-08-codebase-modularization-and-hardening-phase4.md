# Codebase Modularization, Tool Scoping & Hardening Plan (Phase 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix intent prioritization and pack associations for tool scoping, decompose the 1,548-line CLI channel god-file (`src/channels/cli/mod.rs`), extract embedded tests from `src/agent/security.rs`, and release `v0.0.152`.

**Architecture:**
1. **Tool Scoping & Intent Fix**: In [`src/agent/agent_loop/intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent.rs), prioritize `is_cron_query` before `has_live_research_intent` to prevent scheduler commands containing verbs like "browse" from being misclassified as web research. In [`src/tools/defs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/defs.rs), assign `packs: &["core", "local_exec"]` to `manage_servers` and `packs: &["core", "memory"]` to `workflow_memory`. Update legacy assertions in [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs).
2. **CLI TUI Channel Modularization**: Decompose [`src/channels/cli/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/mod.rs) (1,548 lines) by extracting device inventory commands into [`src/channels/cli/device.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device.rs) and interactive slash commands into [`src/channels/cli/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/commands.rs).
3. **Security Test Decomposition**: Extract ~575 lines of embedded unit tests from [`src/agent/security.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security.rs) into [`src/agent/security_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security_tests.rs).
4. **Release Bump & Invariant Verification**: Multi-surface bump to `v0.0.152` across `Cargo.toml`, `onpkg.json`, `README.md`, `CHANGELOG.md`, and `recommendedfix.md`.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Fix Intent Prioritization & Scoped Tool Pack Definitions
**Files:**
- Modify: [`src/agent/agent_loop/intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent.rs)
- Modify: [`src/tools/defs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/defs.rs)
- Modify: [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs)

- [ ] **Step 1:** In `src/agent/agent_loop/intent.rs`, check `is_cron_query` before `has_live_research_intent`.
- [ ] **Step 2:** In `src/tools/defs.rs`, assign packs to `manage_servers` (`&["core", "local_exec"]`) and `workflow_memory` (`&["core", "memory"]`).
- [ ] **Step 3:** In `src/cli/tools.rs`, update tests to align with `ToolScopeEngine` (`cron_scheduler_tools_stay_exposed_under_tool_limit`, `runtime_management_tools_stay_exposed_under_tool_limit`, `openai_format_prioritizes_high_value_tools_when_truncated`, `route_analysis_formats_compact_status_line`, `route_analysis_reports_api_limit_hidden_reason`, `openai_format_reserves_api_slots_for_dynamic_subagents`).
- [ ] **Step 4:** Run `cargo test -p openz --lib cli::tools -j 2` and verify all tests pass.
- [ ] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [ ] **Step 6:** Commit: `git commit -am "fix(router): prioritize cron intent and associate core packs with server and workflow tools"`.

---

### Task 2: Decompose CLI Channel God-File `src/channels/cli/mod.rs`
**Files:**
- Create: [`src/channels/cli/device.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device.rs)
- Create: [`src/channels/cli/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/commands.rs)
- Modify: [`src/channels/cli/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/mod.rs)

- [ ] **Step 1:** Extract `handle_device_command`, `print_device_usage`, `print_device_result`, and `print_device_capability` into `src/channels/cli/device.rs`.
- [ ] **Step 2:** Extract slash command execution into `src/channels/cli/commands.rs`.
- [ ] **Step 3:** Refactor `src/channels/cli/mod.rs` to register submodules and dispatch commands cleanly.
- [ ] **Step 4:** Run `cargo test -p openz --lib channels::cli -j 2`.
- [ ] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [ ] **Step 6:** Commit: `git commit -am "refactor(cli): extract device inventory and slash commands into submodules"`.

---

### Task 3: Extract Embedded Tests from `src/agent/security.rs`
**Files:**
- Create: [`src/agent/security_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security_tests.rs)
- Modify: [`src/agent/security.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security.rs)
- Modify: [`src/agent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/mod.rs)

- [ ] **Step 1:** Extract lines 922–1497 (`mod tests`) from `src/agent/security.rs` into `src/agent/security_tests.rs`.
- [ ] **Step 2:** Update visibilities and imports as necessary.
- [ ] **Step 3:** Register `#[cfg(test)] mod security_tests;` in `src/agent/mod.rs`.
- [ ] **Step 4:** Run `cargo test -p openz --lib agent::security -j 2`.
- [ ] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [ ] **Step 6:** Commit: `git commit -am "refactor(security): extract embedded unit tests into dedicated security_tests module"`.

---

### Task 4: Invariant Verification, Documentation & Release Bump (`v0.0.152`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [ ] **Step 1:** Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [ ] **Step 2:** Bump version to `0.0.152` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [ ] **Step 3:** Add release notes for `v0.0.152` in `CHANGELOG.md` and update `recommendedfix.md`.
- [ ] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [ ] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [ ] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.152 with scoped intent fixes and CLI modularization"`.
