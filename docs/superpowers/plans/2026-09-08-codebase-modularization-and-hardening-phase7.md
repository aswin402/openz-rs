# Codebase Modularization, Test Extraction & Channel Decoupling Plan (Phase 7)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Modularize three oversized root and runtime files by extracting huge embedded test suites and channel command handlers into dedicated submodules:
1. Extract 1,463 lines of embedded unit tests from [`src/tools/memory_extra/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/mod.rs) into `src/tools/memory_extra/tests.rs` (slashing `mod.rs` from 1,496 lines to ~35 lines, ~97% reduction).
2. Extract 801 lines of embedded unit tests from [`src/orchestrator/runtime.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/runtime.rs) into `src/orchestrator/runtime_tests.rs` (slashing `runtime.rs` from 1,420 lines to 618 lines, ~56.5% reduction).
3. Extract model switch commands from [`src/channels/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs) into `src/channels/model_switch.rs` and embedded tests into `src/channels/tests.rs` (slashing `mod.rs` from 1,020 lines to ~280 lines, ~72% reduction).
4. Release bump to `v0.0.155` across all version surfaces with exact 260 native tools invariant and 0 clippy warnings.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Embedded Unit Tests from `src/tools/memory_extra/mod.rs` into `tests.rs`
**Files:**
- Create: [`src/tools/memory_extra/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/tests.rs)
- Modify: [`src/tools/memory_extra/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/mod.rs)

- [x] **Step 1:** Create `src/tools/memory_extra/tests.rs` containing all 36 unit tests and test locks.
- [x] **Step 2:** Update `src/tools/memory_extra/mod.rs` to declare `#[cfg(test)] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::memory_extra -j 2` and verify all 36 tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 5:** Commit: `git commit -am "refactor(memory_extra): extract 1460 lines of unit tests into dedicated tests submodule"`.

---

### Task 2: Extract Embedded Unit Tests from `src/orchestrator/runtime.rs` into `runtime_tests.rs`
**Files:**
- Create: [`src/orchestrator/runtime_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/runtime_tests.rs)
- Modify: [`src/orchestrator/runtime.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/runtime.rs)
- Modify: [`src/orchestrator/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/mod.rs)

- [x] **Step 1:** Create `src/orchestrator/runtime_tests.rs` containing all 21 workflow runtime tests.
- [x] **Step 2:** Update `src/orchestrator/runtime.rs` and `mod.rs` to register the test module.
- [x] **Step 3:** Run `cargo test -p openz --lib orchestrator::runtime -j 2` and verify all 21 tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 5:** Commit: `git commit -am "refactor(orchestrator): extract 800 lines of runtime unit tests into runtime_tests submodule"`.

---

### Task 3: Decompose `src/channels/mod.rs` into `model_switch.rs` and `tests.rs`
**Files:**
- Create: [`src/channels/model_switch.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/model_switch.rs)
- Create: [`src/channels/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/tests.rs)
- Modify: [`src/channels/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs)

- [x] **Step 1:** Create `src/channels/model_switch.rs` with `ModelSwitchCommand` and associated formatting/parsing routines.
- [x] **Step 2:** Create `src/channels/tests.rs` with the extracted channel test suites.
- [x] **Step 3:** Update `src/channels/mod.rs` to re-export `model_switch::*` preserving 100% backward compatibility.
- [x] **Step 4:** Run `cargo test -p openz --lib channels -j 2` and verify all tests pass.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "refactor(channels): extract model switch commands and test suites into dedicated submodules"`.

---

### Task 4: Invariant Verification, Documentation & Release Bump (`v0.0.155`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.155` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.155` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.155 with modularized subsystem roots and decoupled channel commands"`.
