# OpenZ Native Tool Subsystem Architecture & Taxonomy

The OpenZ native tool subsystem provides a high-performance, modular, and security-governed tool registry designed for local LLM pair-programming and autonomous agent workflows. Built entirely in async Rust, it combines in-process native execution with intelligent intent-based scoping, context compression, and multi-domain specialized submodules.

---

## Architecture Overview

```
                          User Prompt / Channel Input
                                      │
                                      ▼
                        Intent & Domain Classifier
                                      │
                        ┌─────────────┴─────────────┐
                        ▼                           ▼
            Explicit Tool Mentions         Intent Packs & Heuristics
                        └─────────────┬─────────────┘
                                      ▼
                           ToolScopeEngine (scope)
                                      │
                             Prompt-Routed Scope
                         (max 128 tools per payload)
                                      │
                                      ▼
                        ToolRegistry (`Tool` Trait)
                                      │
        ┌─────────────────────────────┼─────────────────────────────┐
        ▼                             ▼                             ▼
  Core Engine                  Domain Subsystems               Loose Tools
  - `arguments.rs`             - `browser/`                    - `filesystem.rs`
  - `metadata.rs`              - `graph_memory/`               - `shell.rs`
  - `defs.rs`                  - `headroom/`                   - `cargo_manager.rs`
  - `registry.rs`              - `memory_extra/`               - `git_manager.rs`
  - `routing.rs`               - `opendoc/`                    - `web.rs`
  - `resource_policy.rs`       - `openmedia/`                  - `db_inspector.rs`
  - `scope_engine.rs`          - `searchxyz/`                  - `task_manager.rs`
                               - `self_management/`            - `onpkg.rs`
                               - `sequential_thinking/`        - ... (54 files)
                               - `shared_memory/`
                               - `subagent/`
                                      │
                                      ▼
                          SecurityGuard Permissions
                           & Subprocess Sandboxing
```

---

## 1. Core Tool Subsystem Engine

The core engine drives tool registration, argument normalization, metadata inference, intent-based dynamic routing, and capability policy enforcement.

