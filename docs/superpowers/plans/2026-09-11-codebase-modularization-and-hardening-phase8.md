# Codebase Modularization, ToolRegistry Extraction & Subsystem Hardening (Phase 8)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Modularize the 1,570-line monolithic [`src/tools/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mod.rs) by extracting `ToolRegistry`, route analysis, and dynamic subagent resolution into [`src/tools/registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs), and extracting route cache unit tests into [`src/tools/registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry_tests.rs). Slashes `src/tools/mod.rs` from 1,570 lines to ~210 lines (~86.6% reduction) while preserving 100% backward compatibility via re-exports.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Extract Route Cache & Tool Routing Tests into `src/tools/registry_tests.rs`
**Files:**
- Create: [`src/tools/registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry_tests.rs)

- [x] **Step 1:** Extract lines 906–1385 of `src/tools/mod.rs` (`route_cache_tests`) into `src/tools/registry_tests.rs`.
- [x] **Step 2:** Ensure imports and test utilities (`CacheTestTool`, mock providers, route tests) compile cleanly.

---

### Task 2: Migrate `ToolRegistry` and Route Types into `src/tools/registry.rs`
**Files:**
- Modify: [`src/tools/registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs)

- [x] **Step 1:** Move `ToolRouteEntry`, `ToolRouteAnalysis`, `PendingToolScope`, `ToolRouteCacheKey`, `ToolRegistry`, and all associated methods into `src/tools/registry.rs`.
- [x] **Step 2:** Integrate with existing `insert_unique_tool`, `resolve_static_name`, and `static_tool_drift` mechanics.
- [x] **Step 3:** Include `#[cfg(test)] #[path = "registry_tests.rs"] mod tests;` in `src/tools/registry.rs`.

---

### Task 3: Streamline `src/tools/mod.rs` to Clean Module Facade
**Files:**
- Modify: [`src/tools/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mod.rs)

- [x] **Step 1:** Replace lines 102–1385 with `pub mod registry; pub use registry::*;`.
- [x] **Step 2:** Run `cargo test -p openz --lib tools::registry -j 2` and verify all route tests pass.
- [x] **Step 3:** Run `cargo clippy -p openz --lib -j 2` and verify 0 warnings.
- [x] **Step 4:** Commit: `git commit -am "refactor(tools): extract ToolRegistry and route tests into dedicated registry submodules"`.

---

### Task 4: Invariant Verification, Documentation & Release Bump (`v0.0.156`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.156` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.156` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.156 with modularized tools registry facade"`.
