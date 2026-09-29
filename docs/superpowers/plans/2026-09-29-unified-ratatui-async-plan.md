# Unified Ratatui Async TUI & Background Engine Implementation Plan

- **Spec**: [`docs/superpowers/specs/2026-09-29-unified-ratatui-async-design.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/specs/2026-09-29-unified-ratatui-async-design.md)
- **Target Release**: OpenZ v0.0.237

---

## Tasks Overview

### Task 1: Unify Command Dispatch & Port Background Supervisors
- [ ] In `src/cli/mod.rs`: Wire `Command::Agent` to call `channels::handle_ratatui_tui().await`.
- [ ] In `src/channels/ratatui/mod.rs`: Port daemon auto-starts from `src/cli/agent.rs`:
  - `start_scheduler(config.clone())`
  - Auto-start WebSocket gateway in background task if configured
  - Auto-start Telegram channel in background task if configured
  - Auto-start Discord channel in background task if configured
  - Auto-start WhatsApp channel in background task if configured
  - Auto-start Email channel in background task if configured
  - 5-second `activity.json` session heartbeat updater
- [ ] Verify clean shutdown on exit.
- [ ] Run `cargo check -p openz -j 1`.

### Task 2: Async Input Queueing & Steering in Ratatui
- [ ] In `src/channels/ratatui/app.rs`: Add `pub queued_prompts: std::collections::VecDeque<String>` to `RatatuiApp`.
- [ ] In `src/channels/ratatui/mod.rs`:
  - On `KeyCode::Enter` when `app.is_thinking == true`:
    - Push `input_str` into `app.queued_prompts`.
    - Clear `app.typed_input` and reset `cursor_idx`.
    - Keep prompt dock responsive and editable.
  - In event loop when turn completes:
    - Automatically pop front from `app.queued_prompts` and trigger the next turn.
  - On `Esc` or `Ctrl+C`:
    - Cancel active turn and restore draft/queued prompt to `app.typed_input`.
- [ ] In `src/channels/ratatui/ui.rs`:
  - Render an orange badge in the status bar if `!app.queued_prompts.is_empty()`: `format!("· [{} queued] ·", app.queued_prompts.len())`.
- [ ] Add unit tests for input queueing and prompt popping in `src/channels/ratatui/app_tests.rs`.
- [ ] Run `cargo test -p openz --lib channels::ratatui -j 1`.

### Task 3: Port Full Slash Command Suite to Ratatui
- [ ] In `src/channels/ratatui/mod.rs` & `commands.rs`:
  - Port commands from `src/channels/cli/commands.rs`:
    - `/device`
    - `/servers`
    - `/stop-server`
    - `/settings`
    - `/memory`
    - `/sources`
    - `/workflows`
    - `/audit`
    - `/logs`
    - `/sop`
  - Update `SLASH_COMMANDS` list in Ratatui so autocomplete popup suggests all commands.
- [ ] Run `cargo test -p openz --lib channels::ratatui -j 1`.

### Task 4: Native Ratatui Security Approval Modal
- [ ] In `src/channels/ratatui/modals.rs`:
  - Add `ModalState::SecurityApproval { tool_name, description, options, selected_idx, tx }`.
  - Add renderer for `SecurityApproval` modal with keyboard navigation (Up, Down, Enter, Esc).
- [ ] In `src/agent/security.rs`:
  - If `IS_RATATUI_ACTIVE`, dispatch approval request to Ratatui modal channel and await user decision.
- [ ] Add unit test for security approval modal in `modals_tests.rs`.
- [ ] Run `cargo test -p openz --lib agent::security -j 1`.

### Task 5: Background Task Engine & Status Indicators
- [ ] In `src/channels/ratatui/ui.rs`:
  - Query active background tasks and render `[⚙ task-id: command]` pill in the bottom status line.
- [ ] Run all tests and clippy:
  - `cargo test -p openz --lib test_native_tool_registration_names -j 1` (260 tools invariant)
  - `cargo test -p openz --lib channels::ratatui -j 1`
  - `cargo clippy -p openz -j 1 -- -D warnings`
- [ ] Synchronous SemVer increment to `v0.0.237` across `Cargo.toml`, `onpkg.json`, `README.md`, and `CHANGELOG.md`.
- [ ] Build & install globally via `./localupdate.sh --balanced`.
- [ ] Verify live interactive execution.
