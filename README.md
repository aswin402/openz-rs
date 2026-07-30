# OpenZ 🦊 `v0.0.93`

<p align="center">
  <img src="assets/logo.png" width="200" alt="OpenZ Logo">
</p>

OpenZ is a high-performance personal AI agent framework built in Rust by **Aswin**. It combines an interactive terminal agent, background channels, native tools, memory, SearchXyz research, Headroom compression, OpenMedia generation, OpenDoc document automation, dynamic subagents, and MCP integration in one local-first binary.

**Repository:** [github.com/aswin402/openz-rs](https://github.com/aswin402/openz-rs)

OpenZ was rebranded from `nanobot` and is inspired by Zeroclaw, Nanobot, hermes-agent, loops!, DOX, Headroom, OpenMemory, SearchXyz-style research systems, OpenMedia, OpenDoc, and Rust-native MCP tooling.

---

## What Changed In `v0.0.93`

- **Native Search Rescue**: `web_search` now returns direct docs.rs and crates.io results for known Rust crate queries when SearchXyz scraper backends return no results.
- **Tokio Recovery**: Queries like `rust tokio async runtime` now recover to `https://docs.rs/tokio` and `https://crates.io/crates/tokio` without enabling external search fallback.
- **Low-Cost Path**: Rescue results are generated locally from high-confidence crate names, avoiding network retries when native discovery is blocked.

## What Changed In `v0.0.92`

- **Native Search Ranking**: SearchXyz now reranks search results by query coverage, title/snippet/url matches, phrase matches, domain bonuses, and low-quality URL penalties.
- **Cleaner Result Quality**: Search/tag/login/account/category URLs and tracking parameters are penalized before results are truncated.
- **Merge Ranking Preserved**: Multi-backend merge still deduplicates normalized URLs, then ranks the merged set.
- **Doctor Visibility**: `searchxyz_doctor` now reports ranking as enabled and lists the ranking factors.

## What Changed In `v0.0.91`

- **SearXNG-First Native Discovery**: When `SEARCHXYZ_SEARXNG_URL` is explicitly configured and backend order is not, SearchXyz automatically prefers `searxng` first.
- **Explicit Backend Order Preserved**: `SEARCHXYZ_SEARCH_BACKENDS` now lets users define backend order, and SearchXyz will not override it.
- **Backend Health Labels**: `searchxyz_doctor` now labels backends as preferred, configured, missing_key, disabled, or keyless.
- **Better Native Hints**: Doctor now warns when only scraper backends are enabled, Brave is missing a key, or SearXNG is not in backend order.

## What Changed In `v0.0.90`

- **Actionable Native Search Failures**: `web_search` native-only failures now tell users to run `searchxyz_doctor`, configure `SEARCHXYZ_SEARXNG_URL`, or explicitly use `search_policy=native_then_external` for fallback.
- **Inline Failure Diagnostics**: Added `diagnose_on_failure=true` to append the SearchXyz doctor report when native-only search fails.
- **Shared Doctor Report Builder**: Reused the same health report logic for `searchxyz_doctor` and web search failure diagnostics.

## What Changed In `v0.0.89`

- **SearchXyz Doctor**: Added `searchxyz_doctor` to report native search policy, configured backend order, SearXNG/Brave/headless status, safety settings, cache entries, indexed source count, graph size, and local paths.
- **Native Search Debugging**: Native-only no-result failures now have a first-class diagnostic path before enabling external fallback.
- **Registration Coverage**: Added focused metadata and native tool registration coverage for the new doctor tool.

## What Changed In `v0.0.88`

- **Native Web Search Default**: `web_search` now defaults to `search_policy=native_only`, so OpenZ uses the embedded SearchXyz dispatcher instead of automatically falling through to external APIs/scrapers.
- **Configurable Fallback**: Added `search_policy` with `native_only`, `native_then_external`, and `external_only`, plus `OPENZ_WEB_SEARCH_POLICY` for runtime override.
- **Cleaner Failure Mode**: When native-only search fails, OpenZ returns an explicit policy error instead of silently calling Tavily, Exa, DuckDuckGo, or Mojeek.

## What Changed In `v0.0.87`

- **SearchXyz Hardening**: Added domain include/exclude filters, optional multi-backend merge mode, compact diagnostics, explicit cache modes, and one-off no-save reads.
- **Safer Fetching**: SearchXyz now blocks localhost/private/link-local URL fetches by default and validates redirect targets before returning content.
- **HTTP Cache Discipline**: Exact URL fetches now preserve ETag, Last-Modified, Cache-Control, and content type metadata, with 304 reuse and stale fallback on live fetch failure.
- **Research Evidence Summary**: Deep research now appends deterministic claims, source, conflict, and unknown sections before compiled documents.

## What Changed In `v0.0.86`

- **Vision Routing Discipline**: Non-vision active models now instruct OpenZ to call the direct `vision_agent` tool for image analysis instead of routing through generic `delegate_task`.
- **GUI Open Discipline**: Local GUI display requests now prefer `open_path` and device inventory before trying explicit desktop apps or shell launchers.
- **Old Transcript Cleanup**: Documents the v0.0.81 image/open workflow issues and prevents the same routing mistakes in future turns.

## What Changed In `v0.0.85`

- **Generic Dynamic Loop Guard**: Progress-aware repeat detection now applies beyond browsers to read-only/search/status tools such as web search/fetch, file reads, grep, docs, memory recall, git status, and device inventory.
- **Mutating Tools Stay Strict**: Writes, shell commands, edits, and other side-effect tools still count exact repeats even if their output differs.
- **Progress From Results**: OpenZ derives loop safety from tool-result signatures instead of a global fixed limit or model-controlled override.

## What Changed In `v0.0.84`

- **Progress-Aware Browser Loop Guard**: Browser observation repeats are now judged by result state. Repeated snapshots/page-source reads with changed output are allowed; identical stale output still counts toward loop protection.
- **Safer Than Self-Raising Limits**: The model cannot bypass the guard by asking for a bigger limit; OpenZ derives progress from tool-result signatures.

## What Changed In `v0.0.83`

- **Browser Fill Repair**: `gsd_browser` fill now accepts `text`, `value`, `query`, or `content`, preventing search-box fills from failing on harmless argument aliases.
- **Browser Observation Loop Fix**: Repeated `gsd_browser` snapshots/page-source reads are treated as normal observation steps instead of duplicate loops.
- **Web Acquisition Recovery**: Runtime guidance now keeps trying browser-backed retrieval paths before declaring download/open/show tasks blocked.
- **Research Memory Hygiene**: Download/show workflows no longer auto-save research briefs unless the user explicitly asks for research or comparison.

## What Changed In `v0.0.82`

- **Device Inventory Memory**: Added native `device_inventory` tool for local app/device capability CRUD and suggestions.
- **TUI Device Command**: Added `/device` for listing, suggesting, adding, deleting, and scoring local capabilities from `openz agent`.
- **Automatic Open Learning**: `open_path` now records successful default opens into `~/.openz/device_inventory.json`, so OpenZ remembers what works for images, videos, PDFs, URLs, and editable files.
- **Safe First Step**: The registry stores and ranks capabilities, but does not execute arbitrary remembered commands; execution stays in existing guarded tools.

## What Changed In `v0.0.81`

- **Mivi Model Menu Repair**: `/model` now lists the built-in `mivi` local provider and includes configured default models such as `mivi llm`.
- **Configured Defaults In `/model`**: Built-in providers now merge their configured `default_model` into the selectable model list.
- **Stronger Streaming Recovery**: If a provider streams reasoning-only during both the original and recovery request, OpenZ now uses the recovered reasoning as visible final text instead of the placeholder failure message.

## What Changed In `v0.0.80`

- **Streaming Recovery Actually Streams**: When a provider streams only reasoning first, the final-answer recovery now uses streaming too, so recovered answer text can flow chunk-by-chunk.
- **No Recovery Status Line**: The hidden recovery request stays silent and does not show a user-facing recovery spinner.
- **Cleaner Provider Explanation**: Streaming now distinguishes provider `reasoning_content` deltas from final `content` deltas instead of treating reasoning-only streams as complete answers.

## What Changed In `v0.0.79`

- **Streaming Off By Default**: New configs now default to non-streaming responses for cleaner TUI output and fewer gateway edge cases.
- **TUI Streaming Selector**: Added `/streaming` inside `openz agent` with Enable, Disable, and Back options showing the current state.
- **Quiet Recovery Path**: Internal final-answer recovery no longer shows a visible `Recovering final answer...` status line.
- **Settings Visibility**: `/settings` now shows whether response streaming is enabled or disabled.

## What Changed In `v0.0.78`

- **Final Answer Recovery For Streaming Thoughts**: If a streaming model returns only reasoning, OpenZ now makes one recovery call for the final answer.
- **Thought Then Answer**: The TUI keeps the formatted `Thought for X.Xs` block and then shows the user-facing answer instead of stopping after thoughts.
- **No Raw Reasoning Dump**: The fallback still avoids printing provider reasoning as plain answer text.

## What Changed In `v0.0.77`

- **Streaming Thought Fallback Repair**: Streaming reasoning-only responses no longer dump raw reasoning as plain answer text.
- **Formatted Thought Output**: When thoughts are enabled, fallback reasoning now appears under the normal `Thought for X.Xs` block.
- **No Blank Regression**: The fallback still keeps the turn visible instead of ending after the thinking spinner.

## What Changed In `v0.0.76`

- **Thought Display Restored By Default**: TUI streaming shows the old `Thought for X.Xs` block and live thinking timer again.
- **Configurable TUI Thoughts**: Use `/tui thoughts full|compact|off` to choose full reasoning display, compact summaries, or no thought display.
- **Configure UI Support**: `openz configure` now includes a TUI category for thought display mode.
- **Agent-Controlled Setting**: `manage_config` can update `tui_thought_display`, so you can ask OpenZ itself to change the mode.

## What Changed In `v0.0.75`

- **Private Reasoning In TUI**: Normal streaming answers no longer print provider reasoning as `Thought` before the final response.
- **Quiet Thinking Spinner**: Reasoning chunks no longer leave `▶ Thinking...` artifacts in the terminal for simple answers.
- **Tool Planning Still Visible**: Compact Thought output is preserved only when reasoning leads into tool calls.

## What Changed In `v0.0.74`

- **Reasoning-Only Answer Repair**: Streaming turns that receive only `reasoning_content` no longer end with a visible Thought block and no final answer.
- **TUI Output Fix**: Reasoning-only fallback content is returned as visible final content instead of being marked as already streamed.
- **Regression Coverage**: Added a focused test for reasoning-only streaming behavior so greetings/simple prompts do not disappear after thinking.

## What Changed In `v0.0.73`

- **Canonical Website Topics**: URL-based research topics now keep stable `host/path` form, so `https://sakana.ai/fugu/` saves as `sakana.ai/fugu` instead of a loose space topic.
- **Doctor Memory Repair**: `openz doctor` now repairs legacy research brief aliases using saved source evidence, merging vague topics like `hermes` into canonical repo topics when proven.
- **Duplicate Brief Cleanup**: Existing duplicate website/repo brief topics are renamed or merged without deleting content.
- **Regression Coverage**: Added focused tests for Sakana-style website topics, Hermes-style repo alias repair, and legacy website topic repair.

## What Changed In `v0.0.72`

- **Stable Research Brief Reuse**: Stable definition/comparison questions no longer force live web lookup just because the model put a URL into a tool argument. User text now drives refresh intent.
- **Quieter Memory Context**: Saved source match notifications are suppressed when a fresh research brief already matched, reducing duplicate footers on simple questions.
- **Canonical Research Topics**: Auto-capture now prefers an existing canonical repo topic for short aliases, so follow-ups like `Hermes` reuse `nousresearch/hermes-agent` instead of creating vague duplicate topics.
- **Regression Coverage**: Added focused tests for argument URL policy, brief-first lookup blocking, source notification suppression, and alias-to-repo topic capture.

## What Changed In `v0.0.71`

- **Legacy Source Label Repair**: Old saved GitHub labels such as `github.com - pulls` and `github.com - 6` are repaired at display time using the source URI.
- **Noisy Brief Filtering**: Saved research briefs that are mostly GitHub UI chrome, filters, sort controls, footer text, or cookie/status text are ignored instead of reused.
- **Memory Cleanup Coverage**: Added regressions for legacy GitHub source label repair and GitHub UI-chrome brief rejection.

## What Changed In `v0.0.70`

- **Live Refresh Footer Suppression**: Direct URL, research, and check-again prompts no longer display saved brief/source match notifications after the answer.
- **Cleaner GitHub Source Labels**: GitHub repos, issues, PRs, and raw files now save with readable labels such as `owner/repo`, `owner/repo issue #6`, and `owner/repo PR #12`.
- **Refresh-Only Capture Hygiene**: `check again <url>` style verification prompts no longer auto-save duplicate research briefs when they are only confirming current state.

## What Changed In `v0.0.69`

- **Research Memory Relevance**: Saved research briefs now require topic-anchor relevance, so broad comparison briefs no longer answer unrelated entity questions just because a name appears in the summary.
- **Saved Source Footer Filtering**: Saved source matches now require relevance in label, URI, or aliases, preventing unrelated source footers from appearing on answers.
- **Auto-Capture Hygiene**: Non-research/debug turns are no longer auto-saved as research briefs, while real research prompts and natural definition follow-ups still save useful context.
- **Hardcoded Fix Cleanup**: Removed one-off `/home/aswin` video-render paths, centralized live research policy, replaced fixed-year detection with runtime current-year logic, and removed brittle project/model-specific gates.

## What Changed In `v0.0.68`

- **Cache Validator Refresh**: `web_fetch` now refreshes exact-URL cache validators and expiry metadata on `304 Not Modified`, so validated cached pages do not stay permanently stale.
- **Last-Modified Heuristic**: Pages without `Cache-Control` now use `Last-Modified` heuristic freshness instead of expiring immediately.
- **Cache Correctness Coverage**: Added focused regression coverage for missing `Cache-Control` plus `Last-Modified` behavior.

## What Changed In `v0.0.67`

- **Exact URL Web Cache**: `web_fetch` now stores exact URL responses in SQLite with body text, `ETag`, `Last-Modified`, `Cache-Control`, fetch time, expiry time, status, and use count.
- **HTTP Revalidation**: Stale cached pages are revalidated with `If-None-Match` / `If-Modified-Since`, and `304 Not Modified` returns the cached body without redownloading content.
- **Cache Modes**: `web_fetch` now supports `cache_mode`: `auto`, `prefer_cache`, `revalidate`, and `bypass`.
- **Stale Fallback**: If a live re-fetch fails, OpenZ can fall back to the stale exact-URL cached body instead of failing outright.

## What Changed In `v0.0.66`

- **Generic Live Research Policy**: Saved research briefs no longer block explicit live lookup intent. Direct URLs, URLs inside tool arguments, current/latest prompts, and "check again / verify / refresh / browse" style requests force web/search tools to run.
- **Stable Brief Reuse Preserved**: Fresh cached briefs still short-circuit stable non-live definition/comparison questions, keeping the token-saving behavior without stale answers for exact-page checks.
- **Orchestrator Prompt Hardening**: Research context now tells the agent to refresh exact sources/web whenever the user provides a URL or asks to re-check, instead of relying on broad topic freshness.

## What Changed In `v0.0.65`

- **Configure Exit Reliability**: `openz configure` now exits cleanly after save/back/Esc flows instead of staying alive behind the terminal.
- **Custom OpenAI-Compatible Providers**: Providers configure menu now has `Add Custom Provider` for provider name, API base URL, API key, and default model.
- **Model Picker Integration**: Custom providers now appear in TUI `/model` and text-channel `/switch-model`, with `custom_provider/model` prefix routing.
- **Local Provider Support**: Local custom endpoints on localhost can run without an API key; remote custom providers support `OPENZ_PROVIDER_<NAME>_API_KEY`.

## What Changed In `v0.0.64`

- **Performance & Footprint Optimization**: Integrated `tikv-jemallocator` global allocator, capped SQLite memory caches, 5-minute FastEmbed idle eviction, and automatic DB WAL vacuum on startup.
- **SecurityGuard Hardening**: Added recursive shell subcommand unnesting (`sh -c`, `bash -c`, `eval`, `python -c`) and quote delimiter support to prevent security rule evasion.
- **Resilience & Network Stability**: Subagent git worktrees now unregister cleanly before disk removal, and disconnected WebSocket Gateway clients are safely cleaned up via RAII guard.

- **Native Browser Status Inspection Tool (`inspect_browsers`)**: Added a native diagnostic tool to inspect running Firefox GeckoDriver (port 4444), Chrome CDP (port 9222), `gsd-browser` daemon health/pages, and `logs.db` recent browser errors.
- **De-duplicated Subagent Orchestration**: Consolidated workspace setup, git worktree lifecycle, database branch simulation spaces, cancel guards, schema validation reflection, and evolution review into a shared `SubagentRunContext` helper module.
- **Tool Metadata Architecture Refactor**: Refactored `ToolMetadata` into a clean builder pattern, defined dynamic domain keyword matching, overrode `metadata()` directly on core tools, and eliminated 800+ lines of duplicate match statements in `src/tools/mod.rs`.

## What Changed In `v0.0.62`

- **Operational source suppression:** local action prompts like “open the website in Firefox”, “play the video”, and simple feedback like “that’s good one” now skip saved research/source matching, preventing unrelated source footers on non-research turns.
- **Regression coverage:** added transcript-shaped tests so operational prompts still avoid source-memory injection while explicit comparisons/research prompts continue to use saved sources.

## What Changed In `v0.0.61`

- **Documentation/runtime alignment:** refreshed ONPKG project docs, MCP docs, channel docs, and tool-usage guidance so they match the current native-tool and managed-server runtime.
- **Project backlog cleanup:** replaced skeleton PRD/design/implementation/todo placeholders with concrete OpenZ requirements, architecture, verification rules, and next-priority backlog items.
- **Self-management inventory metadata:** updated ONPKG metadata to list the current self-management tools, including `openz_inventory`, `manage_servers`, `workflow_memory`, and `tool_catalog`.

## What Changed In `v0.0.60`

- **Runtime model identity grounding:** `openz_inventory` now reports configured model/provider, resolved effective provider/model when available, vision support, caveman mode, and streaming status.
- **No model guessing:** questions like “what model are you?” or “which language are you best at?” now require live runtime inventory first and must disclose uncertainty instead of inventing hidden model architecture or benchmark facts.

## What Changed In `v0.0.59`

- **Self-inventory source suppression:** prompts like “what tools do you have?” and “what features do you have?” now skip saved external research/source matching, so unrelated OpenHuman/Hermes source footers should not appear.
- **Comparison-safe guard:** explicit comparison prompts such as “what features do you have vs Hermes?” still allow saved source/research context.

## What Changed In `v0.0.58`

- **Agent-managed server lifecycle:** OpenZ now prompts itself to use `manage_servers` automatically for OpenZ-launched dev servers instead of relying on users to type `/stop-server` or guessing with `pkill`.
- **Live feature inventory:** new `openz_inventory` tool reports the running binary's exact version, commands, channels, registered tools, domains, subagents, and active server state so feature answers come from live data instead of memory.
- **Reusable generation workflows:** self-improvement now explicitly saves `workflows_to_save`, and built-in skills teach chunked static-site creation plus segmented long HTML-video rendering.

## What Changed In `v0.0.57`

- **Managed background servers:** dev-server launches such as `npm run dev`, `bun run dev`, `npx vite`, and `python -m http.server` are tracked with id/pid/command metadata instead of becoming invisible background work.
- **Server controls:** use `/servers` to list OpenZ-launched background servers and `/stop-server <id|all>` to stop them from the TUI or Telegram. Models can also use the new `manage_servers` tool.
- **Permission menu responsiveness:** TUI selection menus drain stale key events before drawing so approval prompts reliably accept the first real `Enter`.

## What Changed In `v0.0.56`

- **TUI cancellation cleanup:** Esc/Ctrl+C cancellation now gives the turn keyboard watcher a short clean shutdown window so stale terminal readers do not steal keystrokes from the next input prompt or approval menu.
- **Desktop app launch handling:** `exec_command` now detects common GUI launchers, browsers, media viewers, editors, and dev-server commands and detaches them instead of waiting forever after the window opens.
- **Viewer retry guard:** `open_path` and detached launches now return `user_visible`/`do_not_retry` guidance so the model treats a successful file/app display as complete instead of trying alternate viewers after you close the first app.

## What Changed In `v0.0.55`

- **Research memory reliability:** simple follow-up questions now reuse fresh canonical research briefs without repeated web calls, while explicit research/link-analysis prompts still fetch README/docs/site pages as needed.
- **Canonical research topics:** URL-plus-instruction prompts, GitHub links, and raw GitHub URLs now save under stable repo topics like `agent0ai/dox` and `tinyhumansai/openhuman` instead of weak aliases like `dox` or `openhuman`.
- **Better brief freshness:** auto-saved repo/docs briefs inherit source TTLs instead of expiring after 60 seconds, so useful research stays fresh for about a week by default.
- **Invalid brief protection:** placeholder summaries such as `skipped` are rejected on save and ignored during retrieval, preventing old corrupted rows from blocking needed refreshes.
- **Skipped lookup guard:** no-fetch responses from the fresh-brief gate are no longer auto-saved as new research briefs.
- **High-signal brief summaries:** auto-capture now prefers definition/architecture sentences and trims leading navigation/sidebar/legal noise before saving summaries.
- **Source-strict answer prompting:** saved-brief context tells models to state only facts present in briefs/sources and say `unknown` for missing details instead of guessing licenses, channels, releases, or integrations.
- **Release baseline:** v0.0.54 introduced automatic source ranking, freshness-aware source memory, reusable workflow guidance, compact TUI match notices, and automatic research capture.

---

## Core Capabilities

### Agent Runtime

- TUI chat loop with slash commands, raw-mode-safe rendering, session persistence, streaming support, and automatic continuation when a provider stops because of output length.
- Multi-channel operation through terminal, WebSocket/WebUI gateway, Telegram, Discord, WhatsApp, and Email.
- OpenAI-compatible local gateway endpoint through `openz gateway`.
- Background self-improvement that can update memories and skills from completed conversations.
- Session integrity via hash-chain verification and the `/audit` command.

### Tools And Automation

OpenZ registers native tools directly in Rust. The major tool families are:

- **Files, shell, code, and git:** read/write/patch/list/find files, line replacement, grep, AST search, code outline, git operations, cargo operations, DB inspection/write, Rust docs, template compilation, WASM and Python sandbox execution.
- **Research and web:** web fetch/search, SearchXyz web/research/cache/graph tools, GitHub repo ingestion, site maps, crawlers, social search, browser automation via GSD/Obscura/Firefox, and vector semantic search.
- **Memory:** cognitive memory, graph memory, working memory, episodic reflections, shared memory, semantic facts, hybrid FTS/vector search, conflict handling, fact extraction, stale/deletion handling, code graph indexing, and memory stats.
- **Headroom compression:** content/file/directory/diff/schema compression, signature-only code compression, CCR cache, FTS cache search, cache import/export, token estimates, stats, usage analytics, and bounded run-and-compress.
- **Subagents:** `delegate_task`, dynamic subagent profiles, `parallel_research`, evaluator/optimizer loops, subagent creation/deletion/optimization, cancellation propagation, and bounded dynamic timeouts.
- **OpenMedia:** SVG/image/chart/icon/video generation, video templates, animated SVG timelines, Lottie conversion, filters, resize/crop/convert/batch processing, quality scoring, and prompt refinement.
- **OpenDoc:** read/search/convert documents, DOCX/PPTX/XLSX/PDF creation and editing, PDF splitting/merging/forms/tables, OCR checks, and archive digests.
- **MCP:** CRUD MCP server configuration, stdio client support, and a gRPC-to-stdio bridge for MCP transport.

### Memory System

`v0.0.50` and `v0.0.51` made memory a first-class OpenZ subsystem:

- `MemoryCoordinator` coordinates semantic, graph, recall, deletion, and stats paths.
- Hybrid search combines FTS5 and deterministic vector embeddings with reciprocal-rank fusion.
- `forget_memory` purges or tombstones across semantic metadata, FTS rows, graph, shared/cognitive memory, research, sessions, and skills-derived facts.
- Prompt memory is query-aware, stale-fact aware, deduplicated, and top-30 budgeted.
- Fact extraction supports multi-word entities, profile facts, chained clauses, `built_with`, `lives_in`, and `prefers` relations.
- Regression coverage includes stale facts, contradictions, deletion, recall relevance, poisoning attempts, prompt budgeting, embeddings, and codebase indexing.

### Safety And Resource Controls

- SecurityGuard intercepts destructive shell commands, privilege escalation, process control, network transfer commands, and risky file writes before execution.
- Optional Linux seccomp BPF sandboxing is available for subprocesses when enabled in config.
- High-risk tools use resource policy checks and approval gates.
- Long-running tools use recommended timeouts and bounded overrides instead of hardcoded 120s limits.
- SearchXyz destructive operations require explicit confirmation, and web/repo ingestion supports output and repository limits.
- Install/update scripts back up global OpenZ data, relocate stray runtime DB files, repair corrupt Cargo registry source unpacks, warn on huge build caches, and can clean `target/` explicitly.

---

## Runtime Commands

| Command | Purpose |
|---|---|
| `openz onboard` | First-time provider setup wizard. |
| `openz configure` | Configure providers, channels, gateway, sandbox, and preferences. |
| `openz agent` | Start the terminal TUI agent. |
| `openz gateway` | Start WebSocket/WebUI gateway and local API endpoint. |
| `openz telegram` | Start Telegram bot listener. |
| `openz discord` | Start Discord gateway listener. |
| `openz whatsapp` | Start WhatsApp webhook receiver. |
| `openz email` | Start Email IMAP/SMTP client. |
| `openz subagent` | Manage subagent profiles. |
| `openz sop list|instances|trigger|resume` | Manage SOP workflow instances. |
| `openz mcp-bridge --port <N> -- <cmd> [args...]` | Bridge gRPC/TCP to stdio MCP. |
| `openz logs [--tail N] [--session S] [--level L]` | View structured logs. |
| `openz changelog` | Print footprint specs and release history. |
| `openz streaming` | Toggle response streaming. |
| `openz doctor` | Check runtime DB placement, archive stale graph branches, and report disk/cache pressure. |

---

## Install And Update

### Install

```bash
./localinstall.sh
```

### Update

```bash
./localupdate.sh
```

### Balanced and low-resource builds

Recommended for most laptops when normal update lags the machine:

```bash
./localupdate.sh --balanced
```

Balanced mode caps Cargo to 2 jobs by default, uses the `release-balanced` profile, skips the duplicate pre-install `cargo check`, and avoids ThinLTO linker spikes. It is usually much faster than `--low-resource` while using less RAM/CPU than the full release path.

For a little more speed on stronger machines:

```bash
OPENZ_BUILD_JOBS=3 ./localupdate.sh --balanced
```

Use minimum-resource mode only when the machine is still lagging or swapping:

```bash
./localupdate.sh --low-resource
```

### Reclaim Cargo build-cache space

Repeated local builds and tests can make `target/` very large. Normal install/update runs warn when `target/` is over 20 GiB. To remove rebuildable Cargo artifacts before compiling:

```bash
./localupdate.sh --clean-target
```

or:

```bash
./localinstall.sh --clean-target
```

This is equivalent to `cargo clean` before the build. It does not delete `~/.openz` runtime data, sessions, memories, or config.

---

## Configuration And Data Locations

- Main config: `~/.openz/config.json` or `$OPENZ_CONFIG_DIR/config.json`
- Sessions: `~/.openz/sessions/`
- Memory databases: `~/.openz/memory.db`, `~/.openz/graph_memory.db`, and related SQLite files
- Tool outputs: `~/.openz/tool_outputs/`
- Traces/logs: `~/.openz/traces/`, `~/.openz/openz.log`
- Subagents: `~/.openz/subagents.json`
- Skills: `~/.openz/skills/` plus SQLite-backed skill/memory storage

Key provider environment variables include `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`, `GROQ_API_KEY`, `OPENROUTER_API_KEY`, `MISTRAL_API_KEY`, `OPENCODE_ZEN_API_KEY`, `GOOGLE_AI_STUDIO_API_KEY`, `TELEGRAM_BOT_TOKEN`, `DISCORD_BOT_TOKEN`, and WhatsApp credentials.

---

## Performance And Footprint

These values are intentionally measured/qualified rather than hard-coded marketing claims:

| Area | Current OpenZ Behavior |
|---|---|
| Binary / install size | Install/update scripts print the exact installed binary size. A recent measured dev install was about 124 MB; exact size depends on compiled heavy stacks such as ONNX embeddings, browser/media tooling, and document processing. |
| Idle RAM | About 15-30 MB in cloud/API mode when local embedding models are not loaded. |
| Active RAM | About 30-80 MB typical; 200 MB+ when local ONNX embeddings are loaded. |
| CPU | Near 0% while idle; Tokio async runtime does work only when active. |
| Startup | Core CLI paths are millisecond-scale; full TUI startup depends on config, DB checks, enabled channels, MCP/tool setup, and provider checks. |
| Build cache | Cargo `target/` can grow into tens of GiB during development. Use `--clean-target` when disk pressure appears. |

---

## Development Commands

| Command | Purpose |
|---|---|
| `cargo check` | Fast type check. |
| `cargo test --lib -- --test-threads=1` | Run the library test suite deterministically. |
| `cargo fmt --check` | Verify Rust formatting. |
| `cargo clippy` | Lint. |
| `cargo build --release` | Build optimized release binary. |
| `cargo clean` | Remove rebuildable Cargo artifacts under `target/`. |

The project has no Makefile. The local install/update scripts wrap the common build/install flow and add OpenZ-specific checks.

---

## Project Map

```text
openz/
├── Cargo.toml              # Rust package metadata and dependencies
├── build.rs                # tonic-build proto compilation
├── CHANGELOG.md            # release history and specs
├── localinstall.sh         # global install helper
├── localupdate.sh          # update helper with backup, checks, and install
├── src/
│   ├── main.rs             # dotenv init, Tokio runtime, CLI dispatch
│   ├── cli/                # clap commands, configure UI, logs, doctor, tool registration
│   ├── config/             # schema, provider defaults, loader, migrations
│   ├── providers/          # OpenAI-compatible and Anthropic provider clients
│   ├── agent/              # agent loop, prompt build, security, skills, TUI style
│   ├── channels/           # CLI, WebSocket, Telegram, Discord, WhatsApp, Email
│   ├── tools/              # native tool implementations
│   ├── cron/               # scheduler
│   ├── sop/                # stateful SOP workflow engine
│   └── subagents/          # profile definitions and manager
├── docs/                   # architecture and subsystem docs
└── assets/                 # logo and bundled assets
```

---

## Documentation

- [Architecture](docs/architecture.md)
- [Security Guard & Permissions](docs/security.md)
- [Channels & Configuration](docs/channels.md)
- [Self-Improvement & Memory](docs/self_improvement.md)
- [Model Context Protocol](docs/mcps.md)
- [Tools Registry](docs/tools.md)
- [ZeroClaw Gap Analysis & Roadmap](docs/zeroclaw_research.md)
- [Changelog](CHANGELOG.md)

---

## Notes For Operators

- Use `openz doctor` if runtime DB files appear in a project directory or disk usage looks wrong. It preserves data, relocates stray artifacts under `~/.openz`, and reports oversized caches such as `target/`, `~/.openz`, SearchXyz, and Cargo cache.
- Use `openz logs --tail 100` when debugging channel or provider issues.
- Use `openz changelog` to see the version shipped in the installed binary and current measured binary size.
- Use `./localupdate.sh --clean-target` after heavy development sessions if disk space drops unexpectedly.
- Keep secrets in environment variables or `~/.openz/config.json`; do not commit local config, sessions, or runtime DBs.
