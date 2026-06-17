# OpenZ Tools Registry & Native Tools 🔧🦀

This document outlines the `Tool` trait, the `ToolRegistry` structure, and the built-in native tools.

---

## 1. The `Tool` Trait

In `src/tools/mod.rs`, every tool must implement the `Tool` trait:

```rust
use anyhow::Result;

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value>;
}
```

---

## 2. Built-in Native Tools

OpenZ packages a comprehensive suite of local tools for file manipulation, system administration, coding support, graphics, browser automation, and networking:

### Filesystem & Database Tools
*   **`read_file`** (`src/tools/filesystem.rs`): Reads the full or partial contents of a file. Supports line ranges (1-indexed).
*   **`write_file`** (`src/tools/filesystem.rs`): Writes text to a file, creating any parent folders automatically.
*   **`list_dir`** (`src/tools/filesystem.rs`): Lists directory entries showing if they are folders and their sizes.
*   **`grep_search`** (`src/tools/grep.rs`): Recursively grep for patterns or regex inside codebase files with optimized binary/ignore filters.
*   **`code_outline`** (`src/tools/outline.rs`): Scans structures (traits, functions, classes, structs) of Rust, Python, Go, and JS/TS files.
*   **`ast_grep`** (`src/tools/ast_grep.rs`): Performs structural code searches across the codebase using AST patterns.
*   **`db_inspector`** (`src/tools/db_inspector.rs`): Inspects SQLite databases, reads schemas, and securely runs SQL queries.
*   **`doc_reader`** (`src/tools/doc_reader.rs`): Reads and extracts text content from PDF, DOCX, and XLSX files.

### Shell & Execution Tools
*   **`exec_command`** (`src/tools/shell.rs`): Runs commands in `/bin/sh` (or `cmd.exe` on Windows) sandboxed using Linux BPF seccomp filters.
*   **`wasm_sandbox`** (`src/tools/wasm_sandbox.rs`): Automatically executes WebAssembly (`.wasm`) files within an in-process, sandboxed `wasmtime` runtime.
*   **`cargo_manager`** (`src/tools/cargo_manager.rs`): Executes cargo toolchain commands (build, test, clippy, fmt) in a workspace.
*   **`js_format`** (`src/tools/js_format.rs`): High-performance JS/TS outlining and formatting utility using the Oxc parser.

### Web & Scraping Tools
*   **`web_fetch`** (`src/tools/web.rs`): Downloads web pages and parses HTML tags into clean markdown text.
*   **`web_search`** (`src/tools/web_search.rs`): Performs web search queries and returns clean lists of titles, URLs, and snippets.
*   **`crawl_website`** (`src/tools/crawl.rs`): Performs asynchronous, multi-threaded website crawls using the high-performance `spider-rs` engine.
*   **`gsd_browser`** (`src/tools/gsd_browser.rs`): Controls a headless Chrome browser to interact with websites and perform browser automation.
*   **`obscura_browser`** (`src/tools/obscura.rs`): Interacts with standard Chrome/Chromium over the WebSocket Chrome DevTools Protocol (CDP) to evaluate JS and navigate.
*   **`firefox`** (`src/tools/firefox.rs`): Alternative browser automation wrapper targeting Firefox environments.

### Visualization & Graphics Tools
*   **`generate_mermaid`** (`src/tools/mermaid.rs`): Renders 23+ diagram types (flowcharts, sequence diagrams, mindmaps, class diagrams) directly to SVG using a pure-Rust parser/renderer (`mermaid-rs-renderer`).
*   **`generate_video`** (`src/tools/video.rs`): Generates simple, clean MP4 videos from programmatic composition timelines specified in JSON (using `wavyte`).
*   **`image_generator`** (`src/tools/image_generator.rs`): Programmatic drawing tool to output custom PNG shapes, lines, and text.

### Subagent & Workflow Tools
For details on subagent execution modes, workspace optimizations, and fallback resolution, see the [Subagents Documentation](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/subagents.md).
*   **`delegate_task`** (`src/tools/subagent.rs`): Spawns a child agent thread with isolated context to execute a specific subtask, and returns a summary.
*   **`optimize_subagent`** (`src/tools/subagent.rs`): Refines a subagent's system prompt using AI based on feedback logs or execution errors.
*   **`create_subagent`** (`src/tools/subagent.rs`): Dynamically creates and saves a new custom specialized subagent profile.
*   **`delete_subagent`** (`src/tools/subagent.rs`): Deletes a custom subagent profile (default subagents are protected).
*   **`trigger_sop`** (`src/tools/sop.rs`): Triggers a stateful closed-loop SOP workflow loop definition (such as 'ship-pr-until-green' or 'pre-commit-guard') dynamically with an optional payload.

### System & Networking Tools
*   **`clipboard`** (`src/tools/clipboard.rs`): Gets or sets text content in the system clipboard.
*   **`open_path`** (`src/tools/open.rs`): Opens a file, folder, or URL using the user's default system application.
*   **`file_watcher`** (`src/tools/watcher.rs`): Starts, stops, or queries a background filesystem watcher to run commands on file modifications.
*   **`network`** (`src/tools/network.rs`): Checks network port availability and connection diagnostics.
*   **`system_info`** (`src/tools/system_info.rs`): Retrieves CPU, memory, OS version, and host environment information.
*   **`send_remote_input`** (`src/tools/remote.rs`): Forwards a prompt or input instruction to another active session (like the TUI terminal prompt) to be executed immediately.
*   **`manage_mcp`** (`src/tools/mcp_manager.rs`): List, add, remove, enable, or disable Model Context Protocol (MCP) server definitions.
*   **`onpkg`** (`src/tools/onpkg.rs`): Scaffold stacks, list available templates, and run package manager commands via `onpkg`.
