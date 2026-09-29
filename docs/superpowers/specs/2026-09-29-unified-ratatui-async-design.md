# Unified Ratatui Async TUI & Background Engine Design Specification

- **Date**: 2026-09-29
- **Target Release**: OpenZ v0.0.237
- **Author**: Antigravity & Aswin

---

## 1. Overview & Problem Statement

Currently, OpenZ maintains two separate interactive terminal user interfaces:
1. **Inline Scrollback CLI Channel** (`src/channels/cli/`): Launched via `openz agent`. While it features rich slash commands, background channel supervisors, and streaming markdown tables, its input loop is completely synchronous and blocking. The moment a user submits a prompt, the bottom input box and status bar are erased from the screen, rendering only a thinking spinner (`▶ Thinking ⠓`). During tool calls or LLM execution, the user is completely locked out: they cannot type, cannot ask follow-up questions, cannot queue prompts, and long-running shell commands or subagents block the entire process.
2. **Ratatui TUI Channel** (`src/channels/ratatui/`): Launched via `openz` (with no subcommands). It features a permanent elevated input dock (`Constraint::Length(3)`), a continuous 50ms non-blocking event loop, and floating modals for models and history. However, it currently lacks the full suite of CLI slash commands (`/device`, `/servers`, `/settings`, `/memory`, `/audit`, etc.), background channel daemons (WebSocket, Telegram, Discord, etc.), tool security modals, and active token streaming. Furthermore, when `app.is_thinking` is true, submitting a prompt is blocked with `"⏳ A turn is still running"`.

This specification defines the unification of OpenZ's interactive interface onto the **Ratatui TUI engine**, shifting all CLI features into Ratatui and introducing a non-blocking **Async Input Queue & Background Task System** inspired by `earendil-works/pi` and `can1357/oh-my-pi`.

---

## 2. Key Inspirations & Competitive Analysis

1. **Pi Agent (`earendil-works/pi` / Mario Zechner)**:
   - **Persistent Bottom Input**: The input editor is always active and visible at the bottom of the screen.
   - **Dual Queueing & Steering**:
     - Submitting a prompt while the agent is running queues it.
     - Steering inputs redirect or provide context at the next tool boundary.
     - Aborting via `Escape` cancels the active turn while preserving the draft input in the editor.
2. **Oh My Pi (`can1357/oh-my-pi`)**:
   - **`async-job` Worker Pool**: Long commands and subagents return an immediate `task_id`, freeing the foreground session for continued conversation.
   - **Async Result Injections**: Background job completions arrive as clean notification events without interrupting active foreground inputs.
3. **Hermes Agent (Nous Research)**:
   - **Non-blocking TUI**: Allows queuing messages while the agent is busy or initializing.
   - **Coalesced Notification Turns**: Clean backlog handling for completed background tasks.

---

## 3. Architecture & System Design

```
┌────────────────────────────────────────────────────────────────────────┐
│                        UNIFIED RATATUI TUI                             │
├────────────────────────────────────────────────────────────────────────┤
│ Top Banner:                                                            │
│   OPENZ ASCII Logo (Orange Z) · Model/Provider · CWD                   │
├────────────────────────────────────────────────────────────────────────┤
│ Conversation Timeline:                                                 │
│   User Prompts · Assistant Responses · Thought Blocks · Tool Executions│
│   Streaming Tokens (live delta rendering with box tables)              │
│                                                                        │
├────────────────────────────────────────────────────────────────────────┤
│ Autocomplete Dock (Dynamic / dropdown when typing '/')                 │
├────────────────────────────────────────────────────────────────────────┤
│ Elevated Input Dock (Always Visible, Pinned):                          │
│   › |Ask OpenZ anything or type '/' for slash commands...              │
├────────────────────────────────────────────────────────────────────────┤
│ Status Bar:                                                            │
│   MiniMax-M2.7 · ~ · [⚙ task-1 running] · [1 queued] · mcp:0 · 0/1M    │
└────────────────────────────────────────────────────────────────────────┘
```

### 3.1. Unified Entry Point Dispatch
- Both `openz` (default) and `openz agent` will dispatch to `channels::handle_ratatui_tui().await`.
- The CLI channel is preserved as an internal fallback or headless engine, while the Ratatui TUI becomes the single, primary interactive agent interface.

