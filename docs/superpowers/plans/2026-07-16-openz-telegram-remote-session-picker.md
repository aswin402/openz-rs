# OpenZ Telegram Remote Session Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Make Telegram `/remote` route to a selected running TUI session instead of the global `cli:direct` inbox.

**Architecture:** Each `openz agent` registers an active TUI descriptor under `~/.openz/active_tui/` and refreshes a heartbeat while running. Telegram `/remote` lists live descriptors with inline keyboard buttons, stores the selected `cli:<hash>` per chat, and forwards messages only to that session. Stale descriptors are pruned using heartbeat age and PID liveness.

**Tech Stack:** Rust, Tokio, serde JSON, Telegram Bot API inline keyboards/callback queries, existing OpenZ session/inbox files, fs/process metadata.

## Global Constraints

- Do not use `cli:direct` for Telegram remote routing when a specific TUI session is selected.
- Keep normal Telegram chat isolated as `telegram:<chat_id>`.
- `/local` and `/exit` must clear remote mode.
- Avoid raw `println!` from background channel code while TUI raw mode is active.
- Use low-resource verification only: `CARGO_BUILD_JOBS=1 cargo test --lib ... -- --test-threads=1`.

---

### Task 1: Active TUI Registry

**Files:**
- Modify: `src/agent/activity.rs`
- Modify: `src/cli/agent.rs`

**Interfaces:**
- Produces: `ActiveTuiSession`, `upsert_active_tui_session`, `remove_active_tui_session`, `list_active_tui_sessions`.
- Consumes: existing `SessionManager`, `get_cli_session_key`, and runtime data directory.

- [x] Add `ActiveTuiSession` with session key, PID, cwd, timestamps, model, provider, and preview.
- [x] Add JSON write/remove/list helpers under `~/.openz/active_tui/`.
- [x] Prune entries when PID is dead or heartbeat is older than 30 seconds.
- [x] Add tests for stale pruning and preview extraction.
- [x] In `handle_agent`, register current TUI immediately and spawn a heartbeat loop.
- [x] Remove registry entry after TUI exits.

### Task 2: Telegram Remote Picker

**Files:**
- Modify: `src/channels/telegram.rs`

**Interfaces:**
- Consumes: `list_active_tui_sessions`.
- Produces: selected remote session map `chat_id -> session_key`.

- [x] Replace remote mode boolean with selected session key map.
- [x] `/remote` sends an inline keyboard listing active TUI sessions.
- [x] Callback `remote:<session_key>` stores the selected session.
- [x] Remote messages forward to the selected `cli:<hash>` inbox.
- [x] `/local` and `/exit` clear selected session.
- [x] `/help` describes the new flow.

### Task 3: Verification And Docs

**Files:**
- Modify: `CHANGELOG.md`

**Verification:**
- Run `cargo fmt --check`.
- Run `git diff --check`.
- Run `CARGO_BUILD_JOBS=1 cargo test --lib agent::activity::tests channels::telegram::tests -- --test-threads=1` or targeted equivalents.
- Do not run a full build/test unless explicitly requested.

**Docs:**
- Add changelog bullet for active TUI registry and Telegram remote picker.
