# OpenZ Ratatui TUI Design Specification

**Date**: 2026-08-03  
**Status**: Approved  
**Author**: Aswin & Antigravity  

---

## 1. Overview & Objective

This specification defines the architecture and design for a new, full-screen **Ratatui-based Terminal User Interface (TUI)** for OpenZ.

The goal is to provide a clean, single-column Codex-style workflow while preserving **OpenZ's exact visual identity, colors, ASCII logo, and status pill layout**.

### Command Routing
- `openz` (default entrypoint with no arguments) $\rightarrow$ Launches the **New Ratatui TUI** (`src/channels/ratatui/`).
- `openz agent` $\rightarrow$ Launches the **Classic Crossterm TUI** (`src/channels/cli/`).

---

## 2. Visual Design & Layout

The Ratatui TUI consists of a vertical layout without noisy outer box panels or multi-column sidebars.

```text
 ┌─────────────────────────────────────────────────────────────────────────────────┐
 │                                                                                 │
 │   ██████╗ ██████╗ ███████╗███╗   ██╗███████╗                                    │
 │  ██╔═══██╗██╔══██╗██╔════╝████╗  ██║╚══███╔╝                                    │
 │  ██║   ██║██████╔╝█████╗  ██╔██╗ ██║  ███╔╝                                     │
 │  ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║ ███╔╝                                      │
 │  ╚██████╔╝██║     ███████╗██║ ╚████║███████╗                                    │
 │   ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝╚══════╝                                    │
 │  openz v0.0.115                                                                 │
 │  opencode_zen | deepseek-v4-flash-free                                          │
 │  ~                                                                              │
 │  ─────────────────────────────────────────────────────────────────────────────  │
 │                                                                                 │
 │  - cargo test scrub_secret_text_redacts_full_and_partial_key_patterns passed    │
 │  - cargo test scrub_secret_file_preserves_json_validity passed                  │
 │                                                                                 │
 │  ⚠ This session was recorded with model `gpt-4o`.                               │
 │                                                                                 │
 │  ─────────────────────────────────────────────────────────────────────────────  │
 │  > Write tests for @filename...                                                 │
 │  ───────────────────────────────────── [ ◇ MCP 0✓ | opencode_zen | gpt-4o | 0/1M ]
 └─────────────────────────────────────────────────────────────────────────────────┘
```

### Components:
1. **Header Banner**:
   - `OPENZ` 3D ASCII block logo (`LIGHT_WHITE` body with `RED_ORANGE` gradient).
   - Version `openz v0.0.115` (`RED_ORANGE`).
   - Provider/Model string `opencode_zen | deepseek-v4-flash-free` (`AURA_SLATE`).
   - Active working directory (`~` or absolute path).
   - Top thin horizontal divider rule (`─────────────────────────────`).

2. **Conversation Scrollback Stream**:
   - Single-column borderless text stream.
   - User prompts displayed with `> ` prefix (`LIGHT_WHITE`).
   - Agent responses rendered with Markdown styling, syntax highlighting, bullet points (`- ` in `EMERALD_GREEN`), and warnings (`⚠` in `AURA_GOLD`).
   - Tool execution status pills (`🛠 [Tool Execution: search_web]`).

3. **Input Prompt Area**:
   - Thin divider line above prompt.
   - Multi-line text wrapping with `> ` for line 0 and `- ` for subsequent lines.
   - Visible hardware terminal cursor (`|`) at active typing index.
   - Auto-complete modal dropdown popup for slash commands (`/clear`, `/model`, `/history`, `/logs`, `/mcps`, etc.).

4. **Bottom Status Line**:
   - Integrated horizontal rule with right-aligned status pill:
     `───────────── [ ◇ MCP 0✓ | provider | model | tokens/limit ]`
   - Exact OpenZ colors: `Aura Purple` for MCP, `Red-Orange` for model & tokens, `Aura Slate` for dividers.

---

## 3. Architecture & Data Flow

### Package Structure:
```text
src/channels/ratatui/
├── mod.rs          # Event loop, raw mode init/cleanup, Tokio event listener
├── app.rs          # RatatuiApp state model (messages, input buffer, cursor, history)
└── ui.rs           # Ratatui widget render functions (header, conversation stream, input, status)
```

### Dependency Updates:
- Add `ratatui = "0.29"` to `Cargo.toml`.

### Command Parsing:
- Update `CliArgs` in `src/cli/args.rs`:
  ```rust
  pub struct CliArgs {
      #[command(subcommand)]
      pub command: Option<Command>,
  }
  ```
- In `src/cli/mod.rs`:
  - `None` $\rightarrow$ calls `channels::handle_ratatui_tui().await`
  - `Some(Command::Agent)` $\rightarrow$ calls `agent::handle_agent().await` (classic TUI)

---

## 4. Safety & Error Handling

- Terminal raw mode clean cleanup via `Drop` guard on `RatatuiGuard`.
- Support for `Ctrl+C` exit, `Esc` cancel, and terminal resize events (`Event::Resize`).
