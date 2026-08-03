# Ratatui OpenZ TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a sleek, single-column Ratatui TUI for `openz` (default entrypoint) matching OpenZ's exact ASCII logo, status pill, and color aesthetic, while preserving `openz agent` as the legacy Crossterm TUI.

**Architecture:** Add `ratatui` crate dependency to `Cargo.toml`. Make `CliArgs.command` an `Option<Command>` in `src/cli/args.rs` so running `openz` (no subcommand) routes to `channels::handle_ratatui_tui()`. Implement `src/channels/ratatui/` containing `mod.rs`, `app.rs`, and `ui.rs` to render the ASCII header banner, scrollable conversation stream, dark-highlighted multi-line input box, and bottom right-aligned status pill `[ ◇ MCP | provider | model | tokens ]`.

**Tech Stack:** Rust (2021 edition), Tokio, Ratatui 0.29, Crossterm 0.29.

## Global Constraints

- Cargo package version: `0.0.115`
- Default command `openz` launches Ratatui TUI; `openz agent` launches classic Crossterm CLI TUI.
- Do NOT run full `cargo check`, `cargo build`, or `cargo test` across the entire workspace (only targeted single-unit tests if required, to avoid laptop CPU overload).
- User handles full `cargo build` on their own system.

---

### Task 1: Add `ratatui` Dependency and Update CLI Command Routing

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/mod.rs`

**Interfaces:**
- Consumes: `clap::Parser`
- Produces: `Option<Command>` in `CliArgs`, `channels::handle_ratatui_tui()` routing branch when `command` is `None`.

- [ ] **Step 1: Add `ratatui` dependency to `Cargo.toml`**

Add `ratatui = "0.29"` under `[dependencies]` in `Cargo.toml`:

```toml
ratatui = "0.29"
```

- [ ] **Step 2: Update `CliArgs` in `src/cli/args.rs`**

Update `CliArgs` in `src/cli/args.rs` to make `pub command: Option<Command>`:

```rust
#[derive(Parser)]
#[command(name = "openz", version = env!("CARGO_PKG_VERSION"), about = "OpenZ - Rebranded Ultra-Lightweight Personal AI Agent")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Command>,
}
```

- [ ] **Step 3: Update `run_cli` routing in `src/cli/mod.rs`**

Match `args.command`:

```rust
    match args.command {
        None => {
            channels::handle_ratatui_tui().await?;
        }
        Some(Command::Onboard) => {
            onboard::handle_onboard().await?;
        }
        Some(Command::Configure) => {
            configure::handle_configure().await?;
            let _ = crossterm::terminal::disable_raw_mode();
            std::process::exit(0);
        }
        Some(Command::Agent) => {
            agent::handle_agent().await?;
        }
        // ... (remaining subcommands) ...
    }
```

- [ ] **Step 4: Commit Task 1**

```bash
git add Cargo.toml src/cli/args.rs src/cli/mod.rs
git commit -m "feat: add ratatui dependency and route default openz command to ratatui TUI"
```

---

### Task 2: Create Ratatui App State & Terminal Safety Guard

**Files:**
- Create: `src/channels/ratatui/mod.rs`
- Create: `src/channels/ratatui/app.rs`
- Modify: `src/channels/mod.rs`

**Interfaces:**
- Consumes: `crate::session::SessionManager`, `crate::config::schema::Config`
- Produces: `RatatuiApp` state struct and `handle_ratatui_tui()` async entrypoint.

- [ ] **Step 1: Create `src/channels/ratatui/app.rs`**

Define `RatatuiApp` to hold conversation messages, typed input buffer, cursor position, scroll offset, model name, provider, token counts, and MCP statistics:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

pub static IS_RATATUI_ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub is_tool: bool,
}

pub struct RatatuiApp {
    pub model: String,
    pub provider: String,
    pub session_key: String,
    pub typed_input: Vec<char>,
    pub cursor_idx: usize,
    pub messages: Vec<ChatMessage>,
    pub scroll_offset: usize,
    pub selected_index: Option<usize>,
    pub approx_tokens: usize,
    pub limit_tokens: usize,
    pub cwd_display: String,
    pub should_exit: bool,
}

impl RatatuiApp {
    pub fn new(model: String, provider: String, session_key: String) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "~".to_string());
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        let cwd_display = if let Some(ref h) = home {
            if cwd.starts_with(h) {
                cwd.replacen(h, "~", 1)
            } else {
                cwd
            }
        } else {
            cwd
        };

        Self {
            model,
            provider,
            session_key,
            typed_input: Vec::new(),
            cursor_idx: 0,
            messages: Vec::new(),
            scroll_offset: 0,
            selected_index: None,
            approx_tokens: 0,
            limit_tokens: 1_000_000,
            cwd_display,
            should_exit: false,
        }
    }
}
```

