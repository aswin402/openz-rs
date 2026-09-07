# Codebase Modularization and Reflection Loop Deduplication Plan (Phase 2 & 3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decompose the remaining god-files (`src/agent/agent_loop/run/mod.rs` and `src/channels/websocket/mod.rs`), unify redundant compile-and-heal reflection loops into a shared engine in `src/core/heal.rs`, and bump the release to `v0.0.151`.

**Architecture:**
1. Extract embedded unit tests (`mod auto_tool_arg_tests` and `mod tests`, ~500 lines) from [`src/agent/agent_loop/run/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/mod.rs) into a dedicated submodule [`src/agent/agent_loop/run/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/tests.rs).
2. Decompose [`src/channels/websocket/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/mod.rs) by extracting `handle_socket` into [`src/channels/websocket/socket.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/socket.rs) and the HTTP REST handlers (`openai_chat_completions`, `trigger_sop_handler`, `resume_sop_handler`) into [`src/channels/websocket/handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/handlers.rs).
3. Create [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs) implementing a shared compile verification, file backup, rollback, and LLM reflection loop, and refactor [`ZenflowEditTool`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs) and [`CompilerAutoHealTool`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs) to use it.
4. Perform a multi-surface release bump to `v0.0.151`.

**Tech Stack:** Rust (edition 2021), tokio, axum, rusqlite, reqwest, serde_json.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Decompose `src/agent/agent_loop/run/mod.rs` by Extracting Embedded Tests
**Files:**
- Create: [`src/agent/agent_loop/run/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/tests.rs)
- Modify: [`src/agent/agent_loop/run/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/mod.rs)

- [x] **Step 1:** Extract `mod auto_tool_arg_tests` (lines 42–248) and `mod tests` (lines 1386–1662) from `src/agent/agent_loop/run/mod.rs` into `src/agent/agent_loop/run/tests.rs`.
- [x] **Step 2:** Ensure all required imports in `tests.rs` and any necessary `pub(crate)` / `pub(super)` visibilities in `run/` submodules are maintained.
- [x] **Step 3:** Register `#[cfg(test)] mod tests;` in `src/agent/agent_loop/run/mod.rs`.
- [x] **Step 4:** Run `cargo test -p openz --lib agent::agent_loop::run -j 2` and verify all tests pass.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2` to verify 0 warnings.
- [x] **Step 6:** Commit changes: `git commit -am "refactor(agent_loop): extract run tests into dedicated tests submodule"`.

---

### Task 2: Decompose WebSocket Server God-File `src/channels/websocket/mod.rs`
**Files:**
- Create: [`src/channels/websocket/socket.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/socket.rs)
- Create: [`src/channels/websocket/handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/handlers.rs)
- Modify: [`src/channels/websocket/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/mod.rs)

- [x] **Step 1:** Extract `handle_socket` (and helper routines for frame reception/transmission) from `src/channels/websocket/mod.rs` into `src/channels/websocket/socket.rs`.
- [x] **Step 2:** Extract `openai_chat_completions`, `trigger_sop_handler`, and `resume_sop_handler` (and `hono_log_middleware`) into `src/channels/websocket/handlers.rs`.
- [x] **Step 3:** Update `src/channels/websocket/mod.rs` to register the new submodules and mount the routes.
- [x] **Step 4:** Run `cargo test -p openz --lib channels::websocket -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2` to verify 0 warnings.
- [x] **Step 6:** Commit changes: `git commit -am "refactor(websocket): extract socket frame handler and HTTP routes from mod.rs"`.

---

### Task 3: Unify Self-Healing Reflection Engine in `src/core/heal.rs`
**Files:**
- Create: [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs)
- Modify: [`src/core/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/mod.rs)
- Modify: [`src/tools/compiler_auto_heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs)
- Modify: [`src/tools/filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs)

- [x] **Step 1:** Implement canonical compile check, backup/restore, self-healing prompt generation, LLM call, and code fence stripping in `src/core/heal.rs`.
- [x] **Step 2:** Add unit tests for `src/core/heal.rs`.
- [x] **Step 3:** Refactor `CompilerAutoHealTool` in `src/tools/compiler_auto_heal.rs` to use `crate::core::heal`.
- [x] **Step 4:** Refactor `ZenflowEditTool` in `src/tools/filesystem.rs` to use `crate::core::heal`.
- [x] **Step 5:** Run `cargo test -p openz --lib core::heal -j 2`, `cargo test -p openz --lib tools::compiler_auto_heal -j 2`, and `cargo test -p openz --lib tools::filesystem -j 2`.
- [x] **Step 6:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 7:** Commit changes: `git commit -am "refactor(heal): unify self-healing compiler reflection loop in core::heal"`.

---

### Task 4: Invariant Verification, Documentation & Multi-Surface Release Bump (`v0.0.151`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [ ] **Step 1:** Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [ ] **Step 2:** Bump version to `0.0.151` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [ ] **Step 3:** Add release notes for `v0.0.151` in `CHANGELOG.md` and update `recommendedfix.md`.
- [ ] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [ ] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [ ] **Step 6:** Commit release bump: `git commit -am "chore(release): bump openz to v0.0.151 with god-file modularization and unified reflection loop"`.
