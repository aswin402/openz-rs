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
*   **`store_memory`** (`src/tools/shared_memory/cognitive.rs`): Stores structured observations, decisions, or facts in the agent's long-term memory.
*   **`recall_memory`** (`src/tools/shared_memory/cognitive.rs`): Retrieves stored memories by query context.
*   **`clear_memory`** (`src/tools/shared_memory/cognitive.rs`): Clears all entries from the agent's memory store.
*   **`delete_memory`** (`src/tools/shared_memory/cognitive.rs`): Deletes specific memory entries.
*   **`update_memory`** (`src/tools/shared_memory/cognitive.rs`): Updates an existing memory entry.
*   **`archive_research`** (`src/tools/shared_memory/research.rs`): Archives research findings into persistent storage.
*   **`search_research`** (`src/tools/shared_memory/research.rs`): Searches archived research content.
*   **`index_notes`** (`src/tools/notes.rs`): Indexes and searches local markdown notes.

### Integrated SearchXyz Tools
*   **`searchxyz_search_web`** (`src/tools/searchxyz/web.rs`): Federated web search dispatcher querying DuckDuckGo, Google, Bing, Brave, and SearXng.
*   **`searchxyz_read_url`** (`src/tools/searchxyz/web.rs`): Fetches and parses URLs, PDFs, YouTube transcripts, or Git repositories into clean Markdown.
*   **`searchxyz_search_and_read`** (`src/tools/searchxyz/web.rs`): Performs a web search and crawls the top results in a single call.
*   **`searchxyz_recall`** (`src/tools/searchxyz/index.rs`): Searches the local index semantically or via keyword query.
*   **`searchxyz_list_sources`** (`src/tools/searchxyz/index.rs`): Lists all cached and indexed document sources.
*   **`searchxyz_deep_research`** (`src/tools/searchxyz/web.rs`): Iterative multi-query crawler compiling a research markdown report.
*   **`searchxyz_index_content`** (`src/tools/searchxyz/index.rs`): Indexes custom text content manually into the index.
*   **`searchxyz_site_map`** (`src/tools/searchxyz/web.rs`): Spiders domain pages to crawl sitemaps or link trees.
*   **`searchxyz_index_relationship`** (`src/tools/searchxyz/graph.rs`): Records entity relationship facts into the Knowledge Graph.
*   **`searchxyz_query_graph`** (`src/tools/searchxyz/graph.rs`): Traverses entity nodes inside the local Knowledge Graph.
*   **`searchxyz_read_github_repo`** (`src/tools/searchxyz/graph.rs`): Clones and recursively indexes repository source code.
*   **`searchxyz_export_research`** (`src/tools/searchxyz/index.rs`): Exports local indexed documents into a JSON bundle.
*   **`searchxyz_import_research`** (`src/tools/searchxyz/index.rs`): Imports external JSON document bundles into the index.
*   **`searchxyz_delete_source`** (`src/tools/searchxyz/index.rs`): Deletes documents and relationships matching a URL prefix.
*   **`searchxyz_clear_index`** (`src/tools/searchxyz/index.rs`): Wipes all indexed document text and Graph databases.