### 3.2. Background Daemons & Supervisor
When Ratatui starts, it initializes the full OpenZ daemon set:
1. **Cron Scheduler**: Starts `start_scheduler(config.clone())`.
2. **Background Channels**: Auto-starts enabled channels in independent Tokio tasks:
   - WebSocket Gateway (`channels.websocket.enabled && start_on_tui`)
   - Telegram Bot (`channels.telegram.enabled`)
   - Discord Bot (`channels.discord.enabled`)
   - WhatsApp Webhook (`channels.whatsapp.enabled`)
   - Email Channel (`channels.email.enabled`)
3. **Heartbeat Supervisor**: Spawns a 5-second recurring loop updating `~/.openz/activity.json` with active TUI metadata, current model, CWD, and session message previews.

### 3.3. Async Input Queueing & Steering
In `RatatuiApp` (`src/channels/ratatui/app.rs`):
- Add `pub queued_prompts: std::collections::VecDeque<String>`.
- In `KeyCode::Enter` handling:
  - If `app.is_thinking == false`:
    - Push to history, set `app.is_thinking = true`, and spawn the agent turn.
  - If `app.is_thinking == true`:
    - Push `input_str` to `app.queued_prompts`.
    - Clear `app.typed_input` and `cursor_idx` so the user can immediately type the *next* prompt.
    - Set status message: `format!("[{} queued]", app.queued_prompts.len())`.
- In `TurnEvent` completion handling:
  - When the active turn finishes (`SyncSession` or `SingleMessage`), the event loop checks:
    ```rust
    if let Some(next_prompt) = app.queued_prompts.pop_front() {
        // Automatically dispatch next_prompt as a new turn!
    }
    ```
- On `Escape` or `Ctrl+C`:
  - If a turn is running, triggers `crate::shutdown::trigger_cli_cancel()`.
  - Restores the active prompt or last queued prompt back into `app.typed_input` so no user input is ever lost.

### 3.4. Full Slash Command Suite in Ratatui
Port all CLI slash commands into `src/channels/ratatui/mod.rs` and `commands.rs`:
- `/device`: OpenZ device & local tool inventory.
- `/servers`: List OpenZ-launched background servers.
- `/stop-server`: Stop background dev servers by ID or all.
- `/settings`: Display active settings summary.
- `/memory`: Inspect working and durable memory facts.
- `/sources`: Search saved source bookmarks.
- `/workflows`: Search reusable workflows.
- `/audit`: View cryptographic Merkle audit chain verification.
- `/logs`: View recent structured logs.
- `/sop`: Trigger, list, or simulate SOP workflows.

### 3.5. Native Tool Security Approval Modal
In `src/channels/ratatui/modals.rs`:
- Add `ModalState::SecurityApproval`:
  ```rust
  ModalState::SecurityApproval {
      tool_name: String,
      description: String,
      options: Vec<String>,
      selected_idx: usize,
      tx: tokio::sync::oneshot::Sender<bool>,
  }
  ```
- In `src/agent/security.rs`, when `ask_approval` is invoked:
  - If Ratatui is active (`IS_RATATUI_ACTIVE`), it registers a channel with the Ratatui event loop and displays the floating modal dialog.
  - User can press Up/Down and Enter to choose `Approve (Allow once)`, `Approve & Trust for this session`, or `Deny (Abort)`.
  - Eliminates raw terminal stdout collisions and screen corruption.

### 3.6. Live Token Streaming in Timeline
- Update `agent_loop.run()` to emit incremental chunks via `TurnEvent::StreamChunk(String)`.
- Ratatui timeline appends chunks to the active in-progress assistant bubble in real-time, redrawing at 50ms intervals.

### 3.7. Background Tasks Engine
- `ExecCommandTool` and subagent delegation support backgrounding:
  - When backgrounded, returns `{"status": "background", "task_id": "task-xyz", "message": "Command running in background..."}`.
- Ratatui status bar queries active background tasks and renders:
  `MiniMax-M2.7 · ~ · [⚙ task-xyz: cargo test] · mcp:0 active · 0/1M`.

---

## 4. Verification & Constraints
- Exact **260 registered native tools** invariant must be maintained.
- Capped resource consumption: strictly `-j 1` for tests/checks, `-j 2` for builds.
- 0 clippy warnings (`-D warnings`).
- Synchronous SemVer bump to `v0.0.237` across `Cargo.toml`, `onpkg.json`, `README.md`, and `CHANGELOG.md`.
