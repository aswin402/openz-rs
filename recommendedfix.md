# OpenZ — Recommended Fixes & Improvements

> Audit based on full codebase analysis at `v0.0.50`.
> Covers architectural debt, correctness bugs, test gaps, and enhancement opportunities.
>
> Historical audit snapshot. Progress notes may describe older layouts or
> versions; use the current project map and architecture docs as the source of truth.

---

## Table of Contents

1. [Critical & High Priority](#1-critical--high-priority)
2. [Medium Priority](#2-medium-priority)
3. [Low Priority / Polish](#3-low-priority--polish)
4. [Enhancements & New Features](#4-enhancements--new-features)
5. [Testing Gaps](#5-testing-gaps)

---

## 1. Critical & High Priority

### 1.1 Subagent Orchestration — Code Duplication (P1)

**Files:** `src/tools/subagent/delegate_task.rs` (1243 lines), `src/tools/subagent/delegate_profile.rs` (~520 lines)

**Problem:** These files still share significant orchestration logic:

**Progress (v0.0.144):** Completed unification of subagent run execution and prompt construction into `src/tools/subagent/mod.rs`. Added `SubagentRunAttempt`, `SubagentRunOutcome`, `run_subagent_attempt()`, `build_subagent_prompt()`, and unified `ensure_markdown_images()`. Both `delegate_task.rs` and `delegate_profile.rs` now delegate their execution loops, cancellation guards, schema validation retries, database branch creation/finalization, and error classification to this shared harness, cutting ~400 lines of duplicated orchestration code across the two tools. Added unit test coverage for prompt building, markdown image wrapping, and early cancellation handling.

**Progress (v0.0.143):** Completed full orchestration deduplication between `delegate_task.rs` and `delegate_profile.rs`. Extracted shared helpers into `src/tools/subagent/mod.rs`: `finalize_simulation_branch()`, `sync_workspace_changes_back()`, `handle_subagent_cancellation()`, and `handle_subagent_success()`. Both tools now share identical branch commit/discard mechanics, worktree teardown logging, workspace sync-back logic with evolution gate reviews, and standardized JSON result wrappers, shrinking both files and preventing divergence.

**Progress (v0.0.138):** Extracted the shared orchestration pieces into `subagent/mod.rs` and `schema_retry.rs`: `execute_with_schema_retries()` (the 42-line ×2 schema retry loop, now with unit tests pinning accept/retry/attempt-limit behavior), `create_workspace_isolation()` (the worktree/scratch/fallback setup block, returning a `WorkspaceIsolation` struct), `CancelOnDrop` (was defined verbatim in both files), `filesystem_write_denied_by_policy()` (single copy + one shared test), and `attach_workspace_fields()` (the cancellation JSON merge). Deleted the never-compiled dead `subagent/context.rs` (stale verbatim copy of `run_evolution_review`). Also fixed three tests made stale by b787471's in-process `spawns_process = false` change: the two metadata assertions and `capability_policy_blocks_static_subagent_wrappers_with_deny_shell` (now pins the intended semantics — deny_shell blocks shell tools while delegation stays available because children inherit the policy). Remaining duplication (workspace setup for image-path scanning, lifecycle display wiring, and the eventual `SubagentRunContext` + `run_subagent()` unification) is still open; the model-cascade vs single-model resolution difference is intentional and stays.
- Workspace isolation (git worktree / recursive copy)
- Image path scanning from goal/context
- Schema validation retry loops (identical 3-attempt blocks copied verbatim)
- Evolution review after completion
- Lifecycle line formatting and status reporting
- CancelOnDrop guard pattern

Progress now done: timeout resolution is shared through `resolve_subagent_timeout_secs()`, and schema parse/validation retry decisions are shared through `schema_retry::evaluate_schema_retry()`. Larger duplication remains in workspace setup, retry execution wiring, lifecycle display wiring, cancellation, and evolution review.

**Recommended fix:** Continue extracting shared pieces incrementally before a full orchestrator split:
- `retry_with_schema_validation()` — shared retry loop
- shared workspace/isolation setup helpers
- shared lifecycle and cancellation result formatting
- eventually `SubagentRunContext` + `run_subagent()` once the smaller helpers are stable

---

### 1.2 Match Statement Bloat — 5 Functions That Grow Linearly with Every Tool (P1)

**Files:** `src/tools/mod.rs`

**Problem:** Five parallel match functions each require updates when a new tool is added:

| Function | Lines | Pattern |
|---|---|---|
| `infer_tool_domain()` | 75+ | `name.starts_with()` / `name.contains()` checks |
| `tool_writes_disk()` | 20+ | explicit name list + pattern checks |
| `tool_uses_network()` | 10+ | name matches |
| `tool_aliases()` | 25+ | per-name match with domain fallback |
| `tool_examples()` | 35+ | per-name match with domain fallback |
| `tool_usage_hints()` | 40+ | per-name match with domain fallback |
| `tool_recommended_timeout()` | 30+ | name patterns (just added) |

Every new tool risks being forgotten in one or more of these functions, leading to incorrect domain classification, missing aliases, or missing timeout hints. First-pass progress is done for subagent tools: `delegate_task`, `delegate_profile`, and `parallel_research` now override `metadata()` through `subagent_tool_metadata()`.

**Progress (v0.0.137):** Consolidated on the `STATIC_TOOL_DEFS` curated data table as the single source for named-tool metadata (the `metadata()` trait override remains the escape hatch, as subagent tools show). All seven functions now consult the table first; `tool_recommended_timeout()` was reduced to dynamic families only (`browser*`/`opendoc_*`/`mcp_*`). Fixed four dead misnamed arms found during the pass — `html_video`→`html_to_video` (900s), `crawl_site`→`crawl_website` (600s + `uses_network=true`), `svg_animator`→`create_animated_svg` (300s), `mermaid`→`render_mermaid` (300s) — whose intended timeouts/network classification silently never applied; also tabled `generate_image`, `generate_video`, `semantic_search`, and `python_sandbox`. Added a registration drift-guard test asserting every `STATIC_TOOL_DEFS` name is a real registered tool, plus regression tests for the fixed timeouts. Remaining heuristics (`infer_tool_domain`, `tool_writes_disk`, `tool_uses_network` substring fallbacks) only serve dynamically-named tools (subagent profiles, MCP wrappers) and are no longer extended for named tools.

**Recommended fix:** Move metadata to the tool implementation itself. Add a method to the `Tool` trait that returns metadata in a structured way:

```rust
trait Tool {
    fn metadata(&self) -> ToolMetadata; // default: infer from name
    fn tool_info(&self) -> ToolInfo {
        ToolInfo { domain: self.metadata().domain, .. }
    }
}
```

Alternatively, use a derive macro or builder pattern so each tool declares its own metadata inline:

```rust
#[tool(domain = "web", risk = Low, timeout = 60)]
struct WebFetchTool;
```

---

### 1.3 `resolve_provider_config()` — 17 Providers, Identical Pattern (Resolved locally)

**File:** `src/config/schema.rs`

**Status:** Implemented in the local working tree. Provider aliases, env vars, default base URLs, and config accessors now live in one `PROVIDER_DEFS` table. `resolve_provider_config()`, `is_provider_configured()`, and `is_provider_available()` now all use the same data source. Regression tests cover alias resolution (`z_ai`), env fallback, `ollama_local` vs configured `ollama`, and the legacy `CEBRAS_API_KEY` fallback for Cerebras.

---

### 1.4 `.unwrap()` / `.expect()` Panic Risk (Resolved locally)

**Problem:** ~150+ calls across the codebase that could panic on error. High-risk patterns:

| Pattern | Count | Risk |
|---|---|---|
| `lock().unwrap()` on poisoned Mutex | ~25 | If a thread panics while holding the lock, all subsequent accesses panic |
| `serde_json::to_string_pretty(...).unwrap()` | ~15 | Panics on non-serializable values (e.g., circular refs, NaN floats) |
| `Receiver::borrow()` on watch channel | ~8 | Panics if sender is dropped while receiver holds reference |
| `String::from_utf8(...).unwrap()` | ~5 | Panics on non-UTF-8 output from subprocesses |
| `PathBuf::to_str().unwrap()` | ~10 | Panics on non-Unicode paths |

**Example (real crash path):** `src/tools/subagent/delegate_task.rs` uses `lock().unwrap()` inside `WorktreeGuard::drop()`. If a subagent panics during workspace operations, the poisoned lock causes a double-panic on cleanup.

**Progress (v0.0.144):** Hardened production call sites across tools, channels, and agent runtime:
- **WebSocket Protocol Serialization:** Replaced 30 `.expect("WebSocket ... must serialize")` calls in `src/channels/websocket/protocol.rs` with `to_value_safe()` helper returning an error payload rather than crashing axum connection tasks. Added unit test `websocket_protocol_events_serialize_safely` in `src/channels/websocket/tests.rs`.
- **SearchXyz Initialization & Concurrency:** Hardened `reqwest::Client` builder with a fallback to `reqwest::Client::new()` in `src/tools/searchxyz/mod.rs`. Added fallback temp directory index creation if `SearchIndex::open` encounters errors. Replaced poisoned mutex expectations with `.unwrap_or_else(|poisoned| poisoned.into_inner())` in `src/tools/searchxyz/web.rs`.
- **Static Regex Invariants:** Replaced `.unwrap()` calls on static regex initializations in `src/tools/db_inspector.rs`, `src/tools/web.rs`, and `src/tools/shared_memory/auto_capture.rs` with descriptive `.expect(...)`.
- **Cancellation Polling Optimization:** Replaced repetitive `.subscribe()` channel allocations inside `is_cancelled` closures in `src/agent/agent_loop/mod.rs` with single pre-subscribed watch receivers.
- **Previous hardening passes:** Regex captures in `src/tools/outline.rs`, JSON assertions in `src/tools/subagent/evaluator_optimizer.rs`, name character checks in `src/tools/subagent/optimize_profile.rs`, temp file names in `src/agent/activity.rs`, loop tags in `src/tools/template_compiler.rs`, IP checking in `src/tools/network.rs`, gRPC client in `src/tools/mcp.rs`, HTML selectors in `src/tools/crawl.rs`, provider message sanitation in `src/providers/openai.rs` and `src/providers/anthropic.rs`, multimodal markdown in `src/providers/mod.rs`, spinner mutexes in `src/agent/style/spinner.rs`, log fallbacks in `src/main.rs`, worktree paths in `src/tools/subagent/delegate_task.rs`, SQLite fallback in `src/tools/graph_memory/`, and fact extraction regex in `src/tools/memory_extra/`.

---

### 1.5 `execute_approved_tool()` Pipeline Refactor (Resolved locally)

**File:** `src/agent/agent_loop/run/mod.rs`

**Status:** Implemented in the local working tree. `execute_approved_tool()` is now a thin wrapper around `ToolExecutionPipeline`. The pipeline separates process-slot acquisition, timeout resolution, cancellation-aware execution, spinner wrapping, success rendering, failure rendering, and error bookkeeping. A focused regression test covers process resource guard behavior used by the pipeline.

---

## 2. Medium Priority

### 2.1 Timeout Lifecycle Status Duration (Resolved locally)

**File:** `src/tools/subagent/lifecycle.rs`

**Status:** Implemented in the local working tree. `SubagentRunStatus::TimedOut` now carries `duration_secs: Option<u64>`, labels include `timed out after <N>s` when available, and `status_json()` includes `durationSecs` for timed-out subagents. Tests cover direct classification, compact lifecycle output, and JSON metadata.

---

### 2.2 Session File Locking — Partial Hardening Done Locally

**File:** `src/session.rs`

**Status:** Partial fix implemented in the local working tree. Session locks now remove stale lock paths older than 60s and use bounded exponential backoff for sync and async acquisition. Tests cover stale corrupted lock cleanup, fresh corrupted lock preservation, and async stale cleanup.

**Remaining caveat:** `fs2::try_lock_exclusive()` is still advisory and may not be reliable on NFS/CIFS. A larger design change would be needed for fully NFS-safe locking.

---

### 2.3 SendNotification — 3 Nearly Identical Channel Blocks (Resolved locally)

**File:** `src/channels/mod.rs`

**Status:** Implemented in the local working tree. `send_notification()` now builds external channel requests through shared request builders for Telegram, Discord, and WhatsApp, then sends them through one `send_external_notification()` path. HTTP send failures and non-success status responses are logged with channel, target, status/error, and response body instead of being silently dropped. Tests cover request construction, invalid Telegram target filtering, Discord auth header, WhatsApp bearer auth, and missing WhatsApp credential skipping.

---

### 2.4 Tool Router Runs on Every LLM Call (Resolved locally)

**File:** `src/tools/mod.rs`

**Status:** Implemented in the local working tree. `ToolRegistry` now caches the last prompt-aware `ToolRouteAnalysis` keyed by prompt, current filter scope, and sorted static tool names. Cache invalidates on tool registration and filter-scope changes, preventing stale routing when the available tool set changes. Tests cover cache reuse and invalidation, and the existing prompt-aware selection regression still passes.

---

### 2.5 No Graceful Degradation for Missing API Keys (Resolved locally)

**File:** `src/providers/resolver.rs`

**Status:** Implemented in the local working tree. `resolve_provider_full()` now fails early with actionable missing-key errors for cloud providers while preserving keyless Ollama. Provider routing now uses `Config::is_provider_available()` as the single source of truth, so configured empty strings no longer count as valid keys for direct routing or OpenRouter/OpenCode fallback. Regression tests cover missing OpenAI/Anthropic keys, empty OpenRouter fallback keys, empty OpenRouter free-model routing, empty NVIDIA key interaction with OpenRouter free routing, and Ollama no-key behavior.

---

### 2.6 Config Schema Drift — Duplicate Aliases (Resolved locally)

**File:** `src/config/loader.rs`

**Status:** Implemented in the local working tree. `load_config()` now detects legacy alias keys under `agents.defaults` and `skills`, loads them through existing serde aliases, and writes the config back in the canonical serialized schema. Existing MCP default migration also now persists instead of staying memory-only. Regression tests cover snake_case agent defaults, snake_case skills config, canonical camelCase rewrite, and old `memory` MCP removal.

---

## 3. Low Priority / Polish

### 3.1 Argument Naming Inconsistency (Resolved)

**Files:** `src/tools/arguments.rs`, `src/agent/agent_loop/tool_execution.rs`, `src/agent/agent_loop/run/tool_pipeline.rs`

**Status:** Resolved. Centralized argument alias definitions in `src/tools/arguments.rs` (`PATH_KEYS`, `COMMAND_KEYS`, `OUTPUT_KEYS`, `QUERY_KEYS`, `URL_KEYS`, `SESSION_KEYS`, `TARGET_KEYS`) and implemented recursive `normalize_tool_args()` that translates camelCase and PascalCase tool arguments to snake_case equivalents while preserving native schemas. `ToolExecutionPipeline` applies `normalize_tool_args()` prior to tool invocation, and `format_tool_args()` uses generic fallback formatting with canonical key extractors. Tests cover recursive alias expansion and preserving explicit arguments.

---

### 3.2 Activity File Write Throttling (Resolved locally)

**File:** `src/agent/activity.rs`

**Status:** Implemented in the local working tree. `update_activity()` now coalesces rapid non-terminal status updates with a 200ms process-local throttle while force-writing terminal statuses such as `Idle`, cancellation, error, or failure. Writes remain atomic and use unique temp files before rename. Regression tests cover coalescing and immediate `Idle` writes.

---

### 3.3 HTTP Client Read Timeout Hardening (Resolved locally)

**Files:** `src/core/http.rs`, `src/tools/web.rs`, `src/providers/openai.rs`, `src/providers/anthropic.rs`, `src/channels/whatsapp.rs`, `src/tools/rust_docs.rs`, `src/tools/github.rs`, `src/tools/browser/firefox.rs`, `src/tools/browser/status.rs`, `src/channels/mod.rs`

**Status:** Implemented in the local working tree. Provider clients now set explicit connect, read, and total request timeouts (`15s` connect, `120s` read, `300s` total). Web fetch and multimodal remote-image fetches use shorter bounded phases (`10s` connect, `30s` read, bounded total request timeout), so slow URLs cannot hold the agent indefinitely.

**Progress (v0.0.149):** Standardized disparate HTTP client initializations across `src/channels/whatsapp.rs`, `src/tools/rust_docs.rs`, `src/tools/github.rs`, `src/tools/browser/firefox.rs`, `src/tools/browser/status.rs`, and `src/channels/mod.rs` onto centralized builders `crate::core::http::default_client()` and `custom_client()`. Guaranteed connection (10s), read (30s), and request (60s) timeouts eliminate all instances of untimed `reqwest::Client::new()` calls.

---

### 3.4 `tokio::select!` Bias — Inconsistent (Resolved locally)

**Status:** Implemented in the local working tree. Cancellation and shutdown races now use `biased;` in the CLI execution race, channel wrappers, Telegram/Discord polling and retry waits, email/cron polling waits, file watcher shutdown handling, MCP health/bridge shutdown, and the shared subagent cancellation token. Fairness-sensitive normal work selects were left unchanged.

---

### 3.5 Worktree Cleanup on Panic / Forced Shutdown (Resolved locally)

**Status:** Implemented in the local working tree. `WorktreeGuard` now registers active isolated worktrees in a shutdown cleanup registry, unregisters on normal drop/deactivate, and `shutdown::trigger()` drains the registry before forced process exit. Focused tests cover normal drop unregister/cleanup and forced-shutdown cleanup before guard drop.

---

## 4. Enhancements & New Features

### 4.1 Streaming Tool Output & Progress Events (Resolved)

**Files:** `src/channels/websocket/protocol.rs`, `src/agent/agent_loop/tool_execution.rs`, `src/channels/websocket/tests.rs`

**Status:** Intermediate progress streaming is now supported across WebSocket and messaging channels. Added typed `WsEvent::ToolProgress` to WebSocket wire protocol and wired `send_progress_update()` to automatically emit `tool_progress` events alongside dual-dispatching `activity_notice` (`kind: "progress"`) for complete backward and forward WebUI compatibility. Serialization and live delivery verified via unit tests. Full intermediate tool token streaming remains a future extension for async generators.

---

### 4.2 Per-Session Config & Memory Override (Resolved)

**Status:** Implemented in the local working tree. `apply_session_overrides()` in `src/agent/agent_loop/mod.rs` applies session-level configuration from session metadata (supporting both root keys and nested `config_override` / `config` dictionaries). Overridable fields include `model`, `provider`, `temperature`, `max_tokens`, `max_messages`, `max_tool_iterations`, `caveman_mode`, `tool_timeout_secs`, and `streaming`. Unit tests cover both root-level and nested override mappings.

---

### 4.3 Tool Call Retry with Backoff (Resolved)

**Status:** Implemented in the local working tree. `ToolExecutionPipeline` in `src/agent/agent_loop/run/tool_pipeline.rs` detects transient failures via `is_transient_error()` (rate limits, HTTP 429, HTTP 502/503/504, connection drops, network errors, temporary DNS failures) and retries up to 3 attempts with exponential backoff (starting at 1s, doubling per attempt) before reporting errors to the LLM. Non-transient errors and explicit user cancellations fail immediately without unnecessary delay.

---

### 4.4 Config Live Reload (Resolved)

**Status:** Implemented in the local working tree. Background configuration watcher `ConfigWatcher` in `src/config/watcher.rs` monitors `config.json` via `notify` with debouncing, non-destructive file reads (`load_config_from_path`), and atomic change comparison. In the WebSocket gateway (`src/channels/websocket/mod.rs`), the watcher keeps `state.live_config` synchronized with external updates and broadcasts `config_updated` events to connected WebUI clients. WebSocket `set_config` also immediately broadcasts changes to all other connected clients (`publish_ws_event_except`). In-flight turn execution in `AgentLoop` continues reading fresh disk configuration per-turn, ensuring complete end-to-end consistency without gateway restart. Tests cover reload on file change, redundant write suppression, corrupt JSON resilience, and WebSocket broadcast delivery.

---

### 4.5 Structured Logging to SQLite (Resolved)

**Status:** Implemented in the local working tree. `src/logs/` (`storage.rs`, `subscriber.rs`, `query.rs`, `tui.rs`, and `mod.rs`) provides `SqliteLogLayer` tracing subscriber integration that writes structured log records directly into SQLite (`~/.openz/logs.db`), with indexed `session` and `timestamp` fields and automatic 7-day log retention purging. The `openz logs` command queries and tails `logs.db` directly with structured filtering by level, session, and search patterns. Unit tests in `src/logs/storage.rs::tests::test_sqlite_logging_workflow` verify database record insertion and query filtering.

---

### 4.6 Cross-Platform Host Shell Centralization & Windows Patch Hardening (Resolved)

**Files:** `src/core/process.rs`, `src/tools/filesystem.rs`, `src/tools/compiler_auto_heal.rs`, `src/tools/shell.rs`

**Status:** Implemented in the local working tree. Centralized host shell command spawning into `src/core/process.rs` (`host_shell_command` and `host_tokio_shell_command`), abstracting `cmd.exe /C` on Windows and `sh -c` on Unix alongside repository working directory synchronization (`set_command_cwd`). Replaced fragmented direct shell invocations and fixed a critical bug in `ApplyPatchTool` (`git apply --check`) which previously hardcoded `sh -c` and failed on Windows without bash installed.

---

### 4.7 Configurable WebSocket CORS Origins (Resolved)

**Files:** `src/config/schema.rs`, `src/channels/websocket/auth.rs`

**Status:** Implemented in the local working tree. Added `cors_origins: Vec<String>` to `WebSocketChannelConfig` with serde aliases (`corsOrigins`, `cors_origins`). Parameterized `is_allowed_origin` in `src/channels/websocket/auth.rs` so deployments can permit explicit domains (e.g. desktop web clients, remote dashboards) alongside default local loops (`http://localhost:*`, `http://127.0.0.1:*`, `tauri://localhost`).

---

### 4.8 Identity Heuristics & Dynamic Prompt Budget Scaling (Resolved)

**Files:** `src/config/schema.rs`, `src/agent/agent_loop/build.rs`

**Status:** Implemented in the local working tree. General persona identity detection in prompt building now checks generic creator inquiry patterns (`who created you`, `who made you`, `your creator`, `who programmed you`) and retrieves identity metadata dynamically. Decoupled hardcoded prompt context token budgets via `prompt_budget_limit` in `AgentDefaults`, enabling dynamic scaling proportional to provider model context windows.

---

### 4.9 Model Preferences & Risk Domain Relocation (Resolved)

**Files:** `src/providers/model_prefs.rs`, `src/providers/risk.rs`, `src/providers/mod.rs`, `src/channels/mod.rs`

**Status:** Implemented in the local working tree. Decoupled provider-specific domain logic from the channel layer. Moved `ModelPrefs`, `ModelRef`, and model preference persistence (`load_model_prefs`, `save_model_prefs`, `toggle_favorite_model`, `record_recent_model`) to `src/providers/model_prefs.rs`. Moved `ModelRisk` and `classify_model_risk` to `src/providers/risk.rs`. Exposed clean provider API surfaces with backward-compatible re-exports in `src/channels/mod.rs`.

---

### 4.10 Modularized Logging Architecture (Resolved in v0.0.150)

**Files:** [`src/logs/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/mod.rs), [`src/logs/storage.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/storage.rs), [`src/logs/subscriber.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber.rs), [`src/logs/query.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/query.rs), [`src/logs/tui.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/tui.rs)

**Status:** Decomposed the 1,636-line `src/logs.rs` monolithic file into clean, single-responsibility submodules:
- `storage.rs`: SQLite connection pool, schema migrations, batch log ingestion, and retention cleanup.
- `subscriber.rs`: `SqliteLogLayer` tracing subscriber and live formatters.
- `query.rs`: Structured query builder and log filtering by session, level, target, and time range.
- `tui.rs`: Terminal TUI log viewer and live tail renderer.
- `mod.rs`: Clean facade with 100% backward-compatible re-exports for `crate::logs::*` and `openz::logs::*`.

---

### 4.11 Centralized Secret Scrubbing & Token Masking (Resolved in v0.0.150)

**Files:** [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs), [`src/cli/doctor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs), [`src/logs/subscriber.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber.rs)

**Status:** Consolidated token regex pattern matching and string redaction into `src/core/secrets.rs`. Standardized scrubbing covers Telegram bot tokens, OpenAI `sk-...` keys, Anthropic/API tokens, and partial secret patterns. Both `doctor` diagnostics and tracing logging subscribers now consume unified redaction logic, eliminating regex drift and duplicated masking routines.

---

### 4.12 Cross-Platform Shell Quoting & Persistence Hardening (Resolved in v0.0.150)

**Files:** [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs), [`src/tools/shell.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs), [`src/tools/filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs), [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs)

**Status:** Added `quote_shell_arg` in `src/core/process.rs` providing Windows `cmd.exe` double-quote escaping (`""`) and Unix POSIX `sh` single-quote escaping (`'\''`). Replaced ad-hoc escaping in `src/tools/shell.rs` and `src/tools/filesystem.rs` to fix Windows command-line quoting bugs. Hardened `save_model_prefs_at` with explicit `sync_all` and automatic `.tmp` file cleanup upon write or rename failure.

---

### 4.13 Native Tool Subsystem Taxonomy Documentation (Resolved in v0.0.150)

**Files:** [`src/tools/README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/README.md), [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs)

**Status:** Formalized architectural documentation for the native tool subsystem in `src/tools/README.md`, detailing core domains, execution safety, registration requirements, and lifecycle contracts. Added automated category integrity tests in `src/cli/builder.rs` preserving the 128 registered native tools invariant.

---

### 4.14 God-File Modularization (Resolved in v0.0.151)

**Files:** [`src/agent/agent_loop/run/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/mod.rs), [`src/agent/agent_loop/run/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run/tests.rs), [`src/channels/websocket/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/mod.rs), [`src/channels/websocket/socket.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/socket.rs), [`src/channels/websocket/handlers.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/handlers.rs)

**Status:** Decomposed large monolithic files into clean domain components:
- Extracted ~500 lines of embedded unit tests from `src/agent/agent_loop/run/mod.rs` into `src/agent/agent_loop/run/tests.rs`, reducing `mod.rs` from 1,662 lines to ~1,170 lines.
- Decomposed `src/channels/websocket/mod.rs` from 1,204 lines down to ~380 lines (~70% reduction) by extracting frame processing into `socket.rs` and REST routes into `handlers.rs`.

---

### 4.15 Unified Self-Healing Reflection Engine (Resolved in v0.0.151)

**Files:** [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs), [`src/tools/compiler_auto_heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs), [`src/tools/filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs)

**Status:** Unified duplicate reflection and compile-and-heal loops into `src/core/heal.rs`. Provides centralized validation against shell injection via `validate_compile_command`, robust markdown code fence stripping (`strip_code_fence`), safe `FileBackupGuard` with rollback-on-drop, `GitSnapshot` worktree preservation, and canonical reflection loop runners (`run_compiler_auto_heal` and `run_transactional_heal_edit`). Both `CompilerAutoHealTool` and `ZenflowEditTool` now delegate to this shared engine.

---

### 4.16 Scoped Intent Prioritization & CLI Channel Modularization (Resolved in v0.0.152)

**Files:** [`src/agent/agent_loop/intent.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/intent.rs), [`src/tools/defs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/defs.rs), [`src/channels/cli/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/mod.rs), [`src/channels/cli/device.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/device.rs), [`src/channels/cli/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/commands.rs), [`src/agent/security.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security.rs), [`src/agent/security_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/security_tests.rs)

**Status:**
- Prioritized cron, workflow, and local repo intent checks before live research heuristics in `src/agent/agent_loop/intent.rs`, preventing scheduling commands from being misrouted to web research.
- Assigned pack scoping for `manage_servers` (`core`, `local_exec`) and `workflow_memory` (`core`, `memory`) in `src/tools/defs.rs`.
- Decomposed the 1,548-line `src/channels/cli/mod.rs` god-file into `device.rs` (253 lines) and `commands.rs` (928 lines), reducing `mod.rs` to 431 lines while maintaining full backward compatibility.
- Extracted 580 lines of unit tests from `src/agent/security.rs` into `src/agent/security_tests.rs`, reducing `security.rs` to 922 lines.

---

### 4.17 Subagent Workspace Modularization & Prompt Test Extraction (Resolved in v0.0.153)

**Files:** [`src/tools/subagent/delegate_task.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_task.rs), [`src/tools/subagent/workspace.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/workspace.rs), [`src/tools/subagent/delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs), [`src/agent/agent_loop/build.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs), [`src/agent/agent_loop/build_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build_tests.rs)

**Status:**
- Extracted 846 lines of workspace isolation, worktree lifecycle, disk quota management, git status filtering, and evolution review out of `src/tools/subagent/delegate_task.rs` into dedicated module `src/tools/subagent/workspace.rs`. Reduced `delegate_task.rs` from 1,126 lines to 286 lines (~74.6% line reduction).
- Decoupled `delegate_profile.rs` from borrowing internal helpers from `delegate_task.rs`.
- Extracted 426 lines of embedded unit tests from `src/agent/agent_loop/build.rs` into `src/agent/agent_loop/build_tests.rs`, reducing `build.rs` from 1,527 lines to 1,101 lines.

---

### 4.18 Ratatui Terminal TUI Decomposition & Session Extraction (Resolved in v0.0.154)

**Files:** [`src/channels/ratatui/ui.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/ui.rs), [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs), [`src/channels/ratatui/timeline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/timeline.rs), [`src/channels/ratatui/modals.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals.rs), [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs), [`src/channels/ratatui/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/mod.rs)

**Status:**
- Decomposed monolithic 1,104-line Ratatui TUI renderer into `markdown.rs`, `timeline.rs`, and `modals.rs`, slashing `ui.rs` to 247 lines (~77.6% line reduction).
- Extracted process marker management and session mutation routines from `mod.rs` into `session.rs` with dedicated unit test coverage.
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.19 Subsystem Root De-Bloat & Test Extraction (Resolved in v0.0.155)

**Files:** [`src/tools/memory_extra/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/mod.rs), [`src/tools/memory_extra/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/tests.rs), [`src/orchestrator/runtime.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/runtime.rs), [`src/orchestrator/runtime_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/orchestrator/runtime_tests.rs), [`src/channels/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs), [`src/channels/model_switch.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/model_switch.rs), [`src/channels/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/tests.rs)

**Status:**
- Extracted 1,463 lines of unit tests from `tools/memory_extra/mod.rs` into `tools/memory_extra/tests.rs`, reducing `mod.rs` from 1,496 to 34 lines (~97.7% reduction).
- Extracted 801 lines of unit tests from `orchestrator/runtime.rs` into `orchestrator/runtime_tests.rs`, reducing `runtime.rs` from 1,420 to 620 lines (~56.3% reduction).
- Extracted model switch command parsing, formatting, and smoke tests from `channels/mod.rs` into `channels/model_switch.rs` (320 lines) and channel tests into `channels/tests.rs` (312 lines), reducing `channels/mod.rs` from 1,020 to 396 lines (~61.2% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.20 Tool Registry Subsystem Modularization & Route Test Extraction (Resolved in v0.0.156)

**Files:** [`src/tools/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mod.rs), [`src/tools/registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs), [`src/tools/registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry_tests.rs)

**Status:**
- Extracted `ToolRegistry`, route analysis types, dynamic subagent resolution, and prompt intent routing into [`src/tools/registry.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry.rs).
- Extracted 480 lines of unit tests into dedicated [`src/tools/registry_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/registry_tests.rs).
- Slashed `src/tools/mod.rs` from 1,571 to 269 lines (~82.9% reduction) while preserving 100% backward compatibility via `pub use registry::*;`.
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.21 Subagent Decomposition & Memory Test Extraction (Resolved in v0.0.157)

**Files:** [`src/subagents/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/mod.rs), [`src/subagents/defaults.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/defaults.rs), [`src/subagents/health.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/health.rs), [`src/subagents/interactive.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/interactive.rs), [`src/subagents/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/subagents/tests.rs), [`src/tools/shared_memory/knowledge.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/knowledge.rs), [`src/tools/shared_memory/knowledge_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/knowledge_tests.rs), [`src/tools/shared_memory/auto_capture.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/auto_capture.rs), [`src/tools/shared_memory/auto_capture_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/auto_capture_tests.rs), [`src/agent/skills.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills.rs), [`src/agent/skills_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills_tests.rs)

**Status:**
- Decomposed monolithic 1,088-line `src/subagents/mod.rs` into focused domain submodules (`defaults.rs`, `health.rs`, `interactive.rs`, `tests.rs`) and reduced `mod.rs` to 210 lines (~80.7% reduction).
- Extracted 365 lines of embedded unit tests from `src/tools/shared_memory/knowledge.rs` into `src/tools/shared_memory/knowledge_tests.rs`, reducing `knowledge.rs` from 1,350 to 988 lines (~26.8% reduction).
- Extracted 399 lines of embedded unit tests from `src/tools/shared_memory/auto_capture.rs` into `src/tools/shared_memory/auto_capture_tests.rs`, reducing `auto_capture.rs` from 1,214 to 816 lines (~32.8% reduction).
- Extracted 110 lines of embedded unit tests from `src/agent/skills.rs` into `src/agent/skills_tests.rs`, reducing `skills.rs` from 1,103 to 995 lines.
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.22 Subagent Runner Extraction & Test Suite Decomposition (Resolved in v0.0.158)

**Files:** [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs), [`src/tools/subagent/runner.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/runner.rs), [`src/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session.rs), [`src/session_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/session_tests.rs), [`src/tools/headroom/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/mod.rs), [`src/tools/headroom/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/tests.rs), [`src/tools/self_management/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/mod.rs), [`src/tools/self_management/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/tests.rs)

**Status:**
- Extracted 872 lines of execution runner and workspace isolation logic from `src/tools/subagent/mod.rs` into `src/tools/subagent/runner.rs`, reducing `subagent/mod.rs` from 921 to 60 lines (~93.5% reduction).
- Extracted 290 lines of interleaved test suites (`hash_tests`, `lock_tests`, `delete_tests`, `summary_tests`) from `src/session.rs` into `src/session_tests.rs`, reducing `session.rs` to an uninterrupted production module.
- Extracted 706 lines of embedded tests from `src/tools/headroom/mod.rs` into `src/tools/headroom/tests.rs`, reducing `headroom/mod.rs` from 739 to 33 lines (~95.5% reduction).
- Extracted 687 lines of embedded tests from `src/tools/self_management/mod.rs` into `src/tools/self_management/tests.rs`, reducing `self_management/mod.rs` from 711 to 26 lines (~96.3% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.23 CLI & Engine Test Suite Decomposition (Resolved in v0.0.159)

**Files:** [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs), [`src/cli/builder_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder_tests.rs), [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs), [`src/cli/tools_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools_tests.rs), [`src/agent/agent_loop/loop_control.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/loop_control.rs), [`src/agent/agent_loop/loop_control_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/loop_control_tests.rs), [`src/providers/resolver.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/resolver.rs), [`src/providers/resolver_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/resolver_tests.rs)

**Status:**
- Extracted 578 lines of embedded unit tests from `src/cli/builder.rs` into `src/cli/builder_tests.rs`, reducing `builder.rs` from 615 to 40 lines (~93.5% reduction).
- Extracted 552 lines of embedded unit tests from `src/cli/tools.rs` into `src/cli/tools_tests.rs`, reducing `tools.rs` from 588 to 37 lines (~93.7% reduction).
- Extracted 397 lines of embedded unit tests from `src/agent/agent_loop/loop_control.rs` into `src/agent/agent_loop/loop_control_tests.rs`, reducing `loop_control.rs` from 779 to 385 lines (~50.6% reduction).
- Extracted 446 lines of embedded unit tests from `src/providers/resolver.rs` into `src/providers/resolver_tests.rs`, reducing `resolver.rs` from 750 to 305 lines (~59.3% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.24 Core Engine & Config Test Suite Decomposition (Resolved in v0.0.160)

**Files:** [`src/config/schema.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs), [`src/config/schema_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema_tests.rs), [`src/config/loader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs), [`src/config/loader_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader_tests.rs), [`src/sop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/sop/mod.rs), [`src/sop/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/sop/tests.rs), [`src/tools/filesystem.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs), [`src/tools/filesystem_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem_tests.rs), [`src/agent/agent_loop/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/mod.rs), [`src/agent/agent_loop/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/tests.rs)

**Status:**
- Extracted 380 lines of embedded unit tests from `src/config/schema.rs` into `src/config/schema_tests.rs`, reducing `schema.rs` from 1,496 to 1,115 lines (~25.5% reduction).
- Extracted 358 lines of embedded unit tests from `src/config/loader.rs` into `src/config/loader_tests.rs`, reducing `loader.rs` from 928 to 572 lines (~38.4% reduction).
- Extracted 265 lines of embedded unit tests from `src/sop/mod.rs` into `src/sop/tests.rs`, reducing `sop/mod.rs` from 677 to 412 lines (~39.1% reduction).
- Extracted 247 lines of embedded unit tests from `src/tools/filesystem.rs` into `src/tools/filesystem_tests.rs`, reducing `filesystem.rs` from 784 to 539 lines (~31.3% reduction).
- Extracted 236 lines of embedded unit tests from `src/agent/agent_loop/mod.rs` into `src/agent/agent_loop/tests.rs` with dedicated test synchronization locks, reducing `agent_loop/mod.rs` from 922 to 686 lines (~25.6% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.25 Subsystem & Search Test Suite Decomposition (Resolved in v0.0.161)

**Files:** [`src/agent/activity.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/activity.rs), [`src/agent/activity_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/activity_tests.rs), [`src/cron/scheduler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/scheduler.rs), [`src/cron/scheduler_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/scheduler_tests.rs), [`src/tools/resource_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy.rs), [`src/tools/resource_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/resource_policy_tests.rs), [`src/tools/searchxyz/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web.rs), [`src/tools/searchxyz/web_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/web_tests.rs), [`src/tools/web_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search.rs), [`src/tools/web_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_search_tests.rs)

**Status:**
- Extracted 229 lines of embedded unit tests from `src/agent/activity.rs` into `src/agent/activity_tests.rs`, reducing `activity.rs` from 707 to 480 lines (~32.1% reduction).
- Extracted 222 lines of embedded unit tests from `src/cron/scheduler.rs` into `src/cron/scheduler_tests.rs`, reducing `scheduler.rs` from 624 to 404 lines (~35.3% reduction).
- Extracted 199 lines of embedded unit tests from `src/tools/resource_policy.rs` into `src/tools/resource_policy_tests.rs`, reducing `resource_policy.rs` from 407 to 211 lines (~48.2% reduction).
- Extracted 229 lines of embedded unit tests from `src/tools/searchxyz/web.rs` into `src/tools/searchxyz/web_tests.rs`, reducing `web.rs` from 1,443 to 1,216 lines (~15.7% reduction).
- Extracted 195 lines of embedded unit tests from `src/tools/web_search.rs` into `src/tools/web_search_tests.rs`, reducing `web_search.rs` from 968 to 774 lines (~20.0% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.26 Interactive, Workflow & Web Tool Test Suite Decomposition (Resolved in v0.0.162)

**Files:** [`src/tools/orchestrator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator.rs), [`src/tools/orchestrator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/orchestrator_tests.rs), [`src/grounding.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/grounding.rs), [`src/grounding_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/grounding_tests.rs), [`src/channels/ratatui/app.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/app.rs), [`src/channels/ratatui/app_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/app_tests.rs), [`src/tools/web.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web.rs), [`src/tools/web_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/web_tests.rs), [`src/tools/cron.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron.rs), [`src/tools/cron_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cron_tests.rs)

**Status:**
- Extracted 175 lines of embedded unit tests from `src/tools/orchestrator.rs` into `src/tools/orchestrator_tests.rs`, cleaning up mid-file test declarations and reducing `orchestrator.rs` from 574 to 401 lines (~30.1% reduction).
- Extracted 171 lines of embedded unit tests from `src/grounding.rs` into `src/grounding_tests.rs`, reducing `grounding.rs` from 445 to 274 lines (~38.4% reduction).
- Extracted 165 lines of embedded unit tests from `src/channels/ratatui/app.rs` into `src/channels/ratatui/app_tests.rs`, reducing `app.rs` from 586 to 421 lines (~28.2% reduction).
- Extracted 161 lines of embedded unit tests from `src/tools/web.rs` into `src/tools/web_tests.rs`, reducing `web.rs` from 942 to 781 lines (~17.1% reduction).
- Extracted 153 lines of embedded unit tests from `src/tools/cron.rs` into `src/tools/cron_tests.rs`, reducing `cron.rs` from 600 to 447 lines (~25.5% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.27 System Core & Inspection Test Suite Decomposition (Resolved in v0.0.163)

**Files:** [`src/core/inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/inventory.rs), [`src/core/inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/inventory_tests.rs), [`src/agent/agent_loop/transcript.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript.rs), [`src/agent/agent_loop/transcript_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/transcript_tests.rs), [`src/tools/db_inspector.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector.rs), [`src/tools/db_inspector_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/db_inspector_tests.rs), [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs), [`src/providers/model_prefs_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs_tests.rs), [`src/config/watcher.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/watcher.rs), [`src/config/watcher_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/watcher_tests.rs)

**Status:**
- Extracted 146 lines of embedded unit tests from `src/core/inventory.rs` into `src/core/inventory_tests.rs`, reducing `inventory.rs` from 759 to 614 lines (~19.1% reduction).
- Extracted 140 lines of embedded unit tests from `src/agent/agent_loop/transcript.rs` into `src/agent/agent_loop/transcript_tests.rs`, reducing `transcript.rs` from 321 to 183 lines (~43.0% reduction).
- Extracted 137 lines of embedded unit tests from `src/tools/db_inspector.rs` into `src/tools/db_inspector_tests.rs`, reducing `db_inspector.rs` from 401 to 265 lines (~33.9% reduction).
- Extracted 133 lines of embedded unit tests from `src/providers/model_prefs.rs` into `src/providers/model_prefs_tests.rs`, reducing `model_prefs.rs` from 262 to 130 lines (~50.4% reduction).
- Extracted 128 lines of embedded unit tests from `src/config/watcher.rs` into `src/config/watcher_tests.rs`, reducing `watcher.rs` from 294 to 167 lines (~43.2% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

### 4.28 Media, Style, Cron & Task Manager Test Suite Decomposition (Resolved in v0.0.164)

**Files:** [`src/tools/svg_animator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator.rs), [`src/tools/svg_animator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/svg_animator_tests.rs), [`src/tools/openmedia/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod.rs), [`src/tools/openmedia/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod_tests.rs), [`src/agent/style/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/style/mod.rs), [`src/agent/style/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/style/mod_tests.rs), [`src/cron/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/mod.rs), [`src/cron/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cron/mod_tests.rs), [`src/tools/task_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager.rs), [`src/tools/task_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/task_manager_tests.rs), [`.cargo/config.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/.cargo/config.toml)

**Status:**
- Extracted 138 lines of embedded unit tests from `src/tools/svg_animator.rs` into `src/tools/svg_animator_tests.rs`, reducing `svg_animator.rs` from 1,158 to 1,020 lines (~11.9% reduction).
- Extracted 136 lines of embedded unit tests from `src/tools/openmedia/mod.rs` into `src/tools/openmedia/mod_tests.rs`, reducing `openmedia/mod.rs` from 899 to 763 lines (~15.1% reduction).
- Extracted 134 lines of embedded unit tests from `src/agent/style/mod.rs` into `src/agent/style/mod_tests.rs`, reducing `style/mod.rs` from 835 to 701 lines (~16.1% reduction).
- Extracted 123 lines of embedded unit tests from `src/cron/mod.rs` into `src/cron/mod_tests.rs`, reducing `cron/mod.rs` from 456 to 333 lines (~27.0% reduction).
- Extracted 119 lines of embedded unit tests from `src/tools/task_manager.rs` into `src/tools/task_manager_tests.rs`, added concurrency test lock `TEST_LOCK`, reducing `task_manager.rs` from 467 to 348 lines (~25.5% reduction).
- Added `.cargo/config.toml` capping compilation jobs to 2, protecting developer hardware from memory spikes and swap thrashing.
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.29 Devices, Secrets, Providers & Source Ledger Test Suite Decomposition (Resolved in v0.0.165)

**Files:** [`src/tools/device_inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory.rs), [`src/tools/device_inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/device_inventory_tests.rs), [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs), [`src/core/secrets_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets_tests.rs), [`src/providers/openai.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/openai.rs), [`src/providers/openai_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/openai_tests.rs), [`src/providers/mock.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mock.rs), [`src/providers/mock_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mock_tests.rs), [`src/agent/source_ledger.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/source_ledger.rs), [`src/agent/source_ledger_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/source_ledger_tests.rs)

**Status:**
- Extracted 118 lines of embedded unit tests from `src/tools/device_inventory.rs` into `src/tools/device_inventory_tests.rs`, reducing `device_inventory.rs` from 575 to 457 lines (~20.5% reduction).
- Extracted 115 lines of embedded unit tests from `src/core/secrets.rs` into `src/core/secrets_tests.rs`, reducing `secrets.rs` from 495 to 380 lines (~23.2% reduction).
- Extracted 108 lines of embedded unit tests from `src/providers/openai.rs` into `src/providers/openai_tests.rs`, eliminated mid-file test placement before `is_reasoning_model`, reducing `openai.rs` from 888 to 780 lines (~12.2% reduction).
- Extracted 100 lines of embedded unit tests from `src/providers/mock.rs` into `src/providers/mock_tests.rs`, reducing `mock.rs` from 289 to 189 lines (~34.6% reduction).
- Extracted 94 lines of embedded unit tests from `src/agent/source_ledger.rs` into `src/agent/source_ledger_tests.rs`, reducing `source_ledger.rs` from 305 to 211 lines (~30.8% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.30 Code Outline, MCP Manager, Shell, Semantic Search & Git Manager Test Suite Decomposition (Resolved in v0.0.166)

**Files:** [`src/tools/outline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline.rs), [`src/tools/outline_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline_tests.rs), [`src/tools/mcp_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager.rs), [`src/tools/mcp_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_manager_tests.rs), [`src/tools/shell.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs), [`src/tools/shell_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell_tests.rs), [`src/tools/semantic_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search.rs), [`src/tools/semantic_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search_tests.rs), [`src/tools/git_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager.rs), [`src/tools/git_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/git_manager_tests.rs)

**Status:**
- Extracted 93 lines of embedded unit tests from `src/tools/outline.rs` into `src/tools/outline_tests.rs`, reducing `outline.rs` from 382 to 289 lines (~24.3% reduction).
- Extracted 89 lines of embedded unit tests from `src/tools/mcp_manager.rs` into `src/tools/mcp_manager_tests.rs`, reducing `mcp_manager.rs` from 277 to 188 lines (~32.1% reduction).
- Extracted 87 lines of embedded unit tests from `src/tools/shell.rs` into `src/tools/shell_tests.rs`, reducing `shell.rs` from 988 to 901 lines (~8.8% reduction).
- Extracted 85 lines of embedded unit tests from `src/tools/semantic_search.rs` into `src/tools/semantic_search_tests.rs`, reducing `semantic_search.rs` from 430 to 345 lines (~19.8% reduction).
- Extracted 85 lines of embedded unit tests from `src/tools/git_manager.rs` into `src/tools/git_manager_tests.rs`, reducing `git_manager.rs` from 208 to 123 lines (~40.9% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.31 CLI Doctor, Render, Research Policy, Workflows & Video Test Suite Decomposition (Resolved in v0.0.167)

**Files:** [`src/cli/doctor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs), [`src/cli/doctor_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor_tests.rs), [`src/channels/cli/render.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render.rs), [`src/channels/cli/render_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/render_tests.rs), [`src/agent/agent_loop/research_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/research_policy.rs), [`src/agent/agent_loop/research_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/research_policy_tests.rs), [`src/tools/shared_memory/workflows.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/workflows.rs), [`src/tools/shared_memory/workflows_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/workflows_tests.rs), [`src/tools/video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video.rs), [`src/tools/video_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/video_tests.rs)

**Status:**
- Extracted 85 lines of embedded unit tests from `src/cli/doctor.rs` into `src/cli/doctor_tests.rs`, reducing `doctor.rs` from 602 to 516 lines (~14.3% reduction).
- Extracted 85 lines of embedded unit tests from `src/channels/cli/render.rs` into `src/channels/cli/render_tests.rs`, reducing `render.rs` from 1,209 to 1,123 lines (~7.1% reduction).
- Extracted 85 lines of embedded unit tests from `src/agent/agent_loop/research_policy.rs` into `src/agent/agent_loop/research_policy_tests.rs`, reducing `research_policy.rs` from 333 to 247 lines (~25.8% reduction).
- Extracted 82 lines of embedded unit tests from `src/tools/shared_memory/workflows.rs` into `src/tools/shared_memory/workflows_tests.rs`, reducing `workflows.rs` from 462 to 379 lines (~18.0% reduction).
- Extracted 80 lines of embedded unit tests from `src/tools/video.rs` into `src/tools/video_tests.rs`, reducing `video.rs` from 210 to 129 lines (~38.6% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.32 Get Logs, Manage Whitelist, Events, Heal & Doc Reader Test Suite Decomposition (Resolved in v0.0.168)

**Files:** [`src/tools/get_logs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs.rs), [`src/tools/get_logs_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/get_logs_tests.rs), [`src/tools/manage_whitelist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist.rs), [`src/tools/manage_whitelist_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/manage_whitelist_tests.rs), [`src/agent/events.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/events.rs), [`src/agent/events_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/events_tests.rs), [`src/core/heal.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal.rs), [`src/core/heal_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/heal_tests.rs), [`src/tools/doc_reader.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader.rs), [`src/tools/doc_reader_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/doc_reader_tests.rs)

**Status:**
- Extracted 80 lines of embedded unit tests from `src/tools/get_logs.rs` into `src/tools/get_logs_tests.rs`, reducing `get_logs.rs` from 239 to 159 lines (~33.5% reduction).
- Extracted 77 lines of embedded unit tests from `src/tools/manage_whitelist.rs` into `src/tools/manage_whitelist_tests.rs`, reducing `manage_whitelist.rs` from 230 to 153 lines (~33.5% reduction).
- Extracted 76 lines of embedded unit tests from `src/agent/events.rs` into `src/agent/events_tests.rs`, reducing `events.rs` from 207 to 131 lines (~36.7% reduction).
- Extracted 74 lines of embedded unit tests from `src/core/heal.rs` into `src/core/heal_tests.rs`, reducing `heal.rs` from 468 to 394 lines (~15.8% reduction).
- Extracted 73 lines of embedded unit tests from `src/tools/doc_reader.rs` into `src/tools/doc_reader_tests.rs`, reducing `doc_reader.rs` from 405 to 332 lines (~18.0% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.33 WhatsApp, Notifications, CLI Input, Ratatui Session & Template Compiler Test Suite Decomposition (Resolved in v0.0.169)

**Files:** [`src/channels/whatsapp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs), [`src/channels/whatsapp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp_tests.rs), [`src/channels/notifications.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/notifications.rs), [`src/channels/notifications_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/notifications_tests.rs), [`src/channels/cli/input.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/input.rs), [`src/channels/cli/input_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/input_tests.rs), [`src/channels/ratatui/session.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session.rs), [`src/channels/ratatui/session_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/session_tests.rs), [`src/tools/template_compiler.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler.rs), [`src/tools/template_compiler_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/template_compiler_tests.rs)

**Status:**
- Extracted 73 lines of embedded unit tests from `src/channels/whatsapp.rs` into `src/channels/whatsapp_tests.rs`, reducing `whatsapp.rs` from 444 to 371 lines (~16.4% reduction).
- Extracted 73 lines of embedded unit tests from `src/channels/notifications.rs` into `src/channels/notifications_tests.rs`, reducing `notifications.rs` from 324 to 251 lines (~22.5% reduction).
- Extracted 65 lines of embedded unit tests from `src/channels/cli/input.rs` into `src/channels/cli/input_tests.rs`, reducing `input.rs` from 827 to 761 lines (~8.0% reduction).
- Extracted 62 lines of embedded unit tests from `src/channels/ratatui/session.rs` into `src/channels/ratatui/session_tests.rs`, reducing `session.rs` from 217 to 155 lines (~28.6% reduction).
- Extracted 62 lines of embedded unit tests from `src/tools/template_compiler.rs` into `src/tools/template_compiler_tests.rs`, reducing `template_compiler.rs` from 278 to 216 lines (~22.3% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.34 AST Grep, GitHub Provider, Arguments, Grep & Image Generator Test Suite Decomposition (Resolved in v0.0.170)

**Files:** [`src/tools/ast_grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep.rs), [`src/tools/ast_grep_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/ast_grep_tests.rs), [`src/tools/github.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github.rs), [`src/tools/github_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_tests.rs), [`src/tools/arguments.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments.rs), [`src/tools/arguments_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/arguments_tests.rs), [`src/tools/grep.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep.rs), [`src/tools/grep_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/grep_tests.rs), [`src/tools/image_generator.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator.rs), [`src/tools/image_generator_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/image_generator_tests.rs)

**Status:**
- Extracted 60 lines of embedded unit tests from `src/tools/ast_grep.rs` into `src/tools/ast_grep_tests.rs`, reducing `ast_grep.rs` from 445 to 385 lines (~13.5% reduction).
- Extracted 56 lines of embedded unit tests from `src/tools/github.rs` into `src/tools/github_tests.rs`, reducing `github.rs` from 519 to 463 lines (~10.8% reduction).
- Extracted 55 lines of embedded unit tests from `src/tools/arguments.rs` into `src/tools/arguments_tests.rs`, reducing `arguments.rs` from 182 to 127 lines (~30.2% reduction).
- Extracted 50 lines of embedded unit tests from `src/tools/grep.rs` into `src/tools/grep_tests.rs`, reducing `grep.rs` from 344 to 294 lines (~14.5% reduction).
- Extracted 53 lines of embedded unit tests from `src/tools/image_generator.rs` into `src/tools/image_generator_tests.rs`, reducing `image_generator.rs` from 649 to 605 lines (~6.8% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.35 Complete Browser Subsystem Test Suite Decomposition (Resolved in v0.0.171)

**Files:** [`src/tools/browser/status.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status.rs), [`src/tools/browser/status_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status_tests.rs), [`src/tools/browser/gsd.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd.rs), [`src/tools/browser/gsd_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/gsd_tests.rs), [`src/tools/browser/firefox.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox.rs), [`src/tools/browser/firefox_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox_tests.rs), [`src/tools/browser/broker.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/broker.rs), [`src/tools/browser/broker_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/broker_tests.rs), [`src/tools/browser/common.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common.rs), [`src/tools/browser/common_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common_tests.rs), [`src/tools/browser/obscura.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura.rs), [`src/tools/browser/obscura_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/obscura_tests.rs)

**Status:**
- Extracted 71 lines of embedded unit tests from `src/tools/browser/status.rs` into `src/tools/browser/status_tests.rs`, reducing `status.rs` from 391 to 320 lines (~18.2% reduction).
- Extracted 67 lines of embedded unit tests from `src/tools/browser/gsd.rs` into `src/tools/browser/gsd_tests.rs`, reducing `gsd.rs` from 361 to 294 lines (~18.6% reduction).
- Extracted 59 lines of embedded unit tests from `src/tools/browser/firefox.rs` into `src/tools/browser/firefox_tests.rs`, reducing `firefox.rs` from 482 to 423 lines (~12.2% reduction).
- Extracted 36 lines of embedded unit tests from `src/tools/browser/broker.rs` into `src/tools/browser/broker_tests.rs`, reducing `broker.rs` from 243 to 207 lines (~14.8% reduction).
- Extracted 13 lines of embedded unit tests from `src/tools/browser/common.rs` into `src/tools/browser/common_tests.rs`, reducing `common.rs` from 195 to 182 lines (~6.7% reduction).
- Extracted 10 lines of embedded unit tests from `src/tools/browser/obscura.rs` into `src/tools/browser/obscura_tests.rs`, reducing `obscura.rs` from 338 to 328 lines (~3.0% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.36 Channels & Core Configuration Test Suite Decomposition (Resolved in v0.0.172)

**Files:** [`src/config/path_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/path_policy.rs), [`src/config/path_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/path_policy_tests.rs), [`src/channels/discord.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord.rs), [`src/channels/discord_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord_tests.rs), [`src/channels/telegram/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/commands.rs), [`src/channels/telegram/commands_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/commands_tests.rs), [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs), [`src/channels/ratatui/markdown_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown_tests.rs), [`src/config/provider_catalog.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/provider_catalog.rs), [`src/config/provider_catalog_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/provider_catalog_tests.rs)

**Status:**
- Extracted 63 lines of embedded unit tests from `src/config/path_policy.rs` into `src/config/path_policy_tests.rs`, reducing `path_policy.rs` from 252 to 189 lines (~25.0% reduction).
- Extracted 45 lines of embedded unit tests from `src/channels/discord.rs` into `src/channels/discord_tests.rs`, reducing `discord.rs` from 450 to 405 lines (~10.0% reduction).
- Extracted 45 lines of embedded unit tests from `src/channels/telegram/commands.rs` into `src/channels/telegram/commands_tests.rs`, reducing `commands.rs` from 521 to 476 lines (~8.6% reduction).
- Extracted 38 lines of embedded unit tests from `src/channels/ratatui/markdown.rs` into `src/channels/ratatui/markdown_tests.rs`, reducing `markdown.rs` from 255 to 217 lines (~14.9% reduction).
- Extracted 34 lines of embedded unit tests from `src/config/provider_catalog.rs` into `src/config/provider_catalog_tests.rs`, reducing `provider_catalog.rs` from 334 to 300 lines (~10.2% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.37 Core Execution & Integration Tools Test Suite Decomposition (Resolved in v0.0.173)

**Files:** [`src/tools/mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp.rs), [`src/tools/mcp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mcp_tests.rs), [`src/tools/clipboard.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard.rs), [`src/tools/clipboard_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/clipboard_tests.rs), [`src/tools/cargo_manager.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager.rs), [`src/tools/cargo_manager_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/cargo_manager_tests.rs), [`src/tools/crawl.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl.rs), [`src/tools/crawl_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/crawl_tests.rs), [`src/tools/open.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open.rs), [`src/tools/open_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/open_tests.rs)

**Status:**
- Extracted 65 lines of embedded unit tests from `src/tools/mcp.rs` into `src/tools/mcp_tests.rs`, reducing `mcp.rs` from 990 to 925 lines (~6.6% reduction).
- Extracted 47 lines of embedded unit tests from `src/tools/clipboard.rs` into `src/tools/clipboard_tests.rs`, reducing `clipboard.rs` from 121 to 74 lines (~38.8% reduction).
- Extracted 40 lines of embedded unit tests from `src/tools/cargo_manager.rs` into `src/tools/cargo_manager_tests.rs`, reducing `cargo_manager.rs` from 354 to 314 lines (~11.3% reduction).
- Extracted 38 lines of embedded unit tests from `src/tools/crawl.rs` into `src/tools/crawl_tests.rs`, reducing `crawl.rs` from 312 to 274 lines (~12.2% reduction).
- Extracted 27 lines of embedded unit tests from `src/tools/open.rs` into `src/tools/open_tests.rs`, reducing `open.rs` from 153 to 126 lines (~17.6% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

### 4.38 Developer Utilities & System Inspection Test Suite Decomposition (Resolved in v0.0.174)

**Files:** [`src/tools/js_format.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format.rs), [`src/tools/js_format_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/js_format_tests.rs), [`src/tools/mermaid.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid.rs), [`src/tools/mermaid_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mermaid_tests.rs), [`src/tools/network.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network.rs), [`src/tools/network_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/network_tests.rs), [`src/tools/system_info.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info.rs), [`src/tools/system_info_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/system_info_tests.rs), [`src/tools/remote.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote.rs), [`src/tools/remote_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/remote_tests.rs)

**Status:**
- Extracted 32 lines of embedded unit tests from `src/tools/js_format.rs` into `src/tools/js_format_tests.rs`, reducing `js_format.rs` from 113 to 81 lines (~28.3% reduction).
- Extracted 28 lines of embedded unit tests from `src/tools/mermaid.rs` into `src/tools/mermaid_tests.rs`, reducing `mermaid.rs` from 92 to 64 lines (~30.4% reduction).
- Extracted 18 lines of embedded unit tests from `src/tools/network.rs` into `src/tools/network_tests.rs`, reducing `network.rs` from 157 to 139 lines (~11.5% reduction).
- Extracted 14 lines of embedded unit tests from `src/tools/system_info.rs` into `src/tools/system_info_tests.rs`, reducing `system_info.rs` from 115 to 101 lines (~12.2% reduction).
- Extracted 11 lines of embedded unit tests from `src/tools/remote.rs` into `src/tools/remote_tests.rs`, reducing `remote.rs` from 95 to 84 lines (~11.6% reduction).
- Maintained exact 260 registered native tools invariant and 0 clippy warnings.

---

## 5. Testing Gaps

### 5.1 Integration Tests (In Progress / Partially Resolved)

**Status (v0.0.147):** Migrated standalone `tests/agent_loop.rs` to in-crate module `src/agent/agent_loop/integration_tests.rs` behind `#[cfg(test)]` (eliminating duplicate binary linking against all ~55 crates while preserving all end-to-end integration tests with a mock provider):
- `test_agent_loop_tool_execution_pipeline`: Validates the complete multi-turn flow (user input → LLM tool call request → tool execution pipeline → tool output formatting → final assistant answer).
- `test_agent_loop_session_overrides_pipeline`: Validates per-session configuration overrides loaded from disk metadata through the execution loop.
- `test_tool_retry_on_transient_error`: Validates `ToolExecutionPipeline` transient error retry with exponential backoff on HTTP 429 rate limit.

Remaining integration test targets:
- MCP stdio/gRPC handshake sequence
- Channel startup/shutdown sequence tests

---

### 5.2 Pre-Existing Test Failures

Status: **Updated (v0.0.138):** a full sequential lib run passes: `cargo test -p openz --lib -- --test-threads=1` = 735 passed, 0 failed. Historical note: five stale tests had silently failed after b787471/ff11b0b reworded prompts and changed subagent metadata semantics — all repaired in v0.0.137/v0.0.138 (metadata `spawns_process`, deny-shell wrapper policy, step-prompt wording, orchestrated-worker scope, version surfaces). Known limitation: with default parallel test threads, ~7 tests fail from cross-test interference (shared env vars, global sender registries, source-scanning tests); run the suite sequentially. Fixing parallel isolation is optional follow-up work.

**Recommended:** Keep this section as a reminder to run the full suite before releases; no immediate fix is needed.

---

### 5.3 No Property-Based Testing

No use of `proptest` or `quickcheck` for:
- Session hash chain invariants (any sequence of messages produces a valid chain)
- Tool argument parsing (any valid JSON schema is accepted)
- Provider key resolution (any config state resolves without panic)

---

## Summary

| Priority | Issues | Lines affected | Risk |
|---|---|---|---|
| **P1** | 1.1 Subagent duplication | ~1700 lines | Bug propagation, slow dev |
| **P1** | 1.2-1.5 Match bloat, unwraps, monolith, provider config | ~500 lines across 5 files | Runtime panics, new provider friction |
| **P2** | 2.1-2.6 Stale errors, locking, notifications, router caching, API key diagnostics, config drift | ~600 lines across 8 files | User confusion, silent failures |
| **P3** | 3.1-3.5 Naming, activity I/O, HTTP timeouts, select bias, cleanup | ~200 lines | Tech debt, marginal reliability |
| **Enhancements** | 4.1-4.33 Streaming, per-session config, retry, live reload, SQLite logs, shell centralization, CORS, provider domain decoupling, modular logs, unified secrets, shell quoting, tool taxonomy, god-file modularization, self-healing reflection, intent prioritization, workspace lifecycle, Ratatui decomposition, test extraction, subagent decomposition, subagent runner modularization, test suite decomposition | — | Feature gap |
| **Testing** | 5.1, 5.3 Integration tests and property tests | — | Coverage gap |