### Integrated OpenMedia Tools
*   **`openmedia_ping`** (`src/tools/openmedia/mod.rs`): Pings the media generation server to check status and health.
*   **`openmedia_model_download`** (`src/tools/openmedia/mod.rs`): Downloads a specified model file (CLIP text/vision or Aesthetic predictor) from Hugging Face Hub with progress tracking.
*   **`openmedia_rasterize_svg`** (`src/tools/openmedia/mod.rs`): Rasterizes an SVG string or file path into a PNG, JPEG, or WebP image.
*   **`openmedia_diagram_generate_mermaid`** (`src/tools/openmedia/mod.rs`): Compiles a Mermaid diagram string into an SVG, PNG, JPEG, or WebP diagram.
*   **`openmedia_html_to_image`** (`src/tools/openmedia/mod.rs`): Renders HTML and CSS templates/files into an image (PNG, JPEG, or WebP).
*   **`openmedia_create_svg`** (`src/tools/openmedia/mod.rs`): Generates custom SVG layouts from a list of shapes.
*   **`openmedia_create_chart`** (`src/tools/openmedia/mod.rs`): Generates vertical bars, lines, area, scatter, radar, and pie charts from raw data.
*   **`openmedia_create_icon`** (`src/tools/openmedia/mod.rs`): Retrieves styled vector icons from the embedded Lucide library.
*   **`openmedia_animate_svg`** (`src/tools/openmedia/mod.rs`): Applies keyframes/SMIL animation presets (fade_in, spin, bounce, etc.) to SVG elements.
*   **`openmedia_animate_create_timeline`** (`src/tools/openmedia/mod.rs`): Coordinately sequences animations of multiple elements over a timeline.
*   **`openmedia_animate_morph_paths`** (`src/tools/openmedia/mod.rs`): Interpolates paths morphing between two vector strings.
*   **`openmedia_animate_generate_spinner`** (`src/tools/openmedia/mod.rs`): Creates beautiful animated loading spinners in SVG.
*   **`openmedia_animate_from_lottie`** (`src/tools/openmedia/mod.rs`): Converts a Lottie JSON animation into an animated SVG.
*   **`openmedia_animate_to_lottie`** (`src/tools/openmedia/mod.rs`): Converts an animated SVG back into Lottie JSON.
*   **`openmedia_image_apply_filter`** (`src/tools/openmedia/mod.rs`): Applies filters (invert, grayscale, etc.) to an image.
*   **`openmedia_image_resize`** (`src/tools/openmedia/mod.rs`): Resizes an image with configurable width and height.
*   **`openmedia_image_crop`** (`src/tools/openmedia/mod.rs`): Crops an image using custom bounding box coordinates.
*   **`openmedia_image_transform`** (`src/tools/openmedia/mod.rs`): Transforms an existing image guided by strength parameters.
*   **`openmedia_image_convert`** (`src/tools/openmedia/mod.rs`): Converts image file format extension target.
*   **`openmedia_image_batch_process`** (`src/tools/openmedia/mod.rs`): Processes image filters in batches.
*   **`openmedia_video_create`** (`src/tools/openmedia/mod.rs`): Compiles frame-by-frame videos defined using a JSON Scene DSL.
*   **`openmedia_video_preview`** (`src/tools/openmedia/mod.rs`): Generates a video preview frame at a specific timestamp offset.
*   **`openmedia_video_create_slideshow`** (`src/tools/openmedia/mod.rs`): Compiles an image sequence slideshow with audio overlays.
*   **`openmedia_video_add_transition`** (`src/tools/openmedia/mod.rs`): Applies scene transition blend clips.
*   **`openmedia_video_add_audio`** (`src/tools/openmedia/mod.rs`): Adds background narration/music tracks to a video.
*   **`openmedia_video_from_template`** (`src/tools/openmedia/mod.rs`): Instantiates a video template replacing placeholder arguments.
*   **`openmedia_video_extract_frames`** (`src/tools/openmedia/mod.rs`): Extracts frames/images from a video at key timestamp offsets.
*   **`openmedia_video_trim`** (`src/tools/openmedia/mod.rs`): Trims a video file to a specific time range.
*   **`openmedia_template_create`** (`src/tools/openmedia/mod.rs`): Creates and saves a custom video scene template.
*   **`openmedia_template_read`** (`src/tools/openmedia/mod.rs`): Reads templates configurations details or list templates.
*   **`openmedia_template_update`** (`src/tools/openmedia/mod.rs`): Updates an existing template definition.
*   **`openmedia_template_delete`** (`src/tools/openmedia/mod.rs`): Deletes an existing template definition.
*   **`openmedia_improve_score_image`** (`src/tools/openmedia/mod.rs`): Scores prompt alignment using CLIP and Aesthetic models.
*   **`openmedia_improve_refine_prompt`** (`src/tools/openmedia/mod.rs`): Gets prompt refinement suffix recommendations based on score feedbacks.
*   **`openmedia_improve_auto_refine`** (`src/tools/openmedia/mod.rs`): Iteratively refines prompts to generate high aesthetic quality assets.
*   **`openmedia_improve_feedback`** (`src/tools/openmedia/mod.rs`): Logs manual ratings score and description feedback on generations.
*   **`openmedia_improve_quality_report`** (`src/tools/openmedia/mod.rs`): Fetches comprehensive statistics report of the generation history DB.

### Ported Native Reasoning & Context Tools (Mega Ports)
*   **Sequential Thinking Reasoning Loop** (`src/tools/sequential_thinking/`): Includes `sequentialthinking` (reasoning chain loop), `analyze_graph` (thought query and quality statistics), `export_session` (mermaid/markdown exporter), `summarize_reasoning` (structural timeline summary), and `reasoning_templates` (reasoning design frameworks).
*   **Context Scoping & Headroom Compression** (`src/tools/headroom/`): Includes `scope_context` (YAGNI contextual filtering), `compress_content` (token-reduction compression), `retrieve_original` (retrieve full output from cached IDs), `compress_file` / `compress_diff` / `compress_url` (specialized format filters), `cache_stats` / `clear_cache` (caching metrics), `summarize_codebase` (code hierarchy summary), and `count_tokens` (FastBPE tokens count).
*   **Knowledge Graph Memory** (`src/tools/graph_memory/`): Includes `create_entities` (graph node insertion), `create_relations` (node relationship links), `add_observations` (append node facts), `read_graph` (retrieve entity scopes), `search_nodes` (entity search), `open_nodes` (open graph nodes), and `create_database_branch` / `commit_database_branch` / `rollback_database_branch` (SQL database transaction branching).
*   **Extended Developer Memory (Memory Extra)** (`src/tools/memory_extra/`): Includes `set_working_memory` / `get_working_memory` (short-term cache memory), `log_execution_episode` / `log_reflection` (reflexive action logs), `record_tool_performance` (latency tracking), `hybrid_search` (BM25 + vector search), `extract_and_store_facts` / `proactive_recall` (background fact curators), and `log_repository_evolution` / `traverse_graph` (codebase architecture community maps).

### System & Networking Tools
*   **`clipboard`** (`src/tools/clipboard.rs`): Gets or sets text content in the system clipboard.
*   **`open_path`** (`src/tools/open.rs`): Opens a file, folder, or URL using the user's default system application.
*   **`file_watcher`** (`src/tools/watcher.rs`): Starts, stops, or queries a background filesystem watcher to run commands on file modifications.
*   **`check_port`** (`src/tools/network.rs`): Checks port availability. **Restricted to localhost only** for security (SSRF prevention).
*   **`system_info`** (`src/tools/system_info.rs`): Retrieves CPU, memory, OS version, and host environment information.
*   **`send_remote_input`** (`src/tools/remote.rs`): Forwards a prompt or input instruction to another active session (like the TUI terminal prompt) to be executed immediately.
*   **`manage_mcp`** (`src/tools/mcp_manager.rs`): List, add, remove, enable, or disable Model Context Protocol (MCP) server definitions.
*   **`onpkg`** (`src/tools/onpkg.rs`): Scaffold stacks, list available templates, and run package manager commands via `onpkg`.
