### v0.0.194 (Latest Release)
- **Ideas**:
  - Comprehensive headless subagent validation across the GSD & Playwright Browser Automation Suite (`gsd_browser`, `obscura_browser`, `web_fetch`, `crawl_website`, `firefox_browser`, `inspect_browsers`).
  - Action normalization and alias mapping: LLMs frequently use variant browser action names (e.g. `goto` or `open` instead of `navigate`, `eval_js` or `evaluate` or `js` instead of `eval`, `snapshot` instead of `render`, `accessibility_tree` with dashes or underscores). Implemented robust action normalization across `gsd_browser`, `obscura_browser`, and `firefox_browser`.
  - Comprehensive parameter alias tolerance: Supported interchangeable naming across all browser tools for URLs (`url`, `target_url`, `uri`, `link`, `target`), selectors (`selector`, `css_selector`, `cssSelector`, `css`, `element`), element references (`ref_id`, `refId`, `ref`, `element`, `id`), file paths (`path`, `output`, `output_path`, `file_path`, `file`), and scripts (`script`, `expression`, `code`, `js`).
  - Robust numeric & boolean coercion in `crawl_website`, `web_fetch`, `obscura_browser`, and `firefox_browser`: Implemented `get_u64_arg` and `get_bool_arg` to safely parse stringified integers and booleans (`"limit": "2"`, `"depth": "1"`, `"timeout_secs": "10"`, `"render_js": "false"`, `"respect_robots_txt": "true"`), eliminating silent fallback to defaults.
  - Stale & unhealthy browser daemon detection in `inspect_browsers`: Fixed a false-positive health status where `gsd-browser daemon health` exiting with code 0 but reporting `Daemon: unhealthy (stale socket)` was incorrectly reported as `running` and recommended as the primary backend. `status_value_to_backend_status` now marks unhealthy daemons as `Broken` so OpenZ avoids routing tasks to broken background daemons.
- **Inspirations**:
  - Headless subagent real-world evaluation findings across browser automation tools.
  - Robustness Principle (Postel's Law): Be liberal in what you accept from LLMs.
  - Resilient multi-tier browser brokering (Obscura CDP -> Firefox Marionette -> GsdBrowser Playwright GUI).
- **Sources & References**:
  - Implementation & Dispatch Sources:
    - [`src/tools/crawl.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl.rs): `get_u64_arg`, `get_bool_arg`, parameter aliases, and string-to-number/bool coercion.
    - [`src/tools/browser/obscura.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura.rs): Action normalization (`eval`, `evaluate`, `js` -> `is_eval`), URL and script aliases, timeout string parsing.
    - [`src/tools/browser/gsd.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd.rs): `build_gsd_browser_command` action normalization (`goto`, `eval_js`, `accessibility_tree`, `page_source`, `save_pdf`) and aliases for `url`, `ref_id`, `path`, `script`.
    - [`src/tools/browser/firefox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox.rs): Action normalization and aliases for `url`, `selector`, `path`, `script`, and `timeout_secs`.
    - [`src/tools/browser/status.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status.rs): Unhealthy daemon detection in `gsd_status` and `status_value_to_backend_status`.
    - [`src/tools/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web.rs): `web_fetch_render_js_enabled` string boolean coercion and URL parameter aliases.
  - Test Modules:
    - [`src/tools/crawl_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl_tests.rs): `crawl_arg_coercion_supports_strings_and_aliases`, string timeout tests.
    - [`src/tools/browser/obscura_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura_tests.rs): `test_obscura_action_recognition`, `test_obscura_timeout_and_aliases`.
    - [`src/tools/browser/gsd_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd_tests.rs): `test_build_gsd_browser_command_aliases_and_action_normalization`.
    - [`src/tools/browser/firefox_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox_tests.rs): `test_firefox_action_normalization_and_timeout_parsing`.
    - [`src/tools/browser/status_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status_tests.rs): `status_value_to_backend_status_marks_unhealthy_as_broken`.
    - [`src/tools/web_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_tests.rs): `web_fetch_browser_retry_can_be_disabled_explicitly` string boolean tests.
- **Details & Metrics**:
  - 100% pass rate achieved across all browser automation tools in real-world headless subagent mode.
  - Execution latencies: `inspect_browsers` ~90ms, `gsd_browser` navigate ~670ms, `gsd_browser` snapshot ~56ms, `gsd_browser` eval ~48ms, `gsd_browser` screenshot ~500ms, `gsd_browser` save_pdf ~170ms, `obscura_browser` render ~1.2s, `obscura_browser` eval_js ~790ms, `web_fetch` ~460ms, `crawl_website` ~2.0s.
  - Resolved 5 friction points and edge cases across action variants, parameter aliases, string numbers/booleans, and daemon health reporting.
- **Verification**: Verified 5 crawl tests pass (`cargo test -p openz --lib tools::crawl -j 1`), 27 browser tests pass (`cargo test -p openz --lib tools::browser -j 1`), 28 web tests pass (`cargo test -p openz --lib tools::web -j 1`), exact 260 registered native tools invariant maintained (`cargo test -p openz --lib test_native_tool_registration_names -j 1`), version sync verified (`cargo test -p openz --lib version_sync_tests -j 1`), 0 clippy warnings (`cargo clippy -p openz -j 1`), and clean build (`cargo build -p openz --bin openz -j 2`).

### v0.0.193
- **Ideas**:
  - Comprehensive headless subagent validation across the complete SearchXYZ & Web Research Engine (17 tools across multi-engine web search, browser automation search, markdown DOM scraping, integrated search-and-read, recursive deep research, sitemap discovery, knowledge graph relationship indexing and querying, GitHub repository ingestion and tree mapping, Tantivy full-text search indexing, semantic recall, source listing, research bundle export/import, source deletion, and index clearing).
  - Bing Click-Tracking Redirect Unwrapping: Bing web search results frequently wrap destination URLs in click-tracking redirect links (`https://www.bing.com/ck/a?!...&u=a1<base64>&...`). Downstream markdown scrapers, readers, and deep research agents following these URLs hit Bing tracking redirects, resulting in 404s, redirect loops, or bot verification barriers. Implemented `unwrap_bing_redirect` using Base64 decoding (supporting both standard and URL-safe base64, with or without padding) on Bing's `u=a1<base64>` query parameter to extract and unmask the genuine destination URL (e.g. `https://en.wikipedia.org/...`). Prioritized `title_el.select(&link_sel)` over container-wide anchors.
  - Robust String-to-Number & Boolean Coercion for LLM Tool Arguments: LLMs routinely emit numeric and boolean parameters as strings (`"max_results": "3"`, `"limit": "10"`, `"depth": "2"`, `"confirm": "true"`). Standard Serde deserialization rejects stringified numbers and booleans with strict type mismatch errors. Implemented `coerce_number_value`, `coerce_numeric_fields`, and `coerce_bool_fields` in `src/tools/searchxyz/mod.rs` and wired them across all web search, reading, graph, and index tools prior to Serde deserialization.
  - Direct JSON Object/Array Tolerance in `searchxyz_import_research`: Serde expected `payload: String` as a stringified JSON blob. However, LLMs frequently pass the parsed research bundle directly as a JSON object or array in `payload`, or omit `payload` and pass bundle fields at top-level. Added automatic payload normalization to serialize direct JSON payloads seamlessly before bundle validation and import.
- **Inspirations**:
  - Headless subagent real-world evaluation findings across all 17 SearchXYZ tools.
  - Robustness Principle (Postel's Law): Be liberal in what you accept from LLMs (stringified numeric parameters, unescaped JSON bundles, and raw Bing redirect URLs).
  - Base64 URL decoding patterns for search engine click-tracking telemetry unwrapping.
- **Sources & References**:
  - Implementation & Dispatch Sources:
    - [`tools/searchxyz/Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/searchxyz/Cargo.toml): Added `base64 = "0.22"` dependency.
    - [`tools/searchxyz/src/search/bing.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/searchxyz/src/search/bing.rs): `unwrap_bing_redirect` Base64 decoding and targeted anchor selection.
    - [`src/tools/searchxyz/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/mod.rs): `coerce_number_value`, `coerce_numeric_fields`, `coerce_bool_fields`.
    - [`src/tools/searchxyz/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web.rs): Numeric coercion across `searchxyz_browser_search`, `searchxyz_search_web`, `searchxyz_read_url`, `searchxyz_search_and_read`, `searchxyz_deep_research`, and `searchxyz_site_map`.
    - [`src/tools/searchxyz/graph.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/graph.rs): Numeric coercion in `searchxyz_query_graph` and `searchxyz_read_github_repo`.
    - [`src/tools/searchxyz/index.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/index.rs): Numeric/boolean coercion and JSON bundle tolerance in `searchxyz_recall`, `searchxyz_list_sources`, `searchxyz_export_research`, `searchxyz_delete_source`, `searchxyz_clear_index`, and `searchxyz_import_research`.
  - Test Modules:
    - [`tools/searchxyz/src/search/bing.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/searchxyz/src/search/bing.rs): `test_unwrap_bing_redirect`.
    - [`src/tools/searchxyz/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/mod_tests.rs): `test_coerce_number_value`, `test_coerce_numeric_fields`, `test_coerce_bool_fields`.
- **Details & Metrics**:
  - Validated all 17 SearchXYZ tools (7 Web & Search + 3 Knowledge Graph & Repos + 7 Index & Recall).
  - 100% pass rate achieved across all suites in real-world headless subagent mode.
  - Subagent real-world execution speeds: web search 350-600ms, markdown DOM read 700ms-1.2s, browser search 1.03s, combined search-and-read 740ms, GitHub clone & tree map 3.0s, graph query ~12ms, recall ~18ms, Tantivy deletion & GC 16ms.
  - Resolved 3 core friction points across Bing tracking redirect loops, Serde string-to-number/bool deserialization failures, and nested JSON import payloads.
- **Verification**: Verified Bing unit tests pass (`cargo test -p searchxyz --lib search::bing -j 1`), 58 SearchXYZ crate tests pass (`cargo test -p searchxyz --lib -j 1 -- --test-threads=1`), 23 SearchXYZ tests in openz pass (`cargo test -p openz --lib tools::searchxyz -j 1`), exact 260 registered native tools invariant maintained (`cargo test -p openz --lib test_native_tool_registration_names -j 1`), version sync verified (`cargo test -p openz --lib version_sync_tests -j 1`), 0 clippy warnings (`cargo clippy -p openz -j 1`), and clean build (`cargo build -p openz -j 2`).

### v0.0.192
- **Ideas**:
  - Comprehensive headless subagent validation across the complete Media Engine suite (52 tools across vector graphics, animation generators, image transformations, video synthesis, template management, and quality evaluation).
  - Numeric coercion for LLM tool arguments: LLMs routinely emit numeric dimensions (e.g. width, height, fps, duration, loops) as strings (`"width": "300"`, `"duration": "2.5"`). Serde standard deserialization rejects these string numbers with type mismatch errors. Implement `coerce_number_value` and `coerce_numeric_fields` across SVG creation/animation and video operations to transparently parse and coerce stringified numbers to valid JSON numbers.
  - Robust batch image operation discriminator parsing: Serde enums default to external tagging (`{"Resize": {...}}`), whereas LLMs frequently emit tagged discriminator objects (`{"operation": "resize", "width": 80, "height": 80}`) or plain string operation identifiers (`"invert"`, `"grayscale"`). Implement `normalize_process_operation_value` in `openmedia_image_batch_process` to convert untagged string and discriminator map inputs into matching internal variant schemas, and provide `#[derive(Default)]` on `ResizeMethod` with `#[serde(default)]` on resize parameters.
  - User & LLM Rating Scale Tolerance: The SQLite database schema for `media_feedback` enforces `CHECK (rating >= 0.0 AND rating <= 1.0)`. However, human users and LLMs naturally rate outputs on 1-5, 1-10, or 1-100 scales (e.g. 5 stars). Automatically normalize ratings `> 1.0` (scale 1-5 to `/ 5.0`, 1-10 to `/ 10.0`, 1-100 to `/ 100.0`) in both OpenZ and OpenMedia MCP before SQLite insertion, eliminating constraint violation failures.
  - Headless CDP Browser Screenshot Resilience: On systems where specialized browser engines (such as Obscura) or minimal CDP daemons are running or listed first in `chrome_paths`, CDP screenshot capturing can hang or fail due to non-standard WebSocket target endpoints or unimplemented `Page.captureScreenshot`. Enhance `obtain_tab_websocket_url` to fall back to `/json/list` scraping, prioritize full `google-chrome` binaries over minimal scrapers, and add automatic fallback in `generate_image` to `openmedia_mcp::html_to_image` when CDP browser rendering fails or is unavailable.
  - Tab lifecycle cleanup in `html_video`: Properly close the specific opened target tab via `tab_id` and CDP client rather than leaving dangling headless tabs.
- **Inspirations**:
  - Headless subagent evaluation findings across all 52 Media Engine tools.
  - Robustness Principle (Postel's Law): Be liberal in what you accept from LLMs (stringified numeric parameters, discriminator-tagged maps, and 5-star ratings).
  - Resilient multi-tier rendering fallback architectures (CDP -> resvg / skia / font-rasterization).
- **Sources & References**:
  - Implementation & Dispatch Sources:
    - [`src/tools/openmedia/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod.rs): `coerce_number_value`, `coerce_numeric_fields`, `normalize_batch_ops`, and rating normalization.
    - [`tools/openmedia/mcp/src/image_handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/openmedia/mcp/src/image_handlers.rs): `normalize_process_operation_value` in batch image processing.
    - [`tools/openmedia/mcp/src/improvement_handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/openmedia/mcp/src/improvement_handlers.rs): Flexible rating normalization (1-5, 1-10, 1-100 scales to 0.0..1.0).
    - [`tools/openmedia/process/src/lib.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/openmedia/process/src/lib.rs): `#[derive(Default)]` on `ResizeMethod` and `#[serde(default)]` on `ProcessOperation::Resize.method`.
    - [`tools/openmedia/mcp/src/lib.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/openmedia/mcp/src/lib.rs): Re-export of `normalize_process_operation_value`.
    - [`src/tools/browser/common.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common.rs): `obtain_tab_websocket_url` with fallback to `/json/list`, `google-chrome` prioritization over `obscura`.
    - [`src/tools/image_generator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator.rs): Fallback to `openmedia_mcp::html_to_image` when CDP screenshot capture encounters an error.
    - [`src/tools/html_video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/html_video.rs): Explicit tab closing by tab id.
  - Test Modules:
    - [`src/tools/openmedia/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod_tests.rs): `test_coerce_number_value`, `test_coerce_numeric_fields`, `test_normalize_batch_ops_string_and_discriminator`, `test_normalize_rating_scale`.
    - [`tools/openmedia/mcp/src/image_handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/openmedia/mcp/src/image_handlers.rs): `test_normalize_process_operation_value_tagged_and_string`.
- **Details & Metrics**:
  - Validated all 52 Media Engine tools (37 `openmedia_*` MCP tools + native core media tools `generate_image`, `render_mermaid`, `create_animated_svg`, `html_to_video`, `generate_video`).
  - 100% pass rate achieved across all suites.
  - Subagent real-world execution speeds: SVG generation ~54ms, image resizing/filtering ~150-300ms, feedback recording ~5.6s, high-resolution 1600x1600 PNG rendering ~2.8s.
  - Resolved 5 friction points and edge cases across string-to-number coercion, discriminator deserialization, SQLite check constraints, and CDP screenshot capture.
- **Verification**: Verified 11 openmedia tests in openz pass (`cargo test -p openz --lib tools::openmedia -j 1`), 29 openmedia-mcp tests pass (`cargo test -p openmedia-mcp -j 1`), exact 260 registered native tools invariant maintained (`cargo test -p openz --lib test_native_tool_registration_names -j 1`), version sync verified (`cargo test -p openz --lib version_sync_tests -j 1`), 0 clippy warnings (`cargo clippy -p openz -j 1`), and clean build (`cargo build -p openz -j 2`).

### v0.0.191
- **Ideas**:
  - Address real-world friction and edge cases discovered during autonomous headless subagent testing of the Document Processing tool suite (35 OpenDoc & DocReader native tools).
  - Robust LLM JSON serialization tolerance: LLMs frequently serialize nested JSON objects and arrays as stringified JSON strings (e.g. `sheets: "[{\"name\": ...}]"` or `variables: "{\"k\": \"v\"}"`). Add `normalize_json_param` fallback to automatically deserialize stringified JSON inputs before validating parameters in `create_xlsx`, `edit_xlsx`, `fill_template`, and `fill_pdf_form`.
  - Flexible document metadata extraction: Make `template_type` optional (`Option<String>`) in `ExtractStructuredMetadataParams`, providing a default `"general"` mode that aggregates timeline, legal, and financial entities into a single unified extraction schema.
  - Magic-byte document format sniffing: Enable content-based file format detection (`%PDF-`, `PK\x03\x04` inspecting zip inner structure for `word/`, `xl/`, `ppt/`) when document file extensions are non-standard (e.g. `.docx.bak`, `.pdf.old`, or `.tmp`), preventing `"Unsupported format: bak"` failures during document diffing and inspection.
  - Binary format protection in `compiler_auto_heal`: Guard against binary office documents and archives (`.docx`, `.xlsx`, `.pdf`, `.pptx`, `.zip`, `.png`, etc.) upfront before attempting to read them as UTF-8 source code, preventing stream decoding panics.
- **Inspirations**:
  - Headless subagent eval findings across the 35 OpenDoc and DocReader tool suites.
  - Robustness principle (Postel's Law): "Be conservative in what you send, be liberal in what you accept" applied to model tool calling parameters.
  - Unix `file(1)` libmagic file sniffing architecture for container format disambiguation.
- **Sources & References**:
  - Implementation & Dispatch Sources:
    - [`tools/opendoc/src/server.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/opendoc/src/server.rs): `normalize_json_param` helper for stringified JSON arrays/objects, optional `general` template type in `extract_structured_metadata`.
    - [`src/tools/opendoc/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod.rs): Optional `template_type: Option<String>` in `ExtractStructuredMetadataParams` with general fallback.
    - [`tools/opendoc/src/handlers/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/opendoc/src/handlers/mod.rs): `sniff_format` magic-byte fallback inspection in `load_to_ir_with_password`.
    - [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs): Binary office format guard in `run_compiler_auto_heal`.
  - Test Modules:
    - [`tools/opendoc/tests/integration.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/opendoc/tests/integration.rs): `test_sniff_format_nonstandard_extension`, `test_create_xlsx_with_stringified_json_sheets`, `test_extract_structured_metadata_general`.
    - [`src/tools/compiler_auto_heal_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal_tests.rs): `test_compiler_auto_heal_rejects_binary_document_formats`.
    - [`src/tools/opendoc/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod_tests.rs): `test_opendoc_extract_structured_metadata_optional_template_type`.
- **Details & Metrics**:
  - Resolved 5 critical edge cases identified by real-world subagents during multi-suite headless document processing.
  - Added 5 new unit and integration tests across `opendoc-mcp` (now 115 passing tests) and `openz`.
  - Bumped `opendoc-mcp` to `v0.0.13` and `openz` to `v0.0.191`.
- **Verification**: Verified 115 opendoc tests pass (`cargo test -p opendoc-mcp -j 1`), 4 compiler auto-heal tests pass (`cargo test -p openz --lib tools::compiler_auto_heal -j 1`), 2 opendoc tests in openz pass (`cargo test -p openz --lib test_opendoc -j 1`), exact 260 registered native tools invariant maintained (`cargo test -p openz --lib test_native_tool_registration_names -j 1`), 0 clippy warnings across workspace (`cargo clippy -p openz -j 1`), and release version sync verified (`cargo test -p openz --lib version_sync_tests -j 1`).

### v0.0.190
- **Ideas**:
  - Eliminate the artificial 16k token context bottleneck in OpenZ when running large-context models (e.g. MiniMax 204.8k, Claude 3.5 Sonnet 200k, Gemini 1M-2M, DeepSeek 1M, GPT-4o 128k).
  - Replace hardcoded static character and message clamps (the 64,000-character clamp in `resolve_prompt_budget`, the 32,000-character fallback in `build.rs`, the 4,000-character tool clamp in `transcript.rs`, and rigid 120-message compaction) with a unified, proportional dynamic context budgeting engine.
  - Implement `DynamicContextRegistry` providing a 4-tier model context resolution hierarchy: user config override (`context_limit`), dynamic JSON catalog (`~/.openz/models.json`), built-in pattern catalog, and safe fallback (128,000 tokens).
  - Implement proportional percentage token budgeting in `AgentDefaults`: `prompt_budget_ratio` (default 25%), `tool_output_ratio` (default 8%), `compaction_threshold_ratio` (default 80%), and `keep_recent_ratio` (default 20%).
  - Token-pressure compaction: Compaction in `compact.rs` now triggers dynamically based on estimated token pressure against the active model's capacity rather than a static message count.
  - Hermes Pre-Compression Hook: Extracts and consolidates critical decisions, preferences, and file modification paths into persistent memory *before* pruning or summarizing older messages.
  - Structure-preserving compactor: Eliminates destructive truncation of JSON arrays (which previously dropped items 2..N), retaining head and tail elements, item counts, and schema keys.
  - Unify the CLI TUI status bar model window resolution with `DynamicContextRegistry`, eliminating 40 lines of duplicate pattern matching.
- **Inspirations**:
  - Nous Research Hermes Agent (dual-layer context engine, pre-compression memory hook `on_pre_compress`).
  - Pi Agent (@earendil-works / Pi-dev) proportional token knobs (`reserveTokens`, `keepRecentTokens`).
  - Prime Agent (Prime Intellect RLM variable context architecture).
  - Academic research: Verma (2026) Active Context Compression, ACM (Li et al., 2026), ACON (2025/2026), EMNLP 2026 Context Compression Survey (Pre-compression Decision Error and Post-compression Access Failure), and Liu et al. (2023) "Lost in the Middle".
- **Sources & References**:
  - Implementation & Dispatch Sources:
    - [`src/providers/context_registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/context_registry.rs): `DynamicContextRegistry`, 4-tier context resolution, dynamic models.json loader, and proportional budgeting.
    - [`src/config/schema.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs): `prompt_budget_ratio`, `tool_output_ratio`, `compaction_threshold_ratio`, and `keep_recent_ratio` fields.
    - [`src/agent/agent_loop/build.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs): Dynamic prompt budget calculation without 64k character clamp.
    - [`src/agent/agent_loop/transcript.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript.rs): Model-aware dynamic tool output limit resolution.
    - [`src/agent/agent_loop/compact.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/compact.rs): Token-pressure compaction triggering and Hermes pre-compression memory preservation.
    - [`src/agent/context_compactor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/context_compactor.rs): Structure-preserving JSON compaction and log formatting.
    - [`src/channels/cli/render.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render.rs): Unified status bar context limit lookup.
  - Test Modules:
    - [`src/providers/context_registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/context_registry_tests.rs): Unit tests for 4-tier resolution, budget scaling, and token pressure.
    - [`src/agent/agent_loop/build_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build_tests.rs): Prompt budget scaling beyond 64k chars.
    - [`src/agent/context_compactor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/context_compactor.rs): Unit tests for small and large JSON array preservation.
  - Planning & Specifications:
    - Spec: [`docs/superpowers/specs/2026-09-17-dynamic-context-engine-design.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/specs/2026-09-17-dynamic-context-engine-design.md)
    - Plan: [`docs/superpowers/plans/2026-09-17-dynamic-context-engine.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-17-dynamic-context-engine.md)
- **Details & Metrics**:
  - `resolve_prompt_budget_chars` on MiniMax 204.8k now resolves to 204,800 characters (~51,200 tokens) vs old 64,000 char clamp.
  - `resolve_tool_output_limit_chars` on MiniMax 204.8k resolves to 65,536 characters (~16,384 tokens) vs old 4,000 char clamp.
  - Eliminated 40 lines of duplicated pattern-matching code from `src/channels/cli/render.rs`.
  - Added 10 new unit tests across `context_registry_tests`, `build_tests`, and `context_compactor`.
- **Verification**: Verified 7 context registry unit tests pass (`cargo test -p openz --lib providers::context_registry_tests -j 1`), 3 context compactor tests pass (`cargo test -p openz --lib agent::context_compactor::tests -j 1`), exact 260 registered native tools invariant maintained (`cargo test --lib -j 1 -- test_native_tool_registration_names`), 0 clippy warnings across workspace (`cargo clippy -p openz -j 1`), and release version sync verified (`cargo test -p openz --lib version_sync_tests -j 1`).

### v0.0.189
- **Ideas**:
  - Introduce full headless CLI execution mode to OpenZ (`openz run "<prompt>"`, `openz exec "<prompt>"`, `openz -p "<prompt>"`, and piped stdin `cat prompt.txt | openz`), allowing autonomous AI agents to use OpenZ as an integrated subagent, enabling automated test harnesses/eval suites to exercise OpenZ end-to-end, and providing humans with non-interactive scriptability without requiring an interactive TUI terminal.
  - Multi-format output streaming pipeline supporting `--output-format <text|json|stream-json>`: clean markdown text on stdout (with structured error alerts on stderr when non-zero exit codes occur), typed JSON output payloads (`status`, `content`, `session_id`, `tools_used`, `duration_ms`, `error`, `exit_code`), and newline-delimited JSON stream events.
  - Non-interactive security enforcement: safe/read-only tools execute automatically while sensitive/mutating tools require explicit authorization via `--yes` / `-y` or inclusion in `--allowed-tools`. Unauthorized tool attempts fail fast with exit code 2 and structured diagnostics without hanging on terminal prompts.
  - Ephemeral session isolation (`cli:headless_<timestamp>_<uuid>`) preventing test and benchmark state pollution by default, while supporting explicit session persistence and thread continuation via `--session <id>` and `--continue`.
  - Scoped silent output isolation via `IS_SILENT.scope(true, ...)` suppressing extraneous MCP and runtime banners, and dynamic runtime configuration persistence preserving CLI `--max-iterations` overrides across turn-level config reloads.
- **Inspirations**:
  - Subagent invocation interfaces and headless tool standards from `claude -p`, `gh run`, and OpenCode Zen autonomous agent protocols.
  - Unix philosophy of composable pipes and predictable exit codes (0 = success, 1 = general error/timeout, 2 = security denial).
  - Non-blocking task-local security policy contexts (`tokio::task_local!`) ensuring thread-safe, non-interactive execution.
- **Sources & References**:
  - Implementation & Dispatch Sources:
    - [`src/cli/args.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/args.rs): `HeadlessArgs` definition, top-level flags, and `Command::Run` (aliased to `exec`).
    - [`src/cli/headless.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless.rs): `HeadlessRunOutput`, `HeadlessFormat`, `HeadlessSecurityPolicy`, `resolve_session_key`, `resolve_prompt`, and `execute_headless_turn`.
    - [`src/cli/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/mod.rs): Top-level `-p` routing, piped stdin detection (`!io::stdin().is_tty()`), and subcommand dispatch.
    - [`src/agent/security.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security.rs): Task-local security policy integration and non-interactive permission evaluation.
    - [`src/agent/agent_loop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/mod.rs): Dynamic config reload preserving CLI `max_tool_iterations` overrides.
  - Test Modules:
    - [`src/cli/headless_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless_tests.rs): 20 comprehensive unit tests covering flags, formats, security policies, turn execution, mock provider loops, timeouts, denials, precedence, and text formatting.
  - Planning & Specifications:
    - Spec: [`docs/superpowers/specs/2026-09-17-headless-cli-design.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/specs/2026-09-17-headless-cli-design.md)
    - Plan: [`docs/superpowers/plans/2026-09-17-headless-cli-mode.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-17-headless-cli-mode.md)
- **Details & Metrics**:
  - Added `HeadlessArgs` CLI options: `--prompt` (`-p`), `--output-format`, `--yes` (`-y`), `--allowed-tools`, `--session`, `--continue`, `--timeout-secs`, and `--max-iterations`.
  - Added `Command::Run` (`openz run`) with subcommand alias `openz exec`.
  - Added `HeadlessRunOutput` JSON serialization structure with full session and diagnostic telemetry.
  - Implemented `HeadlessSecurityPolicy` with `record_denial` and `last_denial` integration inside `SecurityGuard::ask_approval`.
  - Enforced strict exit codes: `0` (turn succeeded), `1` (general error, timeout, or turn failure), `2` (security policy blocked sensitive tool).
  - Maintained zero clippy warnings and verified all 20 headless test suites.
- **Verification**: Verified all 20 headless mode unit tests pass (`cargo test -p openz --lib cli::headless_tests -j 1`), exact 260 registered native tools invariant maintained (`cargo test -p openz --lib test_native_tool_registration_names -j 1`), 0 clippy warnings across workspace (`cargo clippy -p openz -j 1`), and release version sync verified (`cargo test -p openz --lib version_sync_tests -j 1`).

### v0.0.188
- **Ideas**:
  - Fully decompose and delete monolithic `src/channels/websocket/tests.rs` (originally 855 lines, 32 unit tests) into dedicated component-local sibling test modules colocated directly with their target domain components:
    - [`attachments_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/attachments_tests.rs): Attachment policy validation rejecting unsafe MIME types and aggregate size overflow (1 test).
    - [`turns_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/turns_tests.rs): WebSocket turn stop scoping to owner client and cross-client cancellation isolation (2 tests).
    - [`approvals_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/approvals_tests.rs): Security approval request routing, rejection of mismatched client/chat, client disconnect auto-cancellation, and targeted security rejection event construction (4 tests).
    - [`auth_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth_tests.rs): Public bind token requirements, browser origin validation against allowed/untrusted lists, CORS origin expansion, bearer/query token authorization, and custom configured origin matching (5 tests).
    - [`events_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/events_tests.rs): Orchestration lifecycle event delivery under queue pressure, WebUI chat ID normalization and matching, and dual progress/activity notice event dispatching (3 tests).
    - [`handlers_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/handlers_tests.rs): Stop command detection, model name normalization across providers, complex routing to premium models, and simple fallback routing (4 tests).
    - [`socket_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/socket_tests.rs): Request ID whitespace trimming and length bounding, and stable command acknowledgement wire framing (2 tests).
    - [`protocol_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/protocol_tests.rs): Runtime WebUI capabilities policy generation, protocol event serialization safety, and tool progress/activity notice payload validity (3 tests).
    - [`commands/cron_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/cron_tests.rs): Cron update commands (pause, resume, delete) with inventory count updates and run log querying (1 test).
    - [`commands/sessions_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/sessions_tests.rs): Session archive and delete command execution with persisted file relocation/removal (2 tests).
    - [`commands/config_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/config_tests.rs): Secret masking, gateway token validation for security modes, workspaces, channel tokens, and live configuration update broadcasts across active clients (6 tests).
  - Eliminate the 855-line monolithic `src/channels/websocket/tests.rs` entirely, cleaning up obsolete test re-exports from `src/channels/websocket/mod.rs` and achieving 100% component-local test modularity across the WebSocket gateway channel.
- **Inspirations**:
  - Modular channel architecture, localized domain assertions, and clean separation of gateway protocol, socket handling, and command families.
  - Consistent 1:1 test colocation across all major subsystems (following Phases 38 & 39 for `memory_extra` and `self_management`).
- **Sources & References**:
  - Sibling Test Modules & Implementation Sources:
    - [`src/channels/websocket/attachments.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/attachments.rs) & [`src/channels/websocket/attachments_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/attachments_tests.rs)
    - [`src/channels/websocket/turns.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/turns.rs) & [`src/channels/websocket/turns_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/turns_tests.rs)
    - [`src/channels/websocket/approvals.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/approvals.rs) & [`src/channels/websocket/approvals_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/approvals_tests.rs)
    - [`src/channels/websocket/auth.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth.rs) & [`src/channels/websocket/auth_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth_tests.rs)
    - [`src/channels/websocket/events.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/events.rs) & [`src/channels/websocket/events_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/events_tests.rs)
    - [`src/channels/websocket/handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/handlers.rs) & [`src/channels/websocket/handlers_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/handlers_tests.rs)
    - [`src/channels/websocket/socket.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/socket.rs) & [`src/channels/websocket/socket_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/socket_tests.rs)
    - [`src/channels/websocket/protocol.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/protocol.rs) & [`src/channels/websocket/protocol_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/protocol_tests.rs)
    - [`src/channels/websocket/commands/cron.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/cron.rs) & [`src/channels/websocket/commands/cron_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/cron_tests.rs)
    - [`src/channels/websocket/commands/sessions.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/sessions.rs) & [`src/channels/websocket/commands/sessions_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/sessions_tests.rs)
    - [`src/channels/websocket/commands/config.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/config.rs) & [`src/channels/websocket/commands/config_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/commands/config_tests.rs)
    - [`src/channels/websocket/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/mod.rs)
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-websocket-test-decomposition-phase40.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-websocket-test-decomposition-phase40.md)
- **Subsystem Modularization Details**:
  - **Attachments & Turns**: Extracted attachment upload and size quota assertions, turn stop cancellation isolation, and scoped owner validation.
  - **Approvals & Auth**: Extracted approval lifecycle verification, cross-client isolation, origin/token validation, and CORS expansion.
  - **Events & Handlers**: Extracted async event publishing under backpressure, progress update dual dispatching, model routing logic, and stop command checks.
  - **Socket & Protocol**: Extracted wire format validation, request bounding, command ack framing, and capability payload generation.
  - **Commands (Cron, Sessions, Config)**: Extracted cron job status management, session persistence deletion/archival, and configuration broadcasts with secret redaction.
  - **Monolith Elimination**: Completely deleted `src/channels/websocket/tests.rs` (855 lines) with zero test regression (all 33 unit tests pass).
- **Verification**: Verified all 33 WebSocket channel unit tests pass (`cargo test -p openz --lib channels::websocket`), 0 clippy warnings across the workspace, and exact 260 registered native tools invariant maintained.

### v0.0.187
- **Ideas**:
  - Fully decompose and delete monolithic `src/tools/self_management/tests.rs` (originally 682 lines, 14 unit tests) into 8 dedicated 1:1 component-local sibling test modules colocated directly with their target self-management tools:
    - [`catalog_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/catalog_tests.rs): `ToolCatalogTool` metadata exposure, domain filtering, example formats, and resource policy checks (2 tests).
    - [`inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/inventory_tests.rs): `OpenZInventoryTool` live binary capability introspection, runtime identity, and model capability verification (1 test).
    - [`diagnostics_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/diagnostics_tests.rs): `DiagnoseSystemTool` system stats, directory and database health checks, `DiagnoseToolTool` execution, and mock argument normalization (4 tests).
    - [`scope_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/scope_tests.rs): `RequestToolScopeTool` structured requests, and `OptimizeToolScopeTool` dynamic tool prefix filtering and restoration (2 tests).
    - [`skills_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/skills_tests.rs): `CurateSkillTool` full CRUD lifecycle (add, list, delete) against SQLite database (1 test).
    - [`config_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/config_tests.rs): `ManageConfigTool` view/update/credential storage, schema validation, and `redact_secrets` nested credential redaction (2 tests).
    - [`sessions_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/sessions_tests.rs): `ManageSessionsTool` lifecycle (list, archive, delete, prune) guarded with `TestEnvLock` synchronization (1 test).
    - [`backups_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/backups_tests.rs): `ManageBackupsTool` complete backup lifecycle (create, list, restore, delete) (1 test).
  - Eliminate the 682-line monolithic `src/tools/self_management/tests.rs` completely, achieving 100% 1:1 sibling unit test colocation across all 8 self-management source files.
- **Inspirations**:
  - Clean Architecture, high cohesion, and component-colocated test ownership.
  - Granular lifecycle validation for configuration, session archives, and operational tool diagnostics.
- **Sources & References**:
  - Sibling Test Modules & Implementation Sources:
    - [`src/tools/self_management/catalog.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/catalog.rs) & [`src/tools/self_management/catalog_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/catalog_tests.rs)
    - [`src/tools/self_management/inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/inventory.rs) & [`src/tools/self_management/inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/inventory_tests.rs)
    - [`src/tools/self_management/diagnostics.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/diagnostics.rs) & [`src/tools/self_management/diagnostics_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/diagnostics_tests.rs)
    - [`src/tools/self_management/scope.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/scope.rs) & [`src/tools/self_management/scope_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/scope_tests.rs)
    - [`src/tools/self_management/skills.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/skills.rs) & [`src/tools/self_management/skills_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/skills_tests.rs)
    - [`src/tools/self_management/config.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/config.rs) & [`src/tools/self_management/config_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/config_tests.rs)
    - [`src/tools/self_management/sessions.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/sessions.rs) & [`src/tools/self_management/sessions_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/sessions_tests.rs)
    - [`src/tools/self_management/backups.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/backups.rs) & [`src/tools/self_management/backups_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/backups_tests.rs)
    - [`src/tools/self_management/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/mod.rs)
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-self-management-test-decomposition-phase39.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-self-management-test-decomposition-phase39.md)
- **Subsystem Modularization Details**:
  - **Catalog & Inventory Tests**: Extracted tool metadata queries, exposure filters, prompt-aware routing scores, resource policy block decisions, and live binary identity introspection.
  - **Diagnostics & Scope Tests**: Extracted tool health diagnostics, mock argument normalization, directory and database health checks, structured scope requests, and prefix filtering/restoration.
  - **Skills & Config Tests**: Extracted SQLite skill CRUD operations, configuration parameter updates, credential merging with secret redaction, and nested key scrubbing.
  - **Sessions & Backups Tests**: Extracted session lifecycle management (list, archive, delete, prune) and full configuration backup creation, restoration, and deletion.
  - **Monolith Deletion**: Deleted `src/tools/self_management/tests.rs` (682 lines), leaving zero monolithic test files in `src/tools/self_management/`.
- **Verification**: Verified all 14 self_management unit tests pass (`cargo test -p openz --lib tools::self_management`), 0 clippy warnings across the workspace, and exact 260 registered native tools invariant maintained.

### v0.0.186
- **Ideas**:
  - Complete full decomposition and deletion of monolithic `src/tools/memory_extra/tests.rs` (originally 1,460 lines, 36 unit tests), modularizing all unit tests into 6 dedicated component-local sibling test modules:
    - [`working_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/working_tests.rs): Working memory set/get, TTL expiration, layer promotion, and expiration eviction tests (4 tests).
    - [`episodic_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/episodic_tests.rs): Episodic reflections, execution episode logging, and tool performance recording/querying tests synchronized via graph memory test lock (3 tests).
    - [`search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/search_tests.rs): Text similarity scoring, shared team memory storage/retrieval, FTS5 full-text search, and hybrid semantic embedding search tests (4 tests).
    - [`coordinator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/coordinator_tests.rs): Coordinator write/recall/stats/forget operations, auto-importance calculations, exclusive relation resolutions, semantic similarity conflict resolutions, and semantic slot conflicts (5 tests).
    - [`codebase_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/codebase_tests.rs): Codebase indexing capturing Rust impl/trait methods, memory stats snapshot verification across coordinator and cognitive layers, session/skill/working memory layer counts, and context compression (4 tests).
    - [`facts_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/facts_tests.rs): Semantic fact extraction, multi-word and profile entity preservation, product tooling relationships, static semantic store persistence, query fact history, semantic fact invalidation, cross-layer memory forgetting (cognitive, research, shared, session, skills), poisoning attempt mitigations, contradiction deletion, proactive recall, and smart store auto-importance (16 tests).
  - Eliminate the 1,460-line monolithic `src/tools/memory_extra/tests.rs` completely, achieving 100% component-local test modularity across the extended memory subsystem.
  - Purge stale build cache in `target/debug/incremental` (freeing 28 GB of disk space) to prevent memory thrashing and swap exhaustion on resource-constrained development machines.
- **Inspirations**:
  - High cohesion, low coupling, and feature-colocated testing architectures.
  - Granular subsystem test ergonomics and deterministic SQLite test isolation via RAII locking.
  - Memory-safe, low-resource incremental compilation strategies for developer workstations.
- **Sources & References**:
  - Sibling Test Modules & Subsystem Sources:
    - [`src/tools/memory_extra/working.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/working.rs) & [`src/tools/memory_extra/working_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/working_tests.rs)
    - [`src/tools/memory_extra/episodic.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/episodic.rs) & [`src/tools/memory_extra/episodic_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/episodic_tests.rs)
    - [`src/tools/memory_extra/search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/search.rs) & [`src/tools/memory_extra/search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/search_tests.rs)
    - [`src/tools/memory_extra/coordinator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/coordinator.rs) & [`src/tools/memory_extra/coordinator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/coordinator_tests.rs)
    - [`src/tools/memory_extra/codebase.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/codebase.rs) & [`src/tools/memory_extra/codebase_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/codebase_tests.rs)
    - [`src/tools/memory_extra/facts.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/facts.rs) & [`src/tools/memory_extra/facts_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/facts_tests.rs)
    - [`src/tools/memory_extra/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/mod.rs)
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-memory-extra-test-decomposition-phase38.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-memory-extra-test-decomposition-phase38.md)
- **Subsystem Modularization Details**:
  - **Working Memory Tests**: Extracted 4 unit tests covering short-term key-value persistence, TTL expiration, promotion to long-term memory, and cleanup eviction.
  - **Episodic & Search Memory Tests**: Extracted 7 unit tests covering execution episodes, reflection logs, tool latency metrics, string similarity, shared agent memory, FTS5 queries, and vector embeddings.
  - **Coordinator & Codebase Memory Tests**: Extracted 9 unit tests covering multi-scope memory synchronization, auto-importance conflict resolution, AST structural code indexing, cross-layer statistics aggregation, and text compression.
  - **Facts & Recall Memory Tests**: Extracted 16 unit tests covering fact extraction pipelines, entity graph node generation, cross-layer forgetting (session metadata, markdown skills, sqlite tables), and proactive memory recall.
  - **Monolith Deletion**: Deleted the legacy 1,460-line `src/tools/memory_extra/tests.rs` monolith, leaving zero monolithic test files in `src/tools/memory_extra/`.
- **Verification**: Verified all 36 memory_extra unit tests pass (`cargo test -p openz --lib tools::memory_extra`), 0 clippy warnings across the workspace, and exact 260 registered native tools invariant maintained.

### v0.0.185
- **Ideas**:
  - Complete full decomposition and deletion of monolithic `src/tools/subagent/tests.rs` (originally 1,743 lines), modularizing all remaining test suites into dedicated sibling test files colocated with their target subagent subsystems:
    - [`evaluator_optimizer_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/evaluator_optimizer_tests.rs): Schema validation, evaluator loop convergence, and capability policy rejection tests.
    - [`optimize_profile_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/optimize_profile_tests.rs): Subagent fallback candidate count validation tests.
    - [`runner_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/runner_tests.rs): Prompt section construction, markdown image url wrapping, provider prefix handling, and timeout clamping tests.
    - [`delegate_task_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task_tests.rs): Delegation depth bounding, vision-preference model fallback ordering, router tool metadata, and cross-task cancellation propagation tests.
    - [`delegate_profile_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile_tests.rs): Explicit profile denial policy, cancellation propagation, and active run cancellation tests.
    - [`allowlist_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/allowlist_tests.rs): Extended with multi-tool filtering regression tests for default subagent profiles.
    - [`mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod_tests.rs): Root subagent tests covering allowlisted tools registry existence, evolution gate filters, and nested delegation rules.
  - Establish a shared test concurrency guard (`cancel_test_guard` & `TEST_CANCEL_LOCK`) in `src/tools/subagent/mod.rs` to serialize multi-threaded subagent tests that manipulate global environment variables and CLI cancellation signals.
  - Delete `src/tools/subagent/tests.rs` entirely, achieving 100% 1:1 sibling test colocation across all 11 subagent subsystem source modules with zero remaining monolithic test files.
- **Inspirations**:
  - Clean Architecture & package-by-feature modularization patterns.
  - Subsystem test isolation and elimination of monolith test files in large distributed systems.
  - Thread-safe test synchronization for process-global configuration and signal testing in Rust.
- **Sources & References**:
  - Implementation & Sibling Test Modules:
    - [`src/tools/subagent/evaluator_optimizer.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/evaluator_optimizer.rs) & [`src/tools/subagent/evaluator_optimizer_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/evaluator_optimizer_tests.rs)
    - [`src/tools/subagent/optimize_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/optimize_profile.rs) & [`src/tools/subagent/optimize_profile_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/optimize_profile_tests.rs)
    - [`src/tools/subagent/runner.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/runner.rs) & [`src/tools/subagent/runner_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/runner_tests.rs)
    - [`src/tools/subagent/delegate_task.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task.rs) & [`src/tools/subagent/delegate_task_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task_tests.rs)
    - [`src/tools/subagent/delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs) & [`src/tools/subagent/delegate_profile_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile_tests.rs)
    - [`src/tools/subagent/allowlist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/allowlist.rs) & [`src/tools/subagent/allowlist_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/allowlist_tests.rs)
    - [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs) & [`src/tools/subagent/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod_tests.rs)
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-subagent-full-test-decomposition-phase37.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-subagent-full-test-decomposition-phase37.md)
- **Subsystem Modularization Details**:
  - **Evaluator-Optimizer & Profile Optimization Tests**: Extracted schema validation, iteration loop convergence, and orchestrator policy enforcement into `evaluator_optimizer_tests.rs` (4 tests); extracted fallback setting bound validation into `optimize_profile_tests.rs` (1 test).
  - **Runner Execution & Model Fallback Tests**: Extracted prompt templating, markdown image url wrapping, provider prefix handling, and timeout clamping tests into `runner_tests.rs` (7 tests).
  - **Task & Profile Delegation Tests**: Extracted delegation depth bounding, vision-preference model fallback ordering, router tool metadata, and cross-task cancellation propagation into `delegate_task_tests.rs` (6 tests) and `delegate_profile_tests.rs` (3 tests).
  - **Subagent Root Tests & Allowlist Enhancement**: Created `mod_tests.rs` for subagent root capabilities (8 tests) and added `test_filter_tools_for_new_default_subagents` in `allowlist_tests.rs`.
  - **Shared Concurrency Guard**: Centralized `TEST_CANCEL_LOCK` and `cancel_test_guard()` in `subagent/mod.rs` to prevent race conditions during concurrent test execution when altering environment variables or cancellation signals.
  - **Monolith Deletion**: Completely removed `src/tools/subagent/tests.rs` (1,128 lines remaining from original 1,743 lines), achieving zero monolithic test files.
- **Verification**: Maintained zero clippy/compiler warnings across the workspace, verified all 61 subagent unit tests, verified `version_sync_tests`, and validated the exact 260 registered native tools invariant.

### v0.0.184
- **Ideas**:
  - Decompose monolithic subagent test suite (`src/tools/subagent/tests.rs`) into dedicated, component-local sibling unit test suites for core subagent capabilities: workspace isolation & cleanup (`workspace_tests.rs`), lifecycle status & timeout tracking (`lifecycle_tests.rs`), JSON schema validation & self-repair loops (`schema_retry_tests.rs`), cancellation token signaling (`cancellation_token_tests.rs`), and parallel research aggregation (`parallel_research_tests.rs`).
  - Eliminate monolith coupling in `subagent/tests.rs` (reducing it from 1,743 lines down to 1,128 lines, a ~35.3% line reduction) while ensuring localized encapsulation and direct private visibility for component internals.
  - Maintain 100% test coverage across subagents (all 61 tests passing sequentially), 260 registered native tools invariant, and 0 clippy warnings.
- **Inspirations**:
  - Component-colocated testing patterns and high cohesion / low coupling architectural principles.
  - Granular lifecycle observability and deterministic cancellation cascades in actor systems.
  - Micro-harness validation for iterative JSON repair loops.
- **Sources & References**:
  - Implementation & Sibling Test Suites:
    - [`src/tools/subagent/workspace.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/workspace.rs) & [`src/tools/subagent/workspace_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/workspace_tests.rs)
    - [`src/tools/subagent/lifecycle.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/lifecycle.rs) & [`src/tools/subagent/lifecycle_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/lifecycle_tests.rs)
    - [`src/tools/subagent/schema_retry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/schema_retry.rs) & [`src/tools/subagent/schema_retry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/schema_retry_tests.rs)
    - [`src/tools/subagent/cancellation_token.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/cancellation_token.rs) & [`src/tools/subagent/cancellation_token_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/cancellation_token_tests.rs)
    - [`src/tools/subagent/parallel_research.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/parallel_research.rs) & [`src/tools/subagent/parallel_research_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/parallel_research_tests.rs)
    - [`src/tools/subagent/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/tests.rs)
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-subagent-test-decomposition-phase36.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-subagent-test-decomposition-phase36.md)
- **Subsystem Modularization Details**:
  - **Workspace & Worktree Isolation Tests (`tools/subagent/workspace_tests.rs`)**: Extracted 285 lines of tests (8 unit tests) covering RAII `WorktreeGuard` unregistration on drop, forced shutdown path cleanup, stale worktree pruning, total disk size quota enforcement (oldest-first eviction), scratch workspace teardown messaging, home directory copy exclusions, and metadata merging.
  - **Lifecycle & Timeout Status Tests (`tools/subagent/lifecycle_tests.rs`)**: Extracted 116 lines of tests (7 unit tests) covering stable TUI status labels, compact CLI formatting lines, timeout classification with duration seconds, timeout JSON payload serialization, and token-based user cancellation classification.
  - **Schema Retry & JSON Repair Tests (`tools/subagent/schema_retry_tests.rs`)**: Extracted 146 lines of tests (7 unit tests) covering fenced markdown JSON extraction, invalid JSON schema retry limits, schema mismatch auto-recovery, and terminal error propagation.
  - **Cancellation Token Signal Tests (`tools/subagent/cancellation_token_tests.rs`)**: Extracted 28 lines verifying CLI cancel signal observation (`trigger_cli_cancel`) and tokio watch channel notification.
  - **Parallel Research Aggregation Tests (`tools/subagent/parallel_research_tests.rs`)**: Extracted 55 lines of tests (3 unit tests) covering partial success aggregation shapes, aggregate flush deadline calculations, and subagent metadata routing definitions.
  - **Subagent Monolith Reduction (`tools/subagent/tests.rs`)**: Reduced `src/tools/subagent/tests.rs` from 1,743 down to 1,128 lines (~615 lines extracted, 35.3% reduction).
- **Verification**: Maintained zero clippy/compiler warnings across the workspace, verified all 61 subagent unit tests, verified `version_sync_tests`, and validated the exact 260 registered native tools invariant.

### v0.0.183
- **Ideas**:
  - Modularize subagent profile tool filtering, capability allowlists, workspace isolation requirements, and model fallback candidate resolution (`tools::subagent::allowlist`, `tools::subagent::delegate_profile`) into a dedicated domain module with sibling unit test suite.
  - Decouple static profile tool allowlists (for 30+ default subagent roles), dynamic tool pruning (e.g. interactive `send_remote_input`), workspace isolation eligibility rules, and model fallback ordering cascades from the `DelegateProfileTool` execution loop.
  - Reduce `delegate_profile.rs` by ~50% (from 595 down to 300 lines) while preserving 100% backward-compatible public re-exports and ensuring all 61 subagent unit tests pass cleanly.
- **Inspirations**:
  - Principle of Single Responsibility (SRP) and separation of policy from execution mechanisms.
  - Multi-agent role-based access control (RBAC) and capability isolation patterns.
  - Dynamic model tier routing and resilient fallback sequences.
- **Sources & References**:
  - Implementation & Tests: [`src/tools/subagent/allowlist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/allowlist.rs), [`src/tools/subagent/allowlist_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/allowlist_tests.rs), [`src/tools/subagent/delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs), [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-subagent-orchestration-modularization-phase35.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-subagent-orchestration-modularization-phase35.md).
- **Subsystem Modularization Details**:
  - **Subagent Allowlist & Policy Domain (`tools/subagent/allowlist.rs`)**: Extracted 250+ lines of static tool allowlists covering 30+ specialist profiles, `all_static_subagent_allowlist_tools()`, `filter_tools_for_subagent()`, `profile_needs_workspace()`, and `delegate_profile_models_to_try()`.
  - **Subagent Allowlist Test Suite (`tools/subagent/allowlist_tests.rs`)**: Created 129 lines of dedicated unit tests verifying tool allowlisting, interactive tool pruning, workspace policy classification, and model fallback ordering.
  - **Delegate Profile Tool Refactoring (`tools/subagent/delegate_profile.rs`)**: Reduced from 595 down to 300 lines (~49.6% line reduction) while re-exporting all allowlist items for complete compatibility.
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant. All 61 subagent unit tests passed.

### v0.0.182
- **Ideas**:
  - Complete 100% embedded unit test suite decomposition across OpenZ; decouple external MCP server wrappers, deep web research and graph query parsing, OpenDoc OCR receipt/invoice processing, and shared memory Cohere embeddings (`tools::github_mcp`, `tools::docs_mcp`, `tools::searchxyz::mod`, `tools::searchxyz::graph`, `tools::opendoc::mod`, `tools::shared_memory::embeddings`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple GitHub MCP tool registration and schema generation, documentation MCP search and fetch dispatching, SearchXyz domain extraction and crawl error handling, OpenDoc local OCR payload verification, and shared memory remote embedding endpoint resolution from operational tool execution pipelines.
  - Eliminate all remaining inline `mod tests { ... }` blocks across `src/`, achieving 100% modular test architecture where all test suites reside in dedicated `..._tests.rs` sibling files without standalone integration test linking bloat.
- **Inspirations**:
  - Model Context Protocol (MCP) server architecture and dynamic client proxy design patterns.
  - Knowledge graph retrieval, deep web search scraping, and error classification pipelines.
  - Document understanding, multimodal OCR, and tabular receipt extraction boundaries.
  - High-performance vector embeddings and semantic search routing.
- **Sources & References**:
  - Implementation & Tests: [`src/tools/github_mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_mcp.rs), [`src/tools/github_mcp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_mcp_tests.rs), [`src/tools/docs_mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/docs_mcp.rs), [`src/tools/docs_mcp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/docs_mcp_tests.rs), [`src/tools/searchxyz/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/mod.rs), [`src/tools/searchxyz/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/mod_tests.rs), [`src/tools/searchxyz/graph.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/graph.rs), [`src/tools/searchxyz/graph_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/graph_tests.rs), [`src/tools/opendoc/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod.rs), [`src/tools/opendoc/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod_tests.rs), [`src/tools/shared_memory/embeddings.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/embeddings.rs), [`src/tools/shared_memory/embeddings_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/embeddings_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-codebase-modularization-and-hardening-phase34.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-codebase-modularization-and-hardening-phase34.md).
- **Subsystem Test Suite Extractions**:
  - **GitHub MCP Wrapper (`tools/github_mcp_tests.rs`)**: Extracted 19 lines of tool metadata and schema generation tests. Reduced `github_mcp.rs` from 122 down to 104 lines (~14.8% line reduction).
  - **Docs MCP Wrapper (`tools/docs_mcp_tests.rs`)**: Extracted 10 lines of tool discovery and parameter parsing tests. Reduced `docs_mcp.rs` from 192 down to 183 lines (~4.7% line reduction).
  - **SearchXyz Tool Root (`tools/searchxyz/mod_tests.rs`)**: Extracted 54 lines of search tool metadata and query structure tests. Reduced `searchxyz/mod.rs` from 211 down to 159 lines (~24.6% line reduction).
  - **SearchXyz Graph Pipeline (`tools/searchxyz/graph_tests.rs`)**: Extracted 35 lines of graph retrieval error classification tests. Reduced `searchxyz/graph.rs` from 284 down to 251 lines (~11.6% line reduction).
  - **OpenDoc Document OCR (`tools/opendoc/mod_tests.rs`)**: Extracted 22 lines of document processing and OCR response parsing tests. Reduced `opendoc/mod.rs` from 874 down to 853 lines (~2.4% line reduction).
  - **Shared Memory Embeddings (`tools/shared_memory/embeddings_tests.rs`)**: Extracted 44 lines of Cohere / OpenAI embeddings URL resolution tests. Reduced `embeddings.rs` from 663 down to 620 lines (~6.5% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant. Achieved 100% decomposition of all embedded unit tests across the entire codebase.

### v0.0.181
- **Ideas**:
  - Modularize LLM provider multimodal vision heuristics, tool scope dynamic intent routing, package template manager diagnostics, headless animation video render planning, and social intelligence integrations (`providers::mod`, `tools::scope_engine`, `tools::onpkg`, `tools::html_video`, `tools::social_search`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple multimodal vision model string prefix parsing, toolpack whitelist computation, onpkg doctor action invocation, HTML video duration/framerate boundary guards, and Hacker News / Polymarket async API payload verification from operational execution paths.
- **Inspirations**:
  - Multimodal LLM capabilities and vision API specifications (OpenAI, Anthropic, Google Gemini, Meta Llama vision matrices).
  - Intent-driven dynamic tool scoping and policy engines for agent security.
  - Headless browser rendering pipelines (CDP framerate and time budget limits).
  - Web search APIs and public intelligence data source connectors.
- **Sources & References**:
  - Implementation & Tests: [`src/providers/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mod.rs), [`src/providers/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mod_tests.rs), [`src/tools/scope_engine.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine.rs), [`src/tools/scope_engine_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine_tests.rs), [`src/tools/onpkg.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/onpkg.rs), [`src/tools/onpkg_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/onpkg_tests.rs), [`src/tools/html_video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/html_video.rs), [`src/tools/html_video_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/html_video_tests.rs), [`src/tools/social_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/social_search.rs), [`src/tools/social_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/social_search_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-16-codebase-modularization-and-hardening-phase33.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-16-codebase-modularization-and-hardening-phase33.md).
- **Subsystem Test Suite Extractions**:
  - **Provider Vision Model Heuristics (`providers/mod_tests.rs`)**: Extracted 66 lines of vision model capability identification tests covering local MIVI, OpenAI, Anthropic, Google, Meta, and Mistral models. Reduced `providers/mod.rs` from 355 down to 291 lines (~18.0% line reduction).
  - **Tool Scope Intent Engine (`tools/scope_engine_tests.rs`)**: Extracted 43 lines of local repo read, external research, and direct answer tool pack allowance tests. Reduced `scope_engine.rs` from 156 down to 114 lines (~26.9% line reduction).
  - **Onpkg Package Manager (`tools/onpkg_tests.rs`)**: Extracted 25 lines of doctor tool execution and manifest synchronization tests. Reduced `onpkg.rs` from 523 down to 499 lines (~4.6% line reduction).
  - **HTML Video Render Planner (`tools/html_video_tests.rs`)**: Extracted 19 lines of total frame calculations, direct limit checks, and segment guidance tests. Reduced `html_video.rs` from 433 down to 415 lines (~4.2% line reduction).
  - **Social Search Integrations (`tools/social_search_tests.rs`)**: Extracted 27 lines of Hacker News and Polymarket response structure tests. Reduced `social_search.rs` from 407 down to 381 lines (~6.4% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.180
- **Ideas**:
  - Modularize logging infrastructure, log viewer TUI, Ratatui modal layouts, and CLI interactive configuration (`logs::mod`, `logs::subscriber`, `logs::tui`, `channels::ratatui::modals`, `cli::configure`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple log level and session filter argument parsing, log secret scrubbing, SQLite tail log filtering, modal centered rectangle coordinate math, and built-in provider alias base URL resolution from operational production modules.
- **Inspirations**:
  - Structured logging standards (OpenTelemetry, tracing subscriber filters).
  - Secret redaction and credential masking hygiene in observability pipelines.
  - Terminal UI layout engines (Ratatui/Crossterm centered popup geometry).
  - Provider alias dispatch and default API endpoint configuration matrices.
- **Sources & References**:
  - Implementation & Tests: [`src/logs/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/mod.rs), [`src/logs/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/mod_tests.rs), [`src/logs/subscriber.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber.rs), [`src/logs/subscriber_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber_tests.rs), [`src/logs/tui.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/tui.rs), [`src/logs/tui_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/tui_tests.rs), [`src/channels/ratatui/modals.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals.rs), [`src/channels/ratatui/modals_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals_tests.rs), [`src/cli/configure.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/configure.rs), [`src/cli/configure_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/configure_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase32.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase32.md).
- **Subsystem Test Suite Extractions**:
  - **Logging Filter Parsing (`logs/mod_tests.rs`)**: Extracted 21 lines of `LogLevelFilter` and `SessionFilter` string option parsing tests. Reduced `mod.rs` from 38 down to 19 lines (~50.0% line reduction).
  - **Log Secret Scrubbing (`logs/subscriber_tests.rs`)**: Extracted 19 lines of credential masking and token sanitization tests. Reduced `subscriber.rs` from 143 down to 126 lines (~11.9% line reduction).
  - **Log Viewer SQLite Query (`logs/tui_tests.rs`)**: Extracted 57 lines of SQLite logging, table creation, insertion, and tail log query tests. Reduced `tui.rs` from 1223 down to 1168 lines (~4.5% line reduction).
  - **Ratatui Modals Geometry (`channels/ratatui/modals_tests.rs`)**: Extracted 13 lines of centered popup rectangle coordinate and dimension tests. Reduced `modals.rs` from 331 down to 319 lines (~3.6% line reduction).
  - **CLI Provider Configuration (`cli/configure_tests.rs`)**: Extracted 41 lines of provider key update and built-in provider alias endpoint resolution tests. Reduced `configure.rs` from 997 down to 957 lines (~4.0% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.179
- **Ideas**:
  - Modularize LLM provider reliability infrastructure, transport configuration defaults, model risk heuristics, and Telegram channel concurrency/state utilities (`providers::circuit_breaker`, `providers::transport`, `providers::risk`, `channels::telegram::lock`, `channels::telegram::state`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple exponential backoff calculation, HTTP 429/5xx retryability classification, circuit breaker state machine transitions, timeout defaults, provider endpoint resolution, model risk tier heuristics, lock file creation with token hashing, and Telegram TUI active session button formatting from operational runtime logic.
- **Inspirations**:
  - Netflix Hystrix / Polly circuit breaker resilience design patterns.
  - RFC 7231 HTTP status retry semantics and exponential backoff jitter algorithms.
  - LLM risk classification matrices and curated safety boundaries.
  - POSIX file advisory locking (`flock`) process isolation patterns.
- **Sources & References**:
  - Implementation & Tests: [`src/providers/circuit_breaker.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/circuit_breaker.rs), [`src/providers/circuit_breaker_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/circuit_breaker_tests.rs), [`src/providers/transport.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/transport.rs), [`src/providers/transport_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/transport_tests.rs), [`src/providers/risk.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/risk.rs), [`src/providers/risk_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/risk_tests.rs), [`src/channels/telegram/lock.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/lock.rs), [`src/channels/telegram/lock_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/lock_tests.rs), [`src/channels/telegram/state.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/state.rs), [`src/channels/telegram/state_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/state_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase31.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase31.md).
- **Subsystem Test Suite Extractions**:
  - **Provider Circuit Breaker (`providers/circuit_breaker_tests.rs`)**: Extracted 71 lines of backoff calculation, retryable status codes, initial state, threshold trip, and manual/success reset tests. Reduced `circuit_breaker.rs` from 301 down to 232 lines (~22.9% line reduction).
  - **Provider Transport Defaults (`providers/transport_tests.rs`)**: Extracted 35 lines of HTTP timeout defaults and OpenAI/Anthropic endpoint resolution tests. Reduced `transport.rs` from 152 down to 118 lines (~22.4% line reduction).
  - **Provider Model Risk Classifier (`providers/risk_tests.rs`)**: Extracted 49 lines of unknown free model tagging, strong tier defaults, small model warnings, and experimental tier tests. Reduced `risk.rs` from 116 down to 68 lines (~41.4% line reduction).
  - **Telegram Channel Polling Lock (`channels/telegram/lock_tests.rs`)**: Extracted 41 lines of token leakage prevention in lock paths, duplicate process locking rejection, and reusable file lock tests. Reduced `lock.rs` from 82 down to 42 lines (~48.8% line reduction).
  - **Telegram Session State (`channels/telegram/state_tests.rs`)**: Extracted 37 lines of remote session key selection round-trips and button preview formatting tests. Reduced `state.rs` from 223 down to 187 lines (~16.1% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.178
- **Ideas**:
  - Modularize multi-agent workflow orchestration specifications, validation invariants, WebSocket CORS authentication rules, email channel command parsing, and CLI device configuration parsing (`orchestrator::spec`, `orchestrator::validation`, `websocket::auth`, `channels::email`, `channels::cli::device`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple multi-agent declarative workflow JSON round-trip serialization, model-friendly alias translations, dependency graph validation heuristics (unknown agents, duplicate step IDs, missing prerequisites), WebSocket origin CORS evaluation with wildcard handling, email shared `/stop` command detection, and CLI hardware device CSV string tokenization from their operational runtime structures.
- **Inspirations**:
  - Declarative DAG workflow specifications (Argo Workflows, Temporal, Airflow DAG models).
  - Web standard Cross-Origin Resource Sharing (CORS) RFC specifications and origin wildcard validation.
  - Asynchronous multi-channel messaging command protocols.
  - Unix CLI delimiter-separated device and architecture configuration parsers.
- **Sources & References**:
  - Implementation & Tests: [`src/orchestrator/spec.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/spec.rs), [`src/orchestrator/spec_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/spec_tests.rs), [`src/orchestrator/validation.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/validation.rs), [`src/orchestrator/validation_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/validation_tests.rs), [`src/channels/websocket/auth.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth.rs), [`src/channels/websocket/auth_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth_tests.rs), [`src/channels/email.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/email.rs), [`src/channels/email_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/email_tests.rs), [`src/channels/cli/device.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device.rs), [`src/channels/cli/device_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase30.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase30.md).
- **Subsystem Test Suite Extractions**:
  - **Orchestrator Workflow Specification (`orchestrator/spec_tests.rs`)**: Extracted 65 lines of JSON schema deserialization, default field fallbacks, dependency graph linking, and model alias mapping tests. Reduced `spec.rs` from 186 down to 121 lines (~34.9% line reduction).
  - **Workflow DAG Validation (`orchestrator/validation_tests.rs`)**: Extracted 58 lines of valid workflow acceptance, unknown agent rejection, duplicate step ID detection, and missing dependency enforcement tests. Reduced `validation.rs` from 137 down to 79 lines (~42.3% line reduction).
  - **WebSocket CORS Origin Authentication (`channels/websocket/auth_tests.rs`)**: Extracted 19 lines of custom origin matching, local development loopback allowing, untrusted origin rejection, and wildcard origin evaluation tests. Reduced `auth.rs` from 121 down to 102 lines (~15.7% line reduction).
  - **Email Channel Stop Command Detection (`channels/email_tests.rs`)**: Extracted 8 lines of shared channel `/stop` command string matching and general subject disambiguation tests. Reduced `email.rs` from 338 down to 330 lines (~2.4% line reduction).
  - **CLI Device Parser (`channels/cli/device_tests.rs`)**: Extracted 11 lines of comma-separated hardware string splitting, whitespace trimming, and empty token rejection tests. Reduced `device.rs` from 254 down to 243 lines (~4.3% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.177
- **Ideas**:
  - Modularize core system utilities and runtime reliability infrastructure (`core::process`, `core::sqlite`, `core::http`, `shutdown`, `model_registry`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple cross-platform shell process construction (`cmd` vs `sh`), asynchronous tokio subprocess execution, shell argument escaping heuristics, SQLite WAL/synchronous standard pragma application, shared TLS HTTP client construction with timeout constraints, cooperative SIGINT turn cancellation vs graceful process tree termination, and provider model health tracking / error message truncation from operational implementations.
- **Inspirations**:
  - POSIX & Windows command line escaping semantics (RFC/MSDN escaping specifications).
  - SQLite production WAL concurrency and pragma optimization standards (`PRAGMA busy_timeout = 5000`, `PRAGMA synchronous = NORMAL`).
  - Rustls and Tokio timeout-bounded networking patterns.
  - Unix process group session signals (`setsid`, `SIGINT`, `SIGTERM`) and clean tree teardown.
  - Circuit-breaker model health registry and error classification metrics.
- **Sources & References**:
  - Implementation & Tests: [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs), [`src/core/process_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process_tests.rs), [`src/core/sqlite.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/sqlite.rs), [`src/core/sqlite_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/sqlite_tests.rs), [`src/core/http.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http.rs), [`src/core/http_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http_tests.rs), [`src/shutdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/shutdown.rs), [`src/shutdown_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/shutdown_tests.rs), [`src/model_registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/model_registry.rs), [`src/model_registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/model_registry_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase29.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase29.md).
- **Subsystem Test Suite Extractions**:
  - **Cross-Platform Host Process (`core/process_tests.rs`)**: Extracted 37 lines of host shell command resolution (`cmd.exe` vs `sh`), async tokio output execution, and dual-OS shell argument quoting tests. Reduced `process.rs` from 103 down to 66 lines (~35.9% line reduction).
  - **SQLite Standard Pragmas (`core/sqlite_tests.rs`)**: Extracted 32 lines of in-memory database standard pragma configuration, busy timeout verification, and foreign key pragma batching tests. Reduced `sqlite.rs` from 61 down to 29 lines (~52.5% line reduction).
  - **HTTP Client Factory (`core/http_tests.rs`)**: Extracted 17 lines of default rustls HTTP client builder validation and custom timeout instantiation tests. Reduced `http.rs` from 54 down to 37 lines (~31.5% line reduction).
  - **Shutdown & Process Group Manager (`shutdown_tests.rs`)**: Extracted 35 lines of SIGINT active turn cancellation decisions, process group tracking, and background dev-server child termination tests. Reduced `shutdown.rs` from 290 down to 255 lines (~12.1% line reduction).
  - **Model Registry & Health Tracker (`model_registry_tests.rs`)**: Extracted 37 lines of stable provider model registry key generation, long error payload truncation, and success/failure/leak metric recording tests. Reduced `model_registry.rs` from 215 down to 178 lines (~17.2% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.176
- **Ideas**:
  - Modularize the core agent loop, tool argument presentation formatting, self-improvement review debounce, intent classification, and chat stream assembly subsystems (`tool_execution`, `save`, `intent`, `streaming`, `marketplace_intent`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple tool argument formatting rules (native filesystem alias matching, HTML-to-video duration/FPS cost summary, unnormalized argument inspection), background curator session spawn debounce timers, turn intent routing policies (local repo query vs live external research vs model direct vs local execution), streaming chunk argument assembly ordering, and buyer/seller marketplace disambiguation heuristics from their runtime state machine implementations.
- **Inspirations**:
  - Compiler diagnostic and AST formatter separation patterns.
  - State machine lifecycle separation in conversational AI runtimes.
  - Reactive stream chunking, token interleaving, and out-of-order tool call assembly protocols.
  - Natural language intent classification and multi-turn clarifying question design in agent frameworks.
- **Sources & References**:
  - Implementation & Tests: [`src/agent/agent_loop/tool_execution.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tool_execution.rs), [`src/agent/agent_loop/tool_execution_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tool_execution_tests.rs), [`src/agent/agent_loop/save.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/save.rs), [`src/agent/agent_loop/save_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/save_tests.rs), [`src/agent/agent_loop/intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent.rs), [`src/agent/agent_loop/intent_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent_tests.rs), [`src/agent/agent_loop/streaming.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/streaming.rs), [`src/agent/agent_loop/streaming_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/streaming_tests.rs), [`src/agent/marketplace_intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/marketplace_intent.rs), [`src/agent/marketplace_intent_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/marketplace_intent_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase28.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-15-codebase-modularization-and-hardening-phase28.md).
- **Subsystem Test Suite Extractions**:
  - **Tool Execution Formatting (`agent/agent_loop/tool_execution_tests.rs`)**: Extracted 45 lines of raw argument normalization preservation, filesystem alias inspection, and HTML-to-video duration/cost display formatting tests. Reduced `tool_execution.rs` from 518 down to 474 lines (~8.5% line reduction).
  - **Turn Save & Curator Spawn (`agent/agent_loop/save_tests.rs`)**: Extracted 28 lines of self-improvement curator prompt validation, workflow artifact preservation schemas, and rapid repeated session debounce tests. Reduced `save.rs` from 749 down to 722 lines (~3.6% line reduction).
  - **Turn Intent Classifier (`agent/agent_loop/intent_tests.rs`)**: Extracted 40 lines of local repo question detection, live external research trigger heuristics, direct model answering, and local command execution tests. Reduced `intent.rs` from 223 down to 184 lines (~17.5% line reduction).
  - **Streaming Assembly Engine (`agent/agent_loop/streaming_tests.rs`)**: Extracted 33 lines of split tool call chunk reassembly, index-ordered sequencing, and partial JSON payload completion tests. Reduced `streaming.rs` from 126 down to 94 lines (~25.4% line reduction).
  - **Marketplace Intent Disambiguation (`agent/marketplace_intent_tests.rs`)**: Extracted 73 lines of ambiguous buy/sell query detection, buyer-side/seller-side keyword classification, and clarification question generation tests. Reduced `marketplace_intent.rs` from 178 down to 106 lines (~40.4% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.175
- **Ideas**:
  - Modularize workflow automation, notifications, and sandbox tools (`compiler_auto_heal`, `telegram_send`, `wasm_sandbox`, `watcher`, `sop`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple mock LLM iterative compiler error self-healing cycles, Telegram chat ID / username target validation heuristics, WASM bytecode module execution limits, filesystem change notification monitor statuses, and stateful Standard Operating Procedure (SOP) parameter schemas from tool operational implementations.
- **Inspirations**:
  - Rust compiler JSON diagnostics (`rustc --error-format=json`) and automated compiler self-repair loops.
  - Telegram Bot API recipient identifier grammar (numeric chat IDs, supergroup IDs with `-100` prefixes, `@username` tags).
  - WebAssembly (WASM) sandboxed runtime architectures (`wasmtime`).
  - Cross-platform file change polling and notify-debouncing primitives.
  - Stateful SOP workflow engine specifications.
- **Sources & References**:
  - Implementation & Tests: [`src/tools/compiler_auto_heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs), [`src/tools/compiler_auto_heal_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal_tests.rs), [`src/tools/telegram_send.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/telegram_send.rs), [`src/tools/telegram_send_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/telegram_send_tests.rs), [`src/tools/wasm_sandbox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/wasm_sandbox.rs), [`src/tools/wasm_sandbox_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/wasm_sandbox_tests.rs), [`src/tools/watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/watcher.rs), [`src/tools/watcher_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/watcher_tests.rs), [`src/tools/sop.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sop.rs), [`src/tools/sop_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sop_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase27.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase27.md).
- **Subsystem Test Suite Extractions**:
  - **Compiler Auto-Heal Tool (`tools/compiler_auto_heal_tests.rs`)**: Extracted 58 lines of schema definition, required argument validation, and mock provider iterative compile-error fix tests. Reduced `compiler_auto_heal.rs` from 137 down to 79 lines (~42.3% line reduction).
  - **Telegram Direct Message Tool (`tools/telegram_send_tests.rs`)**: Extracted 19 lines of valid numeric/group chat IDs, username strings, and phone-number rejection heuristics. Reduced `telegram_send.rs` from 275 down to 256 lines (~6.9% line reduction).
  - **WASM Sandbox Tool (`tools/wasm_sandbox_tests.rs`)**: Extracted 18 lines of tool metadata definition, security description assertions, and non-existent bytecode error handling tests. Reduced `wasm_sandbox.rs` from 158 down to 140 lines (~11.4% line reduction).
  - **Filesystem Watcher Tool (`tools/watcher_tests.rs`)**: Extracted 17 lines of watcher lifecycle, inactive status reporting, and background thread safety tests. Reduced `watcher.rs` from 249 down to 232 lines (~6.8% line reduction).
  - **SOP Workflow Trigger (`tools/sop_tests.rs`)**: Extracted 16 lines of trigger tool metadata definition, description assertions, and JSON schema property verification tests. Reduced `sop.rs` from 74 down to 58 lines (~21.6% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.174
- **Ideas**:
  - Modularize developer utility and system introspection tools (`js_format`, `mermaid`, `network`, `system_info`, `remote`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple Biome JS/TS parser formatting evaluations, SVG flowchart graph generation assertions, local socket port availability checks, host hardware and OS architecture inspection, and cross-session remote prompt loopback detection from tool runtime structures.
- **Inspirations**:
  - Biome.js (in-process JavaScript / TypeScript AST parsing and code formatting).
  - Mermaid.js diagram generation standards and vector SVG rendering.
  - POSIX and Tokio non-blocking TCP socket binding protocols.
  - Sysinfo OS / CPU / RAM metrics extraction patterns.
  - Inter-agent message forwarding and cross-channel routing architecture.
- **Sources & References**:
  - Implementation & Tests: [`src/tools/js_format.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format.rs), [`src/tools/js_format_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format_tests.rs), [`src/tools/mermaid.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid.rs), [`src/tools/mermaid_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid_tests.rs), [`src/tools/network.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network.rs), [`src/tools/network_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network_tests.rs), [`src/tools/system_info.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info.rs), [`src/tools/system_info_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info_tests.rs), [`src/tools/remote.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote.rs), [`src/tools/remote_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase26.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase26.md).
- **Subsystem Test Suite Extractions**:
  - **JavaScript/TypeScript Formatter (`tools/js_format_tests.rs`)**: Extracted 32 lines of Biome AST syntax formatting, whitespace normalization, and invalid JavaScript syntax error reporting tests. Reduced `js_format.rs` from 113 down to 81 lines (~28.3% line reduction).
  - **Mermaid Diagram Generator (`tools/mermaid_tests.rs`)**: Extracted 28 lines of isolated temporary SVG flowchart generation, XML tag validation, and cleanup tests. Reduced `mermaid.rs` from 92 down to 64 lines (~30.4% line reduction).
  - **Network Port Inspector (`tools/network_tests.rs`)**: Extracted 18 lines of ephemeral port binding, availability inspection, and JSON status formatting tests. Reduced `network.rs` from 157 down to 139 lines (~11.5% line reduction).
  - **Host System Diagnostics (`tools/system_info_tests.rs`)**: Extracted 14 lines of hardware architecture, CPU/OS detection, and memory summary assertions. Reduced `system_info.rs` from 115 down to 101 lines (~12.2% line reduction).
  - **Remote Prompt Forwarder (`tools/remote_tests.rs`)**: Extracted 11 lines of self-target loopback rejection and direct session alias matching tests. Reduced `remote.rs` from 95 down to 84 lines (~11.6% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.173
- **Ideas**:
  - Modularize core tool execution and system integration services (`mcp`, `clipboard`, `cargo_manager`, `crawl`, `open`) by extracting embedded unit test suites into dedicated sibling test modules.
  - Decouple stdio/gRPC bridge protocol tests, headless system clipboard fallback resilience, subprocess cargo self-healing, recursive website crawler timeouts, and cross-platform URL launcher schemas from their operational tool trait implementations.
- **Inspirations**:
  - Model Context Protocol (MCP) specification for JSON-RPC 2.0 / gRPC bridging and dynamic port negotiation.
  - Rust standard library and `arboard` headless display detection protocols.
  - Cargo subprocess orchestration, JSON diagnostics stream parsing, and automated clippy lint remediation.
  - Spider-rs crawler architecture with dynamic timeout clamp guarantees and partial page retention.
  - FreeDesktop XDG / Windows ShellExecute / macOS open URL dispatch patterns.
- **Sources & References**:
  - Implementation & Tests: [`src/tools/mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp.rs), [`src/tools/mcp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_tests.rs), [`src/tools/clipboard.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard.rs), [`src/tools/clipboard_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard_tests.rs), [`src/tools/cargo_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager.rs), [`src/tools/cargo_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager_tests.rs), [`src/tools/crawl.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl.rs), [`src/tools/crawl_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl_tests.rs), [`src/tools/open.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open.rs), [`src/tools/open_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase25.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase25.md).
- **Subsystem Test Suite Extractions**:
  - **MCP Bridge Engine (`tools/mcp_tests.rs`)**: Extracted 65 lines of dynamic port discovery, TCP listener guard binding, client cache invalidation, and mock stdio bridge ping tests. Reduced `mcp.rs` from 990 down to 925 lines (~6.6% line reduction).
  - **Clipboard Integration (`tools/clipboard_tests.rs`)**: Extracted 47 lines of headless container environment resilience, clipboard set/get lifecycle, and graceful error handling tests. Reduced `clipboard.rs` from 121 down to 74 lines (~38.8% line reduction).
  - **Cargo Manager Tool (`tools/cargo_manager_tests.rs`)**: Extracted 40 lines of isolated temporary cargo project scaffolding, clippy diagnostic analysis, and error recovery tests. Reduced `cargo_manager.rs` from 354 down to 314 lines (~11.3% line reduction).
  - **Website Crawler Tool (`tools/crawl_tests.rs`)**: Extracted 38 lines of tool metadata definition, crawl timeout clamping, parameter schema inspection, and partial timeout page retention tests. Reduced `crawl.rs` from 312 down to 274 lines (~12.2% line reduction).
  - **System Open Launcher (`tools/open_tests.rs`)**: Extracted 27 lines of tool schema parsing, target URL validation, and headless CI display server fallback tests. Reduced `open.rs` from 153 down to 126 lines (~17.6% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.172
- **Ideas**:
  - Modularize core communication channels (`discord`, `telegram/commands`, `ratatui/markdown`) and configuration engines (`path_policy`, `provider_catalog`) by decoupling inline test fixtures into dedicated sibling test modules.
  - Separate platform serialization logic, path boundary enforcement rules, and command dispatch routing from test fixtures, improving code readability and compilation caching.
- **Inspirations**:
  - Discord Gateway v10 WebSocket protocol specification (heartbeat negotiation & message create dispatch).
  - Telegram Bot API bot command guidelines (safe alphanumeric naming & `/cancel` cancellation action taxonomy).
  - Ratatui / Crossterm terminal UI styling (ANSI escape sequences & syntax tree span formatting).
  - OpenZ multi-provider model routing cascade and canonical alias resolution patterns.
- **Sources & References**:
  - Implementation & Tests: [`src/config/path_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/path_policy.rs), [`src/config/path_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/path_policy_tests.rs), [`src/channels/discord.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord.rs), [`src/channels/discord_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord_tests.rs), [`src/channels/telegram/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/commands.rs), [`src/channels/telegram/commands_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/commands_tests.rs), [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs), [`src/channels/ratatui/markdown_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown_tests.rs), [`src/config/provider_catalog.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/provider_catalog.rs), [`src/config/provider_catalog_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/provider_catalog_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase24.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase24.md).
- **Subsystem Test Suite Extractions**:
  - **Path Policy Security Engine (`config/path_policy_tests.rs`)**: Extracted 63 lines of workspace boundary isolation, symlink breakout rejections, and headroom-sensitive root protection tests. Reduced `path_policy.rs` from 252 down to 189 lines (~25.0% line reduction).
  - **Discord Gateway Listener (`channels/discord_tests.rs`)**: Extracted 45 lines of stop command detection, Hello payload heartbeat deserialization, and MessageCreate event parsing tests. Reduced `discord.rs` from 450 down to 405 lines (~10.0% line reduction).
  - **Telegram Bot Commands (`channels/telegram/commands_tests.rs`)**: Extracted 45 lines of command name validation, stop/cancel/remote command classification, and keyboard payload generation tests. Reduced `commands.rs` from 521 down to 476 lines (~8.6% line reduction).
  - **Ratatui Terminal Markdown (`channels/ratatui/markdown_tests.rs`)**: Extracted 38 lines of heading span styles, inline bold/code tokenization, and bullet/numbered list parsing tests. Reduced `markdown.rs` from 255 down to 217 lines (~14.9% line reduction).
  - **Provider Catalog & Resolver (`config/provider_catalog_tests.rs`)**: Extracted 34 lines of canonical name normalization, model prefix matching, and keyword candidate heuristic tests. Reduced `provider_catalog.rs` from 334 down to 300 lines (~10.2% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.171
- **Ideas**:
  - Modularize the complete browser automation and preflight subsystem (`status`, `gsd`, `firefox`, `broker`, `common`, `obscura`) by extracting embedded unit test fixtures into dedicated sibling test modules.
  - Enforce clean separation of concerns: browser tool files strictly house the `Tool` trait, CDP/WebDriver commands, lifecycle supervision, and health diagnostics, while all test scaffolding lives in isolated `#[path = "..._tests.rs"] mod tests;` targets.
- **Inspirations**:
  - Chrome DevTools Protocol (CDP) for lightweight, direct WebSocket tab evaluation without external driver daemons.
  - Playwright / Puppeteer architecture for reliable headless browser broker fallback pipelines (`obscura` -> `firefox` -> `gsd`).
  - Mozilla Marionette / GeckoDriver HTTP WebDriver wire protocol specifications.
  - Rust modular testing pattern (`#[path = "..._tests.rs"] mod tests;`).
- **Sources & References**:
  - Implementation & Tests: [`src/tools/browser/status.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status.rs), [`src/tools/browser/status_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status_tests.rs), [`src/tools/browser/gsd.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd.rs), [`src/tools/browser/gsd_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd_tests.rs), [`src/tools/browser/firefox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox.rs), [`src/tools/browser/firefox_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox_tests.rs), [`src/tools/browser/broker.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/broker.rs), [`src/tools/browser/broker_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/broker_tests.rs), [`src/tools/browser/common.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common.rs), [`src/tools/browser/common_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common_tests.rs), [`src/tools/browser/obscura.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura.rs), [`src/tools/browser/obscura_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase23.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase23.md).
- **Subsystem Test Suite Extractions**:
  - **Browser Status & Inspection (`tools/browser/status_tests.rs`)**: Extracted 71 lines of browser health preflight, CDP port evaluation, and structured diagnostic error payload tests. Reduced `status.rs` from 391 down to 320 lines (~18.2% line reduction).
  - **GSD Playwright Automation (`tools/browser/gsd_tests.rs`)**: Extracted 67 lines of parameter aliases, last-resort description validation, receiver disconnection error recovery, and structured preflight failure tests. Reduced `gsd.rs` from 361 down to 294 lines (~18.6% line reduction).
  - **Firefox WebDriver Engine (`tools/browser/firefox_tests.rs`)**: Extracted 59 lines of attach/headless/visible execution mode validation, dedicated port routing, and missing geckodriver actionable diagnostic tests. Reduced `firefox.rs` from 482 down to 423 lines (~12.2% line reduction).
  - **Browser Broker Multiplexer (`tools/browser/broker_tests.rs`)**: Extracted 36 lines of backend priority order assertions (`obscura` -> `firefox` -> `gsd`), daemon cleanup labeling, and fallback execution tracking tests. Reduced `broker.rs` from 243 down to 207 lines (~14.8% line reduction).
  - **Browser Utilities & Obscura CDP (`tools/browser/common_tests.rs` & `obscura_tests.rs`)**: Extracted 23 lines covering default CDP port detection, safe non-existent port termination, and tool metadata schemas. Reduced `common.rs` from 195 to 182 lines (~6.7%) and `obscura.rs` from 338 to 328 lines (~3.0%).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.170
- **Ideas**:
  - Modularize core developer tools (`ast_grep`, `github`, `arguments`, `grep`, `image_generator`) by decoupling embedded test fixtures from tool trait implementations, shrinking source file footprints and isolating test-only dependencies.
  - Fortify headless browser test execution in constrained sandboxes against transient Chrome DevTools Protocol (CDP) `/json/new` socket disconnects.
- **Inspirations**:
  - `ast-grep` (AST-based pattern matching and structural search engines).
  - `ripgrep` (BurntSushi's line-oriented regex search and flag composition).
  - Chrome DevTools Protocol (CDP HTTP/WebSocket target management).
  - Rust modular testing pattern (`#[path = "..._tests.rs"] mod tests;`).
- **Sources & References**:
  - Implementation & Tests: [`src/tools/ast_grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep.rs), [`src/tools/ast_grep_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep_tests.rs), [`src/tools/github.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github.rs), [`src/tools/github_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_tests.rs), [`src/tools/arguments.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs), [`src/tools/arguments_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments_tests.rs), [`src/tools/grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep.rs), [`src/tools/grep_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep_tests.rs), [`src/tools/image_generator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator.rs), [`src/tools/image_generator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator_tests.rs).
  - Execution Plan: [`docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase22.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/docs/superpowers/plans/2026-09-14-codebase-modularization-and-hardening-phase22.md).
- **Subsystem Test Suite Extractions**:
  - **AST Grep Tool (`tools/ast_grep_tests.rs`)**: Extracted 60 lines of language parsing, query schema validation, and syntax matching unit tests. Reduced `ast_grep.rs` from 445 down to 385 lines (~13.5% line reduction).
  - **GitHub Provider Tool (`tools/github_tests.rs`)**: Extracted 56 lines of provider URL construction, parameter verification, and tool schema unit tests. Reduced `github.rs` from 519 down to 463 lines (~10.8% line reduction).
  - **Tool Arguments Parser (`tools/arguments_tests.rs`)**: Extracted 55 lines of case-insensitive alias extraction, string normalization, and integer parsing tests. Reduced `arguments.rs` from 182 down to 127 lines (~30.2% line reduction).
  - **Grep Tool (`tools/grep_tests.rs`)**: Extracted 50 lines of ripgrep argument construction, regex escape handling, and multiline matching unit tests. Reduced `grep.rs` from 344 down to 294 lines (~14.5% line reduction).
  - **Image Generator Tool (`tools/image_generator_tests.rs`)**: Extracted 53 lines of tool metadata definition and resilient headless browser CDP execution unit tests. Reduced `image_generator.rs` from 649 down to 605 lines (~6.8% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.169
- **WhatsApp Channel Test Suite Extraction (`channels/whatsapp_tests.rs`)**:
  - Extracted 73 lines of stop command detection, valid webhook subscription verification, challenge response matching, and invalid token rejection unit tests from [`src/channels/whatsapp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs) into [`src/channels/whatsapp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp_tests.rs).
  - Reduced `src/channels/whatsapp.rs` from 444 lines down to 371 lines (~16.4% line reduction).
- **Notifications Channel Test Suite Extraction (`channels/notifications_tests.rs`)**:
  - Extracted 73 lines of target normalization across Telegram/Discord/WhatsApp channels and error message credential redaction unit tests from [`src/channels/notifications.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/notifications.rs) into [`src/channels/notifications_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/notifications_tests.rs).
  - Reduced `src/channels/notifications.rs` from 324 lines down to 251 lines (~22.5% line reduction).
- **CLI Terminal Input Test Suite Extraction (`channels/cli/input_tests.rs`)**:
  - Extracted 65 lines of terminal control key encodings (Ctrl+C, Ctrl+D, Shift+Ctrl+C), printable key release rejection, and turn cancellation key matching unit tests from [`src/channels/cli/input.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/input.rs) into [`src/channels/cli/input_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/input_tests.rs).
  - Reduced `src/channels/cli/input.rs` from 827 lines down to 761 lines (~8.0% line reduction).
- **Ratatui Session Test Suite Extraction (`channels/ratatui/session_tests.rs`)**:
  - Extracted 62 lines of process liveness checking, TUI marker lifecycle persistence/purging, and last-live TUI directory detection unit tests from [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs) into [`src/channels/ratatui/session_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session_tests.rs).
  - Reduced `src/channels/ratatui/session.rs` from 217 lines down to 155 lines (~28.6% line reduction).
- **Template Compiler Test Suite Extraction (`tools/template_compiler_tests.rs`)**:
  - Extracted 62 lines of simple variable substitution, loop directive rendering, and HTML template compilation file generation unit tests from [`src/tools/template_compiler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler.rs) into [`src/tools/template_compiler_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler_tests.rs).
  - Reduced `src/tools/template_compiler.rs` from 278 lines down to 216 lines (~22.3% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.168
- **Get Logs Test Suite Extraction (`tools/get_logs_tests.rs`)**:
  - Extracted 80 lines of log database insertion, session-filtered querying, and log level filtering unit tests from [`src/tools/get_logs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs.rs) into [`src/tools/get_logs_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs_tests.rs).
  - Reduced `src/tools/get_logs.rs` from 239 lines down to 159 lines (~33.5% line reduction).
- **Manage Whitelist Test Suite Extraction (`tools/manage_whitelist_tests.rs`)**:
  - Extracted 77 lines of command prefix whitelist addition/removal, path whitelist management, and whitelist listing unit tests from [`src/tools/manage_whitelist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist.rs) into [`src/tools/manage_whitelist_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist_tests.rs).
  - Reduced `src/tools/manage_whitelist.rs` from 230 lines down to 153 lines (~33.5% line reduction).
- **Agent Events Test Suite Extraction (`agent/events_tests.rs`)**:
  - Extracted 76 lines of public vs private event visibility, reasoning compaction, and trace event filtering unit tests from [`src/agent/events.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/events.rs) into [`src/agent/events_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/events_tests.rs).
  - Reduced `src/agent/events.rs` from 207 lines down to 131 lines (~36.7% line reduction).
- **Self-Healing Core Test Suite Extraction (`core/heal_tests.rs`)**:
  - Extracted 74 lines of compile command validation, markdown code fence stripping, compile check execution, and file backup guard rollback/defusal unit tests from [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs) into [`src/core/heal_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal_tests.rs).
  - Reduced `src/core/heal.rs` from 468 lines down to 394 lines (~15.8% line reduction).
- **Doc Reader Test Suite Extraction (`tools/doc_reader_tests.rs`)**:
  - Extracted 73 lines of PDF OCR candidate detection, supported document extensions, document complexity analysis, and OCR JSON response parsing unit tests from [`src/tools/doc_reader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader.rs) into [`src/tools/doc_reader_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader_tests.rs).
  - Reduced `src/tools/doc_reader.rs` from 405 lines down to 332 lines (~18.0% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.167
- **CLI Doctor Test Suite Extraction (`cli/doctor_tests.rs`)**:
  - Extracted 85 lines of diagnostic health check, secret pattern scrubbing, and target cache cleaning unit tests from [`src/cli/doctor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs) into [`src/cli/doctor_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor_tests.rs).
  - Reduced `src/cli/doctor.rs` from 602 lines down to 516 lines (~14.3% line reduction).
- **CLI Render Test Suite Extraction (`channels/cli/render_tests.rs`)**:
  - Extracted 85 lines of multiline prompt wrapping, table row parsing, text wrapping, and horizontal rule detection unit tests from [`src/channels/cli/render.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render.rs) into [`src/channels/cli/render_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render_tests.rs).
  - Reduced `src/channels/cli/render.rs` from 1,209 lines down to 1,123 lines (~7.1% line reduction).
- **Research Policy Test Suite Extraction (`agent/agent_loop/research_policy_tests.rs`)**:
  - Extracted 85 lines of live research intent parsing, budget bounding, failure classification, and link analysis unit tests from [`src/agent/agent_loop/research_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/research_policy.rs) into [`src/agent/agent_loop/research_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/research_policy_tests.rs).
  - Reduced `src/agent/agent_loop/research_policy.rs` from 333 lines down to 247 lines (~25.8% line reduction).
- **Shared Memory Workflows Test Suite Extraction (`tools/shared_memory/workflows_tests.rs`)**:
  - Extracted 82 lines of workflow search relevance filtering, rank scoring, and run execution recording unit tests from [`src/tools/shared_memory/workflows.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/workflows.rs) into [`src/tools/shared_memory/workflows_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/workflows_tests.rs).
  - Reduced `src/tools/shared_memory/workflows.rs` from 462 lines down to 379 lines (~18.0% line reduction).
- **Video Generator Test Suite Extraction (`tools/video_tests.rs`)**:
  - Extracted 80 lines of Wavyte programmatic video composition, animation timeline, and video generation unit tests from [`src/tools/video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video.rs) into [`src/tools/video_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video_tests.rs).
  - Reduced `src/tools/video.rs` from 210 lines down to 129 lines (~38.6% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.166
- **Code Outline Test Suite Extraction (`tools/outline_tests.rs`)**:
  - Extracted 93 lines of structural code symbol extraction, TypeScript/JavaScript interface/class/function extraction, and file parsing unit tests from [`src/tools/outline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline.rs) into [`src/tools/outline_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline_tests.rs).
  - Reduced `src/tools/outline.rs` from 382 lines down to 289 lines (~24.3% line reduction).
- **MCP Manager Test Suite Extraction (`tools/mcp_manager_tests.rs`)**:
  - Extracted 89 lines of MCP server configuration lifecycle, add/list/enable/disable/remove operations, and confirmation validation unit tests from [`src/tools/mcp_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager.rs) into [`src/tools/mcp_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager_tests.rs).
  - Reduced `src/tools/mcp_manager.rs` from 277 lines down to 188 lines (~32.1% line reduction).
- **Shell Executor Test Suite Extraction (`tools/shell_tests.rs`)**:
  - Extracted 87 lines of shell argument parsing, WASM file lookup across workspace/repo paths, and sandboxed subprocess execution unit tests from [`src/tools/shell.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs) into [`src/tools/shell_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell_tests.rs).
  - Reduced `src/tools/shell.rs` from 988 lines down to 901 lines (~8.8% line reduction).
- **Semantic Search Test Suite Extraction (`tools/semantic_search_tests.rs`)**:
  - Extracted 85 lines of SQLite embedding cache storage, chunk retrieval, and deleted file cache pruning unit tests from [`src/tools/semantic_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search.rs) into [`src/tools/semantic_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search_tests.rs).
  - Reduced `src/tools/semantic_search.rs` from 430 lines down to 345 lines (~19.8% line reduction).
- **Git Manager Test Suite Extraction (`tools/git_manager_tests.rs`)**:
  - Extracted 85 lines of Git repo initialization, local configuration, status inspection, staging, committing, and log retrieval unit tests from [`src/tools/git_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager.rs) into [`src/tools/git_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager_tests.rs).
  - Reduced `src/tools/git_manager.rs` from 208 lines down to 123 lines (~40.9% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.165
- **Device Inventory Test Suite Extraction (`tools/device_inventory_tests.rs`)**:
  - Extracted 118 lines of device capability inspection, display resolution detection, battery monitoring, and audio device probing unit tests from [`src/tools/device_inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory.rs) into [`src/tools/device_inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory_tests.rs).
  - Reduced `src/tools/device_inventory.rs` from 575 lines down to 457 lines (~20.5% line reduction).
- **Core Secrets Test Suite Extraction (`core/secrets_tests.rs`)**:
  - Extracted 115 lines of secret pattern normalization, multi-secret redaction, regex scrubbing, and credential masking unit tests from [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs) into [`src/core/secrets_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets_tests.rs).
  - Reduced `src/core/secrets.rs` from 495 lines down to 380 lines (~23.2% line reduction).
- **OpenAI Provider Test Suite Extraction (`providers/openai_tests.rs`)**:
  - Extracted 108 lines of OpenAI chat payload serialization, tool response parsing, error handling, and model routing unit tests from [`src/providers/openai.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/openai.rs) into [`src/providers/openai_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/openai_tests.rs).
  - Cleaned up mid-file test block placement and reduced `src/providers/openai.rs` from 888 lines down to 780 lines (~12.2% line reduction).
- **Mock Provider Test Suite Extraction (`providers/mock_tests.rs`)**:
  - Extracted 100 lines of mock response sequences, error injection, tool call synthesis, and call counter tracking unit tests from [`src/providers/mock.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mock.rs) into [`src/providers/mock_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mock_tests.rs).
  - Reduced `src/providers/mock.rs` from 289 lines down to 189 lines (~34.6% line reduction).
- **Source Ledger Test Suite Extraction (`agent/source_ledger_tests.rs`)**:
  - Extracted 94 lines of live claim confidence computation, URL normalization, failure tracking, and nested tool result recording unit tests from [`src/agent/source_ledger.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/source_ledger.rs) into [`src/agent/source_ledger_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/source_ledger_tests.rs).
  - Reduced `src/agent/source_ledger.rs` from 305 lines down to 211 lines (~30.8% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.164
- **SVG Animator Test Suite Extraction (`tools/svg_animator_tests.rs`)**:
  - Extracted 138 lines of animated SVG creation, path drawing animation, gradient definition, and raw SVG injection unit tests from [`src/tools/svg_animator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator.rs) into [`src/tools/svg_animator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator_tests.rs).
  - Reduced `src/tools/svg_animator.rs` from 1,158 lines down to 1,020 lines (~11.9% line reduction).
- **OpenMedia Hub Test Suite Extraction (`tools/openmedia/mod_tests.rs`)**:
  - Extracted 136 lines of media argument normalization, SVG alias mapping, schema example validation, raw scene wrapping, and gRPC ping unit tests from [`src/tools/openmedia/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod.rs) into [`src/tools/openmedia/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod_tests.rs).
  - Reduced `src/tools/openmedia/mod.rs` from 899 lines down to 763 lines (~15.1% line reduction).
- **Terminal Style Engine Test Suite Extraction (`agent/style/mod_tests.rs`)**:
  - Extracted 134 lines of clean tool name mapping, nested delegation depth tree prefix formatting, spinner message construction, and ANSI strip unit tests from [`src/agent/style/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/style/mod.rs) into [`src/agent/style/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/style/mod_tests.rs).
  - Reduced `src/agent/style/mod.rs` from 835 lines down to 701 lines (~16.1% line reduction).
- **Cron Engine Test Suite Extraction (`cron/mod_tests.rs`)**:
  - Extracted 123 lines of cron schedule string duration parsing, wall-clock next run computation, deserialization defaults, and run history record logging unit tests from [`src/cron/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/mod.rs) into [`src/cron/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/mod_tests.rs).
  - Reduced `src/cron/mod.rs` from 456 lines down to 333 lines (~27.0% line reduction).
- **Task Manager Test Suite Extraction & Concurrency Hardening (`tools/task_manager_tests.rs`)**:
  - Extracted 119 lines of managed task registration, turn-end cleanup, external process isolation, and TTL expiration unit tests from [`src/tools/task_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager.rs) into [`src/tools/task_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager_tests.rs).
  - Added thread-safe synchronization guard (`TEST_LOCK`) preventing test race conditions across parallel test workers against the static registry.
  - Reduced `src/tools/task_manager.rs` from 467 lines down to 348 lines (~25.5% line reduction).
- **Hardware Protection Configuration (`.cargo/config.toml`)**:
  - Added repository-level `.cargo/config.toml` capping compilation to 2 concurrent jobs to protect laptops from memory saturation, swap thrashing, and test crashes.
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.163
- **Core Inventory Test Suite Extraction (`core/inventory_tests.rs`)**:
  - Extracted 146 lines of runtime path inventory, subagent capability inspection, vision support classification, and session fixture grouping unit tests from [`src/core/inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/inventory.rs) into [`src/core/inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/inventory_tests.rs).
  - Reduced `src/core/inventory.rs` from 759 lines down to 614 lines (~19.1% line reduction).
- **Agent Loop Transcript Test Suite Extraction (`agent/agent_loop/transcript_tests.rs`)**:
  - Extracted 140 lines of oversized tool output persistence, safe directory path handling, retrieval passthrough, and assistant tool-call appending unit tests from [`src/agent/agent_loop/transcript.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript.rs) into [`src/agent/agent_loop/transcript_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript_tests.rs).
  - Reduced `src/agent/agent_loop/transcript.rs` from 321 lines down to 183 lines (~43.0% line reduction).
- **Database Inspector & Writer Test Suite Extraction (`tools/db_inspector_tests.rs`)**:
  - Extracted 137 lines of in-process SQLite table creation, query validation, mutating query rejection, and path traversal prevention unit tests from [`src/tools/db_inspector.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector.rs) into [`src/tools/db_inspector_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector_tests.rs).
  - Reduced `src/tools/db_inspector.rs` from 401 lines down to 265 lines (~33.9% line reduction).
- **Model Preferences Test Suite Extraction (`providers/model_prefs_tests.rs`)**:
  - Extracted 133 lines of recent model tracking, favorite toggling, bounded history pruning, atomic file swap, and cleanup error handling unit tests from [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs) into [`src/providers/model_prefs_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs_tests.rs).
  - Reduced `src/providers/model_prefs.rs` from 262 lines down to 130 lines (~50.4% line reduction).
- **Config Watcher Test Suite Extraction (`config/watcher_tests.rs`)**:
  - Extracted 128 lines of live config hot-reloading, redundant write debouncing, and malformed JSON recovery unit tests from [`src/config/watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/watcher.rs) into [`src/config/watcher_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/watcher_tests.rs).
  - Reduced `src/config/watcher.rs` from 294 lines down to 167 lines (~43.2% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.162
- **Orchestrator Workflow Tool Test Suite Extraction (`tools/orchestrator_tests.rs`)**:
  - Extracted 175 lines of workflow spec validation, tool filtering, and capability policy governance unit tests from [`src/tools/orchestrator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator.rs) into [`src/tools/orchestrator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator_tests.rs).
  - Cleaned up misplaced mid-file test declarations, reducing `src/tools/orchestrator.rs` from 574 lines down to 401 lines (~30.1% line reduction).
- **Grounding Classification Test Suite Extraction (`grounding_tests.rs`)**:
  - Extracted 171 lines of grounding heuristic classification, research step policies, and evolution suppression unit tests from [`src/grounding.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/grounding.rs) into [`src/grounding_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/grounding_tests.rs).
  - Reduced `src/grounding.rs` from 445 lines down to 274 lines (~38.4% line reduction).
- **Ratatui Terminal UI App Test Suite Extraction (`channels/ratatui/app_tests.rs`)**:
  - Extracted 165 lines of scrolling behavior, sync session notice preservation, git branch caching, and message parsing unit tests from [`src/channels/ratatui/app.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/app.rs) into [`src/channels/ratatui/app_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/app_tests.rs).
  - Reduced `src/channels/ratatui/app.rs` from 586 lines down to 421 lines (~28.2% line reduction).
- **Web Fetch Tool Test Suite Extraction (`tools/web_tests.rs`)**:
  - Extracted 161 lines of HTML tree node parsing, SSRF safe IP validation, HTTP caching heuristics, and browser render broker unit tests from [`src/tools/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web.rs) into [`src/tools/web_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_tests.rs).
  - Reduced `src/tools/web.rs` from 942 lines down to 781 lines (~17.1% line reduction).
- **Cron Management Tools Test Suite Extraction (`tools/cron_tests.rs`)**:
  - Extracted 153 lines of pause/resume job toggling, execution guard, and structured run log retrieval unit tests from [`src/tools/cron.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron.rs) into [`src/tools/cron_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron_tests.rs).
  - Reduced `src/tools/cron.rs` from 600 lines down to 447 lines (~25.5% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.161
- **Activity Tracker Test Suite Decomposition (`agent/activity_tests.rs`)**:
  - Extracted 229 lines of embedded activity, daily rollup, and event tracking unit tests from [`src/agent/activity.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/activity.rs) into [`src/agent/activity_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/activity_tests.rs).
  - Reduced `src/agent/activity.rs` from 707 lines down to 480 lines (~32.1% line reduction).
- **Cron Scheduler Test Suite Extraction (`cron/scheduler_tests.rs`)**:
  - Extracted 222 lines of job dispatch, cron schedule parsing, and execution history unit tests from [`src/cron/scheduler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/scheduler.rs) into [`src/cron/scheduler_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/scheduler_tests.rs).
  - Reduced `src/cron/scheduler.rs` from 624 lines down to 404 lines (~35.3% line reduction).
- **Resource Policy Test Suite Extraction (`tools/resource_policy_tests.rs`)**:
  - Extracted 199 lines of resource envelope, budget enforcement, and capability governance unit tests from [`src/tools/resource_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy.rs) into [`src/tools/resource_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy_tests.rs).
  - Reduced `src/tools/resource_policy.rs` from 407 lines down to 211 lines (~48.2% line reduction).
- **SearchXyz Web Search Suite Extraction (`tools/searchxyz/web_tests.rs`)**:
  - Extracted 229 lines of browser search URL building, DOM extraction, cooldown tracking, and doctor report unit tests from [`src/tools/searchxyz/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web.rs) into [`src/tools/searchxyz/web_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web_tests.rs).
  - Reduced `src/tools/searchxyz/web.rs` from 1,443 lines down to 1,216 lines (~15.7% line reduction).
- **Web Search Tool Test Suite Extraction (`tools/web_search_tests.rs`)**:
  - Extracted 195 lines of search policy resolution, auto-reading heuristics, failure diagnostics, and native rescue unit tests from [`src/tools/web_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search.rs) into [`src/tools/web_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search_tests.rs).
  - Reduced `src/tools/web_search.rs` from 968 lines down to 774 lines (~20.0% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.160
- **Config Schema Test Suite Decomposition (`config/schema_tests.rs`)**:
  - Extracted 380 lines of embedded unit tests (`provider_resolution_tests`, general tests, and `layered_tool_routing_tests`) out of [`src/config/schema.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs) into [`src/config/schema_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema_tests.rs).
  - Slashed `src/config/schema.rs` from 1,496 lines down to 1,115 lines (~25.5% line reduction), isolating data models and serde structures.
- **Config Loader Test Suite Extraction (`config/loader_tests.rs`)**:
  - Extracted 358 lines of configuration caching, path resolution, and legacy alias rewrite tests from [`src/config/loader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs) into [`src/config/loader_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader_tests.rs).
  - Reduced `src/config/loader.rs` from 928 lines down to 572 lines (~38.4% line reduction).
- **SOP Workflow Engine Test Suite Extraction (`sop/tests.rs`)**:
  - Extracted 265 lines of template substitution, validation, and simulated workflow lifecycle tests from [`src/sop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/sop/mod.rs) into [`src/sop/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/sop/tests.rs).
  - Reduced `src/sop/mod.rs` from 677 lines down to 412 lines (~39.1% line reduction).
- **Filesystem Tools Test Suite Extraction (`tools/filesystem_tests.rs`)**:
  - Extracted 247 lines of source-integrity, symlink restriction, and non-destructive transactional rollback tests from [`src/tools/filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs) into [`src/tools/filesystem_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem_tests.rs).
  - Reduced `src/tools/filesystem.rs` from 784 lines down to 539 lines (~31.3% line reduction).
- **Agent Loop Core Test Suite Extraction (`agent/agent_loop/tests.rs`)**:
  - Extracted 236 lines of turn cancellation context, session override mappings, and provider timeout/cancellation tests from [`src/agent/agent_loop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/mod.rs) into [`src/agent/agent_loop/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tests.rs).
  - Hardened parallel test execution against cross-test CLI cancellation interference with dedicated test locking.
  - Reduced `src/agent/agent_loop/mod.rs` from 922 lines down to 686 lines (~25.6% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.159
- **CLI Builder Test Suite Extraction (`cli/builder_tests.rs`)**:
  - Extracted 578 lines of embedded unit tests from [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs) into [`src/cli/builder_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder_tests.rs).
  - Reduced `src/cli/builder.rs` from 615 lines down to 40 lines (~93.5% line reduction), establishing an ultra-clean construction facade.
- **CLI Tools Registration Test Suite Extraction (`cli/tools_tests.rs`)**:
  - Extracted 552 lines of embedded unit tests from [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs) into [`src/cli/tools_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools_tests.rs).
  - Reduced `src/cli/tools.rs` from 588 lines down to 37 lines (~93.7% line reduction).
- **Agent Loop Control Test Suite Extraction (`agent/agent_loop/loop_control_tests.rs`)**:
  - Extracted 397 lines of embedded unit tests from [`src/agent/agent_loop/loop_control.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/loop_control.rs) into [`src/agent/agent_loop/loop_control_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/loop_control_tests.rs).
  - Grouped production loop-control heuristics and self-healing error recovery, reducing `src/agent/agent_loop/loop_control.rs` from 779 lines down to 385 lines (~50.6% line reduction).
- **Provider Resolver Test Suite Extraction (`providers/resolver_tests.rs`)**:
  - Extracted 446 lines of embedded unit tests from [`src/providers/resolver.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/resolver.rs) into [`src/providers/resolver_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/resolver_tests.rs).
  - Reduced `src/providers/resolver.rs` from 750 lines down to 305 lines (~59.3% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.158
- **Subagent Execution Runner Extraction (`tools/subagent/runner.rs`)**:
  - Extracted 872 lines of subagent orchestration, workspace isolation, token cancellation drop guards (`CancelOnDrop`, `WorkspaceIsolation`, `create_workspace_isolation`), and attempt execution out of [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs) into a dedicated module [`src/tools/subagent/runner.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/runner.rs).
  - Slashed `src/tools/subagent/mod.rs` from 921 lines down to 60 lines (~93.5% line reduction) with clean re-exports via `pub mod runner; pub use runner::*;`.
- **Session Test Suite Decomposition (`session_tests.rs`)**:
  - Extracted 290 lines of interleaved unit test suites (`hash_tests`, `lock_tests`, `delete_tests`, `summary_tests`) out of [`src/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session.rs) into [`src/session_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session_tests.rs).
  - Consolidated core production structures (`Message`, `Session`, `ArchivedSession`, `SessionSummary`, `SessionManager`) into an uninterrupted module.
- **Headroom Test Suite Extraction (`tools/headroom/tests.rs`)**:
  - Extracted 706 lines of embedded unit tests from [`src/tools/headroom/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/mod.rs) into [`src/tools/headroom/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/tests.rs).
  - Slashed `src/tools/headroom/mod.rs` from 739 lines down to 33 lines (~95.5% line reduction).
- **Self-Management Test Suite Extraction (`tools/self_management/tests.rs`)**:
  - Extracted 687 lines of embedded unit tests from [`src/tools/self_management/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/mod.rs) into [`src/tools/self_management/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/tests.rs).
  - Slashed `src/tools/self_management/mod.rs` from 711 lines down to 26 lines (~96.3% line reduction).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.157
- **Subagent Subsystem Modularization (`src/subagents/`)**:
  - Decomposed the monolithic 1,088-line [`src/subagents/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/mod.rs) into focused domain submodules:
    - [`src/subagents/defaults.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/defaults.rs): Default profile configurations (`default_profiles()`, `DEFAULT_SUBAGENT_NAMES`, `is_default_subagent`) including orchestrator, planner, researcher, coder, and specialized profiles.
    - [`src/subagents/health.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/health.rs): Adaptive health tracking registry (`SubagentHealthRecord`, `SubagentHealthRegistry`, `record_subagent_success`, `record_subagent_failure`) and automatic fallback cascades.
    - [`src/subagents/interactive.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/interactive.rs): Interactive terminal management wizard (`run_subagent_manager`, `manage_menu`, `create_menu`, `ask_openz_to_design`, `prompt_choose_model`).
    - [`src/subagents/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/tests.rs): Comprehensive unit tests covering health metrics, default policies, and registry state.
  - Slashed `src/subagents/mod.rs` from 1,088 lines down to 210 lines (~80.7% line reduction) as a clean architectural facade with 100% backward-compatible re-exports.
- **Shared Memory Unit Test Extraction (`tools/shared_memory/`)**:
  - Extracted 365 lines of embedded unit tests from [`src/tools/shared_memory/knowledge.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/knowledge.rs) into [`src/tools/shared_memory/knowledge_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/knowledge_tests.rs), reducing `knowledge.rs` from 1,350 lines to 988 lines (~26.8% line reduction).
  - Extracted 399 lines of embedded unit tests from [`src/tools/shared_memory/auto_capture.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/auto_capture.rs) into [`src/tools/shared_memory/auto_capture_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/auto_capture_tests.rs), reducing `auto_capture.rs` from 1,214 lines to 816 lines (~32.8% line reduction).
- **Skills Subsystem Unit Test Extraction (`agent/skills_tests.rs`)**:
  - Extracted 110 lines of embedded unit tests from [`src/agent/skills.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills.rs) into [`src/agent/skills_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills_tests.rs), reducing `skills.rs` from 1,103 lines to 995 lines.
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.156
- **Tool Registry Subsystem Modularization (`tools/registry.rs`)**:
  - Extracted `ToolRegistry`, route analysis structures (`ToolRouteEntry`, `ToolRouteAnalysis`, `PendingToolScope`, `ToolRouteCacheKey`), prompt intent scoring routing, and dynamic subagent resolution out of [`src/tools/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mod.rs) into a dedicated module [`src/tools/registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs).
  - Integrated canonical registration deduplication (`insert_unique_tool`), drift detection (`static_tool_drift`), and name resolution (`resolve_static_name`) seamlessly.
- **Route Cache & Tool Routing Test Extraction (`tools/registry_tests.rs`)**:
  - Extracted 480 lines of embedded unit tests covering route caching, argument normalization, capability policy enforcement, intent scoping, and subagent lookup into [`src/tools/registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry_tests.rs).
- **Tools Subsystem Root Slashed by ~82.9% (`tools/mod.rs`)**:
  - Slashed `src/tools/mod.rs` from 1,571 lines down to 269 lines (~82.9% line reduction), transforming it into a clean, well-documented architectural facade with 100% backward-compatible re-exports via `pub use registry::*;`.
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.155
- **Extended Memory Unit Test Extraction (`tools/memory_extra/tests.rs`)**:
  - Extracted 1,463 lines of embedded unit tests from [`src/tools/memory_extra/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/mod.rs) into a dedicated test submodule [`src/tools/memory_extra/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/tests.rs).
  - Slashed `mod.rs` from 1,496 lines to 34 lines (~97.7% line reduction) while preserving all 36 unit tests and concurrency guards.
- **Orchestrator Workflow Runtime Test Extraction (`orchestrator/runtime_tests.rs`)**:
  - Extracted 801 lines of embedded unit tests from [`src/orchestrator/runtime.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/runtime.rs) into [`src/orchestrator/runtime_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/runtime_tests.rs).
  - Slashed `runtime.rs` from 1,420 lines to 620 lines (~56.3% line reduction) while maintaining all 21 workflow execution tests.
- **Channel Model Switch Command & Test Decomposition (`channels/model_switch.rs` & `channels/tests.rs`)**:
  - Extracted model switch interactive command parsing, terminal formatting, and background smoke test dispatch from [`src/channels/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs) into a dedicated submodule [`src/channels/model_switch.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/model_switch.rs) (320 lines).
  - Extracted 312 lines of embedded channel test suites (`notifications`, `stop_command`, `channel_session`, `model_switch`) into [`src/channels/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/tests.rs).
  - Slashed `src/channels/mod.rs` from 1,020 lines to 396 lines (~61.2% line reduction) with 100% backward-compatible re-exports.
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.154
- **Ratatui TUI Rendering Engine Decomposition (`channels/ratatui/ui.rs`)**:
  - Decomposed the monolithic 1,104-line Ratatui TUI renderer into focused submodules:
    - [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs): inline markdown formatting (bold, code spans), headings, horizontal rules, and bullet/numbered lists with unit test coverage.
    - [`src/channels/ratatui/timeline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/timeline.rs): conversation timeline, authentic ASCII logo banner, system metadata badges, message card rendering, and smooth auto-scrolling logic.
    - [`src/channels/ratatui/modals.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals.rs): modal dialog overlays (provider select, model select, skills, help, approval, confirmation) and geometry helpers (`centered_rect`).
  - Slashed `src/channels/ratatui/ui.rs` from 1,104 lines to 247 lines (~77.6% line reduction) coordinating top-level layout composition, elevated input dock, autocomplete dock, and status bar.
- **Process Marker & Session Mutation Extraction (`channels/ratatui/session.rs`)**:
  - Extracted process marker management (`tui_marker_dir`, `write_tui_marker_in_dir`, `remove_tui_marker_in_dir`, `process_is_alive`, `is_last_live_tui_in_dir`) and session mutation routines (`save_session_model_override`, `save_session_streaming_override`, `save_default_model_selection`, `apply_session_model_selection`, `reset_active_session`) into [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs).
  - Maintained full backward compatibility with clean `pub use session::*;` in [`src/channels/ratatui/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/mod.rs).
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.153
- **Subagent Workspace Lifecycle Modularization (`subagent/workspace.rs`)**:
  - Extracted 846 lines of workspace isolation, worktree lifecycle, disk quota management, git status filtering, and evolution review out of [`src/tools/subagent/delegate_task.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task.rs) into a dedicated module [`src/tools/subagent/workspace.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/workspace.rs).
  - Slashed `delegate_task.rs` from 1,126 lines down to 286 lines (~74.6% line reduction) while providing full backward compatibility via re-exports.
  - Decoupled [`delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs) from borrowing internal symbols from `delegate_task.rs`.
- **Prompt Builder Unit Test Extraction (`build_tests.rs`)**:
  - Extracted 426 lines of embedded unit tests from [`src/agent/agent_loop/build.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs) into a dedicated test submodule [`src/agent/agent_loop/build_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build_tests.rs).
  - Reduced `build.rs` from 1,527 lines to 1,101 lines.
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.152
- **Scoped Intent Prioritization & Pack Routing**:
  - Reordered intent detection in [`src/agent/agent_loop/intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent.rs) so local repository queries, orchestration workflows, and cron schedule intents take precedence over live web research heuristics, preventing queries like "schedule a daily web check" from being prematurely misclassified as web research.
  - Assigned pack scoping in [`src/tools/defs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/defs.rs): associated `manage_servers` with `&["core", "local_exec"]` and `workflow_memory` with `&["core", "memory"]`.
  - Aligned legacy router tests in [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs) with `ToolScopeEngine` dynamic scoring semantics.
- **CLI Channel God-File Modularization (`channels/cli/mod.rs`)**:
  - Slashed the 1,548-line [`src/channels/cli/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/mod.rs) down to 431 lines (~72% line reduction) while preserving 100% backward compatibility.
  - Extracted hardware, network, and battery device commands into [`src/channels/cli/device.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device.rs) (253 lines).
  - Extracted interactive slash command execution (`handle_slash_command`) into [`src/channels/cli/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/commands.rs) (928 lines).
- **Security Unit Test Decomposition**:
  - Extracted 580 lines of embedded unit tests from [`src/agent/security.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security.rs) into a dedicated test submodule [`src/agent/security_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security_tests.rs).
  - Reduced `src/agent/security.rs` from 1,497 lines to 922 lines.
- **Verification**: Maintained zero clippy/compiler warnings and verified the exact 260 registered native tools invariant.

### v0.0.151
- **God-File Modularization (`run/mod.rs` & `websocket/mod.rs`)**:
  - Decomposed [`src/agent/agent_loop/run/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/mod.rs) by extracting ~500 lines of embedded unit tests into a dedicated submodule [`src/agent/agent_loop/run/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/tests.rs).
  - Decomposed [`src/channels/websocket/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/mod.rs) (slashed from 1,204 lines down to ~380 lines) by extracting WebSocket connection frame handling into [`src/channels/websocket/socket.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/socket.rs) and HTTP REST endpoints (`openai_chat_completions`, `trigger_sop_handler`, `resume_sop_handler`, and `hono_log_middleware`) into [`src/channels/websocket/handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/handlers.rs).
- **Unified Compiler Auto-Healing Reflection Engine**:
  - Created [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs) centralizing compile-check command validation, markdown code fence extraction, RAII pre-edit backups (`FileBackupGuard`), git snapshots (`GitSnapshot`), and transactional reflection loops.
  - Refactored both [`CompilerAutoHealTool`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs) and [`ZenflowEditTool`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs) to delegate to `crate::core::heal`, eliminating ~150 lines of duplicate compilation, LLM prompting, and rollback logic.
- **WebSocket Event Isolation Hardening**:
  - Hardened chat-scoped client filtering in [`src/channels/websocket/events.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/events.rs) so unassociated senders no longer leak into targeted chat event broadcasts.
  - Hardened unit tests in [`src/channels/websocket/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/tests.rs) against parallel broadcast noise.
- **Verification**: Maintained zero clippy/compiler warnings and verified the 260 registered native tools invariant.

### v0.0.150
- **Modularized Logging Architecture**: Decomposed the 1,636-line `src/logs.rs` god-file into `src/logs/` submodules (`storage.rs`, `subscriber.rs`, `query.rs`, `tui.rs`, `mod.rs`) while strictly maintaining 100% backward compatibility for all callers.
- **Centralized Secret Scrubbing**: Consolidated token regex pattern matching (Telegram, OpenAI `sk-...`, partial tokens) and string redaction into [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs), eliminating duplication in `doctor` and logging.
- **Cross-Platform Shell Quoting**: Introduced `quote_shell_arg` in [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs) with proper Windows double-quote escaping and Unix single-quote escaping, resolving Windows git-add quoting failures.
- **Hardened Model Preferences Persistence**: Enhanced `save_model_prefs_at` with file `sync_all` and automatic `.tmp` file cleanup upon any write/rename error.
- **Tool Taxonomy Documentation**: Formalized native tool subsystem taxonomy in `src/tools/README.md`.

### v0.0.149
**Codebase Decoupling, Cross-Platform Shell Centralization & Provider Domain Architecture:**
- **Refactor (Cross-Platform Shell Centralization & Windows Patch Fix):** Centralized host shell command construction in `src/core/process.rs` (`host_shell_command` and `host_tokio_shell_command`), utilizing `cmd.exe /C` on Windows and `sh -c` on Unix with automatic repository CWD resolution. Replaced fragmented `sh -c` calls across `src/tools/filesystem.rs`, `src/tools/compiler_auto_heal.rs`, and `src/tools/shell.rs`. Fixed a Windows bug in `ApplyPatchTool` (`git apply --check`) which previously failed by invoking `sh` on Windows platforms without bash in PATH.
- **Refactor (HTTP Client Standardization):** Standardized disparate HTTP client initializations across `src/channels/whatsapp.rs`, `src/tools/rust_docs.rs`, `src/tools/github.rs`, `src/tools/browser/firefox.rs`, `src/tools/browser/status.rs`, and `src/channels/mod.rs` onto `crate::core::http::default_client()` and `custom_client()`, guaranteeing safe connection (10s), read (30s), and request (60s) timeouts and eliminating unbounded network hanging.
- **Feature (Configurable WebSocket CORS Origins):** Added `cors_origins: Vec<String>` to `WebSocketChannelConfig` in `src/config/schema.rs` with camelCase/snake_case aliases (`corsOrigins`, `cors_origins`). Parameterized `is_allowed_origin` in `src/channels/websocket/auth.rs` to permit explicit user-defined origins alongside loopback defaults (`http://localhost:*`, `http://127.0.0.1:*`, `tauri://localhost`), with wildcards (`*`) permitted in development mode.
- **Refactor (Identity Heuristics & Dynamic Prompt Budget):** Replaced hardcoded personal creator string matching in `src/agent/agent_loop/build.rs` with generalized heuristic checks (e.g. `who created you`, `who made you`, `your creator`) looking up dynamic session and environment metadata. Decoupled hardcoded prompt token limits by introducing `prompt_budget_limit` in `AgentDefaults` (`src/config/schema.rs`), allowing dynamic proportional scaling based on provider context windows.
- **Refactor (Provider Domain Architecture):** Relocated model preferences (`ModelPrefs`, `ModelRef`, `load_model_prefs`, `save_model_prefs`, `toggle_favorite_model`, `record_recent_model`) to `src/providers/model_prefs.rs` and model risk classification (`ModelRisk`, `classify_model_risk`) to `src/providers/risk.rs`. Exported clean interfaces from `src/providers/mod.rs` and added backward-compatible re-exports in `src/channels/mod.rs`.
- **Tests & Verification:** Verified exact 128 registered native tool invariant in `test_native_tool_registration_names`; zero compiler or clippy warnings across all workspace crates (`cargo clippy --workspace --lib -j 2`); full unit test coverage across process utilities, HTTP timeouts, CORS authorization, identity heuristics, model preferences, and risk domains.
- **Chore:** Bumped version to `v0.0.149`.

### v0.0.148
**Hardening, In-Crate Test Hygiene, Dynamic CDP Port & Build Cache Cleanup:**
- **Feature (Dual-Dispatch Progress Updates):** Updated `send_progress_update()` in `src/agent/agent_loop/tool_execution.rs` to dual-dispatch both typed `tool_progress` and `publish_activity_notice(&actual_session, "progress", "Progress", text)` so all existing and modern WebUI clients immediately display real-time tool progress in the chat view. Added unit tests for payload structure and live WebSocket broadcasting.
- **Refactor (In-Crate Integration Tests):** Migrated standalone integration test `tests/agent_loop.rs` into an in-crate test submodule under `src/agent/agent_loop/integration_tests.rs` behind `#[cfg(test)]`. Eliminates duplicate external binary linking across ~55 dependencies, preventing unnecessary build cache growth and memory pressure while keeping 100% test coverage.
- **Feature (Configurable Browser CDP Port):** Added `cdp_port: u16` (default 9222) to `BrowserConfig` in `src/config/schema.rs` with camelCase/snake_case aliases (`cdpPort`, `cdp_port`, `chrome_cdp_port`). Parameterized `html_video`, `obscura`, `image_generator`, and `InspectBrowsersTool` to dynamically resolve the configured port. Hardened `kill_browser_on_port` to inspect process command names and safely terminate only known headless browser binaries (`chrome`, `chromium`, `obscura`), avoiding accidental termination of unrelated developer tools.
- **Feature (Doctor Target Cache Purge):** Added `--clean-target` flag to `openz doctor` (`src/cli/args.rs`, `src/cli/mod.rs`, `src/cli/doctor.rs`) allowing developers to safely purge overgrown `target/` build directories and report reclaimed disk space in one command.
- **Tests & Verification:** Verified the 128 registered native tools invariant in `test_native_tool_registration_names`; all in-crate integration and browser tests pass; zero warnings across the workspace with `cargo clippy --workspace --lib -j 2`.
- **Chore:** Bumped version to `v0.0.148`.

### v0.0.147
**Workspace-Wide Zero-Warning Clippy Cleanliness & WebSocket Tool Progress Streaming:**
- **Feature (WebSocket Tool Progress Streaming):** Added typed `WsEvent::ToolProgress` to WebSocket protocol in `src/channels/websocket/protocol.rs`, allowing real-time progress emissions (`tool_progress`) correlated with chat ID, turn ID, and tool call ID. Wired `send_progress_update()` in `src/agent/agent_loop/tool_execution.rs` to automatically publish progress events to connected WebUI clients whenever a turn is in progress. Added unit test validation in `src/channels/websocket/tests.rs`.
- **Code Health (Workspace-Wide Clippy Cleanliness):** Achieved 100% zero compiler and clippy warnings across every crate in the workspace (`cargo clippy --workspace --lib -j 2`), resolving lints in `opendoc-mcp`, `searchxyz`, `openmedia-video`, `openmedia-animate`, `openmedia-svg`, `openmedia-core`, `openmedia-image`, `openmedia-improve`, `openz-docs-mcp`, and `openz-github-mcp`.
- **Optimization & Reliability:** Replaced manual pattern matching with `if let` in `searchxyz` DuckDuckGo and Google engines; hoisted regex compilations out of paragraph loops in `opendoc`; modernized XLSX cell iterations with `iter_mut().enumerate()`; derived `Default` on config structs and diffusion pipelines; used `.saturating_sub()` in image filters; and freed 63 GB of disk space from runaway build cache accumulation.
- **Tests & Verification:** Verified all 128 registered native tools in `test_native_tool_registration_names`; all 30 WebSocket protocol tests pass; full `just check openz` verified.
- **Chore:** Bumped version to `v0.0.147`.

### v0.0.146
**Zero-Warning Clippy Cleanup & Core Code Health:**
- **Code Health (Zero Clippy Warnings):** Resolved all 66 compiler and clippy warnings across the `openz` core library crate, achieving a 100% warning-free build.
- **Hardening (File Lock Semantics):** Explicitly added `.truncate(false)` on `OpenOptions` in `src/agent/agent_loop/run/turn.rs` and `src/channels/telegram/lock.rs` to satisfy `suspicious_open_options` and protect advisory file locks against destructive file truncation.
- **Refactor (Control Flow & Pattern Simplifications):** Streamlined bullet and prefix trimming in `src/agent/style/mod.rs` and `src/channels/cli/render.rs`, collapsed redundant conditional branches in `src/agent/agent_loop/tool_execution.rs` and `src/tools/memory_extra/codebase.rs`, simplified slice indexing to `strip_prefix` in `src/channels/websocket/commands/sessions.rs`, and converted nested loops to `.flatten()` and `.clamp()`.
- **Refactor (Type Aliases & Functions):** Factored deep nested channel map types into `BranchCache` in `src/channels/ratatui/app.rs`, and scoped function signatures across logging, rendering, protocol, and subagent modules.
- **Tests & Verification:** Confirmed `cargo clippy -p openz --lib` emits 0 warnings; verified invariant of 128 registered native tools in `test_native_tool_registration_names`; verified all 30 WebSocket protocol tests pass; full `just check openz` verified.
- **Chore:** Bumped version to `v0.0.146`.

### v0.0.145
**Panic Hardening, Safe WebSocket Serialization & Robust Search Backend:**
- **Hardening (Safe WebSocket Serialization):** Replaced 30 `.expect("WebSocket ... must serialize")` panics across `src/channels/websocket/protocol.rs` with `to_value_safe()` helper returning an error payload rather than crashing active axum client connections. Added unit tests for safe wire protocol event serialization in `src/channels/websocket/tests.rs`.
- **Hardening (SearchXyz Initialization & Concurrency):** Protected `reqwest::Client` builder with a graceful fallback to `reqwest::Client::new()` in `src/tools/searchxyz/mod.rs`. Added fallback temp directory index creation if `SearchIndex::open` encounters permission or file locks. Replaced poisoned mutex expectations with `.unwrap_or_else(|poisoned| poisoned.into_inner())` in `src/tools/searchxyz/web.rs`.
- **Hardening (Static Regex Invariants):** Replaced runtime `.unwrap()` calls on static regex initializations in `src/tools/db_inspector.rs`, `src/tools/web.rs`, and `src/tools/shared_memory/auto_capture.rs` with descriptive `.expect(...)`.
- **Optimization (Cancellation Watch Allocation):** Eliminated repeated watch channel `.subscribe()` heap and atomic allocations inside `is_cancelled` closures in `src/agent/agent_loop/mod.rs` by pre-subscribing receivers.
- **Refactor (Subagent Orchestration):** Unified subagent execution into `run_subagent_attempt()` and prompt generation into `build_subagent_prompt()` in `src/tools/subagent/mod.rs`, eliminating ~400 lines of duplicated lifecycle and error handling code across `delegate_task.rs` and `delegate_profile.rs`.
- **Tests & Verification:** Verified full unit test pass across websocket protocol (30 tests), db_inspector (4 tests), searchxyz (2 tests), web (11 tests), shared_memory (34 tests), and 100% preservation of all 128 registered native tools.
- **Chore:** Bumped version to `v0.0.145`.

### v0.0.144
**Config Live Reload, Subagent Deduplication & Integration Testing:**
- **Feature (Config Live Reload):** Added background configuration watcher `ConfigWatcher` in `src/config/watcher.rs` using `notify` with debouncing, non-destructive file reads (`load_config_from_path`), and atomic change comparison. Synchronized `state.live_config` in `src/channels/websocket/mod.rs` and broadcasts `config_updated` events to all connected WebUI clients in real time when `config.json` changes on disk.
- **Feature (WebSocket Broadcast):** Enhanced WebSocket `set_config` command to immediately broadcast configuration updates to all other connected clients (`publish_ws_event_except`).
- **Refactor (Subagent Orchestration):** Fully deduplicated subagent lifecycle management between `delegate_task.rs` and `delegate_profile.rs` by extracting shared helpers into `src/tools/subagent/mod.rs` (`finalize_simulation_branch`, `sync_workspace_changes_back`, `handle_subagent_cancellation`, `handle_subagent_success`).
- **Fix & Hardening (Runtime Safety):** Replaced `.unwrap()` and `.expect()` panic risks across outline parsing, subagent evaluation/optimization, activity temp files, template compiler loops, loopback IP resolution, YouTube search patterns, MCP channels, SOP template replacement, and HTML crawler selectors.
- **Tests & Verification:** Added end-to-end integration tests in `tests/agent_loop.rs` for multi-turn tool execution pipelines, per-session configuration overrides, and transient error retry backoff; verified native tool registration invariant (128 native tools); verified 100% test pass rate across WebSocket (29 tests) and watcher (3 tests).
- **Chore:** Bumped version to `v0.0.144`.

### v0.0.143
**Architecture Modularization & Code Polish:**
- **Refactor (WebSocket Gateway):** Decomposed `src/channels/websocket/mod.rs` from 1,916 lines down to 1,168 lines by extracting turn cancellation registry and guards into `turns.rs` and unit tests into `tests.rs`.
- **Refactor (Browser Subsystem):** Organized flat browser automation tools into a dedicated `src/tools/browser/` sub-namespace (`broker.rs`, `common.rs`, `firefox.rs`, `gsd.rs`, `obscura.rs`, and `status.rs`) while maintaining 100% backward-compatible aliases in `src/tools/mod.rs`.
- **Fix & Polish (Clippy & Safety):** Fixed clippy lints across the workspace (`manual_strip` in `telegram_send.rs`, `rows.flatten()` in `semantic_search.rs`, needless borrows in `subagent/delegate_profile.rs`, and match pattern cleanups in `sequential_thinking/tools.rs`).
- **Fix (Subagents):** Added `OPENZ_USE_MOCK_PROVIDER` support to `delegate_task.rs` to avoid real outbound network calls during mock testing, and aligned the subagent evolution capture word threshold with guidance expectations.
- **Verification:** Verified 100% test pass rate across WebSocket (28 tests), browser tools (20 tests), subagents (53 tests), telegram send (2 tests), and semantic search (1 test), maintaining the exact 128 registered native tools and 0 frontend TypeScript errors.
- **Chore:** Bumped version to `v0.0.143`.

### v0.0.142
**OpenZ Core Inventory & WebUI Cron Control:**
- **Feature:** Added a WebUI Core Inventory control-center page backed by the gateway runtime inventory, covering core defaults, tools, cron jobs, runtime paths, and channels.
- **Feature:** Added WebSocket cron control commands for pause, resume, delete, and structured run-log retrieval, returning refreshed inventory after mutations.
- **Feature:** Added Inventory Cron actions for pause/resume/delete and run-log inspection without injecting background cron noise into normal chat.
- **Fix:** Restored frontend verification by reinstalling Bun dependencies from `bun.lock` and repaired the local Cargo registry source cache from already-downloaded crate archives.
- **Tests:** Added focused WebSocket cron command coverage plus verified cron quiet-notification behavior.
- **Chore:** Bumped version to `v0.0.142`.

### v0.0.141
**Layered Tool Scope & Balanced Agent Control:**
- **Feature:** Added L0 deterministic turn intent classification for direct answers, local repo reads, local execution, live research, memory, cron, media/document, and orchestration turns.
- **Feature:** Added L1 scoped tool packs so simple prompts do not expose heavyweight shell, web, or subagent tools by default.
- **Feature:** Added `request_tool_scope` as a model-facing escape hatch for exact missing tools/domains.
- **Fix:** Improved local-vs-live routing so repo questions stay local while latest/current/external URL questions route to web research.
- **Fix:** Tightened orchestrator simple-step prompts so trivial planner/reviewer smoke workflows answer directly without unnecessary research or nested delegation.
- **Fix:** Suppressed subagent evolution skill capture for short smoke-test outputs.
- **Chore:** Bumped version to `v0.0.141`.

### v0.0.140
**Cron Inventory & Quiet Background Runs:**
- **Fix:** Cron jobs no longer inject normal start/success/log-saved messages into active CLI/TUI chat.
- **Feature:** Added cron inventory metadata including status, run counts, failure counts, timestamps, last error, and last log path.
- **Feature:** Added structured cron run history in `cron_runs.jsonl` plus `get_job_logs` for agent-readable run inspection.
- **Feature:** Added cron management tools for get, pause, resume, run-now, and logs.
- **Docs:** Documented the cron UX hardening plan and release behavior.
- **Chore:** Bumped version to `v0.0.140`.

### v0.0.139
**Review Fix Hardening:**
- **Fix:** Normalized capitalized filesystem `Path` tool arguments to `path` while preserving explicit native keys.
- **Fix:** Scoped Ratatui git branch cache by workspace so parallel repo TUIs do not share stale branch names.
- **Fix:** Replaced stale subagent allowlist names with registered `crawl_website`, `obscura_browser`, and `read_doc` tools, then added a registry drift guard.
- **Docs:** Removed stale README "What's New" version heading and ignored local `.zcode` session state.
- **Cleanup:** Removed regenerated ignored build/cache artifacts from the local checkout, reducing the working tree from ~80G to ~152M.
- **Chore:** Bumped version to `v0.0.139`.

### v0.0.138
**Subagent Orchestration Deduplication & Stale Test Repairs (recommendedfix §1.1):**
- **Shared orchestration helpers:** extracted the duplicated machinery between `delegate_task` and `delegate_profile` into `subagent/mod.rs` and `schema_retry.rs` — `execute_with_schema_retries()` (the 2-attempt schema-correction loop, previously copy-pasted ~42 lines in each tool), `create_workspace_isolation()` (worktree / scratch / active-workspace fallback setup returning a `WorkspaceIsolation` struct), `CancelOnDrop` (was defined verbatim twice), `filesystem_write_denied_by_policy()` (single shared copy and test), and `attach_workspace_fields()` (cancellation-JSON merge).
- **Dead code removal:** deleted the never-compiled `subagent/context.rs`, a stale verbatim copy of `run_evolution_review`.
- **New tests:** the schema retry loop is now unit-tested end-to-end (fenced-JSON accept with in-place content replacement, retry-then-accept with corrected prompt, attempt-limit error after 2 reruns), plus coverage for the workspace-fields merge.
- **Fixed three tests stale since b787471** (which deliberately set subagent `spawns_process = false` because delegation runs in-process): the two metadata assertions now pin `false`, and the deny-shell registry test now pins the intended policy semantics — `deny_shell` blocks shell tools while delegation wrappers stay available because child agent loops inherit the capability policy on their own registry.
- **Docs:** recorded §1.1 progress in `recommendedfix.md`.
- **Post-release test repairs (suite verification):** ran the full sequential suite (735 tests) after this release and repaired the remaining stale tests — the orchestrator step-prompt wording assertion (reworded in ff11b0b), the orchestrated-worker nested-delegation test (now scopes `ACTIVE_SUBAGENT` to mirror production), and the version-sync surfaces (README badge was stuck at v0.0.124, onpkg.json at v0.0.128). Sequential suite is fully green; ~7 tests remain parallel-run sensitive (documented in recommendedfix §5.2).
- **Chore:** Bumped version to `v0.0.138`.

### v0.0.137
**Tool Metadata Consolidation & Dead Classification Fix (recommendedfix §1.2):**
- **Fixed silently-dead tool classifications:** four tools were referenced by misnamed match arms, so their intended timeouts and network flags never applied at runtime — `html_to_video` (was `html_video`, now 900s), `crawl_website` (was `crawl_site`, now 600s and correctly flagged `uses_network`), `create_animated_svg` (was `svg_animator`, 300s), and `render_mermaid` (was `mermaid`, 300s). Also moved `generate_image`, `generate_video`, `semantic_search`, and `python_sandbox` into the curated table with their intended timeouts.
- **Consolidated on `STATIC_TOOL_DEFS`:** all tool metadata functions (domain, disk/network flags, aliases, examples, usage hints, recommended timeout) now treat the curated data table as the single source for named tools; `tool_recommended_timeout` shrank to dynamic families only (`browser*`, `opendoc_*`, `mcp_*`).
- **Drift guard:** the full-registry registration test now asserts every `STATIC_TOOL_DEFS` entry matches a real registered tool, so future renames can't silently orphan a curated definition again; regression tests pin the fixed timeouts and the crawler's network flag.
- **Docs:** recorded §1.2 progress in `recommendedfix.md`.
- **Chore:** Bumped version to `v0.0.137`.

### v0.0.136
**Robustness & Guard Efficiency Pass:**
- **Panic-free config cache:** `config/loader.rs` recovers from mutex poisoning instead of unwrapping, so a panic elsewhere can no longer take down subsequent config loads at startup. Audit note: the high `unwrap()` counts previously flagged in `providers/resolver.rs`, `memory_extra`, `headroom`, and `self_management` are confined to `#[cfg(test)]` code; production paths were verified unwrap-free apart from the two fixed here.
- **Headroom diff parser:** removed the last production `unwrap()` in `compress.rs` by restructuring the diff-file state transition with `if let`.
- **Security Guard pre-flight discipline:** the system prompt now tells the model exactly which call categories (sudo/su/chmod/chown/eval/source, power commands, destructive deletes and cleans, out-of-workspace writes) always require approval or get denied, steering it toward non-destructive alternatives before it spends a turn generating a doomed tool call.
- **Unified tool-arg display formatting:** `format_tool_args` now uses shared alias tables (`PATH_KEYS`, `COMMAND_KEYS`, `OUTPUT_KEYS`, `QUERY_KEYS`, `URL_KEYS`), a single `clip()` truncation helper, and a consolidated `match` — new tools only need a custom arm when they want special truncation; the generic fallback renders everything else.
- **Chore:** Bumped version to `v0.0.136`.

### v0.0.135
**Ratatui Session Handling Fixes, Scroll Model & In-Flight Guards:**
- **Fix: Session restore now switches the active session.** The active session key is shared mutable state (`Arc<RwLock<String>>`), so prompts after a `/history` restore append to the restored session instead of the original one, and the timeline no longer reverts to the old session after a turn completes. The TUI instance marker is updated on restore.
- **Fix: Session re-sync keeps UI-only notices.** Timeline messages gained an `ephemeral` flag; cancellations, error notices, and switch confirmations survive the full-session reload after each turn (capped at the 5 most recent).
- **Fix: No duplicate sessions on resume.** Startup history selection and `/history` restore adopt the chosen session in place instead of copying its messages into the CLI session key; `/new-session` no longer re-archives an already-archived `cli:history_*` session.
- **In-flight guards:** Enter during a running turn shows a notice and keeps the input instead of invisibly queueing on the agent mutex; `/history` restore and `/new-session` are blocked while a turn runs.
- **Scroll improvements:** Mouse wheel and modifier-arrow scrolling with a clamped scroll model that re-engages auto-scroll at the bottom edge; scroll offsets widened to `u32`; scroll step sizes and the render tick lifted into named constants.
- **Session restore fidelity:** Restored sessions render with full metadata (tool names, reasoning blocks, thinking timers) via `ChatMessage::from_session_message`.
- **Tests:** Added unit coverage for the scroll state machine, session sync merge, and session-message metadata extraction.
- **Chore:** Bumped version to `v0.0.135`.

### v0.0.134
**Ratatui TUI Overhaul (Aura Dark + OpenZ Red-Orange Dual-Tone Theme):**
- **Minicode-Inspired Modern Architecture**: Complete rewrite of the Ratatui TUI channel layout with a 4-tier responsive structure featuring conversation timeline, dynamic slash autocomplete dock, elevated input box, and minimal status bar.
- **Authentic CLI ASCII Wordmark**: Added the authentic dual-color OpenZ ASCII banner where `OPEN` is rendered in bold crisp white (`#f0f0f0`) and `Z` in signature Red-Orange (`#ff5500`), accompanied by workspace and git status metadata.
- **Rich Timeline Rendering**: Markdown headers, bullet points, folded tool output blocks with syntax diff highlighting (`+` green, `-` red), and animated thinking indicators with elapsed execution timers.
- **Floating Centered Modals**: Integrated floating centered modals for interactive LLM provider selection, live model search with instant fuzzy filtering, session history restore, and keyboard shortcuts help.
- **Elevated Input & Autocomplete**: Added elevated input dock styling with floating command autocomplete dock triggered on `/` with smooth keyboard navigation.
- **Chore:** Bumped version to `v0.0.134`.

### v0.0.133
**Obsidian Knowledge Graph Overhaul & Progressive Real-Time Streaming:**
- **Obsidian Graph View Engine**: Built a canvas-based interactive force-directed graph visualizer in the WebUI with Coulomb repulsion, Hooke's spring attraction, velocity damping, dynamic node halos, traveling energy particles, and zoom/pan controls.
- **Progressive One-by-One Stream Engine**: Introduced hierarchical progressive node staging (hub nodes first, then satellites and leaves) that organically streams entities into the graph with expanding entrance wave ripples and dynamic link illumination.
- **Isolated Page-Only Data Lifecycle**: Restricted WebSocket cognitive memory requests and polling timers strictly to the Knowledge View lifecycle, automatically tearing down compute and animation loops when navigating away.
- **Complete Hardcoded Data & Mock Purge**: Removed all placeholder nodes (`OpenZ Agent`, `User Workspace`, `Cognitive Memory`, `Tool Registry`), fake edges, and Cartesian background grids in favor of a clean empty state and genuine SQLite-backed live entity data.
- **Backend Query Scaling**: Lifted artificial database query limits in `fetch_real_cognitive_memory()` from 100 to 2,500 nodes and 5,000 edges; ingested `code_elements`, `code_calls`, `skills`, and `source_bookmarks` into the graph feed.
- **DPR Scaling & Performance Optimizations**: Fixed cumulative canvas device pixel ratio scaling on window resize and added fast Manhattan distance rejection for many-body repulsion calculations.
- **Chore:** Bumped version to `v0.0.133`.

### v0.0.132
**Release Metadata Refresh:**
- **Chore:** Bumped version to `v0.0.132` after confirming workspace directory detection works across OpenZ and NexaDesk workspaces.

### v0.0.131
**Filesystem Directory Evidence Fix:**
- **Fix:** `list_dir` now returns the requested `path`, resolved `canonical_path`, and `entries`, giving the model explicit directory evidence for current-directory questions instead of forcing it to infer paths from file names.
- **Tests:** Updated filesystem alias coverage to assert the resolved directory metadata returned by `list_dir`.
- **Chore:** Bumped version to `v0.0.131`.

### v0.0.130
**Workspace-Scoped Agent Turns and Local Query Grounding:**
- **Fix:** Scoped CLI, Ratatui, WebUI, and OpenAI-compatible API agent turns to the active workspace so local tools resolve `.` from the repo or configured workspace instead of drifting to the process home directory.
- **Fix:** Suppressed live-source verification caveats for local operational questions such as current directory and in-repo implementation lookups while preserving web-required behavior for current external facts.
- **Tests:** Added focused unit coverage for workspace resolution and local-query research intent classification.
- **Chore:** Bumped version to `v0.0.130`.

### v0.0.129
**Typed Multi-Agent Orchestrator Runtime:**
- Added `orchestrate_workflow` for typed sequential, parallel, review-loop, selector, manager-worker, and graph workflow specs.
- Added dependency validation, bounded parallel execution, deterministic selector routing, review-loop termination, and structured workflow result reporting.
- Added `capabilities` policies to restrict worker tools per workflow, including shell/process and filesystem-write controls.
- Added WebUI orchestration lifecycle events and a run-tree panel in Agent Activity, with stop/error/turn-end settlement for stale running state.
- Documented orchestrator runtime usage with minimal sequential and review-loop examples.
- **Fix:** Made `orchestrate_workflow` easier for weaker models by accepting `prompt` as a step-goal alias, accepting `agent` as an agent-name alias, and returning clearer schema repair errors.
- **Fix:** Added balanced grounding policy for orchestrated steps and main-agent guidance so trivial tasks answer directly, current/source-specific facts retrieve sources, nested delegation is capped, and smoke-test outputs do not create noisy skills.
- **Chore:** Bumped version to `v0.0.129`.

### v0.0.128
**WebUI Turn Lifecycle Recovery and Research Source Notice Visibility:**
- **Stale Tool Card Recovery**: WebUI tool completion events now update matching tool cards across the whole chat instead of only the latest assistant message, preventing older research cards from staying stuck on `executing` after late or reordered events.
- **Turn-End Cleanup**: WebUI `turn_end`, `stopped`, and `error` events now settle any still-running tool cards and clear streaming state so the composer does not remain stuck on `Stop` after completed, interrupted, or failed turns.
- **WebSocket Turn-End Consistency**: Gateway WebSocket turn-start failures, including rate-limit rejection and agent-loop construction errors, now emit `turn_end` after the error event.
- **Research Source Notices by Default**: Auto-captured research links and briefs now show visible TUI/WebUI notices by default, and WebUI settings exposes a `Research Source Notices` toggle wired through the gateway config payload.
- **Chore:** Bumped version to `v0.0.128`.

### v0.0.127
**Provider Alias Consistency, Filesystem Tool Compatibility, and Subagent Vision Fallback Routing:**
- **Subagent Provider-Prefix Routing**: Subagent model overrides and fallback models now resolve provider-prefixed entries such as `groq/...`, `openrouter/...`, and `nvidia/...` independently from the parent agent provider, fixing vision fallback attempts that were being sent to the default `opencode_zen` endpoint.
- **Provider Alias Persistence**: Built-in provider aliases (`z_ai`, `opencode zen`, `opencode-zen`, `google ai studio`, and `google-ai-studio`) now save to the correct built-in config fields, use canonical API-key environment variable guidance, and keep their intended default API bases in `openz configure`.
- **Cerebras Env Compatibility**: Corrected the documented Cerebras API key spelling to `CEREBRAS_API_KEY` while preserving compatibility for previous typo variants.
- **Filesystem Tool Alias Clarity**: Filesystem tool schemas now document the accepted argument aliases that the tools already support, reducing model confusion around `filePath`, `file_path`, `startLine`, `endLine`, `content`, `glob`, and `directory`.
- **Chore:** Bumped version to `v0.0.127`.

### v0.0.126
**Fallback Safety, Model Picker Scale, and WebUI Resume Fidelity:**
- **Bounded Provider Fallbacks**: Limited primary-provider fallback attempts by default so failed provider chains stop quickly instead of leaving TUI, WebUI, or subagent turns stuck across multiple retries.
- **WebUI Turn-End Reliability**: WebSocket agent errors now emit an explicit `turn_end` event after sending the error, preventing the WebUI composer from staying in a loading state after provider failures.
- **Configured Model Picker Scale**: WebUI model loading now starts with configured providers and compact previews, then loads a provider's full model list only when selected; recent and favorite models are available for faster switching.
- **TUI Model Picker Parity**: The Ratatui `/model` command now understands recent/favorite model entries and records model selections per session without overwriting other active sessions.
- **Session Resume Tool Rendering**: WebUI session-history replay now merges persisted assistant tool calls, tool outputs, and the final assistant answer back into one visual turn, matching the live active-session layout.
- **Tab-Scoped Session Restore**: WebUI now keeps the current chat through page reloads in the same tab while fresh tabs/windows start on `New Session`, avoiding accidental continuation from old sessions.

### v0.0.125
**Parallel TUI Sessions, Provider Resilience, and Activity Visibility:**
- **Parallel Session Isolation**: Fixed Ratatui model and streaming preferences so `/model` and `/streaming` apply to the active session without changing other open TUI/WebUI sessions; the last closing TUI persists its model as the new default for future sessions.
- **Provider Concurrency and Cancellation**: Added bounded provider-attempt and stream-idle timeouts, stopped retrying a failed primary provider after exhausted fallbacks, and made Ctrl+C cancel active Ratatui turns instead of only tearing down the UI.
- **Optional Provider Turn Locking**: Changed provider turn locking to opt-in via `OPENZ_PROVIDER_TURN_LOCK` (`fragile`, `free`, or `all`) so different OpenZ sessions run concurrently by default while still allowing serialized safety mode for constrained backends.
- **Workflow/Memory Activity Notices**: Added WebUI activity notices for workflow matches, research/source matches, memory captures, and self-improvement saves while preserving CLI `◇ Workflow matched` notifications.
- **Search and Inventory Responsiveness**: Made browser-backed research count toward live-source verification, kept browser search headless-first with cleanup, and made `openz_inventory` compact by default to reduce follow-up LLM latency.

### v0.0.124
**Automatic Task Lifecycle Manager and Headless Browser Search:**
- **Task Lifecycle Registry**: Added the native `manage_tasks` tool and internal registry for OpenZ-owned browsers, servers, agents, subagents, MCP bridges, watchers, and background jobs, with ownership metadata and cleanup policies.
- **Automatic Resource Cleanup**: Agent turns now clean turn-scoped OpenZ-owned tasks automatically, and the system prompt instructs agents to manage lifecycle internally instead of asking users to run manual task commands.
- **Headless-First Browser Broker**: Added a browser broker that routes browser-backed rendering/eval through Obscura headless first, Firefox headless second, and GSD/Chrome GUI only as the final fallback.
- **SearchXyz Browser Reliability**: `searchxyz_browser_search` now uses rendered DOM extraction through the broker, keeps static page-source parsing as fallback, and reports backend, cleanup, fallback, and extraction diagnostics.
- **Managed Process Metadata**: OpenZ-spawned geckodriver and Obscura/Chrome browser daemons now register with lifecycle metadata and appear in browser inspection output.

### v0.0.123
**Approved Credential Management and Secret References:**
- **Permission-Gated Credential Updates**: Added `manage_config` support for approved provider, GitHub/GitLab, Telegram, Discord, and WhatsApp credential updates so users can authorize setup from chat instead of manually editing config JSON.
- **Secret-Safe Approval Prompts**: SecurityGuard now treats credential config writes as sensitive and redacts tokens/API keys in approval details before display.
- **Git Integration Credentials**: Added `integrations.github` and `integrations.gitlab` config sections, and wired `git_provider` to resolve tokens from explicit args, environment variables, approved config, env references, or file references.
- **Safer Provider Key Sources**: Added provider `apiKeyEnv`/`apiKeyFile` support, with process environment variables taking precedence over stored config values.
- **WebUI Settings Parity**: WebSocket settings payloads now preserve provider key reference fields alongside masked direct API keys.

### v0.0.122
**WebUI Session Drafts, Settings Parity, and Navigation Polish:**
- **First-Load Draft Session**: WebUI now opens on the chat page with a visible `New Session` entry in the left sidebar, then promotes it to the saved backend session after the first message completes.
- **Theme & Navigation Reliability**: Replaced full-page theme snapshots with color-only transitions and stabilized the right-side quick action rail so theme toggles no longer blank the app or hide the menu.
- **Workspace Menus & Activity Panel**: Simplified the left sidebar around sessions/settings, moved workspace actions to the vertical right rail, and hide the rail while Agent Activity is open to prevent overlap.
- **Settings Coverage**: Added WebUI editing and WebSocket persistence for workspace, context limit, tool output limit, TUI thought display, and the full built-in provider list.
- **Gateway WebUI Delivery**: Rebuilt and synced the latest WebUI bundle to `~/.openz/web/dist` and kept local install/update scripts wired to refresh gateway assets automatically.

### v0.0.121
**WebUI Workspace Controls, Attachments, and Live Management:**
- **Attachment-Aware Chat**: Added WebUI file selection, drag-and-drop validation, previews, and WebSocket transport so user turns can include persisted attachment metadata alongside text.
- **Live Workspace Management**: Added richer Skills, Agents, Knowledge, Settings, and Dashboard workflows with searchable editors, page-level notices, JSON-safe config updates, protected default subagents, fallback models, and export controls.
- **Agent Activity & Tool UX**: Added a desktop activity panel, improved tool execution cards with compact previews, timing metadata, copy controls, and clearer live execution state.
- **Automatic Gateway WebUI Sync**: Updated `localinstall.sh` and `localupdate.sh` to build the WebUI and refresh `~/.openz/web/dist` automatically, with `--skip-webui-build` available for Rust-only installs.

### v0.0.120
**Custom Providers UI, Identity Recall, and WebSocket Divergence Fixes:**
- **Dynamic Custom LLM Providers**: Added a form and "+ Add Custom LLM Provider Endpoint" button to the "LLM Providers" tab in settings. Users can now input a unique provider key, custom API base, default model, and API key, which are serialized directly to the backend's `others` HashMap in `~/.openz/config.json`.
- **Identity/Persona Memory Pinning**: Lowered the memory importance threshold in `retrieve_pinned_identity_memories` from `0.85` to `0.75` inside `src/agent/agent_loop/build.rs`. This ensures automatically extracted user/agent name, home directory, and persona observations (which carry `0.8` importance) are loaded unconditionally on session start.
- **WebSocket Agent Sync & Streaming**: Resolved a bug where WebUI turns were using a frozen startup copy of the `AgentLoop` by dynamically rebuilding it with live config for every message. Also updated `ws_chat_id` namespace translation to support streaming tokens back to CLI and other channel sessions selected in the WebUI.

### v0.0.119
**WebUI & TUI Layout Real-Time Alignment:**
- **TUI Session History Visual Alignment**: Upgraded the session history printer (`print_session_history` in `render.rs`) to format loaded chat logs identically to active live TUI sessions—displaying user prompts (`> ` and `- ` prefixes), formatting thoughts (`● Thought`), interleaving tool call headers (`● Bash ...`), re-injecting interactive authorization prompts (`> Authorize execution?: Approve (Allow once)`), and formatting outcome checkmarks cleanly without double symbols.
- **WebUI Real-Time Pages**: Wired real-time data for "Skills" and "Agents" views, loading active guidelines and subagent configurations directly from the backend. Fixed the Background Bots & Servers modal configuration status checks for Telegram, Discord, and WhatsApp, and added descriptive empty fallbacks for MCP servers.
- **WebUI Layout Tweaks**: Added smooth rounded corners (`rounded-lg`) to sidebar selection/hover highlights. Fixed the settings dialog scrollbar clipping issue by making the middle form body scrollable and keeping the modal container border clean. Fixed float precision display for settings values (e.g. Temperature).

### v0.0.118
**WebUI Layout & Scrollbar track Fixes:**
- **Double Scrollbar Reset**: Added standard full-viewport reset (`margin: 0; padding: 0; overflow: hidden; height: 100%; width: 100%;`) to `html` and `body` base CSS rules, preventing unexpected browser default scrollbars from appearing on full-screen layouts.
- **Scrollbar Color Evaluation**: Wrapped raw oklch custom properties (like `var(--border)` and `var(--muted-foreground)`) inside `oklch()` calls in base scrollbar declarations. This fixes invalid CSS rules, restoring transparent scrollbar tracks and styled scrollbar thumbs.
- **WebSocket Channel Cleanup**: Removed unused mutable variables in `websocket.rs` command parsing for `/servers` routing.

### v0.0.117
**WebUI Optimization & Smooth Animations:**
- **Smooth Sidebar Transitions**: Refactored the collapse/expand animations in the left menubar (Sidebar). Nav links, settings, and badges now fade and contract using `opacity` and `max-width` transitions rather than disappearing abruptly.
- **Icon Centering**: The "+ New Session", "Clear Active Session", and nav links now transition their padding-left to `20px` when collapsed, centering their icons perfectly on the 68px rail.
- **Clean Header Style**: Removed the permanent brand logo from the sidebar header. Now it only shows the toggle button when collapsed, and displays a styled "OpenZ Agent 🦊" next to the button when expanded.
- **Top Header Removal**: Removed the top header completely. The theme toggle has been moved to the sidebar footer (under Settings), and the model selector was removed to declutter the UI.
- **Mobile Sidebar Toggle**: Added a floating hamburger menu button on mobile views to allow opening the sidebar drawer.


### v0.0.116
**Ratatui OpenZ TUI Integration & Command Routing:**

#### Full-Screen Ratatui TUI (`openz`)
- **Default Entrypoint (`openz`)**: Default command execution now launches the brand new, ultra-fast **Ratatui TUI** interface (`src/channels/ratatui/`).
- **Backward-Compatible Classic TUI (`openz agent`)**: `openz agent` continues launching the legacy Crossterm terminal UI without breaking existing scripts or workflows.
- **OpenZ Visual Aesthetic**: Renders the 3D OPENZ ASCII block logo, version string, provider/model line, and working directory indicator.
- **Multi-line Input Prompt & Hardware Cursor**: Features dark-highlighted input container, multi-line wrapping with `> ` (line 0) and `- ` (line 1+), and precise hardware terminal cursor positioning.
- **Slash Command Autocomplete Overlay**: Typing `/` triggers an interactive autocomplete popup list for slash commands (`/clear`, `/model`, `/history`, `/logs`, `/mcps`, `/memory`, `/servers`, `/exit`).
- **OpenZ Status Pill**: Integrated right-aligned status pill `[ ◇ MCP ... | provider | model | tokens/limit ]` displaying live loaded MCP count and token telemetry in OpenZ colors.

### v0.0.115
**TUI Ergonomics, Cursor Visibility, and Multi-line Prompt Wrapping:**

#### TUI and Input Rendering
- **Visible Terminal Cursor**: Enabled explicit terminal cursor (`crossterm::cursor::Show`) on raw mode start and every render frame, placing the hardware cursor directly at the active typing location.
- **Multi-line Input Prompt Wrapping**: Replaced horizontal single-line scrolling with vertical multi-line prompt wrapping.
- **Dynamic Prompt Prefixes**: Line 0 of the input area displays `> `, while all subsequent wrapped lines display `- `.
- **Formatted Input Submission**: On Enter, submitted user prompts render formatted line-by-line (`> ` for line 0, `- ` for lines 1+).
- **Accurate Cursor Placement**: Calculated exact vertical line index (`cursor_line_idx`) and horizontal column offset (`cursor_col_offset`) for wrapped multi-line input lines.

### v0.0.114
**Automation, Research Reliability, Privacy, and Workspace Safety:**

#### Automatic tool routing
- First file edit per target now auto-runs `scope_context`, loading project instructions before `write_file`, `patch_file`, `replace_lines`, or `zenflow_edit` modifies code.
- Saved `~/.openz/tool_outputs/` reads are automatically rewritten to `retrieve_original`, avoiding repeated truncation of long tool outputs.
- Generated local artifacts auto-run `open_path` when the user asks to show/open/view/play/display the result.
- Failed `open_path` calls auto-run `device_inventory suggest`, producing viewer/app suggestions without requiring the user to know the registry tool.
- Fresh/current/latest/check-again `web_fetch` requests automatically use `cache_mode=revalidate` unless a cache mode is explicitly provided.

#### SearchXyz and web research
- Default `web_search` now follows a local-first cascade: native SearchXyz, native rescue, then provider-free browser discovery. External APIs remain opt-in.
- Added `searchxyz_browser_search`, using local browser automation to discover organic result links without Brave or SearXNG.
- Research-style `web_search` queries automatically read top browser-discovered pages through `searchxyz_read_url`.
- Search failures now classify blocked, rate-limited, timeout, and no-result cases, record backend cooldowns, and attach `searchxyz_doctor` diagnostics by default.
- `web_fetch` detects empty JavaScript app shells and retries through brokered browser rendering in this order: GSD, Firefox, Obscura.
- `searchxyz_read_github_repo` automatically retries small repositories when `max_files` is too low and returns structured auto-retry metadata.

#### Documents and artifacts
- `read_doc` automatically attempts OCR for scanned PDFs and OCR-supported image files when native extraction is empty.
- The top-level Cargo feature `ocr` now enables `opendoc-mcp/ocr`.
- `read_doc` runs PDF complexity analysis by default before extraction/OCR and exposes `complexity_result`; `analyze_complexity=false` opts out.

#### Privacy, traces, and source confidence
- Reasoning/trace output is private by default across normal user-facing channels.
- Added `/tui trace full|compact|off`; legacy `/tui thoughts` and `/thoughts` aliases remain supported.
- Long tool outputs store exact raw files with transcript metadata and remain retrievable through `retrieve_original`.
- Source ledger confidence and final-answer caveats now flag incomplete live verification.

#### Subagents, cancellation, and channel reliability
- Subagents launched from unsafe non-project directories such as home/root/runtime paths now use scratch workspaces under `~/.openz/worktrees` instead of writing directly in the active workspace.
- Normal project/git repo subagent behavior is unchanged: isolated worktrees still sync back on success.
- `parallel_research` returns `partial_success` with completed branch summaries when other branches time out; user cancellation still aborts promptly.
- CLI cancellation and raw-mode cleanup handle Esc, Ctrl+C, Ctrl+D, ETX, and EOT paths more reliably.
- Browser preflight payloads now cover `inspect_browsers`, Firefox/geckodriver, GSD browser, and Obscura/CDP startup failures.

#### Security and maintenance
- `openz doctor --scrub-secrets` can redact historical leaked secrets from sessions, traces, tool outputs, active TUI state, memory, shared data, and runtime data.
- Marketplace research prompts now ask buyer-side vs seller-side clarification only when genuinely ambiguous.
- Fixed the remaining dead-code warnings from test-only helper functions.

### v0.0.112
**Memory Scope Isolation:**
- Fixed `forget_memory` so cognitive-memory deletion is limited to the active workspace.
- Added regression coverage proving matching records in another workspace survive.
- No global cognitive records are removed by a scoped forget operation.
- Graph relations connected to nodes removed through matching observations are now expired too, preventing orphaned active edges.
- `memory_stats` now counts cognitive memories only from the active workspace, matching recall behavior.

### v0.0.111
**Telegram Messaging and Remote Inbox Cleanup:**
- Expired or malformed inbox entries are cleaned globally before new remote prompts are enqueued, preventing stale failed-channel jobs from accumulating.
- Added the native `telegram_send_message` tool with explicit chat-target validation and bounded retry behavior.
- Generalized Telegram text delivery to support numeric chat IDs and validated usernames.
- Added focused inbox, Telegram target, and shared-memory workflow coverage.

### v0.0.110
**Secret-Safe Logging:**
- Added a central in-memory scrubber for configured and environment-backed credentials.
- File logs, stderr logs, and SQLite logs now redact secrets even when embedded in command URLs or tool arguments.
- Added focused regression coverage for Telegram bot tokens and provider keys inside log text.

### v0.0.109
**Telegram Command Registration Reliability:**
- Fixed invalid Telegram slash-command names that used hyphens instead of Telegram-supported underscores.
- Kept legacy hyphenated command input working by normalizing incoming command aliases.
- Added a focused regression test that validates every registered command name before startup.

### v0.0.108
**Shared Tool Argument Compatibility and Secret Redaction:**
- Fixed the shared argument normalizer so schema-native camelCase fields are preserved instead of being destructively rewritten.
- Legacy snake_case aliases are still added recursively, including nested entity and relation payloads.
- Added regression coverage for canonical fields, aliases, nested arguments, and explicit alias precedence.
- Fixed configuration redaction for `apiKey` and other camelCase secret fields, with recursive coverage for token and credential variants.

### v0.0.107
**Remote Inbox Regression Coverage:**
- Added isolated FIFO queue integration coverage.
- Added malformed-entry quarantine coverage.
- Remote inbox behavior is now tested through the real runtime-directory override.

### v0.0.106
**Remote Job Heartbeats:**
- TUI-to-Telegram remote jobs refresh a heartbeat while executing.
- The timeout now detects genuinely stalled or stopped TUI sessions instead of timing out healthy long jobs.
- Added heartbeat cleanup on completion, cancellation, and errors.

### v0.0.105
**Telegram Delivery Hardening:**
- Telegram text sends validate the API JSON ok result.
- Transient network, rate-limit, and server failures retry with bounded backoff.
- Remote timeout is configurable with OPENZ_REMOTE_TIMEOUT_SECS, clamped to 60 seconds through one hour.
- Default remote timeout increased to 15 minutes for long-running TUI work.

### v0.0.104
**Multi-TUI Remote Target Safety:**
- cli:direct resolves only when exactly one active TUI exists.
- Multiple TUIs no longer race to consume a global remote inbox.
- Added focused tests for direct target resolution and inbox expiry.

### v0.0.103
**Reliable Remote Inbox Queue:**
- Remote prompts are stored as separate atomically published queue entries.
- Multiple prompts no longer overwrite each other.
- Expired prompts are discarded after five minutes.
- Malformed inbox entries are quarantined instead of silently blocking the queue.
- Messages are consumed in timestamp order.
- Invalid or unavailable CLI targets are rejected before Telegram reports execution.
- Remote typing indicators expire with a timeout when the selected TUI stops responding.

### v0.0.102
**TUI-to-Telegram Remote Session Identity Fix:**
- Remote prompts executed in a selected TUI session now retain the TUI session identity.
- Prevents the TUI from consuming its own forwarded prompt when Telegram remote control is used.
- Preserves Telegram sender routing only for returning output, errors, and cancellation messages.

### v0.0.101
**Remote Telegram Completion Recovery:**
- Remote prompts sent from Telegram now receive success, error, and cancellation responses.
- Long responses use the same safe Telegram chunking path as normal Telegram replies.
- Self-targeted send_remote_input calls are rejected to prevent TUI input loops.

# OpenZ Changelog & System Specifications 🦊⚡

Welcome to OpenZ! This document provides an official record of the framework's architecture, hardware footprint, system capabilities, Model Context Protocol (MCP) integrations, native tools, and version releases.

---

## 📊 System Specifications & Hardware Footprint

| Category | Specification | Detail |
| :--- | :--- | :--- |
| **ROM (Binary Size)** | **Recent measured dev install: ~124 MB; scripts print exact current size** | Release profile strips symbols and uses thin LTO; exact size depends on compiled optional-heavy dependencies such as ONNX/embedding/browser/media stacks. |
| **RAM Footprint (Cloud)** | **~15 MB - 30 MB** | Expected memory consumption when using remote LLM APIs and no local embedding model loaded. |
| **RAM Footprint (Local)** | **~200 MB+** | Memory consumption when loading local ONNX vector embeddings (`AllMiniLML6V2`) into RAM. |
| **CPU Utilization** | **Near 0% Idle CPU** | Event-driven architecture using Tokio thread pools avoids busy polling when inactive. |
| **Startup Speed** | **Millisecond-scale core CLI; full TUI varies** | Startup depends on config load, DB checks, enabled channels, MCP/tool setup, and provider checks. |
| **Inspired By** | **hermes-agent**, **Zeroclaw**, **Nanobot**, **loops!**, **DOX**, **codegraph**, **tantivy**, **lancedb**, **surrealdb**, **petgraph**, **sentrux**, **tree-sitter-graph**, **mistral.rs**, **agentgateway**, **cowork-forge**, **openhuman**, **mcp-rust-sdk**, **wasserstein-agents**, **gsd-browser**, **chromewright**, **sediment**, **ClawDB**, **ferres-db**, **native-devtools-mcp**, **tokio-cron-scheduler**, **grpc-rust**, **mcp-searxng**, **searxng-mcp**, **opendocswork-mcp**, **slack-mcp-server**, **task-master**, **langgraph**, **crawl4ai**, **websurfx**, **headroom**, **rust-mcp-filesystem**, **novada-mcp**, **obscura**, **crawlee**, **katana**, **librefang**, **openmetadata**, **youtube-transcript-api**, **semble**, **deep-research**, **ocrs**, **agent-skills**, **superpowers**, **OpenMemory**, **SkillSpector**, **OpenHands**, **deer-flow**, **multica**, **ast-grep**, **caveman**, **graphify**, **notify**, **mcp-everything**, **mcp-memory**, **mcp-sequentialthinking**, **mcp-git**, **mcp-fetch**, **mcp-time**, and **openfang** | Synthesizes loops, workflows, code graphs, search indexers, serverless vector DBs, gateways, multi-agent teams, desktop memory, MCP SDKs, browser CDP, office parsers, stateful agent graphs, web crawlers, context scoping, native filesystems, anti-bot stealth scrapers, sandboxed agent operating systems, metadata context layers, lightweight media scrapers, token-saving hybrid search, local deep research, Rust-native machine learning OCR, lifecycle-based engineering skills, structured branch-driven workflows, self-hosted hierarchical memory engines, skill security scanning, autonomous developer agents, agent workspace management, syntax-aware structural code searches, terseness prompt optimizations, codebase knowledge graphs, filesystem file watchers, MCP reference specifications, native Git integration, and WASM execution sandboxes. |

---

## 💡 Architectural Inspirations & Design Similarities

OpenZ synthesizes patterns from several state-of-the-art developer tools to keep its footprint ultra-lightweight:

### 1. [`codegraph`](https://github.com/suatkocar/codegraph) & [`codegraph-rust`](https://github.com/Jakedismo/codegraph-rust) (Code Relationship Mapping)
*   **The Concept:** Map code structures (imports, functions, structs, classes) to represent relationships and dependencies.
*   **In OpenZ:** We employ `code_outline` (`src/tools/outline.rs`) and `ast_grep` to build structural syntax indexes. Furthermore, native graph-memory and code-indexing tools compile entity-relationship views of the codebase so OpenZ can query dependencies and connections (e.g. "what implements this trait?") semantically without loading whole files.

### 2. [`quickwit-oss/tantivy`](https://github.com/quickwit-oss/tantivy) (High-Performance Local Indexing)
*   **The Concept:** A fast, low-memory local text indexer written in pure Rust.
*   **In OpenZ:** OpenZ prioritizes 100% Rust-native, local-first search (via optimized ripgrep wrappers and SQLite indexers) over heavy external databases (such as Elasticsearch or cloud indices). This matches `tantivy`'s philosophy: keeping search local, using zero-cost abstractions, and delivering instant boot times (<5ms) and tiny ROM/RAM footprint.

### 3. [`lancedb/lancedb`](https://github.com/lancedb/lancedb) (Serverless Vector Database)
*   **The Concept:** Serverless, disk-backed, local-first vector database storing embeddings and metadata co-located on the user's disk.
*   **In OpenZ:** OpenZ's semantic memory and research archives (`src/tools/shared_memory.rs`, `src/tools/semantic_search.rs`) implement file-backed vector search. It executes ONNX embedding models completely locally using the `fastembed` library and stores metadata in a local SQLite database (`~/.openz/memory.db`), enabling offline semantic search and zero-latency lookups without remote server overhead.

### 4. [`surrealdb/surrealdb`](https://github.com/surrealdb/surrealdb) (Embedded Multi-Model Database)
*   **The Concept:** Embedded, serverless, multi-model (document, graph, relational) database engine in Rust.
*   **In OpenZ:** OpenZ's memory system combines structured document logs, relational columns (SQLite), and entity-relationship links. Native memory tools mimic this multi-model philosophy, executing embedded document and graph queries co-located on disk (`~/.openz/memory.db`) without requiring external database servers.

### 5. [`petgraph/petgraph`](https://github.com/petgraph/petgraph) (Graph Structures & DAG Workflows)
*   **The Concept:** Standard Rust graph representation, manipulation, and traversal library.
*   **In OpenZ:** The **SOP Workflow Engine** (`src/sop/`) represents tasks and workflows as Directed Acyclic Graphs (DAGs) and executes independent steps in parallel. It uses topological sorting and performs graph dependency cycle detection on startup. Additionally, native graph-memory tools use graph traversals (BFS/DFS) to explore code relationships.

### 6. [`sentrux/sentrux`](https://github.com/sentrux/sentrux) (Architectural Sensors & Quality Gates)
*   **The Concept:** Real-time codebase quality sensing, dependency analysis, and quality gates to prevent code decay.
*   **In OpenZ:** The built-in SOP workflow templates (such as `ship-pr-until-green` and `pre-commit-guard`) act as automated quality gates. They verify tests, compile workspaces, auto-heal syntax/borrow-checker errors in a closed loop via `CompilerAutoHealTool`, and automatically roll back code corruption using checkpointed Zenflow snapshots.

### 7. [`tree-sitter/tree-sitter-graph`](https://github.com/tree-sitter/tree-sitter-graph) (Syntactic-to-Semantic Graph Mapping)
*   **The Concept:** Constructing arbitrary graph structures directly from AST parsing syntax trees.
*   **In OpenZ:** We leverage `ast_grep` (built on `tree-sitter`) and `code_outline` to parse files structurally. Native code-graph indexing transforms these syntax trees into relational code graphs, mapping callers, interfaces, and implementations dynamically.

### 8. [`EricLBuehler/mistral.rs`](https://github.com/EricLBuehler/mistral.rs) (Local LLM & Embedding Inference)
*   **The Concept:** Fast, local LLM and embedding inference engine written in Rust.
*   **In OpenZ:** Although we interface with cloud providers, OpenZ supports 100% private, offline executions by routing to local LLM providers (e.g. Ollama via our OpenAI API compatibility) and running vector embeddings locally on CPU/GPU via ONNX and the `fastembed` library.

### 9. [`agentgateway/agentgateway`](https://github.com/agentgateway/agentgateway) (Unified Agent Router & Traffic Plane)
*   **The Concept:** Rust-native, high-performance API gateway and traffic proxy routing agentic traffic, bridging MCP servers, and enforcing AI prompt security.
*   **In OpenZ:** The WebSocket Gateway (`openz gateway`) operates as a single-user agent traffic router. It hosts an OpenAI-compatible Completions API (`/v1/chat/completions`) that handles multi-model provider routing, translates JSON-RPC requests over an in-process gRPC Tonic bridge for stdio MCP servers, tracks token billing/usage, and logs security checks.

### 10. [`sopaco/cowork-forge`](https://github.com/sopaco/cowork-forge) (Multi-Agent Workspaces & Actor-Critic Verification)
*   **The Concept:** Automating software development by organizing specialized virtual developer teams (PMs, Architects, Engineers) and verifying code quality using Actor-Critic reviews.
*   **In OpenZ:** OpenZ implements this collaborative multi-agent structure inside `src/subagents/` with dedicated profiles (`planner`, `architect`, `reviewer`, `test_engineer`). It coordinates these subagents inside stateful loops (such as the `EvaluatorOptimizerLoopTool` and `CompilerAutoHealTool`) which act as Actor-Critic loops to review, compile, lint, and repair code iterations recursively.

### 11. [`tinyhumansai/openhuman`](https://github.com/tinyhumansai/openhuman) (Desktop-First Memory Curation)
*   **The Concept:** Desktop-first, offline-first personal AI assistant focused on memory synthesis, data integration, and private local tools.
*   **In OpenZ:** Designed as a desktop-first, highly private developer workspace assistant. Its self-improvement curate loop asynchronously reads chat logs and synthesizes raw logs into clean, editable memory facts and SQLite skills, keeping prompt contexts compact while enabling full user privacy.

### 12. [`modelcontextprotocol/rust-sdk`](https://github.com/modelcontextprotocol/rust-sdk) (Official MCP Specifications)
*   **The Concept:** Standardized JSON-RPC protocol specifications for tools/resources/prompts sharing.
*   **In OpenZ:** The client implementation (`src/tools/mcp.rs`) complies with the JSON-RPC handshake, `tools/list` schema queries, and `tools/call` executions defined in the official MCP specifications, making OpenZ fully extensible with any standard MCP tool server.

### 13. [`wasserstein-agents`](https://crates.io/crates/wasserstein-agents) (Optimal Multi-Agent Task Distribution)
*   **The Concept:** Mathematics-based optimal transport, Wasserstein distance computations, and coordinate routing of multi-agent distributions.
*   **In OpenZ:** In terms of operational coordination, OpenZ runs specialized subagents concurrently via `ParallelResearchTool` and `EvaluatorOptimizerLoopTool`. It allocates tasks dynamically to isolated sub-workspaces, preventing overlapping work and optimizing the computational transport plan of multi-agent systems.

### 14. [`gsd-browser`](https://opengsd.net/products/gsd-browser) & [`bnomei/chromewright`](https://github.com/bnomei/chromewright) (CDP-Based Browser Automation)
*   **The Concept:** Controlling web browsers natively over Chrome DevTools Protocol (CDP) WebSocket endpoints and Playwright automation.
*   **In OpenZ:** We natively register `GsdBrowserTool` (Playwright-based automation) and `ObscuraBrowserTool` (pure CDP-based WebSocket automation). This mirrors `chromewright`'s design, giving the agent direct control over CDP without needing heavy Playwright browser compilation, enabling extremely fast, lightweight web navigations.

### 15. [`rendro/sediment`](https://github.com/rendro/sediment) & [`ClawDB`](https://github.com/Claw-DB/ClawDB) (Local-First Semantic Memory & Gateways)
*   **The Concept:** Rust-based single binary local-first MCP semantic memory systems with graphs, decay, and multi-channel messaging integrations.
*   **In OpenZ:** OpenZ natively supports `sediment` as a pre-configured MCP tool server. Additionally, our native memory consolidation and multi-channel chat gateway listeners (WhatsApp, Telegram, Discord, Email, WebSockets) share this identical architectural philosophy: keeping all semantic indexing local (`~/.openz/memory.db`), managing conversational histories securely, and acting as a personal self-hosted gateway.

### 16. [`ferres-db/ferres-db`](https://github.com/ferres-db/ferres-db) (High-Performance Vector Search Engines)
*   **The Concept:** Self-hosted vector databases featuring low latency and robust write-ahead log (WAL) persistence in Rust.
*   **In OpenZ:** Our local vector database wrappers and SQLite schema implement WAL persistence for memory items. Using local embedding models, OpenZ achieves sub-millisecond local semantic lookups, matching FerresDB's goal of fast, reliable, co-located vector search.

### 17. [`sh3ll3x3c/native-devtools-mcp`](https://github.com/sh3ll3x3c/native-devtools-mcp) (Native Debugging & Computer Use)
*   **The Concept:** MCP server giving AI agents direct control over native desktop applications, browsers (via CDP), and system devtools.
*   **In OpenZ:** OpenZ integrates native tools such as `SystemInfoTool` and CDP-based `ObscuraBrowserTool` alongside our sandboxed `exec_command` shell execution. This maps directly to the native desktop and browser CDP debugging control model.

### 18. [`mvniekerk/tokio-cron-scheduler`](https://github.com/mvniekerk/tokio-cron-scheduler) (Asynchronous Job Schedulers)
*   **The Concept:** Asynchronous task scheduling loop written in Rust using Tokio for managing cron jobs.
*   **In OpenZ:** OpenZ incorporates a fully native cron scheduling architecture (`src/cron/` and `src/tools/cron.rs`). It uses standard cron formats and duration intervals to dispatch background agent routines asynchronously, ensuring non-blocking execution on active user channels.

### 19. [`grpc/grpc-rust`](https://github.com/grpc/grpc-rust) (gRPC Communication Channels)
*   **The Concept:** A high-performance Rust gRPC implementation for client-server protocol execution.
*   **In OpenZ:** We leverage `tonic` (the modern hyper-based gRPC implementation in Rust) to build the in-process MCP bridge (`src/tools/mcp.rs`). It encapsulates standard stdio MCP JSON-RPC protocols inside structured gRPC channels, providing robust and noise-isolated tool execution APIs.

### 20. [`ihor-sokoliuk/mcp-searxng`](https://github.com/ihor-sokoliuk/mcp-searxng) & [`varlabz/searxng-mcp`](https://github.com/varlabz/searxng-mcp) (Private Federated Search APIs)
*   **The Concept:** High-performance Model Context Protocol (MCP) servers for SearXNG engines, enabling private, structured web search queries and response parsing.
*   **In OpenZ:** OpenZ's modular MCP integration allows users to easily register local or private search aggregators (such as `mcp-searxng` or `searxng-mcp` via `manage_mcp` configurations), enabling highly private web fetching and query capabilities co-located on your network.

### 21. [`aimino-tech/opendocswork-mcp`](https://github.com/aimino-tech/opendocswork-mcp) (Document Extraction & Office MCPs)
*   **The Concept:** An MCP server designed to extract and parse tables, text, and metadata from `.docx`, `.xlsx`, and `.pptx` files.
*   **In OpenZ:** OpenZ natively pre-configures and supports the `office` tool powered by the `opendocswork-mcp` binary compiled locally. We also register `DocReaderTool` (`src/tools/doc_reader.rs`) which uses Rust document libraries to read PDF, spreadsheet, and text files.

### 22. [`slack-samples/bolt-js-slack-mcp-server`](https://github.com/slack-samples/bolt-js-slack-mcp-server) (Collaborative Slack MCP Bridge)
*   **The Concept:** Slack integration via MCP client structures, enabling agents to parse messages and manage workspace channels.
*   **In OpenZ:** We natively build messaging gateway adapters (TUI, WebSocket gateway, WhatsApp, Discord, Telegram, and Email IMAP/SMTP). Additionally, users can bridge Slack using `manage_mcp` to connect `bolt-js-slack-mcp-server` in-process, allowing OpenZ to monitor channels and coordinate tasks.

### 23. [`eyaltoledano/claude-task-master`](https://github.com/eyaltoledano/claude-task-master) (AI Task Execution & Complexity Scoring)
*   **The Concept:** Structured task parsing from PRD documents, complexity scoring, and test-driven autopilot compilation checks.
*   **In OpenZ:** Our **SOP Workflow Engine** compiles structural task steps into Directed Acyclic Graphs (DAGs). It runs TDD autopilots (like `ship-pr-until-green` and `pre-commit-guard`) that check builds, capture stderr compile blocks, and automatically repair code in a loop via `CompilerAutoHealTool` until all checks are verified.

### 24. [`langchain-ai/langgraph`](https://github.com/langchain-ai/langgraph) (Stateful Agentic Graph Loops)
*   **The Concept:** Modeling multi-actor agent loops as stateful graphs (nodes, edges, cycles) with structured memory persistence.
*   **In OpenZ:** OpenZ's core chat runtime runs on a **Stateful TurnState machine** (Restore → Compact → Command → Build → Run → Save → Respond → Done) designed as a cyclic state graph. Our SOP workflow engine executes tasks concurrently as Directed Acyclic Graphs (DAGs) and records execution instance states locally on disk.

### 25. [`unclecode/crawl4ai`](https://github.com/unclecode/crawl4ai) (LLM-Friendly Scrapers & Crawlers)
*   **The Concept:** Web crawlers built to compile dynamic web pages and output token-efficient clean Markdown formats optimized for LLM consumption.
*   **In OpenZ:** OpenZ features `web_fetch` (scrapes pages and converts them to clean structured markdown) and `crawl_website` (`CrawlSiteTool` using `spider-rs` for concurrent multi-threaded crawling). This aligns with `crawl4ai`'s goal: stripping HTML DOM paths into compact markdown nodes to optimize prompt context token budgets.

### 26. [`neon-mmd/websurfx`](https://github.com/neon-mmd/websurfx) (Rust Meta-Search Engines)
*   **The Concept:** High-performance, privacy-respecting, and secure search aggregators built natively in Rust.
*   **In OpenZ:** We share Websurfx's design criteria: using Rust's concurrency and memory safety to write extremely fast, low-overhead search tools (`web_search` and scrapers) that run entirely locally and aggregate information privately.

### 27. [`chopratejas/headroom`](https://github.com/chopratejas/headroom) (Context Compression & Scope Management)
*   **The Concept:** Walking directory paths to resolve local guidelines (`AGENTS.md`) and compressing logs to respect token limits.
*   **In OpenZ:** OpenZ registers native Headroom-style tools such as `scope_context`, `compress_content`, and `retrieve_original`. These compile folder-specific `AGENTS.md` guidelines before file edits and compress tool outputs >4000 characters using context compactor states (`src/agent/context_compactor.rs`) to prevent prompt token drift.

### 28. [`rust-mcp-stack/rust-mcp-filesystem`](https://github.com/rust-mcp-stack/rust-mcp-filesystem) (Native Rust MCP Filesystems)
*   **The Concept:** Safe, standard filesystem manipulation tools implemented as MCP servers in Rust.
*   **In OpenZ:** Instead of spawning external Node.js/Python server binaries for file checks, OpenZ implements native Rust filesystem tools (`read_file`, `write_file`, `list_dir`, `patch_file`) directly in-process (`src/tools/filesystem.rs`), delivering sub-millisecond, zero-overhead file operations.

### 29. [`NovadaLabs/novada-mcp`](https://github.com/NovadaLabs/novada-mcp) (Unified Scraper & Research MCP)
*   **The Concept:** A unified MCP server offering web search, browser automation, anti-bot handling, residential proxy integration, and autonomous multi-source research.
*   **In OpenZ:** We share this vision of a multi-purpose research toolkit. OpenZ packs native search (`web_search`), concurrent crawling (`crawl_website`), and browser automation (`gsd_browser`, `obscura_browser`), acting as an all-in-one local equivalent to Novada MCP's unified scraper/researcher interface without requiring third-party SaaS API keys.

### 30. [`h4ckf0r0day/obscura`](https://github.com/h4ckf0r0day/obscura) (Stealth CDP-Based Headless Browser)
*   **The Concept:** A lightweight, dependency-free Rust-based headless browser engine consuming minimal RAM (~30MB) and offering built-in fingerprint randomization and ad/tracker blocking via Chrome DevTools Protocol.
*   **In OpenZ:** The native `ObscuraBrowserTool` (`src/tools/obscura.rs`) integrates directly with the Obscura client. By leveraging its headless CDP controls, OpenZ performs fast, stealthy, and low-resource web navigation and DOM rendering without the heavy RAM overhead of traditional Chromium/Playwright bundles.

### 31. [`apify/crawlee`](https://github.com/apify/crawlee) (Reliable Browser & Request Crawling)
*   **The Concept:** An open-source web scraping and browser automation library featuring automated proxy rotation, session handling, and robust HTML/DOM extraction pipelines.
*   **In OpenZ:** OpenZ's web fetching and crawler pipelines (`web_fetch`, `crawl_website`) mimic Crawlee's robust request loop. We handle dynamic JS rendering via CDP, fall back to high-performance raw response parsing, and clean raw HTML into token-efficient Markdown structures optimized for LLMs.

### 32. [`projectdiscovery/katana`](https://github.com/projectdiscovery/katana) (Security-First High-Speed Spidering)
*   **The Concept:** Next-generation, fast web crawling and endpoint discovery spidering tool supporting standard and headless browser modes for SPA discovery.
*   **In OpenZ:** Our concurrent `CrawlSiteTool` (powered by `spider-rs`) adopts Katana's dual-mode speed: it uses raw request spidering for sub-millisecond static page traversal and switches to headless browser rendering for dynamic paths, building a comprehensive page/endpoint index of targets rapidly.

### 33. [`librefang/librefang`](https://github.com/librefang/librefang) (Rust Agent Operating System)
*   **The Concept:** An open-source, Rust-native agent operating system managing processes, isolation, scheduling, and Merkle audit trails.
*   **In OpenZ:** We share this systems-centric design philosophy. OpenZ functions as a lightweight agent runtime that manages task execution loops, applies process sandboxing (seccomp BPF filters), and builds cryptographic Merkle hash-chain ledgers to track actions securely and transparently.

### 34. [`open-metadata/openmetadata`](https://github.com/open-metadata/openmetadata) (Unified Metadata Context Layer)
*   **The Concept:** A centralized open-source data discovery, cataloging, and collaboration platform providing a shared context layer for humans and AI.
*   **In OpenZ:** OpenZ implements local context discovery and metadata co-location. It maps code outlines structurally and resolves folder-specific instructions dynamically, acting as an in-process, developer-first metadata layer.

### 35. [`jdepoix/youtube-transcript-api`](https://github.com/jdepoix/youtube-transcript-api) (Lightweight Captions Extractor)
*   **The Concept:** A fast, dependency-free scraper that fetches video transcripts and subtitles without requiring Google API keys or headless browsers.
*   **In OpenZ:** We prioritize lightweight, zero-key scraping alternatives. Just as the YouTube transcript API avoids heavy Selenium stacks, OpenZ's native `DocReaderTool` and `web_fetch` scraper extract structured content directly with minimal overhead.

### 36. [`minishlab/semble`](https://github.com/minishlab/semble) (Token-Saving Hybrid Code Search)
*   **The Concept:** A fast CPU-only code search library combining semantic embeddings and BM25 lexical search to retrieve precise code chunks instead of reading entire files.
*   **In OpenZ:** OpenZ implements local code search (`grep_search`, `ast_grep`) and local vector-based semantic search (`FastEmbed`). This matches Semble's core mission: indexing structures locally and scope-limiting code retrievals to prevent prompt token drift.

### 37. [`u14app/deep-research`](https://github.com/u14app/deep-research) (Private Deep Research Report Engine)
*   **The Concept:** An open-source tool designed to generate in-depth, privacy-focused research reports using LLMs, local storage, and MCP interfaces.
*   **In OpenZ:** We share the goal of privacy-focused deep research. OpenZ coordinates specialized subagents (such as in `ParallelResearchTool`) to perform concurrent multi-source queries, scraping, and synthesis, storing all research data locally without external SaaS trackers.

### 38. [`robertknight/ocrs`](https://github.com/robertknight/ocrs) (Rust-Native OCR Engine)
*   **The Concept:** A modern, native Rust OCR library using neural networks and the RTen runtime to extract text from images.
*   **In OpenZ:** Just as ocrs replaces external binary dependencies with pure-Rust machine learning models, OpenZ prioritizes Rust-native local engines (like ONNX models via fastembed) for private, lightweight in-process metadata extraction.

### 39. [`addyosmani/agent-skills`](https://github.com/addyosmani/agent-skills) (Production-Grade Engineering Workflows)
*   **The Concept:** A collection of structured, lifecycle-based skills (Define, Plan, Build, Verify, Ship) with standardized slash commands designed to enforce rigorous engineering habits in AI coding agents.
*   **In OpenZ:** The GSD workflow system implements this lifecycle directly. We use specs, plans, compiler check loops, and self-healing tools to enforce TDD and prevent lazy code edits.

### 40. [`obra/superpowers`](https://github.com/obra/superpowers) (7-Stage Disciplined Engineering Methodology)
*   **The Concept:** A modular agentic skills framework guiding AI assistants through isolated feature branches, TDD, code review, and branch merges to ensure code quality.
*   **In OpenZ:** OpenZ's workflow loops mirror these 7 stages. We execute parallel tasks via topological DAG sorting, verify output builds natively, auto-heal errors, and use cryptographic Merkle ledger transitions to secure updates.

### 41. [`CaviraOSS/OpenMemory`](https://github.com/CaviraOSS/OpenMemory) (Self-Hosted Hierarchical Memory Engine)
*   **The Concept:** An open-source, self-hosted memory engine providing long-term contextual memory using a Hierarchical Memory Decomposition architecture.
*   **In OpenZ:** We share this local-first structured memory philosophy. OpenZ utilizes a multi-tier memory architecture co-locating episodic facts (in session metadata) and procedural instructions/skills (within a local SQLite database), enabling humans and agents to access structured context securely.

### 42. [`NVIDIA/SkillSpector`](https://github.com/NVIDIA/SkillSpector) (AI Agent Skill Security Evaluator)
*   **The Concept:** An open-source security tool that scans AI capabilities and skills for vulnerabilities, malicious logic, and data exfiltration vectors.
*   **In OpenZ:** Security and safety are core constraints. OpenZ's native `SecurityGuard` acts as an active active verification gate, intercepting destructive tools, network requests, and out-of-workspace commands to validate compliance before code changes are made.

### 43. [`OpenHands/OpenHands`](https://github.com/OpenHands/OpenHands) (Model-Agnostic AI Developer Harness)
*   **The Concept:** An open-source generalist agent platform that automates software engineering tasks within secure sandboxed environments.
*   **In OpenZ:** OpenZ implements this fully developer-centric execution paradigm. It combines multi-agent TDD loops with direct workspace sandboxing, letting developers run workflows securely on local systems or cloud channels.

### 44. [`bytedance/deer-flow`](https://github.com/bytedance/deer-flow) (Long-Horizon Multi-Agent Task Harness)
*   **The Concept:** A SuperAgent harness designed for autonomous task planning, multi-agent decomposition, and isolated tool execution.
*   **In OpenZ:** OpenZ adopts a similar multi-agent orchestration pattern. It breaks complex objectives into subtasks, delegates them to specialized profiles (like planner, reviewer, etc.) inside isolated sub-workspaces, loads modular `skills/*.md` on demand, and maintains cross-session memory.

### 45. [`multica-ai/multica`](https://github.com/multica-ai/multica) (Autonomous Team & Agent Workspace)
*   **The Concept:** An open-source managed platform designed to orchestrate and manage AI coding agents as if they were real team members, using a local daemon to route tasks.
*   **In OpenZ:** We share this focus on developer agent teams. The local WebSocket gateway (`openz gateway`) acts as a single-user agent router, hosting completions APIs, managing multiple model provider sessions, and routing tool tasks dynamically.

### 46. [`ast-grep/ast-grep`](https://github.com/ast-grep/ast-grep) (AST-Based Structural Code Search & Replace)
*   **The Concept:** A high-performance command-line tool written in Rust for syntax-aware code search, linting, and rewriting using abstract syntax trees.
*   **In OpenZ:** OpenZ integrates `ast_grep` natively as a core tool (`src/tools/ast_grep.rs`). This allows OpenZ to perform structure-aware refactoring, syntax pattern matches, and precise edits without relying on raw regex or simple string searches.

### 47. [`juliusbrussee/caveman`](https://github.com/juliusbrussee/caveman) (Terseness-Driven Token Compression)
*   **The Concept:** An AI coding skill designed to reduce token usage by forcing models to communicate in an ultra-compressed "caveman-like" style.
*   **In OpenZ:** OpenZ natively integrates this via the `caveman_mode` setting (ON by default). This injects a specific system prompt instruction that strips pleasantries and filler words, reducing token overhead while maintaining complete technical accuracy.

### 48. [`safishamsi/graphify`](https://github.com/safishamsi/graphify) (Codebase-to-Knowledge-Graph Builder)
*   **The Concept:** An agentic skill that processes codebases and document folders to compile queryable entity-relationship knowledge graphs.
*   **In OpenZ:** OpenZ incorporates this capability through native code indexing, graph-memory tools, and reusable skills, mapping structural code imports and file hierarchies into queryable graph nodes.

### 49. [`notify-rs/notify`](https://github.com/notify-rs/notify) (Cross-Platform File Watching)
*   **The Concept:** A standard cross-platform file system monitoring library in Rust that watches files/directories for modifications, creations, and deletions.
*   **In OpenZ:** Utilized directly by the `FileWatcherTool` (`src/tools/watcher.rs`) to track workspace folder changes and trigger automated compilation/test suites when source code changes.

### 50. [`modelcontextprotocol/servers/src/everything`](https://github.com/modelcontextprotocol/servers/tree/main/src/everything) (MCP Everything Reference)
*   **The Concept:** A reference Model Context Protocol server demonstrating resources, prompts, and tools implementation specifications.
*   **In OpenZ:** OpenZ's modular MCP client (`src/tools/mcp.rs`) supports all standard JSON-RPC capability sets shown in the `everything` reference, allowing the agent to dynamically inspect tools and compile prompts.

### 51. [`modelcontextprotocol/servers/src/memory`](https://github.com/modelcontextprotocol/servers/tree/main/src/memory) (MCP Graph-Based Semantic Memory)
*   **The Concept:** An MCP server that maintains persistent semantic entity-relationship graphs.
*   **In OpenZ:** We leverage this exact pattern to build entity-relation indices. OpenZ uses native graph-memory tools to maintain knowledge graphs and execute semantic context traversals.

### 52. [`modelcontextprotocol/servers/src/sequentialthinking`](https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking) (MCP Sequential Planning)
*   **The Concept:** An MCP server designed to support step-by-step reasoning and logical progression before code execution.
*   **In OpenZ:** OpenZ ships native sequential-thinking tools to help the model plan structural edits and reason systematically in complex files without requiring a separate MCP server.

### 53. [`modelcontextprotocol/servers/src/git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git) (MCP Git Integration)
*   **The Concept:** An MCP server exposing git repository tools (status, diff, log, commit).
*   **In OpenZ:** OpenZ implements a native Rust-based `GitManagerTool` (`src/tools/git_manager.rs`) to track repository status, commits, and diffs directly in-process without requiring external daemon executions.

### 54. [`modelcontextprotocol/servers/src/fetch`](https://github.com/modelcontextprotocol/servers/tree/main/src/fetch) (MCP Web Fetching & Markdown Converter)
*   **The Concept:** An MCP server that fetches pages and parses them into clean Markdown.
*   **In OpenZ:** We implement this capability directly in-process via our native `web_fetch` scraper (utilizing the `scraper` and `html2md` crates), delivering sub-millisecond page fetches and cleaner token-saving formatting.

### 55. [`modelcontextprotocol/servers/src/time`](https://github.com/modelcontextprotocol/servers/tree/main/src/time) (MCP Time & Date Context)
*   **The Concept:** An MCP server providing local time and timezone transformations.
*   **In OpenZ:** OpenZ uses the native `chrono` library to manage date and time boundaries inside scheduled cron tasks and logs, rendering time contexts locally.

### 56. [`nousresearch/hermes-agent`](https://github.com/nousresearch/hermes-agent) (Self-Evolution & Skill Optimization)
*   **The Concept:** An autonomous agent platform designed for long-term memory, self-learning, asynchronous subagents, and self-evolution to optimize its own skills over time.
*   **In OpenZ:** OpenZ incorporates a closed-loop self-improvement curator that asynchronously reviews conversation histories, extracts factual memories, and updates procedural skill guidelines dynamically inside `~/.openz/skills/*.md`.

### 57. [`HKUDS/nanobot`](https://github.com/HKUDS/nanobot) (Lightweight AI Developer Core)
*   **The Concept:** An ultra-lightweight, personal AI developer assistant written in Rust supporting multi-platform integrations, MCP server bridging, and cron task scheduling.
*   **In OpenZ:** OpenZ is the direct successor and rebrand of `nanobot`, preserving its clean event-driven agent loop while expanding it with robust multi-channel adapters, gRPC bridges, and SQLite database storage.

### 58. [`zeroclaw-labs/zeroclaw`](https://github.com/zeroclaw-labs/zeroclaw) (Secure Zero-Overhead Runtimes)
*   **The Concept:** A high-performance, zero-overhead autonomous AI agent runtime written entirely in Rust, prioritizing secure local executions and systems-level efficiency.
*   **In OpenZ:** We target an identical lightweight systems footprint and execution speed. OpenZ's `SecurityGuard` permissions and BPF-based seccomp sandboxing align directly with `zeroclaw`'s secure, deny-by-default runtime execution.

### 59. [`RightNow-AI/openfang`](https://github.com/RightNow-AI/openfang) (Modular Agent Operating Systems)
*   **The Concept:** An Agent Operating System in Rust using modular capability packages ("Hands"), WASM engines, and cryptographic Merkle audit trails.
*   **In OpenZ:** OpenZ implements modular, pre-configured capability sets, supports WASM execution sandboxes (`wasm_sandbox`), and utilizes a cryptographic Merkle Hash-Chain ledger to track and audit all actions securely.

### 60. [`agent0ai/dox`](https://github.com/agent0ai/dox) (Hierarchical Context Resolution)
*   **The Concept:** A token-efficient codebase context framework that establishes a hierarchical tree of `AGENTS.md` files (from project-level down to folder-specific instructions) so AI agents can navigate directories dynamically.
*   **In OpenZ:** We natively support this hierarchical folder rules resolution. By utilizing the native `scope_context` tool, OpenZ automatically traverses directory structures to parse local `AGENTS.md` context layers and scope-limits code files during workspace edits.

### 61. [`loops!`](https://github.com/agent-skills) (Iterative Loop Engineering)
*   **The Concept:** Designing autonomous AI workflows as persistent feedback loops (Action → Observation → Decision/Refinement → Repeat) that iteratively test and self-heal code execution rather than single-shot prompts.
*   **In OpenZ:** The Stateful TurnState machine and the **SOP Workflow Engine** run on this exact looping model. Workflows (like `ship-pr-until-green`) compile, execute, read compiler stderr/stdout logs, and feed errors back to the LLM to refine and heal code iterations recursively.

---

## ⚡ Key Features & Subsystems

### 1. Memory & Skill Self-Improvement 🧠
*   **Dual-Tier Memory System:**
    *   *Tier 1 (Factual Memory):* Captures user preferences, persona, and session facts inside the session JSON (`session.metadata["memory"]`).
    *   *Tier 2 (Procedural Skills):* Stores recipes, conventions, and troubleshooting guidelines inside a local SQLite database (`~/.openz/memory.db`).
*   **Closed-Loop Background Curator:** Asynchronously reviews conversation history after each turn (using `tokio::spawn`), compiles new guidelines, and isolates skills by subagent profile (e.g. `profile = 'planner'`).
*   **Curator Throttling:** Avoids redundant LLM review calls on simple queries by throttling runs (requiring >4000 tokens of context or a tool call that modifies files/executes commands/uses the web).
*   **Stale Skills Archival Throttling:** Throttles background database cleaning checks (`archive_stale_skills`) to run at most once every 24 hours.

### 2. Native Compiler Auto-Healing 🛠️
*   **Self-Healing Loop (`compiler_auto_heal`):** Native reflection tool that executes build/compile commands (e.g., `cargo check` or `npm run build`), captures compiler errors (stderr/stdout), feeds them back to the LLM, and refines edits in an iterative loop (up to 5 iterations) until compilation succeeds.

### 3. Stateful SOP Workflow Engine (loops!-inspired) 📋
*   **DAG Execution:** Executes multi-step Directed Acyclic Graph (DAG) procedures in parallel using Tokio.
*   **Built-in SOPs:**
    *   `ship-pr-until-green`: Feature implementation, PR creation, CI verification loop, and self-healing.
    *   `pre-commit-guard`: Pre-commit hook configuration and workspace validation.

---

## 🔌 Model Context Protocol (MCP) Integration & Servers

OpenZ communicates with external tool servers using the Model Context Protocol (MCP) to extend its capabilities. It supports two primary communication architectures:
1.  **Stdio JSON-RPC:** Spawns external processes with standard pipe redirection (`stdin`/`stdout`).
2.  **Unified gRPC (Tonic):** To prevent third-party logging output ("stdio pollution") from breaking the JSON-RPC parser, OpenZ runs an automatic in-process bridge. It maps stdio-based servers to an ephemeral gRPC port on localhost, automatically filtering out non-JSON log lines.

### Native Tool Replacements And External MCP Servers
OpenZ prioritizes native Rust tools for core capabilities that used to live behind MCP servers:
*   **Headroom/context compression:** Native `scope_context`, `compress_content`, `retrieve_original`, and CCR/cache tools scan `AGENTS.md` context and compress long outputs.
*   **Sequential thinking:** Native `sequentialthinking`, `analyze_graph`, `export_session`, `summarize_reasoning`, and `reasoning_templates` tools support structured planning.
*   **Memory graph:** Native graph-memory, extended memory, research brief, knowledge source, and workflow tools store persistent knowledge in SQLite.
*   **External MCP:** OpenZ still supports stdio/gRPC MCP servers for optional integrations and tools that remain external.

### MCP Management:
*   **Dynamic configuration:** The agent manages server registrations via the `manage_mcp` tool.
*   **`mcps_manager` subagent:** A protected subagent profile equipped to download, compile, configure, and install MCP servers on demand.

---

## 🔧 Core Tools Registry & Usages

OpenZ exposes a robust, local tool set categorized below:

### 1. Filesystem & Repository Analysis
*   `read_file` / `write_file` / `patch_file`: Reads, writes, or patches target text files recursively.
*   `find_files`: Searches for files matching glob patterns with size and time filtering.
*   `replace_lines`: Replaces exact line sequences within a file (surgical line-level edits).
*   `zenflow_edit`: Multi-file structural editing with smart context matching (requires git repository).
*   `list_dir`: Lists directory contents including sizes and subfolders.
*   `grep_search`: Highly optimized ripgrep wrapper for locating patterns across codebases.
*   `code_outline`: Generates class, struct, function, and interface outline trees (Rust, Python, Go, JS/TS).
*   `ast_grep`: Executes structural AST searches (e.g. matching syntax patterns).
*   `index_codebase`: Indexes codebase structure into a structured JSON summary.
*   `git_manager`: Executes git operations (status, diff, log, commits).
*   `db_inspector` / `db_write`: Secure SQLite database reader and query writer.
*   `doc_reader`: Extracts text from PDF, DOCX, and XLSX files.
*   `rust_docs`: Queries Rust documentation from docs.rs for crate API references.
*   `compile_template`: Compiles Handlebars/Mustache templates with provided context data.

### 2. Sandbox & Compilation
*   `exec_command`: Runs sandboxed shell commands using a Linux BPF seccomp sandbox filter (if enabled).
*   `python_sandbox`: Executes Python scripts in an isolated subprocess with resource limits.
*   `wasm_execute`: Executes WebAssembly (`.wasm`) binaries inside a secure, sandboxed `wasmtime` engine.
*   `cargo_manager`: Runs compilation and testing (`cargo check`, `cargo build`, `cargo test`).
*   `js_format`: Fast JavaScript/TypeScript syntax formatting.
*   `compiler_auto_heal`: Automatically diagnoses and fixes compilation errors.

### 3. Web Search, Scraping & Social
*   `web_search`: Conducts Tavily web queries to return search results.
*   `web_fetch`: Scrapes HTML pages and converts them to formatted markdown.
*   `social_search`: Searches Hacker News, Reddit, and other social platforms for content.
*   `crawl_website`: Performs multi-threaded async site spidering via `spider-rs`.
*   `gsd_browser`: Direct headless Chrome automation (Playwright-based).
*   `obscura_browser` / `firefox_browser`: CDP-based headless browser controls.
*   `semantic_search`: Performs vector-based semantic search across a codebase using embeddings.

### 4. Job Scheduling & Cron
*   `schedule_job`: Registers recurring background cron tasks or one-time timers.
*   `list_jobs` / `remove_job`: Lists or deletes registered jobs.
*   `file_watcher`: Watches local folders to trigger scripts/commands when files change.

### 5. Memory & Knowledge
*   `store_memory`: Stores structured observations, decisions, or facts in the agent's long-term memory.
*   `recall_memory`: Retrieves stored memories by query context.
*   `clear_memory`: Clears all entries from the agent's memory store.
*   `archive_research`: Archives research findings into persistent storage.
*   `search_research`: Searches archived research content.
*   `index_notes`: Indexes and searches local markdown notes.

### 6. Graphics & Visuals
*   `render_mermaid`: Renders 23+ diagram formats directly to SVG.
*   `generate_video`: Compiles JSON timeline descriptions to MP4 files via `wavyte`.
*   `generate_image`: Generates PNG images programmatically from HTML/CSS/SVG or URL.
*   `html_to_video`: Renders timeline-based MP4 videos from HTML/CSS animation templates.
*   `create_animated_svg`: Creates animated SVG files from motion descriptions.

### 7. Subagents, Messaging & Workflows
*   `delegate_task`: Runs isolated subtasks in a separate subagent context.
*   `parallel_research`: Runs multiple research subtasks in parallel and merges results.
*   `evaluator_optimizer_loop`: Iteratively generates and evaluates responses until quality criteria met.
*   `optimize_subagent`: Refines a subagent's system prompt using AI based on feedback.
*   `create_subagent` / `delete_subagent`: Dynamically creates or removes custom subagent profiles.
*   `trigger_sop`: Instantiates stateful workflows (SOPs).
*   `send_remote_input`: Forwards commands to other active agent sessions.
*   `onpkg`: Integrates with the `onpkg` package and stack manager.

---

## 🎮 Basic Usage & Console Commands

### Running Channels
*   **Terminal TUI:** `openz agent` (launches interactive terminal prompt).
*   **WebSocket gateway:** `openz gateway` (launches static Web UI server & completions API).
*   **Telegram bot:** `openz telegram` (polls configured bot token).
*   **Discord bot:** `openz discord` (connects as a Discord gateway bot).
*   **WhatsApp API:** `openz whatsapp` (spawns an Axum webhook receiver on port 8090).

### TUI Terminal Slash Commands
Inside `openz agent`, the user can issue direct slash commands:
*   `/memory` / `/memory add <fact>` / `/memory clear`: Manage Tier-1 facts.
*   `/skills` / `/skill view <name>` / `/skill add <name>` / `/skill delete <name>`: Manage Tier-2 database skills.
*   `/sop list` / `/sop instances` / `/sop trigger <id>`: Manage SOP workflow loops.
*   `/audit`: Verifies the cryptographic Merkle hash-ledger integrity and lists recent transactions.
*   `/clear`: Resets active conversation context window history.
*   `/status`: Lists loaded MCP servers, active session information, and resource use.

---

## 📅 Version Release History

### v0.0.100

**Safe Telegram Document Delivery:**
*   **Native Tool:** Added `telegram_send_document` for sending local files through the active OpenZ Telegram bot.
*   **Explicit Recipients:** Accepts only numeric Telegram chat IDs or `@username`; phone numbers and guessed/old chat targets are rejected.
*   **Approval-Gated:** Outbound file delivery is high-risk and requires user approval.
*   **Safety Limits:** Validates regular files and enforces Telegram's 50 MB document limit.
*   **No Polling Conflict:** Uses the active channel's native HTTP client instead of calling `getUpdates` or raw shell commands.
*   **Tests:** Added target validation regressions for chat IDs, usernames, and phone-number rejection.
*   **Chore:** Bumped version to `v0.0.100`.

### v0.0.99

**Extraction-Aware URL Scope:**
*   **Scrape/Download Support:** Explicit scrape, download, source-code, asset, and local-copy tasks may follow related embed and asset URLs.
*   **Research Safety:** Ordinary direct-page research remains page-local unless broader research is explicitly requested.
*   **Prompt Alignment:** Runtime guidance now matches the extraction exception.
*   **Chore:** Bumped version to `v0.0.99`.

### v0.0.98

**Direct Page Research Scope:**
*   **Page-Local Default:** A request containing one direct URL now stays on that page after the first successful fetch.
*   **No Unrequested Expansion:** Automatic GitHub, web-search, or related-source lookups are skipped unless the user asks for deeper, broader, comparative, or multi-source research.
*   **Opt-In Depth:** Explicit broader research still permits additional sources.
*   **Tests:** Added a regression for direct-page scope and explicit broader intent.
*   **Chore:** Bumped version to `v0.0.98`.

### v0.0.97

**Direct URL Research Deduplication:**
*   **Single Reader:** Direct URL research now uses one URL reader per turn; `web_fetch` and `searchxyz_read_url` cannot fetch the same document twice.
*   **Fragment-Aware:** URL fragments such as `#get-started` are ignored for network deduplication while remaining available to research-topic normalization.
*   **Retry Safety:** A failed first fetch is not marked complete, so fallback readers remain available when genuinely needed.
*   **Prompt Guidance:** Tool descriptions and runtime guidance now tell the orchestrator to choose one direct URL reader.
*   **Tests:** Added focused regressions for cross-reader and fragment-insensitive URL deduplication.
*   **Chore:** Bumped version to `v0.0.97`.

### v0.0.96

**Automatic Native Merge Retry:**
*   **Quality Gate:** `web_search` now checks top native SearchXyz result coverage for multi-term queries before returning first-backend results.
*   **Retry:** Weak/off-topic result sets automatically retry native SearchXyz with `merge_backends=true`, improving searches where one scraper returns unrelated pages.
*   **Efficiency:** Relevant native results still return immediately, avoiding extra backend calls when first-pass quality is good.
*   **Scope:** The retry rule is generic and query-driven, not hardcoded to Sakana, Rust, or any specific website.
*   **Tests:** Added focused regressions for weak-result retry and relevant-result no-retry behavior.
*   **Chore:** Bumped version to `v0.0.96`.

### v0.0.95

**Generic Search Ranking Hardening:**
*   **Dynamic Rarity:** SearchXyz ranking now weights query terms by how rare they are within the returned result set, so specific entities/pages beat broad generic matches.
*   **Coverage:** Results matching more distinct query terms receive stronger coverage bonuses.
*   **Specificity:** Entity/title starts and domain/path matches receive boosts, while generic root/docs/blog pages are demoted when they only match one term from a multi-term query.
*   **Scope:** This is generic ranking behavior for all SearchXyz queries, not a Rust-only special case.
*   **Tests:** Added focused regressions for `rust tokio async runtime` and non-Rust `sakana fugu pricing` ranking.
*   **Chore:** Bumped version to `v0.0.95`.

### v0.0.94

**Research Topic Hygiene:**
*   **URL Topics:** Direct URL research now preserves meaningful URL fragments, so `https://9router.com/#get-started` canonicalizes to `9router.com/get-started` instead of collapsing to only the host.
*   **Search Topics:** Saved `web_search` topics now strip tool-call phrasing such as `call web_search with query ...` before archive/brief normalization.
*   **Rust Crates:** Rust crate searches normalize to stable topics like `rust/tokio`, preventing noisy saved topics such as `call web search query rust tokio async runtime`.
*   **Tests:** Added a focused unit regression for URL fragment preservation and web-search phrase cleanup.
*   **Chore:** Bumped version to `v0.0.94`.

### v0.0.93

**Native Search Rescue:**
*   **Recovery:** `web_search` now returns direct docs.rs and crates.io results for known Rust crate queries when native SearchXyz scraper backends return no usable results.
*   **Tokio Case:** Queries such as `rust tokio async runtime` now recover to `https://docs.rs/tokio` and `https://crates.io/crates/tokio` without enabling external fallback.
*   **Efficiency:** Rescue results are generated locally from high-confidence crate names, avoiding extra network retries under scraper blocking.
*   **Tests:** Added focused regressions for Tokio rescue and unknown-crate non-rescue behavior.
*   **Chore:** Bumped version to `v0.0.93`.

### v0.0.92

**Native SearchXyz Result Ranking:**
*   **Ranking:** SearchXyz now scores results using query term coverage, title/snippet/url matches, phrase matches, trusted-domain bonuses, and low-quality URL penalties.
*   **Quality:** Search/tag/login/account/category URLs and common tracking parameters are penalized before first-success and merged results are truncated.
*   **Merge:** Multi-backend merge still deduplicates normalized URLs, then reranks the merged result set.
*   **Doctor:** `searchxyz_doctor` now reports ranking as enabled and lists the ranking signals.
*   **Tests:** Added focused regressions for title phrase ranking, low-quality URL penalties, and merge dedupe after ranking.
*   **Chore:** Bumped version to `v0.0.92`.

### v0.0.91

**Stronger Native SearchXyz Discovery:**
*   **Discovery:** When `SEARCHXYZ_SEARXNG_URL` is explicitly configured and backend order is not, SearchXyz now automatically moves `searxng` to the front for the strongest private/native discovery path.
*   **Control:** Added `SEARCHXYZ_SEARCH_BACKENDS` for explicit backend ordering; user order is preserved even when SearXNG is configured.
*   **Doctor:** `searchxyz_doctor` now labels each backend as `preferred`, `configured`, `missing_key`, `disabled`, or `keyless`.
*   **Hints:** Doctor now warns when SearXNG is not enabled, Brave is missing `SEARCHXYZ_BRAVE_API_KEY`, or only keyless scraper backends are active.
*   **Tests:** Added focused regressions for SearXNG preference, explicit backend order preservation, backend-list parsing, health labels, and scraper-only detection.
*   **Chore:** Bumped version to `v0.0.91`.

### v0.0.90

**Actionable Native Search Failures:**
*   **Improvement:** `web_search` native-only failures now include concrete next steps: run `searchxyz_doctor`, configure `SEARCHXYZ_SEARXNG_URL`, enable `diagnose_on_failure=true`, or explicitly use `search_policy=native_then_external` for one-off fallback.
*   **Feature:** Added `diagnose_on_failure` to `web_search`, appending the SearchXyz doctor report when native-only search fails.
*   **Refactor:** Extracted a shared SearchXyz doctor report builder so `searchxyz_doctor` and web search failure diagnostics stay consistent.
*   **Tests:** Added focused regressions for schema exposure and actionable native-only failure text.
*   **Chore:** Bumped version to `v0.0.90`.

### v0.0.89

**SearchXyz Native Doctor:**
*   **Feature:** Added `searchxyz_doctor`, a native health report for SearchXyz configuration, backend order, SearXNG/Brave/headless status, safety settings, cache/index/graph state, and local storage paths.
*   **Debugging:** Native-only search failures now have a direct diagnostic tool before users enable `native_then_external` fallback.
*   **Tests:** Added focused metadata and native tool registration coverage for the new doctor tool.
*   **Chore:** Bumped version to `v0.0.89`.

### v0.0.88

**Native Web Search Policy:**
*   **Default:** `web_search` now defaults to `search_policy=native_only`, using the embedded SearchXyz dispatcher as the primary and only path unless fallback is explicitly enabled.
*   **Feature:** Added `search_policy` values `native_only`, `native_then_external`, and `external_only`, with `OPENZ_WEB_SEARCH_POLICY` as a runtime override.
*   **Hardening:** Native-only failures now return an explicit policy error instead of silently falling through to Tavily, Exa, DuckDuckGo, or Mojeek.
*   **Tests:** Added focused regressions for native-only default behavior and fallback policy aliases.
*   **Chore:** Bumped version to `v0.0.88`.

### v0.0.87

**SearchXyz Research Hardening:**
*   **Feature:** Added SearchXyz include/exclude domain filters, optional multi-backend merge mode, compact diagnostics, explicit fetch cache modes, and `save_mode=none` for one-off checks.
*   **Security:** SearchXyz fetches now block localhost, private, link-local, documentation, and unspecified network targets by default, including redirect targets.
*   **Cache:** HTTP fetches now preserve ETag, Last-Modified, Cache-Control, and content type metadata, support 304 cache reuse, and fall back to stale cached bodies on live fetch failure when allowed.
*   **Research Quality:** Deep research now includes deterministic evidence summaries with claims, evidence sources, potential conflicts, and unknowns before compiled documents.
*   **Tests:** Added focused regressions for backend merge/dedupe, domain filters, diagnostics, private-network blocking, cache validators, stale fallback, Cache-Control TTLs, and evidence summaries.
*   **Chore:** Bumped version to `v0.0.87`.

### v0.0.86

**Vision & GUI Open Workflow Discipline:**
*   **Fix:** Non-vision active models now instruct OpenZ to call the specialized `vision_agent` tool directly for image analysis instead of using generic `delegate_task`, avoiding non-vision model routing failures.
*   **Improvement:** Local GUI display requests prefer `open_path` and device inventory before ad hoc shell launchers or explicit desktop app guesses.
*   **Analysis:** The v0.0.81 transcript issues were generic routing/orchestration problems, not image-generation failures: delegate misuse, noisy GUI retries, malformed shell copy attempts, and unclear GUI/headless distinction.
*   **Chore:** Bumped version to `v0.0.86`.

### v0.0.85

**Generic Dynamic Loop Guard:**
*   **Improvement:** Progress-aware repeat detection now applies to read-only/search/status tools, not only browser observation tools.
*   **Safety:** Mutating and side-effect tools keep strict exact-repeat protection even when outputs differ.
*   **Design:** Loop safety is derived from tool-result signatures, avoiding global fixed limits and avoiding model-controlled limit escalation.
*   **Tests:** Added focused regressions for changed-state read-only repeats, stale read-only repeats, and mutating repeat strictness.
*   **Chore:** Bumped version to `v0.0.85`.

### v0.0.84

**Progress-Aware Browser Loop Guard:**
*   **Fix:** Browser observation repeats now compare tool-result signatures, so repeated `snapshot`, `page_source`, `accessibility_tree`, and `screenshot` calls are allowed when the observed page state changes.
*   **Safety:** Identical stale browser observations still count toward loop protection, preserving runaway-loop defense without a fixed blanket cap for useful progress.
*   **Tests:** Added focused regressions for changed-state and same-state browser observation repeats.
*   **Chore:** Bumped version to `v0.0.84`.

### v0.0.83

**Browser Acquisition & Research Hygiene Hardening:**
*   **Fix:** `gsd_browser` fill accepts `text`, `value`, `query`, and `content` aliases, avoiding missing-text failures for search-box input.
*   **Fix:** Repeated read-only browser observation actions (`snapshot`, `page_source`, `accessibility_tree`, `screenshot`) no longer trigger duplicate-loop blocking.
*   **Improvement:** Runtime guidance now uses a browser-backed fallback ladder for find/download/open/show workflows before declaring a task blocked.
*   **Improvement:** Research auto-capture skips download/display workflows unless the user explicitly asks for research, comparison, or analysis.
*   **Docs:** Added implementation plan for browser acquisition hardening.
*   **Tests:** Added focused browser schema, loop-control, and research auto-capture regressions.
*   **Chore:** Bumped version to `v0.0.83`.

### v0.0.82

**Device Inventory Memory:**
*   **Feature:** Added native `device_inventory` tool for local device/app capability CRUD: `list`, `get`, `add`, `update`, `delete`, `record_success`, `record_failure`, and `suggest`.
*   **Feature:** Added `/device` TUI command for manually listing, suggesting, adding, deleting, and scoring local capabilities from the terminal UI.
*   **Improvement:** `open_path` records successful default opens into `~/.openz/device_inventory.json`, so generated images, videos, PDFs, URLs, and editable files build local app preference memory automatically.
*   **Safety:** Device inventory stores and ranks known capabilities but does not execute arbitrary remembered commands; execution remains in existing guarded open/command tools.
*   **Tests:** Added focused device inventory CRUD, suggestion ranking, auto-learn, and native registry coverage.
*   **Chore:** Bumped version to `v0.0.82`.

### v0.0.81

**Mivi Model Menu & Streaming Recovery Hardening:**
*   **Fix:** `/model` now includes the built-in `mivi` local provider in the provider list.
*   **Fix:** Built-in providers merge configured `default_model` values into selectable model options, so custom names like `mivi llm` appear.
*   **Fix:** Streaming recovery no longer falls through to the placeholder failure text when the provider emits reasoning-only output twice; it uses the recovered reasoning as visible final content.
*   **Tests:** Added focused mivi default-model regression coverage and reran streaming assembly coverage.
*   **Chore:** Bumped version to `v0.0.81`.

### v0.0.80

**Streaming Recovery Stream Repair:**
*   **Fix:** Reasoning-only streaming fallback now recovers the final answer via `chat_stream_with_fallback`, so recovered answer content can still render chunk-by-chunk.
*   **Fix:** The hidden recovery request remains silent and does not show a visible recovery spinner label.
*   **Behavior:** OpenZ now treats provider `reasoning_content` deltas and final `content` deltas as separate phases, which explains why some providers can stream thoughts before answer text.
*   **Tests:** Ran focused streaming assembly coverage and compile validation.
*   **Chore:** Bumped version to `v0.0.80`.

### v0.0.79

**Streaming Defaults & TUI Control:**
*   **Behavior:** Response streaming now defaults to disabled for new configs, matching the cleaner non-streaming TUI output path.
*   **Feature:** Added `/streaming` inside `openz agent` with Enable, Disable, and Back menu options plus current-state display.
*   **Fix:** Internal final-answer recovery no longer displays a visible `Recovering final answer...` spinner label.
*   **Feature:** `/settings` now includes the active streaming mode.
*   **Chore:** Bumped version to `v0.0.79`.

### v0.0.78

**Streaming Thought Final-Answer Recovery:**
*   **Fix:** Streaming reasoning-only responses now make one answer-recovery call instead of ending after a Thought block.
*   **Behavior:** The TUI prints formatted `Thought for X.Xs` output first, then displays the recovered user-facing answer.
*   **Fix:** The fallback continues to avoid dumping provider reasoning as plain final text.
*   **Chore:** Bumped version to `v0.0.78`.

### v0.0.77

**Streaming Thought Fallback Repair:**
*   **Fix:** Streaming reasoning-only responses no longer dump provider reasoning as unformatted plain answer text.
*   **Fix:** When TUI thoughts are enabled, reasoning-only fallback output now uses the normal `Thought for X.Xs` tree block.
*   **Behavior:** The fallback remains visible so reasoning-only model responses do not disappear after the thinking spinner.
*   **Chore:** Bumped version to `v0.0.77`.

### v0.0.76

**Configurable TUI Thought Display:**
*   **Behavior:** Restored the old default TUI reasoning display with live `▶ Thinking...` seconds and `Thought for X.Xs` blocks before final answers.
*   **Feature:** Added `/tui thoughts full|compact|off` and `/tui settings` slash commands.
*   **Feature:** Added a TUI category to `openz configure` for changing thought display mode.
*   **Feature:** `manage_config` can now update `tui_thought_display`, allowing the agent to change the setting when asked.
*   **Tests:** Added focused coverage for TUI thought display mode normalization and compact summary truncation.
*   **Chore:** Bumped version to `v0.0.76`.

### v0.0.75

**TUI Reasoning Privacy Repair:**
*   **Fix:** Normal streaming answers no longer print provider reasoning as `Thought` before the final answer.
*   **Fix:** Reasoning chunks no longer leave `▶ Thinking...` artifacts in the terminal for simple answer turns.
*   **Behavior:** Tool-call turns may still show compact Thought context so tool execution remains understandable.
*   **Chore:** Bumped version to `v0.0.75`.

### v0.0.74

**Reasoning-Only TUI Response Repair:**
*   **Fix:** Streaming responses that contain only model reasoning no longer finish with a `Thought` block and no assistant answer.
*   **Fix:** Reasoning-only fallback content is no longer marked as already streamed, so the CLI/TUI prints a visible final response.
*   **Tests:** Added focused coverage for reasoning-only streaming behavior.
*   **Chore:** Bumped version to `v0.0.74`.

### v0.0.73

**Research Topic Canonicalization Repair:**
*   **Fix:** Non-GitHub URL research topics now use stable `host/path` canonical form, preventing duplicate topics such as `sakana.ai fugu` and `sakana.ai/fugu`.
*   **Fix:** Repository topic promotion now verifies actual repo source evidence instead of treating every slash-containing website topic as a repo topic.
*   **Improvement:** `openz doctor` repairs legacy research brief topics by merging or renaming vague aliases into source-proven canonical topics without deleting brief content.
*   **Tests:** Added focused regressions for Sakana-style website topics, Hermes-style repo alias repair, and legacy website space-topic repair.
*   **Chore:** Bumped version to `v0.0.73`.

### v0.0.72

**Stable Query Memory Efficiency:**
*   **Fix:** Stable definition/comparison questions now keep using fresh research briefs even if the model-generated tool arguments contain a URL; explicit user live intent still refreshes exact sources.
*   **Fix:** Saved source match notifications are suppressed when fresh research brief context already matched, preventing duplicate memory/source footers on simple questions.
*   **Improvement:** Research auto-capture prefers existing canonical repo topics for short aliases such as `Hermes`, avoiding vague duplicate brief topics like `hermes`.
*   **Tests:** Added focused regressions for stable argument URL policy, brief-first lookup blocking, source notification suppression, and alias-to-repo topic capture.
*   **Chore:** Bumped version to `v0.0.72`.

### v0.0.71

**Memory Database Display Repair:**
*   **Fix:** Legacy saved GitHub source labels are repaired at display time from their URI, so existing database rows no longer show labels like `github.com - pulls` or `github.com - 6`.
*   **Fix:** Research brief search now ignores summaries dominated by GitHub UI chrome, sort/filter controls, footer navigation, cookie text, or action-denied boilerplate.
*   **Tests:** Added focused coverage for legacy GitHub label repair and noisy GitHub brief rejection while preserving normal source/research behavior.
*   **Chore:** Bumped version to `v0.0.71`.

### v0.0.70

**Live Refresh Source Hygiene:**
*   **Fix:** Saved research/source match notifications are now suppressed on live-intent prompts such as direct URLs, explicit research, and check-again/refresh requests.
*   **Fix:** Refresh-only URL checks no longer auto-save duplicate research briefs when the user is only verifying whether a page changed.
*   **Improvement:** GitHub source labels now use readable repo/issue/PR forms instead of lossy labels like `github.com - 6`.
*   **Tests:** Added focused regressions for live notification suppression, GitHub label formatting, refresh-only auto-capture skipping, and preserved normal research capture.
*   **Chore:** Bumped version to `v0.0.70`.

### v0.0.69

**Research Memory Relevance & Hardcoded Cleanup:**
*   **Fix:** Research brief lookup now requires topic-anchor relevance, preventing broad comparison briefs from matching unrelated entity questions through summary-only terms.
*   **Fix:** Saved source lookup now requires label, URI, or alias relevance before injecting source context or displaying source-match notifications.
*   **Fix:** Auto-capture now skips non-research/debug turns such as local explanation prompts, while preserving real research prompts and natural definition follow-ups.
*   **Hardening:** Centralized live research intent policy, replaced fixed-year matching with runtime current-year detection, removed brittle external-project inventory gates, and replaced exact model exceptions with generic strong-model heuristics.
*   **Tooling:** `render_video` no longer hardcodes `/home/aswin` paths and now accepts portable CLI arguments for input/output, dimensions, FPS, duration, frame tick JavaScript, and render delays.
*   **Tests:** Added focused regressions for unrelated source suppression, topic-anchor brief matching, non-research auto-capture skipping, and preserved source/brief behavior.
*   **Chore:** Bumped version to `v0.0.69`.

### v0.0.68

**Web Cache Validator Hardening:**
*   **Fix:** `web_fetch` now refreshes exact-URL cache validators, fetch time, expiry time, and use count when an origin returns `304 Not Modified`.
*   **Fix:** Responses without `Cache-Control` now use `Last-Modified` heuristic freshness instead of expiring immediately.
*   **Tests:** Added focused regression coverage for missing `Cache-Control` plus `Last-Modified` behavior.
*   **Chore:** Bumped version to `v0.0.68`.

### v0.0.67

**Exact URL Web Cache & HTTP Revalidation:**
*   **Feature:** Added an exact URL `web_fetch_cache` SQLite table storing body text, `ETag`, `Last-Modified`, `Cache-Control`, fetch time, expiry time, status code, and use count.
*   **Feature:** `web_fetch` now supports `cache_mode` values `auto`, `prefer_cache`, `revalidate`, and `bypass` for deterministic cache policy control.
*   **Feature:** Stale cached pages are revalidated with `If-None-Match` and `If-Modified-Since`; `304 Not Modified` reuses the cached body without redownloading content.
*   **Resilience:** If a live fetch fails or returns an error and an exact URL cache entry exists, OpenZ falls back to stale cached content instead of dropping the answer path.
*   **Prompt Discipline:** The orchestrator is instructed to use `cache_mode="revalidate"` for direct URL checks and check-again/verify/refresh/browse requests.
*   **Tests:** Added focused cache-control and cache-mode regression tests.
*   **Chore:** Bumped version to `v0.0.67`.

### v0.0.66

**Generic Live Research Policy:**
*   **Fix:** Saved research briefs no longer block explicit live web/search tool calls when the user provides a URL, the tool arguments contain a URL, the prompt asks for current/latest information, or the user asks to check/verify/refresh again.
*   **Fix:** Direct URL prompts now keep research context available but force exact-source/web refresh before final answers, preventing broad topic briefs from answering specific pages.
*   **Preserved:** Fresh briefs still short-circuit stable non-live definition and comparison prompts so repeated simple questions remain fast and token-efficient.
*   **Tests:** Added focused regression coverage for live lookup bypass and prompt-context behavior.
*   **Chore:** Bumped version to `v0.0.66`.

### v0.0.65

**Configure Exit Reliability & Custom Provider Support:**
*   **Fix:** `openz configure` now exits cleanly after save/back/Esc flows by disabling raw mode and terminating the command process, preventing background maintenance tasks from keeping configure alive.
*   **Feature:** Added `Add Custom Provider` in the Providers configure menu for OpenAI-compatible endpoints with provider name, API base URL, API key, and default model fields.
*   **Feature:** Custom providers are now first-class in TUI `/model` and text-channel `/switch-model` flows, including `custom_provider/model` prefix routing through the OpenAI-compatible provider adapter.
*   **Feature:** Local custom providers on `localhost`/`127.0.0.1` can run without an API key; remote custom providers support env fallback keys like `OPENZ_PROVIDER_<NAME>_API_KEY`.
*   **Tests:** Added focused coverage for custom provider config resolution, resolver routing, and model-switch rendering.
*   **Chore:** Bumped version to `v0.0.65`.

### v0.0.64

**Comprehensive Performance & Footprint Optimization (RAM, CPU, Storage, Binary Size):**
*   **SQLite Memory Tuning:** Added memory-constraining PRAGMAs (`cache_size=-2000`, `mmap_size=0`, `synchronous=NORMAL`, `wal_autocheckpoint=1000`) across all 7 database connection initialization sites, capping page cache memory from ~56 MB to ~14 MB.
*   **FastEmbed Idle Eviction:** Implemented `ManagedModel` with automatic 5-minute idle eviction in `src/tools/shared_memory/embeddings.rs` and background lifecycle loop in `src/cli/mod.rs`, freeing ~130 MB ONNX vector model weights from resident memory.
*   **Tokio Runtime Right-Sizing:** Replaced default `#[tokio::main]` runtime (16 worker threads, 2 MB stacks = 32 MB) with a right-sized multi-thread runtime of **4 worker threads** and **512 KB stack size** in `src/main.rs`.
*   **jemalloc Global Allocator:** Integrated `tikv-jemallocator` in `Cargo.toml` and `src/main.rs` to purge unallocated heap memory back to the kernel within 1 second, eliminating glibc heap fragmentation.
*   **Binary Size Profile:** Added `[profile.release-small]` profile in `Cargo.toml` featuring `opt-level = "z"`, `lto = "fat"`, `panic = "abort"`, and `strip = true` for minimal executable footprint.
*   **Auto Maintenance & Feature Pruning:** Added startup background SQLite auto-optimizer (`start_database_auto_optimizer`) and pruned unneeded Tokio features.
*   **Chore:** Bumped version to `v0.0.64`.

### v0.0.63

**Native Browser Status Inspection & Subagent Orchestration Consolidation:**
*   **Feature:** Built native `inspect_browsers` tool for OpenZ to monitor running browser sessions (Firefox GeckoDriver port 4444, Chrome CDP port 9222), `gsd-browser` background daemons/pages, and recent browser error logs from `logs.db`.
*   **Architecture:** De-duplicated subagent orchestration into `SubagentRunContext` and `CancelGuard`, removing 800+ lines of duplicate workspace, git worktree, database branch, and reflection loop boilerplate.
*   **Architecture:** Refactored `ToolMetadata` into a clean builder pattern, defined dynamic domain keyword matching, and overrode `metadata()` directly on tool implementations in `src/tools/mod.rs`.
*   **Chore:** Bumped version to `v0.0.63`.

### v0.0.62

**Operational Source-Context Guard:**
*   **Fix:** Local operational prompts such as opening a generated website/video now skip research/source-memory retrieval, so unrelated saved source footers do not appear during app/file launch turns.
*   **Fix:** Acknowledgement-only replies such as “that’s good one” now skip research/source matching instead of pulling arbitrary fresh sources.
*   **Tests:** Added regression coverage for the v0.0.60 transcript pattern while preserving saved-source retrieval for explicit external comparisons and research questions.
*   **Chore:** Bumped version to `v0.0.62`.

### v0.0.61


**Documentation & Inventory Alignment:**
*   **Docs:** Refreshed ONPKG PRD/design/implementation/todo docs with concrete OpenZ runtime requirements and backlog items instead of skeleton placeholders.
*   **Docs:** Updated MCP documentation to distinguish native Rust tools from external MCP servers, avoiding stale claims that Headroom, sequential thinking, and memory graph are default MCP servers.
*   **Docs:** Updated channel and tool-usage guidance for Email, 300s default tool timeout, managed server lifecycle, and detached GUI launches.
*   **Metadata:** Updated ONPKG self-management tool inventory to include `tool_catalog`, `openz_inventory`, `manage_servers`, `workflow_memory`, `manage_config`, `diagnose_system`, `manage_sessions`, and `manage_backups`.
*   **Chore:** Bumped version to `v0.0.61`.

### v0.0.60


*   **Fix: runtime model identity grounding:** `openz_inventory` now reports live runtime identity fields including configured model, configured provider, resolved effective provider/model when available, vision support, caveman mode, and streaming status.
*   **Prompt hardening:** Model/provider identity questions such as “what model are you?” and model-capability questions such as “which programming language are you best at?” now require `openz_inventory` before answering and explicitly forbid guessing hidden architecture, training data, parameter count, or benchmark ranking.
*   **Chore:** Bumped version to `v0.0.60`.

### v0.0.59

*   **Fix: self-inventory source suppression:** OpenZ now treats prompts like “what tools do you have?” and “what features do you have?” as internal inventory questions, so saved external research sources such as OpenHuman/Hermes are not injected or shown in the source footer.
*   **Guard: comparison prompts still use sources:** Explicit comparison prompts such as “what features do you have vs Hermes?” continue to allow saved source/research context.
*   **Chore:** Bumped version to `v0.0.59`.

### v0.0.58

*   **Feature: live OpenZ inventory:** Added the native `openz_inventory` tool so agents can report exact running-version capabilities, registered tools by domain, channels, commands, subagents, and active managed servers without guessing from memory.
*   **Hardening: automatic server lifecycle behavior:** Elevated `manage_servers` to a high-priority core tool and updated runtime prompt discipline so OpenZ uses it automatically for registered dev-server cleanup instead of relying on user slash commands or shell `pkill` guesses.
*   **Hardening: reusable generation workflows:** The self-improvement curator now documents and accepts `sources_to_save` and `workflows_to_save`, with explicit patterns for chunked website writes, segmented HTML-video rendering, managed server cleanup, and live inventory answers.
*   **Skill: generated media/site procedures:** Added built-in skills for chunked static-site generation and segmented HTML video rendering so future website/video tasks reuse verified workflows instead of rediscovering them.
*   **Chore:** Bumped version to `v0.0.58`.

### v0.0.57

*   **Feature: managed background servers:** Detached dev-server commands are now registered with id, pid, command, kind, and start time metadata so they can be inspected and stopped later.
*   **Feature: server lifecycle controls:** Added `/servers` and `/stop-server <id|all>` in TUI and Telegram, plus a native `manage_servers` tool so OpenZ can list or stop its own launched servers automatically.
*   **Bugfix: approval menu responsiveness:** TUI menus now drain stale key events after enabling raw mode, reducing delayed `Enter`/arrow handling after cancellations and long tool turns.
*   **Chore:** Bumped version to `v0.0.57`.

### v0.0.56

*   **Bugfix: TUI cancellation cleanup:** Esc/Ctrl+C turn cancellation now gives the keyboard watcher a short clean shutdown window so stale terminal readers do not keep stealing keystrokes from the next prompt or security approval menu.
*   **Bugfix: detached desktop/server launches:** `exec_command` now detects common GUI launchers, browsers, media players, editors, and dev-server commands and starts them detached, preventing OpenZ from sitting forever on `bash command running...` after the app opens.
*   **Bugfix: viewer retry suppression:** `open_path` and detached shell launches now return `user_visible` and `do_not_retry` guidance so models stop retrying with browser/VLC/alternate viewers after a file or app has already been shown.
*   **Chore:** Bumped version to `v0.0.56`.

### v0.0.55

*   **Hardening: research memory reliability:** Simple follow-up prompts now reuse fresh canonical research briefs without repeated web calls, while explicit research/link-analysis prompts bypass the no-fetch gate so OpenZ can fetch README/docs/site pages when the user asks for research.
*   **Hardening: canonical research topics:** URL-plus-instruction prompts, GitHub links, and raw GitHub URLs now save under stable repo topics like `agent0ai/dox` and `tinyhumansai/openhuman` instead of weak aliases like `dox` or `openhuman`.
*   **Hardening: repo/docs brief freshness:** Auto-saved repo/docs briefs now inherit source TTLs instead of expiring after 60 seconds, keeping useful repository and documentation research fresh for about a week by default.
*   **Hardening: skipped and invalid brief protection:** Fresh-brief skip responses are no longer auto-saved, and placeholder summaries such as `skipped` are rejected on save and ignored during retrieval so corrupted rows cannot block needed refreshes.
*   **Hardening: high-signal research summaries:** Auto-captured research briefs now prefer definition/architecture sentences and trim leading navigation/sidebar/legal noise before saving summaries.
*   **Hardening: source-strict saved-brief prompting:** Saved research context now instructs models to state only facts present in briefs/sources and say `unknown` for missing details instead of guessing licenses, channels, releases, integrations, or comparisons.
*   **Chore:** Bumped version to `v0.0.55`.

### v0.0.54

*   **Feature: freshness-aware source memory:** Source bookmarks now carry `fresh`, `stale`, or `unknown` status from TTL-aware timestamps. Newly saved sources are marked checked, volatile source types get shorter default TTLs, and stale/current-sensitive sources are injected with explicit refresh guidance.
*   **Feature: deterministic source ranking:** Automatic source retrieval now prefers exact label/alias matches, official docs/repos/local paths, higher trust, higher reuse, and fresh sources. `/sources` remains only an inspection/debug command; the prompt uses ranked sources automatically.
*   **Feature: stronger reusable workflow prompting:** Workflow matches now inject preconditions, concise steps, verification, risk, score, and instructions to record success/failure with `workflow_memory.record_run`, helping weak models reuse proven procedures instead of rediscovering tools.
*   **UX: compact automatic context notices:** TUI now emits small `Sources matched` and `Workflow matched` notifications when saved context is automatically injected.
*   **Feature: automatic research capture:** Successful web/search/research tool calls now save discovered URLs as `knowledge_source` bookmarks and persist a canonical `research_brief`, so repeated questions can reuse sources without manual commands.
*   **Feature: brief-first recall:** Fresh research briefs are injected before saved sources with explicit no-fetch guidance for simple definition/comparison prompts, so saved memory reduces unnecessary web calls.
*   **UX: batched auto-capture notices:** Multiple research tool calls in one iteration now emit one compact auto-save notification instead of one notice per tool.
*   **Hardening: research memory cleanup:** Casual prompt prefixes like `hey whats` are stripped from saved research topics, duplicate auto-capture topics are collapsed in TUI notices, source/brief match thresholds are stricter, and the self-improvement curator is debounced for fast repeated turns.
*   **Hardening: generic research guard:** Topicless update prompts like `hey whats new` no longer inject saved research/source memory, preventing unrelated source notices on fresh sessions. Topic-specific prompts like `whats new in hermes` still use saved context.
*   **Hardening: forgiving research brief tool args:** `research_brief` now accepts weak-model aliases like `goal`, `context`, and `content`, and infers save/search action when `action` is omitted.
*   **Chore:** Bumped version to `v0.0.54`.

### v0.0.53

*   **Bugfix: TUI answer formatting restored:** Fixed the `<think>` sanitizer so it preserves markdown, bullets, and newlines in normal assistant answers instead of collapsing output into one long wrapped line.
*   **Cleanup: canonical new-session command:** Removed duplicate `/new` slash command from TUI and Telegram. `/new-session` is now the single command for starting a clean session.
*   **Feature: selectable resume UX:** Telegram `/resume` now presents inline buttons for previous sessions and a `Continue current session` option. TUI now uses `/history` as the single interactive session restore command with a `Continue Current Session` option.
*   **Bugfix: installer/update version reporting:** Local install/update scripts now extract a single plain `openz vX.Y.Z` line from `openz --version` so the post-install report no longer embeds a misaligned second ASCII logo.
*   **Feature: model reliability registry:** Added persistent `~/.openz/model_registry.json` health tracking for model risk, failures, blank replies, think leaks, and fallback success.
*   **Hardening: weak-model context support:** Added pinned identity/persona memory, recent session context, small-model operating rules, reasoning tag normalization, and self-improvement curator debounce.
*   **Feature: knowledge and workflow memory:** Added source bookmarks, research briefs, reusable workflow cards, CRUD tools (`knowledge_source`, `research_brief`, `workflow_memory`), prompt retrieval, curator learning hooks, and `/sources` + `/workflows` inspection commands.

### v0.0.52
*   **Feature: Multi-channel TUI remote control and interruption:** Hardened channel-to-TUI control so Telegram can select among live `openz agent` sessions and all remote channels can interrupt active turns.
    *   Added an active TUI session registry and Telegram `/remote` picker so one bot can select among currently running `openz agent` terminals, route messages to the selected session, and leave remote mode with `/local` or `/exit`.
    *   Added `/stop` interruption support across Telegram, Discord, WhatsApp, WebSocket browser chat, and Email so remote channels can cancel active turns through the same path as Esc/Ctrl+C in the TUI.
    *   Added `/cancel`, `/tui-esc`, and `/tui-cancel` as channel aliases for `/stop`, giving remote users explicit TUI-style interrupt commands.
*   **Feature: Channel model switching parity:** Added `/switch-model` outside the TUI so background channels can inspect configured providers and change the default provider/model without opening the terminal model picker.
    *   Telegram now exposes `/switch-model` as inline provider/model buttons and saves the selected default.
    *   Discord, WhatsApp, WebSocket browser chat, and Email support text commands: `/switch-model`, `/switch-model <provider>`, and `/switch-model <provider> <model>`.
    *   Agent turns now resolve the updated provider/model from config on reload, so provider changes take effect on later channel turns.
*   **Optimization: Balanced local update mode:** Added `--balanced`/`--moderate` to `localinstall.sh` and `localupdate.sh`, backed by `release-balanced` and `release-low-resource` Cargo profiles. Balanced mode caps jobs, skips the duplicate pre-install check during updates, and disables ThinLTO to reduce RAM/ROM pressure without the extreme slowdown of `--low-resource`.
*   **Chore:** Bumped version to `v0.0.52`.

### v0.0.51 (Previous Release)
*   **Feature: Dynamic tool timeout — orchestrator-driven adaptive execution:** Replaced the static 120s tool timeout with a 300s default and a bounded override system that lets tool metadata and orchestrator hints adapt timeouts per task complexity.
    *   Raised default tool timeout from 120s to 300s (`config/schema.rs`).
    *   Added `_timeout_secs` override field with bounded execution (`5s..1800s`) so any tool call can request a custom timeout without disabling safety limits.
    *   Added `recommended_timeout_secs` to `ToolMetadata` — tools declare their ideal timeout (delegate_task=600s, browser=600s, video=900s, crawl=600s, etc.) as hints when no explicit override is given.
    *   Added `timeout_secs` parameter to `delegate_task`, `delegate_profile`, and `parallel_research`; subagent timeouts now raise the outer tool timeout when needed and are clamped to the shared safety range.
    *   Replaced hardcoded 300s subagent timeouts in delegate_task.rs and delegate_profile.rs with bounded dynamic resolution.
    *   Made `parallel_research` actually apply per-task `timeout_secs` instead of only advertising the schema field.
*   **Fix: Runtime reliability regressions found during v0.0.50/v0.0.51 verification:** Hardened the worst-case paths that caused runtime diagnostics to disagree with source-level features.
    *   Fixed `ast_grep_index_codebase` so it writes structural symbols into the same `code_elements` store consumed by `query_code_graph`; code graph queries now work immediately after ast-grep indexing.
    *   Accepted both camelCase and snake_case memory scope arguments (`sessionId`/`session_id`, `userId`/`user_id`, `agentId`/`agent_id`) so scoped graph and code queries do not silently miss indexed rows.
    *   Improved fact extraction for compound build clauses such as `Alice built AppX with Rust` and chained clauses like `Bob created ToolY and built it with Go`.
    *   Migrated stale historical `toolTimeoutSecs: 120` config values to the new 300s default while preserving intentional custom timeout values.
    *   Improved subagent unsafe-workspace fallback reporting with actionable guidance and `workspaceIsolation` metadata.
*   **Chore:** Bumped version to `v0.0.51`.
*   **Optimization: Measured footprint reporting and release-size hardening:** Made local install/update scripts print installed binary size and version-command smoke time, and corrected footprint docs to report measured values instead of overclaiming a 10-15 MB binary. A `panic = "abort"` experiment was rejected because release linking conflicts with dependencies that require unwind support.
*   **Fix: Build-cache disk pressure guard:** Added `--clean-target` to `localinstall.sh` and `localupdate.sh`, a 20 GiB `target/` warning, and an `openz doctor` disk/cache report covering repo `target/`, `~/.openz`, SearchXyz, and Cargo caches so repeated OpenZ builds/tests cannot silently consume tens of gigabytes without an explicit cleanup path.

### v0.0.50 (Previous Release)
*   **Feature: Native memory coordinator and reliability overhaul:** Unified semantic, graph, recall, deletion, stats, and prompt-memory flows behind a coordinator path with regression coverage for worst-case memory behavior.
    *   Added a native `MemoryCoordinator` for semantic writes, graph relation writes, hybrid recall, forget operations, and memory health stats.
    *   Made semantic memory store deterministic embedding blobs and upgraded `hybrid_search` to combine FTS5 and vector similarity using reciprocal-rank fusion.
    *   Added `forget_memory` and routed deletion through semantic metadata, FTS rows, graph nodes/edges, shared memory, cognitive memory, research archives, session metadata, and skills-derived facts.
    *   Made cross-session prompt memory query-aware, stale-fact aware, deduplicated, and top-30 budgeted so unrelated memories do not leak into every prompt.
    *   Ported memory_rs-inspired auto-importance scoring and conservative conflict resolution for exclusive graph relations plus semantic slots such as current job, location, and preferences.
    *   Improved `extract_and_store_facts` to handle multi-word entities, profile-style facts, carried subjects across `and`, and richer relations like `built_with`, `lives_in`, and `prefers`.
    *   Expanded `memory_stats` with coordinator-backed active counts, embedding coverage, cognitive memory, research archive, session metadata memory, skills memory, working memory, and total active memory metrics.
    *   Improved codebase memory indexing for Rust trait impl blocks such as `impl Trait for Type`, preserving the implemented type and signature for code graph queries.
    *   Added semantic similarity conflict resolution so near-duplicate lower-importance facts are expired when a stronger replacement is written.
    *   Added regression eval tests for stale facts, contradictions, deletion, recall relevance, poisoning attempts, prompt budgeting, semantic embeddings, memory layer stats, codebase indexing, and coordinator write/forget flows.
*   **Chore:** Bumped version to `v0.0.50`.

### v0.0.49 (Previous Release)
*   **Feature: Headroom parity and hardening:** Brought the native Headroom tools closer to the original `agentcpower`/Headroom MCP behavior while keeping OpenZ's safer native execution model.
    *   Added `threshold`, `signatures_only`, and `model_hint` compression controls for content, file, and directory compression workflows.
    *   Added syntax-aware signature-only code compression so agents can preserve public structure while dropping function bodies.
    *   Added FTS5-backed cache search, optional session metadata, TTL eviction, byte-budget eviction, and environment-controlled cache limits.
    *   Added `headroom_stats` and `headroom_usage` analytics tools for compression history and model-attributed token savings.
    *   Hardened Headroom path handling with sensitive-file blocks, optional `HEADROOM_WORKSPACE` enforcement, safer export/import paths, SSRF URL validation, bounded `run_and_compress`, and confirmation-gated cache clearing.
    *   Expanded Headroom tests for security, FTS search, cache eviction, analytics logging, thresholds, and signature extraction.
*   **Chore:** Bumped version to `v0.0.49`.

### v0.0.48
*   **Fix: OpenMedia SVG text alignment:** Improved SVG/animated SVG logo quality by making centered text vertically align correctly by default.
    *   Added `dominant_baseline` support to OpenMedia SVG JSON text elements and emitted it as `dominant-baseline` in SVG output.
    *   Added OpenZ normalization for `dominantBaseline` / `alignmentBaseline` camelCase aliases.
    *   Defaulted OpenZ `openmedia_create_svg` text elements to `text_anchor=middle` and `dominant_baseline=middle` unless explicitly overridden, improving generated logo alignment from common prompts.
    *   Added `dominant_baseline` support to `create_animated_svg` text elements and defaulted centered animated text to a middle baseline.
*   **Chore:** Bumped version to `v0.0.48`.

### v0.0.47
*   **Fix: SearchXyz safety, persistence, and resource controls:** Hardened SearchXyz so web research, GitHub repo ingestion, and destructive maintenance tools behave safely and remain immediately searchable.
    *   Added `max_chars` output budgets for `read_url`, `search_and_read`, `deep_research`, `read_github_repo`, and `export_research`, with explicit truncation metadata.
    *   Added GitHub repository ingestion limits: `max_files`, `max_total_bytes`, and per-command `git_timeout_secs` with bounded defaults and caps.
    *   Wrapped Git clone/fetch/reset/rev-parse/diff with timeout enforcement and captured stdout/stderr correctly for incremental sync.
    *   Persisted SearchXyz graph/cache mutations and reloaded the Tantivy reader after index mutations so recall sees updates immediately.
    *   Required explicit `confirm=true` for `searchxyz_delete_source` and `searchxyz_clear_index`.
    *   Exposed the new SearchXyz safety knobs through OpenZ wrapper schemas.
*   **Chore:** Bumped version to `v0.0.47`.

### v0.0.46
*   **Fix: OpenMedia SVG generation workflow:** Hardened `openmedia_create_svg` so agents can create cleaner, better-aligned SVG logos without schema guessing.
    *   Added native JSON support for `line` elements plus text `font_weight`, `text_anchor`, opacity, stroke width, and linecap attributes in the OpenMedia SVG core.
    *   Replaced the generic OpenZ `openmedia_create_svg` wrapper with a custom schema containing concrete examples for logo-style SVGs.
    *   Added argument normalization for common model mistakes: `shapes` → `elements`, text `text` → `content`, and camelCase style aliases such as `fontWeight`, `textAnchor`, and `strokeWidth`.
    *   Added optional `output_path` copy support so generated SVGs can land directly under `~/.openz` or the requested workspace path instead of only `.openmedia/output`.
    *   Added OpenMedia SVG-specific self-healing hints for missing `width`, `elements`, text `content`, and alignment fields.
*   **Chore:** Bumped version to `v0.0.46`.

### v0.0.45
*   **Fix: Media tool robustness and guidance:** Hardened OpenMedia and HTML video workflows so agents stop guessing schemas and starting doomed long renders.
    *   Added a concrete valid `openmedia_video_create` / `openmedia_video_preview` scene example to the exposed schema, including required scene, text element, style, position, anchor, and transition rules.
    *   Brightened `diagnose_tool` OpenMedia placeholder scenes so video diagnostics produce visible output instead of near-black tiny text.
    *   Added OpenMedia-specific self-healing hints for common schema errors such as missing `anchor`, invalid transition names, numeric `font_weight`, and wrong element types.
    *   Changed loop detection to fingerprint `scene_path` file contents, preventing false duplicate blocks after a scene JSON file is edited while preserving duplicate blocking for unchanged files.
    *   Added `html_to_video` render planning: direct renders above 300 frames now fail fast with guidance to segment, lower FPS, use OpenMedia templates, or explicitly set `allow_long_render`.
    *   Improved TUI display for `html_to_video` calls to show duration, FPS, and total frame count.
*   **Chore:** Bumped version to `v0.0.45`.

### v0.0.44
*   **Fix: Subagent lifecycle status output:** Compact subagent TUI status lines now show `name | model | status` for completion, cancellation, and failures instead of generic or repeated status text.
    *   Added an explicit `cancelling` lifecycle line when Esc/Ctrl+C cancellation reaches delegate task/profile runs.
    *   Kept cancellation propagation tests green for both `delegate_task` and profile-backed subagent tools.
    *   Added regression coverage to ensure cancellation status output does not fall back to generic `Running...` text.
*   **Chore:** Bumped version to `v0.0.44`.

### v0.0.43
*   **Fix: OpenMedia tool argument normalization:** Hardened OpenMedia video and raw-JSON tool calls so agents can pass structured objects, JSON strings, scene file paths, and raw top-level `VideoScene` objects without hitting misleading `missing field scene` or `expected struct VideoScene` errors.
    *   `openmedia_video_create` and `openmedia_video_preview` now accept `scene`, `scene_path`, JSON-string scenes, and raw `VideoScene` inputs.
    *   `openmedia_video_from_template`, `openmedia_create_svg`, `openmedia_image_batch_process`, `openmedia_template_create/update`, and Mermaid custom themes now parse JSON-string fields before execution.
    *   `diagnose_tool` now supplies a minimal valid OpenMedia video scene for placeholder video diagnostics instead of calling those tools with invalid `{ "test": true }` arguments.
*   **Chore:** Bumped version to `v0.0.43`.

### v0.0.42
*   **Feature: Prompt-aware tool router (HIGH)**:
    *   Added tool metadata for domains, risk, resource usage, aliases, examples, and use/avoid guidance so OpenZ can expose the most relevant tools per prompt.
    *   Tool descriptions sent to providers now include compact routing hints to improve tool choice without inflating the full system prompt.
    *   Added optional TUI router visibility through `showToolRouterStatus` for debugging which tools were selected and why.
*   **Feature: Native tool catalog and observability (HIGH)**:
    *   Added the `tool_catalog` native tool so agents can inspect available tools, domains, aliases, risk levels, examples, and current router/resource status.
    *   Extended config management support for router and resource-policy settings.
*   **Guardrail: Tool resource policy (HIGH)**:
    *   Added disk-space, network-tool, expensive-tool, and concurrent process-tool checks before native tool execution.
    *   Added configurable `minFreeDiskGb`, `allowNetworkTools`, `maxConcurrentProcessTools`, and `warnBeforeExpensiveTools` settings.
    *   Reused existing approval flow for expensive tools while avoiding duplicate approval prompts after the security guard has already asked.
*   **Tests:** Added targeted coverage for tool routing metadata, API truncation behavior, tool catalog output, self-management config updates, and resource-policy decisions.
*   **Chore:** Bumped version to `v0.0.42`.
*   **Feature: Runtime database placement & doctor (MEDIUM)**:
    *   Added centralized `runtime_data_dir()` / `runtime_db_path()` helpers in `config/loader.rs` so all SQLite databases and caches resolve consistently under `~/.openz/` — never the workspace/repo root.
    *   Refactored 6 DB-path call sites (`shared_memory`, `graph_memory`, `sequential_thinking`, `headroom`, `skills`, `semantic_search`) to use the centralized resolution.
    *   Added `RUNTIME_DB_FILENAMES` constant and `check_root_runtime_dbs()` startup doctor that warns if stale artifacts are found in the working directory.
    *   Added `openz doctor` CLI command that migrates stray root DBs into `~/.openz/` (or archives them if the global copy already exists) and prunes stale `graph_memory.db.branch_*` files — all non-destructive, data preserved in `legacy-root-backup/`.
    *   Hardened `.gitignore` for all known runtime DB artifacts (`memory.db`, `graph_memory.db`, `thoughts.db`, `ccr_cache.db`, `embeddings_cache.json`, `*.db`, `*.db-wal`, `*.db-shm`, and `.openz/`).
    *   Updated `localinstall.sh` / `localupdate.sh` to self-clean stray runtime DBs from the working directory before compilation.
    *   **Tests:** Added 4 new tests covering runtime-DB path isolation from workspace root, `OPENZ_CONFIG_DIR` redirect, root-memory.db detection, and `.gitignore` pattern verification.
    *   **Fix:** Changed stale `test_get_version_history` assertion from hardcoded `v0.0.36` to `env!("CARGO_PKG_VERSION")` so it tracks the current release.

### v0.0.41 (Previous Release)
*   **Fix: OpenZ worktree disk quota guard (CRITICAL)**:
    *   Added size-aware cleanup for `~/.openz/worktrees/openz_worktree_*` to prevent stale subagent worktrees from consuming large disk space.
    *   Worktree cleanup now enforces max age, max count, max total bytes, and a minimum free-space safety margin.
    *   Added tests to verify old worktrees are removed, non-OpenZ directories are preserved, and oldest worktrees are pruned first when the quota is exceeded.
*   **Fix: Ctrl+C / exit shutdown hang (HIGH)**:
    *   Distinguishes idle-prompt Ctrl+C from active-turn cancellation so idle Ctrl+C triggers app shutdown instead of only sending a turn-cancel signal.
    *   Bounds gateway shutdown and outbound offline-notification HTTP calls with short timeouts so OpenZ cannot sit indefinitely after `Goodbye!` / `Shutting down gateways...`.
*   **Feature: Subagent lifecycle status model (HIGH)**:
    *   Added structured subagent lifecycle states for queued, running, fallback, cancelling, cancelled, timed out, failed, and completed states.
    *   Subagent JSON tool results now include lifecycle metadata so TUI and future monitoring can render clear status instead of relying on raw strings.
*   **Chore: Version drift guard**:
    *   Added a test that verifies Cargo, README, ONPKG, Markdown changelog, and CLI changelog version surfaces stay synchronized.
*   **Chore:** Bumped version to `v0.0.41`.

### v0.0.40 (Previous Release)
*   **Fix: Subagent cancellation caused invisible input / frozen prompt text (CRITICAL)**:
    *   **Root cause:** When a user cancelled a turn (using `Esc` or `Ctrl+C`), the outer future (`run_fut`) was aborted immediately. However, because the future wrapping the subagent spinner (`with_spinner`) was dropped mid-execution, the clean-up block that popped the spinner from the `ACTIVE_SPINNERS` stack was bypassed. This left the spinner on the stack indefinitely.
    *   **Consequence:** The TUI print shims (`tui_print_fn` and `tui_println_fn`) constantly saw an active spinner and printed `\r\x1b[2K` on every print, immediately clearing the prompt and the user's typed input characters as they typed them.
    *   **Fix:** Introduced `SpinnerGuard`, a struct implementing the Rust `Drop` trait. Because `drop()` is guaranteed to execute when the guard goes out of scope (even if the future is dropped or cancelled), the spinner is now always reliably popped from the coordination stack.
*   **Fix: TUI spinner overlapping/misaligned rendering during cancel and thinking states (HIGH)**:
    *   **Root cause:** Direct raw `print!` statements were used to render the turn cancellation warning (`▲ Turn cancelled by user.`) and the model's thinking duration badges (`L ● Thought for Xs`). Because they bypassed the global `STDOUT_MUTEX` lock and active spinner checks, the background spinner thread wrote progress characters (e.g. `⠋`) side-by-side on the same line.
    *   **Fix:** Converted all raw warning and thought-duration prints in `run.rs` to `crate::tui_println!`. They now correctly lock the stdout mutex and clear any active spinner line before rendering.
*   **Chore:** Bumped version to `v0.0.40`.

### v0.0.39 (Previous Release)
*   **Fix: Esc / Ctrl+C cancellation was unreliable during subagent execution (CRITICAL)**:
    *   **Root cause:** Crossterm raw mode converts `Ctrl+C` from a signal (`SIGINT`) into a key event, bypassing the OS signal handler. The keyboard listener was running inside `tokio::task::spawn` with blocking `crossterm::event::poll/read` calls, starving the Tokio runtime and preventing the cancellation future from making progress.
    *   **Fix:** Moved keyboard input polling to a dedicated **OS thread** (`std::thread::spawn`) instead of a Tokio task. The OS thread polls crossterm events every 100ms without blocking the async runtime, and signals cancellation via the existing `CLI_CANCEL_TX` watch channel.
    *   **Fix:** Added a `std::sync::mpsc` channel for the main task to signal the keyboard thread to stop cleanly after the agent turn completes, preventing leaked threads.
*   **Fix: LLM streaming loop was not cancellation-aware (CRITICAL)**:
    *   The `while let Some(chunk) = stream.next().await` loop in `run.rs` would block indefinitely on slow LLM responses with no cancellation check.
    *   **Fix:** Replaced with a `tokio::select!` loop that races each `stream.next()` against a **cancel-safe** `watch::Receiver::changed()` listener (not `Notify`, which loses permits when dropped in `select!`).
*   **Fix: Tool execution (subagent delegation) was not cancellable (HIGH)**:
    *   Tool calls (especially `delegate_task`, `parallel_research`) ran behind `tokio::time::timeout` but had no way to be interrupted by user input.
    *   **Fix:** Wrapped tool execution in `tokio::select!` racing the timeout+execution against the `CLI_CANCEL_TX` watch channel. Cancellation and timeout errors both propagate to the turn-level `CancellationToken` to break the agent loop immediately.
    *   Flattened the match arms from `Ok(Ok(res)) / Ok(Err(e)) / Err(timeout)` to `Ok(res) / Err(e)` since timeout is now folded into the `Err` variant.
*   **Fix: Subagent tool spinners were silently suppressed (MEDIUM)**:
    *   `with_spinner()` in `spinner.rs` returned early (no visual feedback) when `DELEGATION_DEPTH > 0`, making subagent tool execution appear frozen.
    *   **Fix:** Spinners now render at all delegation depths with tree-prefix indentation (using `get_tree_prefix()`), giving visual feedback during subagent work.
*   **Fix: `openz logs` missed subagent log lines when filtering by session (HIGH)**:
    *   Nested tracing spans produce lines like `turn{session=cli:abc}:turn{session=subagent:vision:123}: ...` with multiple `session=` values. The old `extract_session_from_line` only found the FIRST `session=` (the parent CLI session), so filtering by the subagent session would miss these lines.
    *   **Fix:** Replaced `extract_session_from_line` with `extract_all_sessions_from_line` that collects ALL `session=` values from the entire line. Updated `session_matches` to check if ANY of the line's sessions match the filter. The session badge now displays the LAST (most specific) session.
*   **Fix: Non-streaming LLM call path was completely un-cancellable (CRITICAL)**:
    *   When `streaming = false`, `chat_with_fallback()` calls `active_provider.chat()` wrapped in `with_spinner()` — neither races against any cancel signal. The code blocks on the HTTP response indefinitely, ignoring Esc/Ctrl+C entirely.
    *   **Fix:** Wrapped the non-streaming `chat_with_fallback` call in `tokio::select!` racing against the `CLI_CANCEL_TX` watch channel. On cancel, sets `turn_cancel.cancel()` and returns `TurnState::Save` immediately.
*   **Fix: `chat_with_fallback` retry loop ignored cancellation (HIGH)**:
    *   When the primary provider fails, the fallback loop iterates through up to 3 fallback models + retries the original — none checked for cancellation. With network issues, this means 4+ HTTP timeouts before cancel takes effect.
    *   **Fix:** Added cancel-before-attempt checks at each stage: before each fallback model attempt, and before the final retry of the original provider. Returns `Err("Cancelled by user")` immediately.
*   **Fix: Auto-continuation loop ignored cancellation (MEDIUM)**:
    *   When `finish_reason == "length"`, up to 3 additional `chat_with_fallback` calls run with no cancel check, compounding the non-streaming blocking bug.
    *   **Fix:** Added `turn_cancel.is_cancelled()` check at the start of each continuation iteration.
*   **Fix: Multi-tool-call batch didn't break on cancel (MEDIUM)**:
    *   When the LLM returns multiple tool calls in one response, the `for call in resp.tool_calls` loop continues after the first tool is cancelled, starting and immediately cancelling each subsequent tool (unnecessary overhead + error accumulation).
    *   **Fix:** Added `turn_cancel.is_cancelled()` check at the start of each tool call iteration to break immediately.
*   **Fix: Vision subagent model routing and fallback isolation (CRITICAL)**:
    *   Dynamic subagent tools are now reserved inside the 128-tool OpenAI-compatible payload limit, preventing `vision_agent` from being described in prompts but missing from executable tool schemas.
    *   Subagent runtime config now preserves the selected profile model/provider/fallbacks across per-turn config reloads, so `vision_agent` no longer reverts internally to the main orchestrator model.
    *   Profile subagents no longer inherit the main agent's global fallback models; each profile owns its configured primary/fallback chain.
    *   OpenRouter-style `*:free` model IDs (for example `google/gemma-4-31b-it:free` and `nvidia/...:free`) route to OpenRouter instead of being sent to OpenAI or stripped incorrectly.
*   **Fix: Delegate-task cancellation and false interrupt reporting (HIGH)**:
    *   Active agent turns now keep crossterm raw mode enabled while the keyboard cancellation watcher is running, so Esc/Ctrl+C are delivered during long `delegate_task` and subagent calls.
    *   Tool/subagent timeouts no longer set the same turn-cancel flag used by real user interrupts, preventing misleading `Turn cancelled by user` messages when the provider timed out or rate-limited.
    *   Generic delegated workers no longer receive recursive `delegate_task` registration by default, preventing runaway nested delegation loops.
*   **Chore:** Bumped version to `v0.0.39`.

### v0.0.38 (Previous Release)
*   **Fix: Live `openz logs` streaming was unusably delayed (CRITICAL)**:
    *   Linked `CancellationToken` to `CLI_CANCEL_TX` (per-turn Ctrl+C signal) in addition to the existing `SHUTDOWN_TX` (global SIGTERM) listener. Previously, pressing Ctrl+C during a subagent task only cancelled the outer CLI loop but left subagent background tasks running.
    *   Added `biased;` to all 8 `tokio::select!` cancellation branches in `delegate_task.rs`, `delegate_profile.rs`, and `parallel_research.rs` — ensures cancellation is checked before running the child agent.
    *   Fixed `CancellationToken` background task unconditionally firing when `shutdown::receiver()` returned `None` — now only cancels if a signal actually fired.
    *   Fixed `parallel_research.rs` spawned tasks: added `tokio::select!` with `biased;` cancellation to race `join_all` against `wait_for_cancellation()`, so the parent returns immediately on Ctrl+C instead of waiting for spawned tasks.
*   **Fix: Multi-workspace session conflicts (HIGH)**:
    *   Replaced the hardcoded `"cli:direct"` session key with a workspace-unique key derived from the current working directory hash (`cli:{cwd_hash}`).
    *   This allows multiple `openz agent` instances to run simultaneously in different project directories without lock contention.
    *   Remote input (Telegram, watcher, `send_remote_input`) still works via a `"cli:direct"` global inbox fallback in the CLI input loop.
    *   Added `get_cli_session_key()` to `config/loader.rs`. Updated `channels/cli/mod.rs`, `cli/agent.rs`, `channels/cli/input.rs`, `agent/security.rs`, and `agent/agent_loop/mod.rs`.
*   **Fix: Background channel `eprintln!` calls corrupt TUI (MEDIUM)**:
    *   Replaced 15 `eprintln!()` calls in Discord, Telegram, Email, agent subroutines, and config loader with `tracing::error!()` or `tui_println!()` to prevent terminal corruption when running background channels concurrently.
*   **Refactor: Eliminated 30 `#[allow(unused_macros)]` boilerplate blocks (LOW)**:
    *   Defined shared `#[macro_export]` versions of `println!`, `print!`, `eprintln!`, `eprint!` in `agent/style/mod.rs`.
    *   All CLI modules now use `use crate::{println, ...};` instead of redefining the same 3-4 macros locally (11 files, ~270 lines removed).
*   **Refactor: Deduplicated provider config resolution (MEDIUM)**:
    *   Added `Config::resolve_provider_config()` in `config/schema.rs` as the single source of truth for provider API key + base URL resolution.
    *   Removed ~500 lines of identical 17-way match arms from `providers/resolver.rs`, `channels/mod.rs`, and `cli/builder.rs`.
*   **Chore: `.gitignore` cleanup, removed old branding**:
    *   Added `/memory.db-shm` and `/memory.db-wal` to `.gitignore`.
    *   Removed stale `nanobotdocs/` directory (old branding) and duplicate `assets/logo1.png`.
    *   Bumped version to `v0.0.38`.

### v0.0.37 (Previous Release)
*   **Feature: Bot Detection Bypass (Stealth) for SearchXyz (HIGH)**:
    *   Implemented dynamic **Rotating Proxies** inside DuckDuckGo and Google search backends.
    *   Implemented **Headless Browser Fallback** (`js-rendering` feature utilizing `chromiumoxide`) to bypass Cloudflare captchas and rate-limiting blocks automatically on Raw HTTP request failure.
    *   Wired up the `Crawler` pool of client proxies and headless browser instances directly into backends initialization in `tools/searchxyz/src/bin/searchxyz.rs` and `src/tools/searchxyz/mod.rs`.
*   **Feature: Structured Error Self-Healing Recovery ("Errors as Instructions") (HIGH)**:
    *   Upgraded the tool execution loop in [run.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run.rs) to natively parse and forward structured JSON error responses returned by tools.
    *   Merges local `self_healing_suggestion` recovery hints seamlessly into structured tool errors if missing, allowing tools to define their own explicit semantic healing protocols.
*   **Feature: TUI Formatting, Casing, and Duplicate Redundant Suffix/Prefix Fixes (MEDIUM)**:
    *   Resolved argument casing parameter mismatches (`query`/`Query` and `path`/`Path`) across all native tools (`grep_search`, `read_file`, `view_file`, `write_file`, `list_dir`, and `doc_reader`) to ensure file names and query details print successfully in the terminal.
    *   Fixed TUI friendly name duplicate suffixing (e.g. `● Grep Search Search` deduplicated to `● Grep Search`).
*   **Feature: Subagent Visual Image Delegation & Instant TUI Cancellation (HIGH)**:
    *   Implemented standard, non-blocking **asynchronous SIGINT / Ctrl+C cancellation** inside [cli/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/mod.rs#L741-L770), replacing the blocking background event polling thread and preventing lock contention over standard input.
    *   Implemented **Automatic Image Path Scanning & Markdown Linking** in `DelegateTaskTool` and `DelegateProfileTool` to resolve subagent blindness. Subagents now automatically receive image attachments parsed from paths in their goal or context text, enabling seamless visual task delegation to vision models like Gemini 2.5 Flash on the very first turn.
*   **Documentation & Guidelines (MEDIUM)**:
    *   Authored a workspace-level procedural tools selection, safety, and error recovery guide in [skills/tool_usage_guide.md](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/skills/tool_usage_guide.md) and [onpkg_docs/tool_usage_guide.md](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg_docs/tool_usage_guide.md).
    *   Bumped project and Cargo workspace package version to `v0.0.37`.

### v0.0.36 (Previous Release)
*   **Feature: System Diagnostics, Session Management, and Safe Test Suites (HIGH)**:
    *   Implemented `DiagnoseSystemTool` (`diagnose_system`) to profile OpenZ storage directories (`sessions/`, `tool_outputs/`, `traces/`, and `skills/`), check SQLite database connectability, size, and integrity (`PRAGMA integrity_check;`) for all 5 databases, and test LLM provider endpoint request latency.
    *   Implemented `ManageSessionsTool` (`manage_sessions`) to list active session files with message counts/sizes, prune old temporary tool outputs to prevent disk exhaustion, archive sessions, or permanently delete session files.
    *   Created a safe test execution script `run_tests_safely.sh` that compiles with restricted CPU cores and runs tests sequentially by module to prevent memory exhaustion and laptop crashes on developer machines.
    *   Cleaned up corrupted Cargo dependency directory caches.
    *   Bumped framework version to `v0.0.36` across Cargo, README, and `onpkg.json`.

### v0.0.35 (Previous Release)
*   **Feature: Real-Time Configuration Dynamic Reloading & Management (HIGH)**:
    *   Dynamic reload of configuration (`config.json`) at the start of each turn loop iteration inside `AgentLoop::run_inner` synced via `TurnContext`.
    *   Implemented `ManageConfigTool` (`manage_config`) to view the active configuration with recursive secret key redaction (`api_key`, `bot_token`, `verify_token`, `password`, `secret`) and update agent hyper-parameters (`model`, `provider`, `max_tokens`, `temperature`, `caveman_mode`, `tool_timeout_secs`, `streaming`, `max_tool_iterations`).
    *   Added dedicated unit tests verifying that configuration viewing, secret redaction, and hyper-parameter updates behave correctly.
    *   Bumped framework version to `v0.0.35` across Cargo, README, and `onpkg.json`.

### v0.0.34 (Previous Release)
*   **Feature: Self-Management & Self-Healing Toolkit (HIGH)**:
    *   Implemented `DiagnoseToolTool` (`diagnose_tool`) to allow the agent to test any native tool in the registry, check schema validity, measure execution latency, and capture standard errors.
    *   Implemented `CurateSkillTool` (`curate_skill`) to allow the agent to CRUD its own guidelines dynamically in the SQLite database skills store.
    *   Implemented `OptimizeToolScopeTool` (`optimize_tool_scope`) to allow the agent to filter the set of active tool schemas exposed to the LLM by prefix, saving prompt tokens and avoiding hallucinations.
    *   Exposed dynamic prefix filtering in `ToolRegistry` across `get`, `get_static_tools`, and `to_openai_format` methods.
    *   Added dedicated unit tests verifying that all three self-management tools and scope filters behave correctly.
    *   Bumped framework version to `v0.0.34` across Cargo, README, and `onpkg.json`.

### v0.0.33 (Previous Release)
*   **Feature: Native Integration of GitHub & Docs MCP Servers (HIGH)**:
    *   Ported the entire `openz_github_mcp` and `openz_docs_mcp` packages directly into the workspace under `tools/openz-github` and `tools/openz-docs`.
    *   Converted both codebases to standard Cargo library targets (`lib.rs`) and removed standalone executable entry points.
    *   Exposed 3 GitHub management tools under the `github_` prefix (e.g. `github_create_pull_request`, `github_search_issues`, `github_get_issue_comments`) using re-exported `octocrab` client integration.
    *   Exposed 6 local and remote documentation search/caching tools under the `docs_` prefix (e.g. `docs_list_docsets`, `docs_install_docset`, `docs_search_docs`, `docs_read_doc_page`, `docs_search_rust_crate`, `docs_read_rust_docs`).
    *   Registered all 9 new tools natively inside `build_agent_loop` in [src/cli/builder.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
    *   Wrote unit tests verifying client and database schema initialization.
*   **Prompting & Version Update**:
    *   Added `github_` and `docs_` tool prefixes to the system guidelines prompt list in [src/agent/agent_loop/build.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs).
    *   Bumped framework version to `v0.0.33` across Cargo, README, and `onpkg.json`.

### v0.0.32 (Previous Release)
*   **Feature: Integrated OpenDoc-MCP Native Tool Port (HIGH)**:
    *   Ported the entire `opendoc-mcp` codebase into the workspace under [tools/opendoc/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/opendoc) to enable off-grid document intelligence.
    *   Upgraded the workspace rust toolchain to `stable` to cleanly resolve dependencies (like edition 2024 dependencies).
    *   Updated the visibility of all 36 document creation, extraction, diffing, and templating methods inside [tools/opendoc/src/server.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/opendoc/src/server.rs) to `pub fn`.
    *   Wrapped all 36 tools natively under the `opendoc_` prefix in [src/tools/opendoc/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod.rs).
    *   Registered the tools inside `build_agent_loop` in [src/cli/builder.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
    *   Wrote unit tests verifying initialization of the opendoc server and basic OCR availability check.
*   **Refactor: SearchXyz Modularization (MEDIUM)**:
    *   Split the `searchxyz` monolithic wrapper into separate submodules (`web`, `index`, `graph`) under [tools/searchxyz/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/searchxyz) to follow standard codebase modularization guidelines.
*   **Prompting & Documentation Update**:
    *   Added all `opendoc_` tools to the agent's core system prompt guidelines in [src/agent/agent_loop/build.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs).
    *   Bumped the project version to `v0.0.32`.

### v0.0.31 (Previous Release)
*   **Feature: Integrated OpenMedia-RS Crate Workspace (HIGH)**:
    *   Migrated all 8 crates of `openmedia-rs` directly into the `openz` workspace under [tools/openmedia](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/openmedia), making `openz` fully portable and self-contained.
    *   Exposed all 37 OpenMedia tools natively under the `openmedia_` prefix, including `openmedia_create_chart` (charts), `openmedia_video_create` (DSL video composition), `openmedia_animate_svg` (SMIL layout animations), and `openmedia_improve_score_image` (aesthetics prompt alignment).
    *   Constructed a thread-safe static `OnceLock` singleton wrapper in [src/tools/openmedia/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod.rs) for in-memory execution without JSON-RPC stdio overhead.
    *   Used `schemars::schema_for!` to automatically generate and validate parameters JSON schemas from Rust structs.
*   **Workspace Manifest Consolidation**:
    *   Added workspace members and workspace dependencies tables to `openz` root `Cargo.toml`.
    *   Registered and validated the `openmedia-core` and `openmedia-mcp` crates via standard Cargo workspace dependency inheritance (`workspace = true`).
*   **Testing & Prompting Integration**:
    *   Added `openmedia_` tools to the agent's core system prompt template guidelines in [src/agent/agent_loop/build.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs).
    *   Wrote `test_openmedia_server_ping` unit tests verifying singleton server status.
    *   Bumped version to `v0.0.31`. All 204 tests passing cleanly.

### v0.0.30 (Previous Release)
*   **Feature: Integrated SearchXyz Tool Suite (HIGH)**:
    *   Fully integrated the `searchxyz` crate into the Cargo workspace.
    *   Wrapped all 15 tools from `searchxyz` as native OpenZ tools, prefix-registered under `searchxyz_`:
        *   `searchxyz_search_web`: Web search dispatcher.
        *   `searchxyz_read_url`: Document/media/git repo parser.
        *   `searchxyz_search_and_read`: Multi-step web search and crawl.
        *   `searchxyz_recall`: Semantic & keyword lookup.
        *   `searchxyz_list_sources`: Local document source lister.
        *   `searchxyz_deep_research`: Recursive multi-query crawler & report compiler.
        *   `searchxyz_index_content`: Custom text manual indexer.
        *   `searchxyz_site_map`: Fast sitemap/tree crawl discovery.
        *   `searchxyz_index_relationship`: Knowledge Graph node insertion.
        *   `searchxyz_query_graph`: Graph query & traversal.
        *   `searchxyz_read_github_repo`: Repository codebase cloner & indexer.
        *   `searchxyz_export_research`: Portable JSON metrics/doc exporter.
        *   `searchxyz_import_research`: JSON bundle importer.
        *   `searchxyz_delete_source`: URL-based source eviction.
        *   `searchxyz_clear_index`: Clear all documents and Graph memory.
*   **Search Integration (HIGH)**:
    *   Integrated `SearchXyz`'s federated `SearchDispatcher` into `WebSearchTool` (`web_search`) as the primary web search engine.
    *   Preserved original search providers (Tavily, Exa, DuckDuckGo scraper, Mojeek scraper) as robust automatic fallbacks in case `SearchXyz` encounters network errors or is unconfigured.
*   **Maintenance & Testing**:
    *   Added `rmcp` and `schemars` dependencies to the workspace root `Cargo.toml`.
    *   Wiped local package/registry cache to resolve resolving and download contentions.
    *   Implemented `test_searchxyz_tools_metadata` unit tests verifying wrapper registry.
    *   Bumped version to `v0.0.30`. All 202 native tests and 38 integrated `searchxyz` tests passing.

### v0.0.29
*   **Security: SSRF & timing attack mitigations (HIGH)**:
    *   Implemented constant-time WhatsApp HMAC signature validation in `src/channels/whatsapp.rs` to protect webhook endpoints from timing attacks.
    *   Added WebSocket frame size and message limits (16MB) in `src/channels/websocket.rs` to prevent DoS attacks.
    *   Implemented HTTP chunked response body limits (10MB) in `src/tools/web.rs` to block memory exhaustion.
    *   Introduced IP pinning for SSRF validation in `src/tools/web.rs` to eliminate the DNS-rebinding TOCTOU race window.
*   **Database: Hardened shared memory and graph databases (HIGH)**:
    *   Replaced per-call connection patterns in `shared_memory` with a thread-safe singleton connection using WAL mode and a 5s busy timeout.
    *   Unified branch and main schema DDLs in `graph_memory` to eliminate schema corruption.
    *   Established deadlock-free lock ordering (`db_static()` lock before `BRANCH_MUTEX`).
    *   Optimized queries with `LIMIT` clauses and capped the O(n²) consolidation to the newest 200 entries.
    *   Added eviction limits to in-memory fallback stores to prevent memory leaks.
*   **Reliability & Channel Enhancements (HIGH)**:
    *   Registered global panic hook to restore raw terminal mode and exit alternate screens cleanly.
    *   Redacted Discord bot tokens in error logs.
    *   Added concurrency semaphores to WhatsApp webhook axum handlers.
    *   Updated atomic ordering to prevent race conditions in Discord gateway heartbeat routines.
*   **Performance: Async I/O and System Prompt budgets (HIGH)**:
    *   Migrated hot-path filesystem operations in `run.rs` and `session.rs` to async `tokio::fs` or `spawn_blocking`.
    *   Optimized save frequency to write session incrementally every 5 iterations.
    *   Added character/token budget caps (32k) for system prompts to prevent context token overflow.
    *   Implemented path traversal constraints in filesystem tools to prevent unauthorized file access.
*   **Maintenance: Code Cleanups**:
    *   Removed `AgentError` dead code and resolved 13 unused compiler warnings across the repository.
    *   Bumped version to v0.0.29. All 201 unit tests passing cleanly.

### v0.0.28
*   **Refactor: Codebase Modularization (MEGA):** Modularized all remaining monolithic files into cleanly structured, package-based submodules:
    *   Split CLI raw terminal input/render/channel loop (`src/channels/cli.rs`) into [src/channels/cli/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/).
    *   Split core agent loop state machine (`src/agent/agent_loop.rs`) into [src/agent/agent_loop/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/).
    *   Split CLI subcommands & configuration menus (`src/cli.rs`) into [src/cli/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/).
    *   Split memory extra tools (`src/tools/memory_extra.rs`) into [src/tools/memory_extra/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/).
    *   Split headroom compression tools (`src/tools/headroom.rs`) into [src/tools/headroom/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/).
    *   Split shared memory tools (`src/tools/shared_memory.rs`) into [src/tools/shared_memory/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/).
    *   Split sequential thinking tools (`src/tools/sequential_thinking.rs`) into [src/tools/sequential_thinking/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sequential_thinking/).
    *   Split graph memory tools (`src/tools/graph_memory.rs`) into [src/tools/graph_memory/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/graph_memory/).
*   **Docs: Repository Architecture Update (MEDIUM):** Updated architecture and tools documentation to match modular packages layout.
*   **Maintenance: Version Bump:** Bumped to v0.0.28. All 200 unit tests passing sequentially.

### v0.0.27
*   **Feat: Cross-Session Memory Persistence (HIGH):** Implemented automatic persistence of user/project facts and observations. The background self-improvement curator now parses extracted facts from markdown and stores them permanently into the SQLite database. Added automatic retrieval in `TurnState::Build` that queries all active semantic facts and graph node observations across past sessions and injects them dynamically into the system prompt. Enabled fact-sharing keyword checks to trigger curators even on simple/short turns.
*   **Fix: Test Isolation & Test Path Caching (MEDIUM):** Isolated `cargo test` database paths to temporary files (`openz_test_graph_memory_<uuid>.db`) and cached the path via a `OnceLock` within the test process. Corrected mathematical expectation in `test_text_similarity` to match Jaccard word-overlap math. All 198 tests now compile and pass cleanly.
*   **Refactor: MCP-to-Native Tool Port — Sequential Thinking, Memory, Headroom (MEGA):** Ported all 67 tools from 3 external MCP servers to native Rust implementations across 4 new files (`sequential_thinking.rs`: 5 tools, `headroom.rs`: 19 tools, `graph_memory.rs`: 12 tools, `memory_extra.rs`: 31 tools). Eliminates external binary spawns, JSON-RPC overhead, and stdio polling for these servers. Compilation is clean (0 new warnings), 198/198 tests pass.
*   **Fix: Dual SQLite Connection Elimination (HIGH):** Both `graph_memory.rs` and `memory_extra.rs` now share a single `OnceLock<Mutex<Connection>>` via `pub(crate) with_db()`. Removed ~170 lines of duplicated DB infrastructure from `memory_extra.rs` (`db_static()`, `init_db()`, `get_db_path()`, `with_db()`, `scope_from_args`). All table DDL merged into `graph_memory::init_db()` — eliminates `SQLITE_BUSY` errors from concurrent connections.
*   **Fix: Name Collision Resolution (MEDIUM):** Renamed `ast_grep::IndexCodebaseTool` to `AstGrepIndexCodebaseTool` (tool name: `ast_grep_index_codebase`) to avoid collision with `memory_extra::IndexCodebaseTool`.
*   **Config: MCP Server Pruning (MEDIUM):** Removed 5 MCP servers from `~/.openz/config.json` and `config/schema.rs` defaults: `sequential-thinking`, `memory`, `headroom`, `database`, `context-bus`. `database-mcp` was duplicated by native `DbInspectorTool`/`DbWriteTool`; `context-bus-mcp` had no native equivalent and was removed at user request.
*   **Maintenance: Version Bump:** Bumped to v0.0.27. All 67 native tools registered with zero orphans, zero name collisions.

### v0.0.26
*   **Feat: Official Repository Awareness (HIGH):** Updated core system prompt guidelines in `src/agent/agent_loop.rs` to make the agent explicitly aware of its official GitHub repository and source code at `https://github.com/aswin402/openz-rs` for advanced self-querying.
*   **Feat: Indented and Aligned Monologue Formatting (MEDIUM):** Redesigned thought/reasoning blocks in the TUI to wrap paragraphs dynamically according to the active terminal width. The tree connector (`  L `) is printed only on the very first line of a thought, and subsequent paragraphs/wrapped lines are space-padded to align neatly under the start.
*   **Style: Custom Color System Update (MEDIUM):** Updated global theme colors in `src/agent/style/colors.rs`: AURA_PURPLE is set to `#6F00FF`, AURA_GREEN to `#00FF00`, and error/fail reds to `#FF0000`. Original `EMERALD_GREEN` was restored.
*   **Fix: Duplicated Tool Name Display (MEDIUM):** Introduced a clean extraction parser `clean_tool_args_msg` that prevents friendly tool names from duplicating start message outputs (e.g. converting `● Web Search WebSearch` to just `● Web Search`).
*   **Maintenance: Version Bump:** Bumped to v0.0.26.

### v0.0.25
*   **Feat: Structured Live Log Visualizer (HIGH):** Redesigned the terminal-based log follow screen in `openz logs` ([`src/logs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs.rs)) to output high-fidelity trace representations with customized semantic icons, colors, and bold labels representing different workflow events:
    * 👤 `[USER]` — Human prompt message (Cyan).
    * 🧠 `[THINKING]` — Model reasoning/thought tokens (Orange).
    * 📡 `[LLM CALL]` — Outgoing model invocation requests (Slate).
    * 🤖 `[RESPONSE]` — Model completions output (White).
    * 🛠️ `[TOOL START]` / `[TOOL DONE]` / `[TOOL FAIL]` — Full tool lifecycle tracking with clean arguments parsing and return statuses (Gold/Green/Rose).
    * 🤖 `[SUBAGENT START]` / `[SUBAGENT DONE]` / `[SUBAGENT FAIL]` — Correlated trace tracking of child agent delegations (Purple/Green/Rose).
    * 🛡️ `[BLOCKED]` — Commands intercepted by the SecurityGuard or user denials (Gold).
    * 🧹 `[CURATOR]` — Progress logs for the background self-improvement curator (Purple).
    * 💾 `[SAVED]` / `🗜️ [COMPACT]` — History compaction and database transaction saving (Green/Slate).
*   **Reliability: Comprehensive Execution Tracing (HIGH):** Instrumented the core `AgentLoop` state machine ([`src/agent/agent_loop.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop.rs)) with detailed tracing statements covering context compaction, LLM completions, tool executions, security approvals, and background curator tasks to ensure all developer actions are fully visible in the live logs stream.
*   **Maintenance: Version Bump:** Bumped to v0.0.25.

### v0.0.24
*   **Fix: Tokio TcpListener from_std Panic in gRPC MCP Bridge (HIGH):** Resolved a Tokio runtime panic during stdio-based MCP bridge startup by explicitly invoking `port_guard.set_nonblocking(true)?` before converting the standard socket to a Tokio listener via `TcpListener::from_std`.
*   **Fix: Direct gRPC Server Startup Connection (HIGH):** Replaced the static 500ms sleep and single connect attempt for direct gRPC servers (such as `openmemory_rs` on port 50051) with a robust 20-attempt retry loop (sleeping 150ms between retries, up to 3 seconds total) to prevent early `Connection refused (os error 111)` failures on heavier startup routines.
*   **Maintenance: Compiled Workspace Binaries:** Recompiled and resolved path routing for local workspace subprojects `openmemory_rs`, `mcp-server-sequential-thinking`, and `context-bus-mcp` to ensure all 10 enabled MCP servers initialize successfully.
*   **Maintenance: Version Bump:** Bumped to v0.0.24. All 125 tests passing sequentially.

### v0.0.23
*   **Fix: LLM Parameter Mapping for Non-OpenAI Reasoning Models (HIGH):** Modified request payload logic to exclude non-OpenAI models like DeepSeek V4/R1, QwQ, etc. from `max_completion_tokens` parameter routing. They are now queried using standard `temperature` and `max_tokens` parameters, which prevents completion token starvation and resolves early truncations / cutoffs on OpenAI-compatible gateways (like OpenCode Zen).
*   **Feat: CLI Response Streaming Toggle Wizard (MEDIUM):** Implemented a new global CLI subcommand `openz streaming` that runs an interactive terminal menu/wizard. This allows users to easily enable or disable response streaming globally for default agent configurations without manually editing files.
*   **Maintenance: Version Bump:** Bumped to v0.0.23. All 124 tests passing sequentially.

### v0.0.22
*   **MCP Server Health Monitoring (HIGH):** Implemented a background monitoring task (`start_mcp_health_checks`) running every 30 seconds to monitor spawned MCP servers via lightweight `"tools/list"` ping calls. Seamlessly handles connection drop detection, invalidates stale connections, performs background auto-reconnections, and emits warning/recovery notifications across CLI and WebSocket channels.
*   **Git Integration Tool (HIGH):** Created a native `git_provider` tool in `src/tools/github.rs` to interact with GitHub and GitLab API endpoints natively using `reqwest`. Supports creating pull requests (`create_pr`), listing issues (`list_issues`), searching repository code (`search_code`), and fetching PR/MR diff contents (`get_pr_diff`) without calling external shell processes.
*   **Security: Git Base URL SSRF Hardening (HIGH):** Hardened the `git_provider` tool against Server-Side Request Forgery (SSRF) by validating `api_base` values against standard IP checking rules and applying an IP-restricted redirect client policy.
*   **Maintenance: Version Bump:** Bumped to v0.0.22. All 124 tests passing sequentially.

### v0.0.21
*   **Reliability: Tool Timeout Enforcement (HIGH):** Wrapped all tool calls in `tokio::time::timeout` using the configured `tool_timeout_secs` to prevent infinite hangs from unresponsive tools or external subprocesses.
*   **Resilience: Graceful Shutdown Coordination (HIGH):** Implemented a global shutdown token and registered SIGTERM/SIGINT signal handlers in `main.rs`. Seamlessly integrated the sync terminal raw-mode input loop in `CliChannel` to exit raw mode cleanly on shutdown, avoiding terminal formatting issues.
*   **Safety: Cross-Process Session File Locking (HIGH):** Added file-based advisory locking using the `fs2` crate in `SessionManager`. Prevents multiple concurrent `openz` processes from using the same session file and causing data corruption.
*   **Performance: Real-time Response Streaming (HIGH):** Added `chat_stream` support to `LLMProvider` and implemented SSE chunk parsing in `OpenAIProvider`. Wired up response streaming directly in `AgentLoop`'s Run state for CLI and WebSocket channels, yielding text delta-by-delta while maintaining full tool-call accumulation capability.
*   **Maintenance: Version Bump:** Bumped to v0.0.21. All 121 tests passing sequentially, 0 clippy warnings.

### v0.0.20
*   **Security: Default-Deny Gateway Auth (CRITICAL):** Changed authorization to default-reject gateway requests if `OPENZ_GATEWAY_TOKEN` is unset or empty. Added timing-attack protection by hashing tokens with SHA-256 before comparing them in constant-time.
*   **Security: SSRF Redirect & IPv6 Hardening (CRITICAL):** Configured `WebFetchTool`'s reqwest client with a custom redirect policy validating every hop against `validate_url_sync`. Hardened IPv6 network detection to block loopback, unspecified, multicast, Unique Local Addresses (ULA), link-local unicast, and IPv4-mapped IPv6 addresses.
*   **Security: SQL Injection CLI Spawning Elimination (CRITICAL):** Replaced all subprocess shell-outs to the `sqlite3` CLI process in `DbInspectorTool` and `DbWriteTool` with an in-process integration using the `rusqlite` crate, completely eliminating shell/argument injections and CLI dot-command executions.
*   **Maintenance: Version Bump:** Bumped to v0.0.20. All 121 tests passing, 0 clippy warnings.

### v0.0.19
*   **Security: Subagent Loopback Isolation:** Excluded `SendRemoteInputTool` (`send_remote_input`) from dynamically constructed subagent tool lists (`delegate_task`, `parallel_research`, `evaluator_optimizer_loop`, and custom profiles) to prevent loopback command/prompt injection from nested child loops.
*   **Refactor: Subagent Tool Filtering:** Added a secondary restriction in `filter_tools_for_subagent` to strip out `send_remote_input` from all allowed profile lists.
*   **Maintenance: Version Bump:** Bumped to v0.0.19. All 118 tests passing, 0 clippy warnings.

### v0.0.18
*   **Bugfix: Discord Sequence & Heartbeat Tracking:** Renamed `_s` to `s` in `GatewayMessage` to correctly deserialize Discord's sequence numbers, and populated the sequence tracker inside the background heartbeat payload to prevent prolonged session disconnections.
*   **Refactor: Raw-Mode Output & Custom Error Stream:** Added raw-mode compatible `tui_eprintln!` and `tui_eprint!` helpers to prevent line formatting corruption when writing to `stderr`. Updated gateway shutdown logs to use `tui_println!`.
*   **Refactor: Regex Pre-compilation & Caching:** Swapped inline Regex compilations inside terminal formatting functions with static precompiled `OnceLock` instances.
*   **Bugfix: Vision Model Matching & Coverage:** Explicitly whitelisted `gpt-4-turbo` and `gpt-4-vision-preview` in `model_supports_vision()`.
*   **Refactor: Tool Registry Determinism & Profile Cache:** Added alphabetical sorting by function name to the registered tools list to stabilize the system prompt. Added a thread-safe modification time (mtime) cache to `load_profiles()` to optimize dynamic subagent resolution checks.
*   **Bugfix: Subagent Task Scoping:** Scoped `ACTIVE_WORKSPACE` and `DELEGATION_DEPTH` task-local variables inside `ParallelResearchTool` spawned tasks.
*   **Refactor: Logging Comment Strip Protection:** Prevented stripping URLs (`http://`, `https://`) in context compaction by ignoring double slashes preceded by colons.
*   **Security: Hardened Session Hash Chains:** Checked for hash presence inside `verify_hash_chain()` to block tampering via stripping the message validation hashes.
*   **Bugfix: Cargo.toml Inline Table Dependency Parser:** Fixed parsing of `[dependencies.foo]` tables in `onpkg` package scanner to ignore subproperties like `version` or `features`.
*   **Maintenance: Version Bump:** Bumped to v0.0.18. All 118 tests passing, 0 clippy warnings.

### v0.0.17
*   **Refactor: MCP Dual Cache Consolidation:** Removed `LAZY_MCP_CLIENTS` cache, consolidated to single `SPAWNED_MCP_CLIENTS`. `LazyMcpToolWrapper::call()` now delegates to `McpClient::spawn()` which handles both fast and slow paths. Eliminates first-call cache miss.
*   **Cleanup: Dead Code Removal:** Removed `McpClientType::Stdio` variant and all associated match arms (~60 lines of unreachable code). All spawns use gRPC exclusively.
*   **Bugfix: find_free_port TOCTOU Race:** `find_free_port()` now returns a bound `TcpListener` guard. The listener is passed to `run_mcp_bridge()` and only dropped right before `tonic::Server::serve()` binds, shrinking the race window from ~100ms to <1µs.
*   **Bugfix: Bridge Child Process Monitoring:** Added `child_exit` monitor task in `tokio::select!` inside `run_mcp_bridge()`. If the stdio child crashes, the gRPC bridge shuts down instead of returning stale errors.
*   **Bugfix: Stderr Reader Cancellation:** Reader and stderr forwarding tasks are now aborted via `.abort()` on bridge shutdown, preventing orphaned tasks.
*   **Test: MCP Unit Tests:** Added 4 tests for `find_free_port()` (race-free listener binding behavior) and `McpClient::invalidate()` (cache entry lifecycle).
*   **Maintenance: Version Bump:** Bumped to v0.0.17. All 118 tests passing, 0 clippy warnings.

### v0.0.16
*   **Security: SSRF DNS Rebinding Defense (CRITICAL):** Replaced string-only URL validation in `web_fetch` with DNS resolution checks. After validating URL syntax and hostname patterns, resolves the hostname to IP addresses via `tokio::task::spawn_blocking` + `ToSocketAddrs`, then verifies all resolved IPs are safe (not private, loopback, link-local, unspecified, broadcast, or multicast). Prevents DNS rebinding attacks where a malicious DNS server returns a private IP after initial validation.
*   **Security: SSRF Protection for Crawler (CRITICAL):** Added the same `validate_url()` and `is_safe_ip()` functions to `CrawlSiteTool` in `crawl.rs`. Blocks crawling of internal/private endpoints before `Website::new()` is called.
*   **Security: Port Scanner Restriction (CRITICAL):** Added localhost-only restriction to `CheckPortTool` in `network.rs`. Only allows `127.0.0.1`, `localhost`, `::1`, `[::1]`. If a non-allowed host is given, resolves it and checks if any resolved IP is loopback. Prevents internal network enumeration.
*   **Security: Command Injection in xdg-open (CRITICAL):** Added shell metacharacter validation for URLs in `open.rs`. Blocks `;`, `|`, `&`, `$`, `` ` ``, `\n` characters. Separated URL and file path handling paths, both using `tokio::task::spawn_blocking`.
*   **Security: SQL Injection Enhancement (HIGH):** Expanded SQL injection defense in `db_inspector.rs` with Unicode confusable normalization (zero-width chars, fullwidth digits), additional blocked keywords (`UNION`, `EXCEPT`, `INTERSECT`, `LOAD`, `OVERWRITE`, `CALL`, `EXECUTE`, `HAVING`, `GROUPBY`, `ORDERBY`), semicolon blocking (allows trailing `;` only), and SQL comment blocking (`--`, `/*`).
*   **Bugfix: File Size Guard (HIGH):** Added 50MB file size limit on `ReadFileTool` in `filesystem.rs`. Returns error with guidance to use line ranges for large files.
*   **Bugfix: DOCX Size Limit + Table Recursion (HIGH):** Added 50MB limit on DOCX file reads in `doc_reader.rs`. Added depth parameter to `extract_table()` with `MAX_TABLE_DEPTH = 20` guard to prevent stack overflow from deeply nested tables.
*   **Bugfix: Blocking I/O in ast_grep (MEDIUM):** Wrapped `Command::output()` in `tokio::task::spawn_blocking` in `ast_grep.rs` to prevent blocking the tokio runtime thread pool. Also fixed clippy redundant closure warning.
*   **Bugfix: Blocking I/O in git_manager (MEDIUM):** Wrapped `cmd.output()` in `tokio::task::spawn_blocking` in `git_manager.rs` to prevent blocking the tokio runtime thread pool.
*   **Bugfix: Blocking I/O in system_info (MEDIUM):** Wrapped all 7+ `Command::output()` calls in `tokio::task::spawn_blocking` in `system_info.rs` to prevent blocking the tokio runtime thread pool.
*   **Bugfix: Cron Serialization Panic (MEDIUM):** Changed `.unwrap()` to `.filter_map(|j| serde_json::to_value(j).ok())` in `cron.rs` to prevent panics on serialization failures.
*   **Bugfix: Outline String Slicing Panic (MEDIUM):** Added bounds checking on all 4 visitor methods in `outline.rs` (`visit_function`, `visit_class`, `visit_ts_interface_declaration`, `visit_ts_type_alias_declaration`). Changed `self.source_text[..start]` to safe conditional with length check.
*   **Bugfix: Batch Insert Performance (MEDIUM):** Wrapped the INSERT loop in `notes.rs` in a SQLite transaction (`BEGIN TRANSACTION` / `COMMIT`) for batch insert performance.
*   **Bugfix: LLM Code Backup (MEDIUM):** Added `.bak` backup creation before writing LLM-generated code in `cargo_manager.rs`. On write failure, restores from backup.
*   **Security: Chrome Flags Hardening (CRITICAL):** Removed `--disable-web-security` and `--allow-file-access-from-files` from `image_generator.rs` and `html_video.rs` for consistency with `obscura.rs`.
*   **Security: API Key Redaction (HIGH):** Removed raw tool call argument logging from `openai.rs` to prevent API key leakage in debug logs.
*   **Security: MCP Server Removal Confirmation (HIGH):** Added `confirm` parameter requirement for `manage_mcp` remove action to prevent accidental bulk deletion.
*   **Bugfix: Dead Social Search Backends (HIGH):** Replaced dead Nitter and Invidious instances in `social_search.rs`. Twitter search now returns clear error. YouTube search uses direct scraping fallback. Added error propagation in `search_all`.
*   **Bugfix: Reddit Rate-Limit Handling (HIGH):** Added retry logic with exponential backoff for HTTP 429 responses in Reddit search.
*   **Bugfix: Shared Memory DB Corruption Recovery (HIGH):** Added `PRAGMA journal_mode=WAL`, integrity check, and automatic recovery (rename corrupt DB, recreate) in `shared_memory.rs`.
*   **Bugfix: Cron Job Timing (MEDIUM):** Fixed `last_run` being set before execution — now set after completion. `next_run` calculated from actual completion time.
*   **Bugfix: MCP Stale Client Recovery (MEDIUM):** Added `clear_memory_mcp_client()` and retry logic in `LazyMcpToolWrapper::call()` to reconnect when MCP server crashes.
*   **Bugfix: Obscura Tab Leak (MEDIUM):** Restructured `call()` to ensure tab is always closed via scope guard pattern, even on error.
*   **Bugfix: Obscura CDP Timeout (MEDIUM):** Added 30-second timeout to `send_cdp_cmd()` to prevent infinite hang on browser crash.
*   **Bugfix: Crawl Empty Results (MEDIUM):** Added error when crawl returns zero results instead of silently returning empty array.
*   **Bugfix: DDG Search Fallback Logging (LOW):** Added warning log when DuckDuckGo scraping fails before falling back to Mojeek.
*   **Maintenance: Version Bump:** Bumped to v0.0.16. All 114 tests passing, 0 clippy warnings.

### v0.0.15
*   **Security: SQL Injection Defense (CRITICAL):** Replaced trivially-bypassable keyword blocklist in `DbInspectorTool` with comprehensive SQL injection defense: normalized whitespace removal, blocklist of dangerous SQL keywords (INSERT, UPDATE, DELETE, DROP, ALTER, CREATE, ATTACH, DETACH, PRAGMA, etc.), blocklist of sqlite3 dot-commands (.shell, .import, .output, .read, .system), and whitelist requiring queries start with SELECT or EXPLAIN.
*   **Security: Shell Command Allowlist (CRITICAL):** Added compile command allowlist validation to `CompilerAutoHealTool` (cargo, rustc, gcc, clang, make, npm, python, etc.), enforced `max_iterations` cap of 5, added backup file creation before AI-generated overwrites.
*   **Security: SSRF Prevention (CRITICAL):** Added `validate_url()` to `web_fetch` blocking localhost, loopback, cloud metadata endpoints (169.254.169.254), private/reserved IP ranges, and non-HTTP schemes. Restricted `rust_docs` `sub_path` to only `https://docs.rs/` or `https://crates.io/` URLs.
*   **Security: WhatsApp Webhook Signature Verification (CRITICAL):** Added HMAC-SHA256 signature verification using `X-Hub-Signature-256` header. Reads `WHATSAPP_APP_SECRET` env var; returns 403 on invalid signatures when configured.
*   **Security: CORS Hardening (CRITICAL):** Replaced `allow_origin(Any)` in WebSocket gateway with explicit localhost origins (localhost, 127.0.0.1 on ports 3000/8765). Restricted methods to GET, POST, OPTIONS.
*   **Security: Hardcoded Path Removal (CRITICAL):** Replaced hardcoded `AI_AGENT_TOOLS_BASE` and `PARENT_WORKSPACE_TARGET` constants with functions reading from `AI_AGENT_TOOLS_BASE` and `OPENZ_WORKSPACE_TARGET` env vars, falling back to `dirs::home_dir()`-based defaults.
*   **Security: Browser Flags Hardening (CRITICAL):** Removed `--allow-file-access-from-files` and `--disable-web-security` Chrome flags from `ObscuraBrowserTool`.
*   **Security: JS Injection Fix (CRITICAL):** Replaced naive selector escaping in `GenerateImageTool` with comprehensive escaping for `\`, `"`, `'`, `\n`, `\r`. Restructured JS to pass selector as function argument.
*   **Security: Unsafe `env::set_var` (CRITICAL):** Wrapped `set_var("OPENZ_SILENT")` in `unsafe` block with explanation (safe because it runs before spawning threads).
*   **Security: IMAP TLS (CRITICAL):** Restored `imap::ClientBuilder::new().connect()` (the `imap` crate with `rustls-tls` feature handles TLS automatically).
*   **Bugfix: UTF-8 Panics (HIGH):** Fixed 4+ byte-slicing panic locations: `social_search.rs` (selftext and YouTube snippet), `agent_loop.rs` (tool args and message truncation), `menu.rs` (display title) — all now use `.chars().count()` and `.chars().take().collect()`.
*   **Bugfix: Unbounded Disk Usage (HIGH):** Added `cleanup_old_files()` to `AgentLoop` that deletes files older than 7 days in `~/.openz/traces/` and `~/.openz/tool_outputs/`. Called at start of each turn.
*   **Bugfix: Mutex Poisoning Panics (HIGH):** Changed all 4 `.lock().unwrap()` in `watcher.rs` to `.lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoned mutexes.
*   **Bugfix: NaN Panic (HIGH):** Changed `.partial_cmp().unwrap()` to `.unwrap_or(Ordering::Equal)` in `semantic_search.rs`.
*   **Bugfix: SOP Crash (HIGH):** Replaced `.expect()` with proper error propagation in `sop/engine.rs` for malformed context steps.
*   **Bugfix: Template Recursion Stack Overflow (HIGH):** Added `MAX_TEMPLATE_DEPTH = 10` limit to `template_compiler.rs` recursive rendering.
*   **Bugfix: Security Bypass in Loose Mode (HIGH):** Pipe-to-shell blocking (`| sh`, `| bash`, `| python`) now enforced in ALL modes, not just strict mode.
*   **Bugfix: Activity File Race Condition (HIGH):** Replaced direct `fs::write` with atomic write (temp file + rename) in `activity.rs` to prevent partial reads from concurrent sessions.
*   **Bugfix: Ollama Double-Spawn Race (HIGH):** Combined port check and child guard check into single lock scope in `ollama_manager.rs` to eliminate TOCTOU window.
*   **Bugfix: Silent Error Swallowing (MEDIUM):** Added `eprintln!` for directory creation failures in `main.rs`, replaced nested `if let Ok` with `match` blocks logging via `tracing::warn!` in `activity.rs`.
*   **Bugfix: Vision Model False Positives (MEDIUM):** Changed `m.contains("o1")` to `m.starts_with("o1")` and `m.contains("o3")` to `m.starts_with("o3")` in `model_supports_vision()`.
*   **Bugfix: MockProvider Atomic Race (MEDIUM):** Replaced load-then-store with `fetch_update` using `Ordering::SeqCst` for thread-safe error injection counter.
*   **Bugfix: Unbounded Crawl Parameters (MEDIUM):** Added `.min(1000)` to limit, `.min(10)` to depth, `.max(50)` to delay in `CrawlSiteTool`.
*   **Bugfix: Lost Trailing Newline (MEDIUM):** `ReplaceLinesTool` now preserves trailing newline when original content had one.
*   **Bugfix: Empty API Key Warning (MEDIUM):** Added `tracing::warn!` when API key resolution returns empty string for a provider.
*   **Bugfix: HTTP Client Reuse (MEDIUM):** Replaced per-call `reqwest::Client` creation in multimodal parsing with a shared static client via `OnceLock`.
*   **Bugfix: Regex Recompilation (MEDIUM):** Replaced per-call `Regex::new()` in `context_compactor.rs`, `cli.rs`, and `web.rs` with static `OnceLock<Regex>` patterns.
*   **Bugfix: SVG Attribute Injection (MEDIUM):** Applied `escape_xml()` to all attribute values in `SvgElement::to_svg_string()`.
*   **Bugfix: CSS Template Injection (MEDIUM):** Added backslash escaping to CSS injection in `GenerateImageTool` to prevent template literal breakout.
*   **Bugfix: Python Execution Timeout (MEDIUM):** Added 60-second `tokio::time::timeout` to `PythonSandboxTool` to prevent infinite hangs.
*   **Bugfix: Port Allocation Warning (MEDIUM):** Added `tracing::warn!` when `find_free_port()` exhausts 100 attempts.
*   **Bugfix: Cron Scheduler Shutdown (LOW):** `start_scheduler()` now returns `JoinHandle<()>` for graceful shutdown.
*   **Bugfix: Discord Infinite Reconnect (LOW):** Added `MAX_RETRIES = 10` cap with attempt count in error messages.
*   **Bugfix: WASM Exit Code (LOW):** Extracts real WASI exit code via `I32Exit` downcast instead of hardcoding `1`.
*   **Bugfix: Empty Embeddings (LOW):** Skips DB insert when embedding generation fails (empty vec) in research archive.
*   **Bugfix: Subagent Empty Fallback (LOW):** Filters empty strings from fallback model list before padding.
*   **Bugfix: Biased Random Selection (LOW):** `select_random_message` now uses all 16 UUID bytes for unbiased selection instead of just byte 0.
*   **Code Quality: Clippy Cleanup:** Fixed all 112 clippy warnings across the codebase (strip_prefix, matches! macro, while let loops, static regex, too_many_arguments suppression, etc.).
*   **Maintenance: Version Bump:** Bumped to v0.0.15. All 114 tests passing.

### v0.0.14
*   **Feature: Incremental Session Saving:** Upgraded `AgentLoop` to save the active conversation session (`cli_direct.json`) incrementally to disk: (1) immediately upon receiving a user prompt in `Restore` state, and (2) at the end of each successful turn iteration inside the run loop. This ensures that even if an execution is interrupted via `Esc` or `Ctrl+C` midway, the prompt, thoughts, and intermediate tool outputs are fully persisted on disk. When restarted, typing "continue" allows the agent to resume execution with complete context.
*   **Feature: Resumed Session History Visualization:** Implemented `print_session_history` helper in the CLI channel (`cli.rs`) to format and render previous messages, assistant thoughts, and tool executions. This automatically displays the loaded session's history upon startup/resume or when switching sessions via the `/history` command menu, resolving the visual blank-screen confusion.

### v0.0.13
*   **Bugfix: ANSI Code Log File Pollution:** Configured separate registry layers for the file writer and standard error in `tracing-subscriber` setup inside `main.rs`. This prevents ANSI escape codes from being written to the log file `openz.log`, which was causing level and target parsing failures in the log viewer (`openz logs`) when background/server subcommands were run in a terminal.
*   **Bugfix: Consistent Log Path Resolution:** Updated `default_log_path()` in `logs.rs` to resolve relative to `crate::config::config_dir()` instead of hardcoding `~/.openz/openz.log`. This ensures path alignment whenever `OPENZ_CONFIG_DIR` is customized.
*   **Feature: Real-Time Stream Default:** Changed the default value of the `--tail` parameter from `200` to `0` lines for `openz logs` and channel logs subcommands. This allows `openz logs` to start tailing immediately from the current file end (showing only live logs one by one as they happen, like a Hono server) while still supporting historical inspection via manual `--tail N`.
*   **Bugfix: Backtrace Pruning Regex Correction:** Fixed a typo in `context_compactor.rs` where the backtrace regex pattern had a double caret `^^` instead of a single caret `^`, enabling correct frame pruning.

### v0.0.12
*   **Feature: High-Fidelity HTML-to-Image Generation:** Rewrote `GenerateImageTool` (`generate_image`) to render and capture complex HTML, CSS grid/flex layouts, Tailwind CDN styles, web fonts, and custom SVGs using a local headless Chrome/Chromium instance via CDP at high-DPI Retina resolution (`device_scale_factor: 2.0`). Supports custom CSS injections and element-specific crops (`selector`).
*   **Feature: Remotion-Equivalent Video rendering:** Added `HtmlToVideoTool` (`html_to_video`) to load custom HTML/CSS timelines, tick frames programmatically via JS, capture snapshots via CDP, and stitch frames into final MP4 files using FFmpeg.
*   **Feature: Asynchronous Command Interruption:** Upgraded shell command execution (`ExecCommandTool`, `PythonSandboxTool`) and cargo compilations to asynchronous processes (`tokio::process::Command`) with `.kill_on_drop(true)`, enabling instant child process termination when an agent turn is interrupted via `Esc` or `Ctrl+C`.
*   **Bugfix: Modern Chrome CDP Verb Compatibility:** Patched all browser engines (`image_generator.rs`, `obscura.rs`, `html_video.rs`) to use a cascading `PUT` request with `GET` fallback on the `/json/new` endpoint, resolving `405 Method Not Allowed` failures enforced by modern Chrome (149+).
*   **Bugfix: Headless Browser & Video Generator UTF-8 Charset Encoding:** Added custom middleware to force the `text/html; charset=utf-8` header on the local static file servers in both `HtmlToVideoTool` (`html_to_video`) and `GenerateImageTool` (`generate_image`). This ensures all emojis, icons, and special UTF-8 characters (like middle dots) render correctly without text corruption (such as `ðŸ”—`, `âŒ¨ï¸`) in output images and videos.
*   **Bugfix: Infinite Argument-Correction Loop:** Patched raw newline handling and root parameter fallbacks inside `extract_tool_call` to prevent infinite tool calling errors.
*   **Bugfix: Response Continuation Tool-Calling & Loop Detection:** Disabled tool definitions during response continuation to prevent models from generating malformed tool calls, and enabled re-parsing of fallback tool calls on completed accumulated responses. Fixed a bug in `count_previous_tool_calls` and prompt history construction where OpenAI-style nested tool calls (nested under `function`) bypassed loop detection and context formatting.
*   **Configuration: Local-First Embeddings:** Locked configuration files to `"embeddings": { "mode": "local" }` to ensure vector lookups run entirely offline via FastEmbed and avoid remote cloud connection calls.
*   **Feature: Raw SVG & Global Styling in SVG Animator:** Enhanced `SvgAnimatorTool` (`create_animated_svg`) by adding support for the `raw_svg` parameter (allows direct raw code injection or partial code wrapped automatically inside an SVG envelope). Enabled common styling attributes (`class`, `style`, `transform`, `filter`, `clip_path`, `mask`) globally on all shapes, and implemented attribute deduplication/overwriting in `SvgElement` construction.
*   **Documentation: Workspace Alignment:** Documented the new visual schemas in `AGENTS.md` and active agent skill instructions under `onpkg_docs/image_generator.md`.
*   **Bugfix: Real-time Log Streaming & Buffering:** Rewrote the file tailing logic inside the logs viewer (`openz logs`) using periodic file reopenings to handle file rotation/inode recreation reliably on all platforms, alongside Unix inode checks to detect recreated files. Implemented a trailing buffer to slice and print only complete lines. Fixed a seek-index calculation bug where the file offset pointer was advanced by the processed buffer offset instead of the raw read bytes size, preventing duplicate reads and out-of-sync pointer resets. This completely resolves terminal truncations, duplicate entries, lockups, and output delays. Additionally, updated log initialization in `main.rs` to stream logs live to `stderr` with ANSI colors during background server subcommands (e.g. `gateway`, `telegram`, `discord`, `whatsapp`), making server operation visible in real-time.
*   **Bugfix: Headless Browser Local File Sandboxing (Snap/Flatpak Compatibility):** Replaced sandboxed `file://` URLs in `generate_image` and `html_to_video` with a dynamically spawned, temporary, local Axum web server on a random free port (e.g. `http://127.0.0.1:PORT/file`). Added `--allow-file-access-from-files` and `--disable-web-security` flags. Resolved relative path bug where parent directories resolved to empty string `""` by enforcing absolute paths, preventing "This site can't be reached" (isolated `/tmp` and local file access blockages) on Ubuntu and other systems running Chrome via Snaps or Flatpaks. Also added a configurable `load_delay_ms` parameter (default `1500` ms) to the `html_to_video` tool to allow custom timing configuration for heavy pages to fully load/mount JS bundle animations before frame screenshots are captured.
*   **Prompt: Agent System Prompt Guidelines:** Made the OpenZ agent system prompt aware of its creator (Aswin), inspirations, specifications, features, and `changelog` command.
*   **Documentation: README.md Updates:** Updated README.md documentation for the `changelog` command.
*   **Maintenance: Version Bump:** Staged and committed all outstanding code changes and version bump to GitHub.

### v0.0.11
*   **Feature: Changelog Subcommand:** Added `openz changelog` command to display features, specifications, and version history.
*   **Feature: Changelog File:** Added `CHANGELOG.md` in the project root.
*   **Optimization: Curator Throttling:** Implemented context length and tool-use checks inside the background curator to prevent unnecessary API expenses.
*   **Optimization: Stale Skills Archival:** Throttled the skills database archiver to a 24-hour interval via persistent JSON timestamps.
*   **Feature: Cloud-First Embeddings:** Integrated remote vector embedding fallbacks and a `"cloud_only"` low-RAM mode to skip downloading local ONNX weights.
*   **Feature: Compiler Auto-Healing:** Added the `CompilerAutoHealTool` to automate syntax and compilation repair loops in Rust/JS.
*   **Maintenance: Startup Cleanup:** Automated git worktree and temporary workspace pruning on startup.
*   **Optimization: Low-Resource Build Mode:** Added a `--low-resource` (or `--low-mem`) flag to `localinstall.sh` and `localupdate.sh` to restrict parallel compiler jobs and codegen-units, preventing high memory and CPU utilization during installation/updates.
*   **Optimization: Cargo.toml Release Profile:** Configured custom release build settings (`codegen-units = 1`, `lto = "thin"`, `strip = true`, `debug = false`) to natively reduce peak RAM usage and compiler threads during production builds, reducing final ROM footprint.

### v0.0.10
*   **Feature: SQLite Memory Layer:** Shifted skills and long-term facts to a SQLite database (`~/.openz/memory.db`) with auto-migration.
*   **Feature: Code Semantic Search:** Embedded structural search using `ast_grep` and fast vector indexing.
*   **Feature: subagents Registration:** Registered specialized subagent profiles as dynamic LLM tools.
*   **Feature: `mermaid_designer`:** Added a dedicated subagent for generating SVG flowcharts.

### v0.0.9
*   **Feature: Cryptographic Ledger:** Added SHA-256 Merkle chain hash ledger for auditing agent loops (`/audit` command).
*   **Feature: WhatsApp Channel:** Built Axum webhook channel for WhatsApp API integration.
*   **Feature: Auto-Continuation:** Stitched assistant messages seamlessly when hitting token limits (`finish_reason = "length"`).

### v0.0.8
*   **Feature: Email Channel:** Added pure Rust IMAP/SMTP email client.
*   **Feature: Discord Channel:** Added Discord Gateway WebSockets channel support.

### v0.0.7
*   **Feature: Telegram Bot:** Added Telegram polling channel.
*   **Feature: WebSocket Gateway:** Built static UI server and local OpenAI endpoint.

### v0.0.1 - v0.0.6
*   **Core Foundation:** Initial Clap CLI parser, sandboxed execution, filesystem tools, and basic Anthropic/OpenAI provider trait routing.
