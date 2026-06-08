# OpenZ Architecture 🦀⚡

This document describes the design, execution flow, and module architecture of the Rust rewrite of `openz`.

---

## 1. Architectural Overview

`openz` is a modular, high-performance, asynchronous AI agent and gateway designed in Rust. It utilizes the `tokio` runtime for executing non-blocking I/O, subprocess spawns, and networking concurrently.

```mermaid
graph TD
    CLI[CLI Main / Commands] --> |onboard / agent / gateway| CLI_RUN[cli.rs]
    CLI_RUN --> |config_path| CONF[config/loader.rs]
    CLI_RUN --> |start channel| CHANNELS[channels/ mod.rs]
    
    CHANNELS --> |CLI loop| CLI_CHAN[channels/cli.rs]
    CHANNELS --> |Axum WS Server| WS_CHAN[channels/websocket.rs]
    CHANNELS --> |HTTP Long Polling| TG_CHAN[channels/telegram.rs]

    CLI_CHAN & WS_CHAN & TG_CHAN --> |execute prompt| LOOP[agent/agent_loop.rs]
    
    LOOP --> |load/save history| SESS[session.rs]
    LOOP --> |chat request| PROV[providers/ mod.rs]
    LOOP --> |tool calls| TOOLS[tools/ mod.rs]
    
    PROV --> |OpenAI/DeepSeek/Ollama| openai.rs
    PROV --> |Anthropic Messages| anthropic.rs
    
    TOOLS --> |read/write/list| filesystem.rs
    TOOLS --> |bash execution| shell.rs
    TOOLS --> |fetch url text| web.rs
    TOOLS --> |stdio JSON-RPC| mcp.rs
```

---

## 2. Core Modules

* **`config/`**: Handles loading, updating, and writing configurations to `~/.openz/config.json`.
* **`providers/`**: Implementations for LLM APIs (OpenAI-compatible and Anthropic).
* **`tools/`**: Registry and implementations for native tools, subagent delegation, and MCP stdio wrapper tools.
* **`cron/`**: Handles scheduling and execution of background cron tasks.
* **`session.rs`**: Stores conversation message logs, dynamic summaries, and long-term memory prompts in JSON files under `~/.openz/sessions/`.
* **`agent/agent_loop.rs`**: The core execution state machine (`TurnState`) that manages conversation restoration, context compaction (LLM short-term summarization and long-term memory updates), command extraction, context loading, LLM completions, tool call routing, session saving, and message responses.
* **`channels/`**: Concrete triggers that capture user queries and dispatch replies. Supports Terminal CLI, WebUI WebSocket Server, and Telegram Long-Polling.
