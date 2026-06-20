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

### Filesystem & Repository Tools
*   **`read_file`** (`src/tools/filesystem.rs`): Reads the full or partial contents of a file. Supports line ranges (1-indexed).
*   **`write_file`** (`src/tools/filesystem.rs`): Writes text to a file, creating any parent folders automatically.
*   **`patch_file`** (`src/tools/filesystem.rs`): Applies a targeted find-and-replace edit on a file.
*   **`find_files`** (`src/tools/filesystem.rs`): Searches for files matching glob patterns with size and time filtering.
*   **`replace_lines`** (`src/tools/filesystem.rs`): Replaces exact line sequences within a file (surgical line-level edits).
*   **`zenflow_edit`** (`src/tools/filesystem.rs`): Multi-file structural editing with smart context matching. **Requires a git repository** for change tracking.
*   **`list_dir`** (`src/tools/filesystem.rs`): Lists directory entries showing if they are folders and their sizes.
*   **`grep_search`** (`src/tools/grep.rs`): Recursively grep for patterns or regex inside codebase files with optimized binary/ignore filters.
*   **`code_outline`** (`src/tools/outline.rs`): Scans structures (traits, functions, classes, structs) of Rust, Python, Go, and JS/TS files.
*   **`ast_grep`** (`src/tools/ast_grep.rs`): Performs structural code searches across the codebase using AST patterns.
*   **`index_codebase`** (`src/tools/ast_grep.rs`): Indexes codebase structure into a structured JSON summary.
*   **`git_manager`** (`src/tools/git_manager.rs`): Executes git operations (status, diff, log, commits).
*   **`db_inspector`** (`src/tools/db_inspector.rs`): Inspects SQLite databases, reads schemas, and securely runs SQL queries.
*   **`db_write`** (`src/tools/db_inspector.rs`): Writes data to SQLite databases.
*   **`doc_reader`** (`src/tools/doc_reader.rs`): Reads and extracts text content from PDF, DOCX, and XLSX files.
*   **`rust_docs`** (`src/tools/rust_docs.rs`): Queries Rust documentation from docs.rs for crate API references.
*   **`compile_template`** (`src/tools/template_compiler.rs`): Compiles Handlebars/Mustache templates with provided context data.

### Shell & Execution Tools
*   **`exec_command`** (`src/tools/shell.rs`): Runs commands in `/bin/sh` (or `cmd.exe` on Windows) sandboxed using Linux BPF seccomp filters.
*   **`python_sandbox`** (`src/tools/shell.rs`): Executes Python scripts in an isolated subprocess with resource limits.
*   **`wasm_execute`** (`src/tools/wasm_sandbox.rs`): Automatically executes WebAssembly (`.wasm`) files within an in-process, sandboxed `wasmtime` runtime.
*   **`cargo_manager`** (`src/tools/cargo_manager.rs`): Executes cargo toolchain commands (build, test, clippy, fmt) in a workspace.
*   **`js_format`** (`src/tools/js_format.rs`): High-performance JS/TS outlining and formatting utility using the Oxc parser.
*   **`compiler_auto_heal`** (`src/tools/compiler_auto_heal.rs`): Automatically diagnoses and fixes compilation errors.

### Web & Scraping Tools
*   **`web_fetch`** (`src/tools/web.rs`): Downloads web pages and parses HTML tags into clean markdown text.
*   **`web_search`** (`src/tools/web_search.rs`): Performs web search queries and returns clean lists of titles, URLs, and snippets.
*   **`social_search`** (`src/tools/social_search.rs`): Searches Hacker News, Reddit, and other social platforms for content.
*   **`crawl_website`** (`src/tools/crawl.rs`): Performs asynchronous, multi-threaded website crawls using the high-performance `spider-rs` engine.
*   **`gsd_browser`** (`src/tools/gsd_browser.rs`): Controls a headless Chrome browser to interact with websites and perform browser automation.
*   **`obscura_browser`** (`src/tools/obscura.rs`): Interacts with standard Chrome/Chromium over the WebSocket Chrome DevTools Protocol (CDP) to evaluate JS and navigate.
*   **`firefox_browser`** (`src/tools/firefox.rs`): Alternative browser automation wrapper targeting Firefox environments.
*   **`semantic_search`** (`src/tools/semantic_search.rs`): Performs vector-based semantic search across a codebase using embeddings.