| Module | File Link | Primary Purpose | Key Types / Functions |
|---|---|---|---|
| **Arguments** | [`src/tools/arguments.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs) | Normalizes model tool arguments across multiple casing and naming conventions; maps parameter aliases (e.g. `TargetFile`, `AbsolutePath`, `file` → `path`). | [`normalize_tool_args`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs#L50), [`to_snake_case`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs#L10) |
| **Metadata** | [`src/tools/metadata.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/metadata.rs) | Declares tool metadata, risk classifications, approval triggers, network/disk/process capabilities, and canonical name mapping. | [`ToolMetadata`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/metadata.rs#L10), [`ToolRisk`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/metadata.rs#L30), [`ToolSpec`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/metadata.rs#L50) |
| **Definitions** | [`src/tools/defs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/defs.rs) | Curated static tool specifications, semantic pack memberships, guidelines (`when_to_use`, `when_not_to_use`), aliases, and concrete examples. | [`STATIC_TOOL_DEFS`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/defs.rs#L7), [`all_tool_specs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/defs.rs#L890) |
| **Registry** | [`src/tools/registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs) | Concurrency-safe static tool container, canonical registration deduplication, alias resolution, and drift detection. | [`insert_unique_tool`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs#L15), [`resolve_static_name`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs#L40), [`static_tool_drift`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs#L60) |
| **Routing** | [`src/tools/routing.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/routing.rs) | Heuristic prompt classification, domain matching, scoring algorithms, and explicit tool mention extraction. | [`select_domains_for_prompt`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/routing.rs#L15), [`tool_selection_score`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/routing.rs#L60) |
| **Resource Policy** | [`src/tools/resource_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy.rs) | Sandbox resource limits, CPU/memory quotas, allowed syscall policies, and policy enforcement boundaries. | [`ToolResourcePolicy`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy.rs#L15), [`PolicyEnforcer`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy.rs#L45) |
| **Scope Engine** | [`src/tools/scope_engine.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine.rs) | Dynamic turn-by-turn tool scoping, tool pack assembly, and model context window truncation (limiting exposed tools to 128). | [`ToolScopeEngine`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine.rs#L20), [`ToolPack`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine.rs#L45) |

---

## 2. Grouped Domain Subsystems (11 Subdirectories)

Large or highly cohesive tool families are partitioned into 11 dedicated domain subdirectories under [`src/tools/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/):

### 1. `browser/` — Browser Automation & CDP
- **Directory:** [`src/tools/browser/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/)
- **Description:** Headless and automated browser control across Playwright/CDP and native Firefox.
- **Key Modules:**
  - `broker.rs`: Shared browser daemon process management and IPC.
  - `common.rs`: DOM extraction, navigation, and page interaction helpers.
  - `firefox.rs`: Native Firefox Marionette/CDP controller (`firefox_browser`).
  - `gsd.rs`: Playwright-based browser driver wrapper (`gsd_browser`).
  - `obscura.rs`: Pure CDP lightweight browser engine (`obscura_browser`).
  - `status.rs`: Active browser inspection and diagnostics (`inspect_browsers`).

### 2. `graph_memory/` — Knowledge Graph & Database Branching
- **Directory:** [`src/tools/graph_memory/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/graph_memory/)
- **Description:** Entity-relation-observation knowledge graph storage and SQLite database branch checkpointing.
- **Key Tools (12 tools):** `create_entities`, `create_relations`, `add_observations`, `delete_entities`, `delete_observations`, `delete_relations`, `read_graph`, `search_nodes`, `open_nodes`, `create_database_branch`, `commit_database_branch`, `rollback_database_branch`.

### 3. `headroom/` — Context Compression & Retrieval (CCR)
- **Directory:** [`src/tools/headroom/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/)
- **Description:** Context window token compaction, cache alignment, and CCR retrieval (ported from MCP to native Rust).
- **Key Tools (21 tools):** `scope_context`, `compress_content`, `retrieve_original`, `ping`, `server_info`, `count_tokens`, `cache_stats`, `headroom_stats`, `headroom_usage`, `clear_cache`, `search_cache`, `cache_align`, `compress_schema`, `compress_file`, `compress_diff`, `export_cache`, `import_cache`, `compress_url`, `run_and_compress`, `compress_directory`, `summarize_codebase`.

### 4. `memory_extra/` — Working Memory & Reflection Telemetry
- **Directory:** [`src/tools/memory_extra/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/)
- **Description:** Episodic recall, working memory scratchpad, execution reflections, tool performance tracking, smart facts extraction, and codebase evolution graphs.
- **Key Tools (32 tools):** `set_working_memory`, `get_working_memory`, `evict_expired_working_memory`, `promote_working_memory`, `log_execution_episode`, `log_reflection`, `retrieve_episodic_reflections`, `record_tool_performance`, `query_tool_performance`, `store_shared_team_memory`, `retrieve_shared_team_memory`, `search_text`, `hybrid_search`, `invalidate_fact`, `forget_memory`, `query_fact_history`, `query_as_of`, `smart_store`, `extract_and_store_facts`, `proactive_recall`, `compress_context`, `memory_stats`, `log_repository_evolution`, `query_repository_evolution`, `traverse_graph`, `find_path`, `analyze_graph_communities`, `detect_and_resolve_conflicts`, `compact_memories`, `index_codebase`, `query_code_graph`, `analyze_code_impact`.

### 5. `opendoc/` — Comprehensive Document Processing
- **Directory:** [`src/tools/opendoc/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/)
- **Description:** In-process reading, editing, diffing, conversion, and validation across PDF, DOCX, XLSX, and PPTX formats.
- **Key Tools (35 tools):** `opendoc_open_document`, `opendoc_read_document_text`, `opendoc_search_document`, `opendoc_replace_text`, `opendoc_diff_documents`, `opendoc_diff_documents_visual`, `opendoc_chunk_for_embedding`, `opendoc_fill_template`, `opendoc_validate_document`, `opendoc_validate_pdf_a_compliance`, `opendoc_extract_structured_metadata`, `opendoc_convert`, `opendoc_extract_images`, `opendoc_split_pdf`, `opendoc_create_html`, `opendoc_batch_convert`, `opendoc_create_docx`, `opendoc_docx_add_paragraph`, `opendoc_docx_add_table`, `opendoc_docx_add_image`, `opendoc_create_pptx`, `opendoc_pptx_add_slide`, `opendoc_create_xlsx`, `opendoc_edit_xlsx`, `opendoc_create_pdf`, `opendoc_create_formatted_pdf`, `opendoc_merge_pdfs`, `opendoc_extract_pdf_text`, `opendoc_list_pdf_fields`, `opendoc_fill_pdf_form`, `opendoc_find_tables`, `opendoc_analyze_document_complexity`, `opendoc_ocr_document`, `opendoc_check_ocr_available`, `opendoc_extract_archive_digest`.

### 6. `openmedia/` — Media Generation & Manipulation
- **Directory:** [`src/tools/openmedia/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/)
- **Description:** SVG animation, timeline synthesis, rasterization, video stitching, audio merging, image enhancement, and template rendering.
- **Key Tools (37 tools):** `openmedia_model_download`, `openmedia_rasterize_svg`, `openmedia_diagram_generate_mermaid`, `openmedia_html_to_image`, `openmedia_create_svg`, `openmedia_create_chart`, `openmedia_create_icon`, `openmedia_animate_svg`, `openmedia_animate_create_timeline`, `openmedia_animate_morph_paths`, `openmedia_animate_generate_spinner`, `openmedia_animate_from_lottie`, `openmedia_animate_to_lottie`, `openmedia_image_apply_filter`, `openmedia_image_resize`, `openmedia_image_crop`, `openmedia_image_transform`, `openmedia_image_convert`, `openmedia_image_batch_process`, `openmedia_video_create`, `openmedia_video_preview`, `openmedia_video_create_slideshow`, `openmedia_video_add_transition`, `openmedia_video_add_audio`, `openmedia_video_from_template`, `openmedia_video_extract_frames`, `openmedia_video_trim`, `openmedia_template_create`, `openmedia_template_read`, `openmedia_template_update`, `openmedia_template_delete`, `openmedia_improve_score_image`, `openmedia_improve_refine_prompt`, `openmedia_improve_auto_refine`, `openmedia_improve_feedback`, `openmedia_improve_quality_report`, `openmedia_ping`.

### 7. `searchxyz/` — Multi-Engine Web Discovery
- **Directory:** [`src/tools/searchxyz/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/)
- **Description:** Deep web exploration, source crawling, sitemaps, relationship indexing, and multi-source research briefs.
- **Key Tools (17 tools):** `searchxyz_doctor`, `searchxyz_search_web`, `searchxyz_browser_search`, `searchxyz_read_url`, `searchxyz_search_and_read`, `searchxyz_recall`, `searchxyz_list_sources`, `searchxyz_deep_research`, `searchxyz_index_content`, `searchxyz_site_map`, `searchxyz_index_relationship`, `searchxyz_query_graph`, `searchxyz_read_github_repo`, `searchxyz_export_research`, `searchxyz_import_research`, `searchxyz_delete_source`, `searchxyz_clear_index`.

### 8. `self_management/` — Runtime Diagnostics & Configuration
- **Directory:** [`src/tools/self_management/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/)
- **Description:** Self-inspection of tool inventories, dynamic scope requests, configuration edits, session recovery, and skill curation.
- **Key Tools (10 tools):** `diagnose_tool`, `tool_catalog`, `openz_inventory`, `request_tool_scope`, `curate_skill`, `optimize_tool_scope`, `manage_config`, `diagnose_system`, `manage_sessions`, `manage_backups`.

### 9. `sequential_thinking/` — Structured Multi-Step Reasoning
- **Directory:** [`src/tools/sequential_thinking/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sequential_thinking/)
- **Description:** Step-by-step hypothesis generation, revision tracking, branch exploration, and reasoning graph analysis (native Rust port).
- **Key Tools (5 tools):** `sequentialthinking`, `analyze_graph`, `export_session`, `summarize_reasoning`, `reasoning_templates`.

### 10. `shared_memory/` — Team Knowledge Persistence
- **Directory:** [`src/tools/shared_memory/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/)
- **Description:** Global persistent key-value and semantic research brief storage shared across sessions and subagents.
- **Key Tools (10 tools):** `store_memory`, `recall_memory`, `clear_memory`, `delete_memory`, `update_memory`, `archive_research`, `search_research`, `knowledge_source`, `research_brief`, `workflow_memory`.

### 11. `subagent/` — Subagent Delegation & Orchestration
- **Directory:** [`src/tools/subagent/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/)
- **Description:** Subagent task delegation, evaluation loops, profile lifecycle management, and cooperative cancellation tokens.
- **Key Tools (7 tools):** `delegate_task`, `parallel_research`, `evaluator_optimizer_loop`, `optimize_subagent`, `create_subagent`, `delete_subagent`, `update_subagent_settings`.

---

## 3. Loose Native Tools Taxonomy

Loose native tools sit directly in [`src/tools/*.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/) and are categorized by functional domain:

### Filesystem & Documents
- [`filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs): `read_file`, `write_file`, `patch_file`, `replace_lines`, `list_dir`, `find_files`, `zenflow_edit`.
- [`doc_reader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader.rs): Multi-format PDF/DOCX/XLSX text extraction.
- [`notes.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/notes.rs): Local Markdown note catalog and indexing (`index_notes`).
- [`watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/watcher.rs): Filesystem event listener (`file_watcher`).

### Shell & Execution Sandboxing
- [`shell.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs): Sandboxed command execution (`exec_command`), long-running server manager (`manage_servers`), Python REPL (`python_sandbox`).
- [`wasm_sandbox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/wasm_sandbox.rs): In-process isolated WebAssembly execution (`wasm_sandbox`).

### Code Intelligence & Build Tooling
- [`ast_grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep.rs): Structural AST pattern matching (`ast_grep`, `ast_grep_index_codebase`).
- [`cargo_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager.rs): Rust project compile, check, test, and clippy driver (`cargo_manager`).
- [`compiler_auto_heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs): Autonomous build error correction loop (`compiler_auto_heal`).
- [`git_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager.rs): Local Git status, diff, commit, and branch control (`git_manager`).
- [`github.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github.rs): GitHub/GitLab REST & GraphQL provider client (`git_provider`).
- [`github_mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_mcp.rs): PR and issue tracking bridge (`github_create_pull_request`, `github_search_issues`, `github_get_issue_comments`).
- [`docs_mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/docs_mcp.rs): Docset query and indexing bridge (`docs_search_docs`, `docs_read_doc_page`, etc.).
- [`grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep.rs): Ripgrep-accelerated code search (`grep_search`).
- [`js_format.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format.rs): JS/TS AST formatting (`js_format`).
- [`onpkg.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/onpkg.rs): Online package & template manager integration (`onpkg`).
- [`outline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline.rs): Structural code outline extractor (`code_outline`).
- [`rust_docs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/rust_docs.rs): Local and online Rust documentation lookup (`rust_docs`).

### Web & Network
- [`web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web.rs): Fast DOM scraping and HTML-to-markdown conversion (`web_fetch`).
- [`web_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search.rs): Tavily web search integration (`web_search`).
- [`crawl.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl.rs): Multi-threaded web crawling via spider-rs (`crawl_website`).
- [`network.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network.rs): TCP/UDP connectivity check (`check_port`).
- [`social_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/social_search.rs): Social platform search integration (`social_search`).

### System & Utility
- [`clipboard.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard.rs): System clipboard read/write (`clipboard`).
- [`db_inspector.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector.rs): In-process SQLite query inspection and execution (`db_inspector`, `db_write`).
- [`desktop_notify.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/desktop_notify.rs): System notification dispatch (`desktop_notify`).
- [`device_inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory.rs): Local peripherals and GPU inventory (`device_inventory`).
- [`get_logs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs.rs): Runtime structured log reader (`get_logs`).
- [`manage_whitelist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist.rs): Dynamic command allowlisting (`manage_whitelist`).
- [`open.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open.rs): Open URLs or files in system default application (`open_path`).
- [`system_info.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info.rs): Hardware, memory, and OS metrics (`system_info`).
- [`task_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager.rs): Asynchronous background task supervisor (`manage_tasks`).

### Automation, Workflows & Messaging
- [`cron.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron.rs): Scheduled job management (`schedule_job`, `list_jobs`, `remove_job`, `pause_job`, `resume_job`, `get_job`, `get_job_logs`, `run_job_now`).
- [`orchestrator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator.rs): Multi-agent workflow spec executor (`orchestrate_workflow`).
- [`remote.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote.rs): Cross-channel prompt forwarding (`send_remote_input`).
- [`sop.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sop.rs): Standard Operating Procedure workflow trigger (`trigger_sop`).
- [`telegram_send.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/telegram_send.rs): Telegram message and document delivery (`telegram_send_message`, `telegram_send_document`).

### Media & Visuals
- [`html_video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/html_video.rs): CDP-rendered HTML animation timeline to MP4 (`html_to_video`).
- [`image_generator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator.rs): HTML/CSS/SVG rendering to PNG (`generate_image`).
- [`mermaid.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid.rs): Mermaid diagram rendering (`render_mermaid`).
- [`svg_animator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator.rs): Pure SVG animation compiler (`create_animated_svg`).
- [`template_compiler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler.rs): Template processing engine (`compile_template`).
- [`video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video.rs): Programmatic MP4 video generation (`generate_video`).

### Search & Vector Embeddings
- [`semantic_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search.rs): Local FastEmbed vector search (`semantic_search`).

### MCP Integration & Bridging
- [`mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp.rs): MCP stdio client, gRPC bridge, and `LazyMcpToolWrapper`.
- [`mcp_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager.rs): MCP server configuration CRUD (`manage_mcp`).

---

## 4. Modular Registration Facades

Tool registration is organized into four modular facades under [`src/cli/tool_registration/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tool_registration/) and unified by [`register_all_tools`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs#L8):

```
                       register_all_tools(&registry, ...)
                                       │
        ┌───────────────────┬──────────┴────────┬───────────────────┐
        ▼                   ▼                   ▼                   ▼
    core.rs             memory.rs            media.rs         integrations.rs
  - Foundation       - Sequential         - OpenMedia         - Searchxyz
  - Filesystem         Thinking             Suite             - GitHub MCP
  - Shell / Tasks    - Headroom (CCR)     - Opendoc           - Docs MCP
  - Subagents        - Graph Memory         Suite             - Lazy MCP
  - Cron / Git       - Memory Extra
```

1. **Core Registration** ([`src/cli/tool_registration/core.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tool_registration/core.rs)):
   - `register_foundation_tools`: Baseline tools (`read_file`, `find_files`, `doc_reader`, `wasm_sandbox`, `js_format`, `semantic_search`, and baseline shared memory).
   - `register_core_tools`: Registers filesystem editors, shell sandbox, subagents, cron jobs, git managers, SQLite tools, browsers, and self-management tools.
2. **Memory Registration** ([`src/cli/tool_registration/memory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tool_registration/memory.rs)):
   - Registers all 4 memory subsystems: sequential thinking (5), Headroom CCR (21), graph memory (12), and memory extra (32).
3. **Media Registration** ([`src/cli/tool_registration/media.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tool_registration/media.rs)):
   - Registers the comprehensive OpenMedia suite (37 tools) and Opendoc document engine (35 tools).
4. **Integrations Registration** ([`src/cli/tool_registration/integrations.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tool_registration/integrations.rs)):
   - Registers searchxyz (17 tools), GitHub MCP client (3 tools), Docs MCP client (6 tools), and spawns asynchronous background workers for lazy external MCP servers.

---

## 5. Architectural Rules & Invariants

1. **Exact 128 Tool Limit for Model Payloads:**
   - OpenAI-compatible function calling payload sizes are strictly capped at 128 tools per turn.
   - The [`ToolScopeEngine`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine.rs) dynamically routes tools matching prompt intent and explicit requests, prioritizing critical and high-frequency tools while shedding irrelevant tools.
2. **No Orphan Tools:**
   - Every registered tool must be strictly categorized into an architectural domain with non-empty presentation name, domain, and risk classification.
   - Architectural invariant tests in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs) guard against unclassified orphan tools and duplicate canonical registrations.
3. **Parameter Normalization Invariant:**
   - Tool arguments must pass through [`normalize_tool_args`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs#L50) before execution. Explicit aliases (`file`, `TargetFile`, `Path`) map to canonical fields without overwriting user-provided values.
4. **Subagent Cancellation Lifecycles:**
   - All nested subagent tools (`delegate_task`, `parallel_research`, `evaluator_optimizer_loop`) share the parent turn's `CancellationToken` to ensure immediate cascade cancellation upon user interrupt or timeout.
5. **Low-Resource Compilation Discipline:**
   - In accordance with developer preferences, all testing and compilation must be executed with capped parallelism (`-j 2`):
     ```bash
     cargo test -p openz --lib test_native_tool_registration_names -j 2
     cargo clippy -p openz --lib -j 2
     ```
