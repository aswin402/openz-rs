---
name: implementation
description: "Technical Implementation Plan — details system architecture, database schema, data flow, API routing, and code-outline analyses."
---

# Technical Implementation Plan

## 1. System Architecture
- **Tech Stack**: Rust 2021, Tokio, Clap, Ratatui/Crossterm-style terminal control, Axum, reqwest/rustls, rusqlite, Tonic, FastEmbed, OpenMedia, OpenDoc, SearchXyz, and MCP-compatible tooling.
- **Entrypoint**: `src/main.rs` initializes environment/runtime and dispatches into `src/cli/mod.rs`.
- **Agent Loop**: `src/agent/agent_loop/` owns turn restoration, compaction, prompt building, tool execution, persistence, response rendering, and background self-improvement.
- **Tool Registry**: `src/cli/tools.rs` registers native tools. `src/tools/mod.rs` handles metadata, prompt-aware tool exposure, dynamic subagent tools, and routing.
- **Channels**: `src/channels/` implements TUI, WebSocket/WebUI, Telegram, Discord, WhatsApp, and Email adapters.

## 2. Data Flow & State Management
- Runtime config lives in `~/.openz/config.json` or `$OPENZ_CONFIG_DIR/config.json`.
- Sessions are stored under `~/.openz/sessions/` with hash-chain verification.
- Memory, knowledge sources, research briefs, workflow cards, skills, and tool performance data are SQLite-backed.
- Tool outputs larger than inline limits are stored under `~/.openz/tool_outputs/` and represented with compressed references.
- OpenZ-launched background servers are registered through the managed child-process registry and controlled by `manage_servers`.

## 3. Structural Analysis
- Use `rg` for fast text search and `ast_grep`/`code_outline` for structural code discovery.
- Use `openz_inventory` for exact current tool/channel/subagent/runtime identity information instead of copying counts into docs manually.
- Use `tool_catalog` when schema, aliases, risk, or prompt-routing metadata must be audited.

## 4. Verification
- Run `cargo fmt --check` for formatting.
- Run `cargo check` for fast compile validation.
- Run `cargo test --lib` for the main regression suite.
- Run `git diff --check` before release commits.