### Visualization & Graphics Tools
*   **`render_mermaid`** (`src/tools/mermaid.rs`): Renders 23+ diagram types (flowcharts, sequence diagrams, mindmaps, class diagrams) directly to SVG using a pure-Rust parser/renderer (`mermaid-rs-renderer`).
*   **`generate_video`** (`src/tools/video.rs`): Generates simple, clean MP4 videos from programmatic composition timelines specified in JSON (using `wavyte`).
*   **`generate_image`** (`src/tools/image_generator.rs`): Generates premium, high-fidelity PNG images from HTML/CSS, local files, or online URLs using headless Chromium. Supports custom CSS injection, selector cropping, and Retina resolution scale configurations.
*   **`html_to_video`** (`src/tools/html_video.rs`): Renders high-fidelity timeline-based MP4 videos from HTML/CSS/JS templates frame-by-frame using headless Chrome and FFmpeg (similar to Remotion).
*   **`create_animated_svg`** (`src/tools/svg_animator.rs`): Creates animated SVG files from motion descriptions (paths, morphing, draw-on effects).

### Subagent & Workflow Tools
For details on subagent execution modes, workspace optimizations, and fallback resolution, see the [Subagents Documentation](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/subagents.md).
*   **`delegate_task`** (`src/tools/subagent.rs`): Spawns a child agent thread with isolated context to execute a specific subtask, and returns a summary.
*   **`parallel_research`** (`src/tools/subagent.rs`): Runs multiple research subtasks in parallel across subagents and merges results.
*   **`evaluator_optimizer_loop`** (`src/tools/subagent.rs`): Iteratively generates and evaluates responses until quality criteria are met.
*   **`optimize_subagent`** (`src/tools/subagent.rs`): Refines a subagent's system prompt using AI based on feedback logs or execution errors.
*   **`create_subagent`** (`src/tools/subagent.rs`): Dynamically creates and saves a new custom specialized subagent profile.
*   **`delete_subagent`** (`src/tools/subagent.rs`): Deletes a custom subagent profile (default subagents are protected).
*   **`trigger_sop`** (`src/tools/sop.rs`): Triggers a stateful closed-loop SOP workflow loop definition (such as 'ship-pr-until-green' or 'pre-commit-guard') dynamically with an optional payload.

### Memory & Knowledge Tools
*   **`store_memory`** (`src/tools/shared_memory.rs`): Stores structured observations, decisions, or facts in the agent's long-term memory.
*   **`recall_memory`** (`src/tools/shared_memory.rs`): Retrieves stored memories by query context.
*   **`clear_memory`** (`src/tools/shared_memory.rs`): Clears all entries from the agent's memory store.
*   **`archive_research`** (`src/tools/shared_memory.rs`): Archives research findings into persistent storage.
*   **`search_research`** (`src/tools/shared_memory.rs`): Searches archived research content.
*   **`index_notes`** (`src/tools/notes.rs`): Indexes and searches local markdown notes.

### System & Networking Tools
*   **`clipboard`** (`src/tools/clipboard.rs`): Gets or sets text content in the system clipboard.
*   **`open_path`** (`src/tools/open.rs`): Opens a file, folder, or URL using the user's default system application.
*   **`file_watcher`** (`src/tools/watcher.rs`): Starts, stops, or queries a background filesystem watcher to run commands on file modifications.
*   **`check_port`** (`src/tools/network.rs`): Checks port availability. **Restricted to localhost only** for security (SSRF prevention).
*   **`system_info`** (`src/tools/system_info.rs`): Retrieves CPU, memory, OS version, and host environment information.
*   **`send_remote_input`** (`src/tools/remote.rs`): Forwards a prompt or input instruction to another active session (like the TUI terminal prompt) to be executed immediately.
*   **`manage_mcp`** (`src/tools/mcp_manager.rs`): List, add, remove, enable, or disable Model Context Protocol (MCP) server definitions.
*   **`onpkg`** (`src/tools/onpkg.rs`): Scaffold stacks, list available templates, and run package manager commands via `onpkg`.