- [ ] **Step 2: Create `src/channels/ratatui/mod.rs` scaffolding & Guard**

```rust
pub mod app;
pub mod ui;

use anyhow::Result;
use app::{RatatuiApp, IS_RATATUI_ACTIVE};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::atomic::Ordering;

struct RatatuiGuard;

impl Drop for RatatuiGuard {
    fn drop(&mut self) {
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = stdout().execute(crossterm::cursor::Show);
        IS_RATATUI_ACTIVE.store(false, Ordering::SeqCst);
    }
}
```

- [ ] **Step 3: Export `ratatui` module in `src/channels/mod.rs`**

Add `pub mod ratatui;` and export `pub use ratatui::handle_ratatui_tui;` in `src/channels/mod.rs`.

- [ ] **Step 4: Commit Task 2**

```bash
git add src/channels/ratatui/app.rs src/channels/ratatui/mod.rs src/channels/mod.rs
git commit -m "feat: implement Ratatui app state model and terminal safety guard"
```

---

### Task 3: Implement OpenZ-Styled Ratatui UI Renderer (`src/channels/ratatui/ui.rs`)

**Files:**
- Create: `src/channels/ratatui/ui.rs`

**Interfaces:**
- Consumes: `RatatuiApp`, Ratatui `Frame`
- Produces: `render_ratatui_ui(f: &mut Frame, app: &mut RatatuiApp)` rendering OPENZ ASCII header banner, scrollback stream, dark-highlighted multi-line input box, and bottom right-aligned status pill.

- [ ] **Step 1: Create `src/channels/ratatui/ui.rs`**

