# Ratatui Terminal TUI Modularization Plan (Phase 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Modularize the monolithic Ratatui TUI channel by decomposing [`src/channels/ratatui/ui.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/ui.rs) (1,104 lines) into `markdown.rs`, `timeline.rs`, and `modals.rs`, extracting session/process state from [`src/channels/ratatui/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/mod.rs) into `session.rs`, and releasing `v0.0.154`.

**Architecture:**
1. **UI Subsystem Decomposition (`channels/ratatui/ui.rs`)**:
   - Extract inline markdown regex compilation and span generation into [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs) (~205 lines).
   - Extract conversation timeline, ASCII logo banner, system metadata badges, message card rendering, and auto-scroll logic into [`src/channels/ratatui/timeline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/timeline.rs) (~345 lines).
   - Extract modal dialog overlays (provider select, model select, skills, help, approval, confirmation) and geometry helpers into [`src/channels/ratatui/modals.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals.rs) (~305 lines).
   - Retain top-level layout composition (`render_ratatui_ui`), input dock (`render_input_dock`), autocomplete popup (`render_autocomplete_dock`), and status bar (`render_status_bar`) in [`src/channels/ratatui/ui.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/ui.rs), shrinking it from 1,104 lines to ~250 lines (~77% reduction).
2. **Session and Process State Decomposition (`channels/ratatui/mod.rs`)**:
   - Extract multi-instance process markers (`tui_marker_dir`, `write_tui_marker_in_dir`, `remove_tui_marker_in_dir`, `process_is_alive`, `is_last_live_tui_in_dir`) and session mutation routines (`save_session_model_override`, `save_session_streaming_override`, `save_default_model_selection`, `apply_session_model_selection`, `reset_active_session`) into [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs) (~160 lines).
3. **Release Bump & Invariant Verification**:
   - Multi-surface bump to `v0.0.154` across `Cargo.toml`, `onpkg.json`, `README.md`, `CHANGELOG.md`, and `recommendedfix.md`.
   - Verify exact 260 native tools invariant and 0 clippy warnings.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Decompose `src/channels/ratatui/ui.rs` into `markdown.rs`, `timeline.rs`, and `modals.rs`
**Files:**
- Create: [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs)
- Create: [`src/channels/ratatui/timeline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/timeline.rs)
- Create: [`src/channels/ratatui/modals.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals.rs)
- Modify: [`src/channels/ratatui/ui.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/ui.rs)
- Modify: [`src/channels/ratatui/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/mod.rs)

- [x] **Step 1:** Create `src/channels/ratatui/markdown.rs` with `markdown_line_to_spans` and `parse_inline_markdown`.
- [x] **Step 2:** Create `src/channels/ratatui/timeline.rs` with `render_timeline`.
- [x] **Step 3:** Create `src/channels/ratatui/modals.rs` with `render_modal_overlay` and `centered_rect`.
- [x] **Step 4:** Refactor `src/channels/ratatui/ui.rs` to register submodules and dispatch to `timeline::render_timeline` and `modals::render_modal_overlay`.
- [x] **Step 5:** Run `cargo test -p openz --lib channels::ratatui -j 2` and verify all tests pass.
- [x] **Step 6:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 7:** Commit: `git commit -am "refactor(ratatui): decompose ui.rs into markdown, timeline, and modals submodules"`.

---

### Task 2: Extract Process Markers & Session Mutation into `src/channels/ratatui/session.rs`
**Files:**
- Create: [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs)
- Modify: [`src/channels/ratatui/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/mod.rs)

- [x] **Step 1:** Extract process marker helpers and session mutation helpers into `src/channels/ratatui/session.rs`.
- [x] **Step 2:** Update `src/channels/ratatui/mod.rs` to register `pub mod session;` and import the helpers.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::ratatui -j 2`.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 5:** Commit: `git commit -am "refactor(ratatui): extract process marker and session routines into session submodule"`.

---

### Task 3: Invariant Verification, Documentation & Release Bump (`v0.0.154`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 2:** Bump version to `0.0.154` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.154` in `CHANGELOG.md` and update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.154 with modular ratatui channel architecture"`.
