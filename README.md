# OpenZ 🦊 `v0.0.45`

<p align="center">
  <img src="assets/logo.png" width="200" alt="OpenZ Logo">
</p>

OpenZ is a high-performance, asynchronous, ultra-lightweight personal AI agent framework built entirely in Rust. 

**Official GitHub Repository:** [github.com/aswin402/openz-rs](https://github.com/aswin402/openz-rs)

Rebranded and migrated from `nanobot`, it maintains a clean, object-safe agent loop while packaging essential developer utilities: native console chat, WebSocket WebUI gateways, Telegram/Discord/WhatsApp channels, local tool calls, stdio-based MCP servers, and OpenAI/Anthropic/Azure LLM client routing.

*Vibe coded by **Aswin**.*  
*Inspired by **Zeroclaw**, **Nanobot**, **hermes-agent**, **loops!**, and **DOX**.*

---

## 🚀 Key Features

* **Hierarchical Context Scoping (DOX-inspired):** Built-in folder-level context management. Integrates with the `headroom` MCP server (`scope_context`) to walk up the directory tree, compile relevant `AGENTS.md` instructions, and supply localized target rules to the agent before making edits. Ensures zero context drift.
* **Stateful SOP Workflow Engine (loops!-inspired):** Resilient, multi-step Directed Acyclic Graph (DAG) templates executing independent steps in parallel via Tokio. Contains pre-configured, default stateful closed-loop SOPs such as `ship-pr-until-green` (feature implementation, PR creation, CI verification loop, and self-healing) and `pre-commit-guard` (pre-commit testing hooks configuration and verification).
* **Zenflow Checkpointed Transactions:** Automatically takes a directory/git snapshot before executing file edits, runs tests/compilations, attempts to self-heal errors, and automatically rolls back to the clean snapshot if compilation/healing fails.
* **Semantic Repository Indexing:** Indexes structural code elements (structs, functions, classes) using `ast_grep` and fast vector embeddings to let agents semantically lookup dependencies instantly.
* **Sandboxed Data Execution:** Provides a secure local Python/WASM data sandbox allowing research agents to run data analysis scripts and output visual charts.
* **Cryptographic Merkle Hash-Chain Audit Ledger:** Every message, state transition, and tool call is hashed and linked via SHA-256 to form an immutable, tamper-evident ledger. The hash chain integrity is verified automatically on session startup, and the `/audit` slash command outputs a formatted ledger.
* **SQLite-Backed Memory & Skill Layer:** Migrated long-term skills and facts storage from slow Markdown/JSON flat files into a dedicated SQLite database (`~/.openz/memory.db`) with auto-migration on startup.
* **Security Guard Interceptor (BPF sandbox):** Safeguards the host environment using a Linux BPF seccomp filter on subprocesses to intercept destructive commands (`rm`, `dd`, etc.), privilege escalation (`sudo`), process controls (`kill`), system actions (`reboot`), network transfers (`curl`, `wget`, `scp`), and out-of-workspace writes. Supports `strict`, `normal`, and `loose` modes.
* **New Specialized subagents:**
  * **`mermaid_designer`:** Specializes in parsing code structures and rendering elegant systems flowcharts or diagrams natively.
  * **`video_editor`:** Orchestrates and compiles video timeline compositions into final neat MP4 files.
* **Auto-Continuation (Truncation Prevention):** Detects when model responses are cut off due to hitting output token limits (using `finish_reason: "length"`) and automatically prompts the model to continue, stitching the segments together seamlessly.
* **Memory & Skill Self-Improvement:** Asynchronous background curator reviews chat transcripts, updates memory facts, and curates skills. Accepts GitHub repository URLs to dynamically clone, install, compile, and register them as active tools.
* **Pluggable Channel Adapters:** Conforming to a unified `Channel` trait, offering:
  * **Console CLI (`agent`):** Direct interactive terminal chat with full slash commands support.
  * **WebSocket Gateway (`gateway`):** WebUI workbench connection. Includes an OpenAI-compatible local completions endpoint (`/v1/chat/completions`) with dynamic LLM routing.
  * **Email Client (`email`):** 100% pure Rust IMAP polling and SMTP dispatch client parsing nested MIME envelopes with `mailparse` and sending replies concurrently.
  * **Telegram Polling (`telegram`):** Bot listener with parallel loop handling.
  * **Discord Gateway (`discord`):** Gateway client listening for events via WebSocket.
  * **WhatsApp API (`whatsapp`):** Axum webhook receiver server verifying and processing incoming messages.
* **Changelog & System Specifications:** The `openz changelog` command prints system hardware specifications (ROM/RAM footprint, CPU load, boot time), architectural inspirations, key capabilities, model protocol integrations, and release history directly to the terminal.
* **Runtime DB Doctor:** The `openz doctor` command verifies that all runtime databases (`memory.db`, `graph_memory.db`, etc.) live under `~/.openz` and automatically relocates any stray artifacts found in the working directory (data is preserved, never deleted).

---

## 🛠️ Core Tools Registry

OpenZ exposes a powerful set of local tools to the LLM:
* **Filesystem & Code Analysis:** `read_file`, `write_file`, `patch_file`, `find_files`, `replace_lines`, `zenflow_edit`, `list_dir`, `grep_search`, `code_outline`, `ast_grep`, `index_codebase`, `git_manager`, `db_inspector`, `db_write`, `doc_reader`, `rust_docs`, `compile_template`.
* **System & Environment:** `exec_command` (sandboxed), `python_sandbox`, `clipboard`, `open_path`, `system_info`, `file_watcher`, `check_port` (localhost-only).
* **Web, Search & Social:** `web_search` (SearchXyz Dispatcher / Tavily / Exa), `social_search` (HN/Reddit), `crawl_website` (spider-rs), `gsd_browser` (Playwright), `obscura_browser` / `firefox_browser` (CDP), `semantic_search` (vector embeddings).
* **Integrated SearchXyz Tools:** `searchxyz_search_web`, `searchxyz_read_url`, `searchxyz_search_and_read`, `searchxyz_recall`, `searchxyz_list_sources`, `searchxyz_deep_research`, `searchxyz_index_content`, `searchxyz_site_map`, `searchxyz_index_relationship`, `searchxyz_query_graph`, `searchxyz_read_github_repo`, `searchxyz_export_research`, `searchxyz_import_research`, `searchxyz_delete_source`, `searchxyz_clear_index`.
* **Automation & Cron:** `schedule_job`, `list_jobs`, `remove_job`, `compiler_auto_heal`.
* **Memory & Knowledge:** `store_memory`, `recall_memory`, `clear_memory`, `archive_research`, `search_research`, `index_notes`.
* **Subagents & Orchestration:** `delegate_task`, `parallel_research`, `evaluator_optimizer_loop`, `optimize_subagent`, `create_subagent`, `delete_subagent`, `trigger_sop`.
* **Visuals & Graphics:** `generate_image` (HTML/CSS→PNG), `html_to_video` (HTML→MP4), `render_mermaid` (diagram→SVG), `generate_video` (JSON→MP4), `create_animated_svg`.
* **MCP Integration:** `manage_mcp` (CRUD configs). All MCP servers use a **unified gRPC Tonic transport** + an in-process TCP port bridge with robust non-JSON noise filtering.

---

## 📊 Performance Benchmarks (OpenZ vs Nanobot)

| Benchmark Category | Original Python `nanobot` | New Rust `openz` | Improvement Factor |
| :--- | :--- | :--- | :--- |
| **ROM (Disk Space)** | **150 MB - 250 MB** | **~10 MB - 15 MB** | **~15x smaller footprint** |
| **Idle RAM** | **60 MB - 80 MB** | **~4 MB - 6 MB** | **~12x less memory used** |
| **Active Loop RAM** | **120 MB - 180 MB** | **~12 MB - 20 MB** | **~8x less peak memory** |
| **Startup Time** | **500 ms - 1500 ms** | **< 5 ms** | **100x - 300x faster boot** |
| **CPU Overhead** | Higher (GC & GIL locks) | Negligible (Tokio work pool) | **Significantly more efficient** |

---

## ⚙️ Quick Start

### 1. Compile and Install Globally
```bash
./localinstall.sh
```

### 2. Configure LLM Providers & Channels
```bash
openz configure
```
*Settings are saved to `~/.openz/config.json`.*

### 3. Run Agent Chat (Terminal)
```bash
openz agent
```
*Use `/help`, `/history`, `/clear`, `/status`, `/memory`, `/skills`, `/skill`, `/sop` slash commands.*

### 4. Start WebSocket Gateway
```bash
openz gateway
```

### 5. View System Specifications & Changelog
```bash
openz changelog
```
*View system hardware footprint specifications, design inspirations, capabilities, tools, and version release history.*

### 6. View Live Structured Logs
```bash
openz logs
```
*Tails and streams real-time logs from `~/.openz/openz.log` with support for tail length limits and session filtering.*

---

## 📚 Documentation Directory

* **System Architecture:** [architecture.md](docs/architecture.md)
* **Security Guard & Permissions:** [security.md](docs/security.md)
* **Pluggable Channels & Configuration:** [channels.md](docs/channels.md)
* **Self-Improvement & Memory Guide:** [self_improvement.md](docs/self_improvement.md)
* **Model Context Protocol (MCP):** [mcps.md](docs/mcps.md)
* **Tools Registry Details:** [tools.md](docs/tools.md)
* **ZeroClaw Gap Analysis & Roadmap:** [zeroclaw_research.md](docs/zeroclaw_research.md)