```rust
use super::app::RatatuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_ratatui_ui(f: &mut Frame, app: &RatatuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),      // Conversation scrollback + Header ASCII logo
            Constraint::Length(3),   // Highlighted Input Box
            Constraint::Length(1),   // Bottom Status Line with Pill
        ])
        .split(f.area());

    // 1. Render Scrollback Conversation Area with Header Banner
    let mut text_lines = Vec::new();

    // OPENZ ASCII Header Logo
    text_lines.push(Line::from(vec![Span::styled(
        "  ██████╗ ██████╗ ███████╗███╗   ██╗███████╗",
        Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ██╔═══██╗██╔══██╗██╔════╝████╗  ██║╚══███╔╝",
        Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ██║   ██║██████╔╝█████╗  ██╔██╗ ██║  ███╔╝ ",
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ╚██████╔╝██║     ███████╗██║ ╚████║███████╗",
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));

    text_lines.push(Line::from(vec![Span::styled(
        format!(" openz v{}", env!("CARGO_PKG_VERSION")),
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        format!(" {} | {}", app.provider, app.model),
        Style::default().fg(Color::Rgb(98, 114, 164)),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        format!(" {}", app.cwd_display),
        Style::default().fg(Color::Rgb(98, 114, 164)),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ─────────────────────────────────────────────────────────────────────────────",
        Style::default().fg(Color::Rgb(62, 72, 104)),
    )]));

    for msg in &app.messages {
        let prefix = if msg.role == "user" { "> " } else { "  " };
        let style = if msg.role == "user" {
            Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD)
        } else if msg.is_tool {
            Style::default().fg(Color::Rgb(189, 147, 249))
        } else {
            Style::default().fg(Color::Rgb(248, 248, 242))
        };
        text_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Rgb(255, 85, 51))),
            Span::styled(&msg.content, style),
        ]));
    }

    let conversation = Paragraph::new(Text::from(text_lines))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset as u16, 0));
    f.render_widget(conversation, chunks[0]);

    // 2. Render Dark-Highlighted Input Area
    let typed_str: String = app.typed_input.iter().collect();
    let input_block = Block::default()
        .style(Style::default().bg(Color::Rgb(25, 25, 35)));
    let input_p = Paragraph::new(format!("> {}", typed_str))
        .block(input_block)
        .style(Style::default().fg(Color::Rgb(255, 255, 255)));
    f.render_widget(input_p, chunks[1]);

    // Place cursor inside input box
    let cursor_x = chunks[1].x + 2 + (app.cursor_idx as u16);
    let cursor_y = chunks[1].y + 1;
    f.set_cursor_position((cursor_x, cursor_y));

    // 3. Render Status Line with Embedded Pill
    let (mcp_loaded, _, _) = crate::tools::mcp::get_mcp_stats();
    let pill_text = format!(
        "[ ◇ MCP {}✓ | {} | {} | {}/{} ]",
        mcp_loaded, app.provider, app.model, app.approx_tokens, "1M"
    );
    let rule_len = (chunks[2].width as usize).saturating_sub(pill_text.len() + 2);
    let rule_str: String = std::iter::repeat_n('─', rule_len).collect();

    let status_line = Line::from(vec![
        Span::styled(rule_str, Style::default().fg(Color::Rgb(62, 72, 104))),
        Span::styled(" ", Style::default()),
        Span::styled(format!("[ ◇ MCP {}✓ ", mcp_loaded), Style::default().fg(Color::Rgb(189, 147, 249))),
        Span::styled(format!("| {} | {} ", app.provider, app.model), Style::default().fg(Color::Rgb(255, 85, 51))),
        Span::styled(format!("| {} ]", app.approx_tokens), Style::default().fg(Color::Rgb(255, 85, 51))),
    ]);

    let status_p = Paragraph::new(status_line);
    f.render_widget(status_p, chunks[2]);
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src/channels/ratatui/ui.rs
git commit -m "feat: implement OpenZ-styled Ratatui UI renderer with ASCII logo and status pill"
```

---

### Task 4: Connect Ratatui TUI Event Loop & `AgentLoop`

**Files:**
- Modify: `src/channels/ratatui/mod.rs`

**Interfaces:**
- Consumes: `RatatuiApp`, `render_ratatui_ui`, `crate::agent::agent_loop::AgentLoop`
- Produces: `handle_ratatui_tui()` full interactive async execution loop.

- [ ] **Step 1: Implement `handle_ratatui_tui()` in `src/channels/ratatui/mod.rs`**

```rust
pub async fn handle_ratatui_tui() -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    IS_RATATUI_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RatatuiGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let config = crate::config::loader::load_config().unwrap_or_default();
    let model = config.agents.defaults.model.clone();
    let provider = config.agents.defaults.provider.clone();
    let session_key = "default".to_string();

    let mut app = app::RatatuiApp::new(model, provider, session_key);

    loop {
        terminal.draw(|f| ui::render_ratatui_ui(f, &app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == crossterm::event::KeyEventKind::Press {
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        break;
                    }
                    match key.code {
                        KeyCode::Char(c) => {
                            app.typed_input.insert(app.cursor_idx, c);
                            app.cursor_idx += 1;
                        }
                        KeyCode::Backspace => {
                            if app.cursor_idx > 0 {
                                app.typed_input.remove(app.cursor_idx - 1);
                                app.cursor_idx -= 1;
                            }
                        }
                        KeyCode::Enter => {
                            let input_str: String = app.typed_input.iter().collect();
                            if !input_str.trim().is_empty() {
                                if input_str.trim() == "/exit" {
                                    break;
                                }
                                app.messages.push(app::ChatMessage {
                                    role: "user".to_string(),
                                    content: input_str,
                                    is_tool: false,
                                });
                                app.typed_input.clear();
                                app.cursor_idx = 0;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_exit {
            break;
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Commit Task 4**

```bash
git add src/channels/ratatui/mod.rs
git commit -m "feat: connect Ratatui event loop and interactive key handling"
```
