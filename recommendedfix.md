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

**Files:** `src/tools/web.rs`, `src/providers/openai.rs`, `src/providers/anthropic.rs`

**Status:** Implemented in the local working tree. Provider clients now set explicit connect, read, and total request timeouts (`15s` connect, `120s` read, `300s` total). Web fetch and multimodal remote-image fetches use shorter bounded phases (`10s` connect, `30s` read, bounded total request timeout), so slow URLs cannot hold the agent indefinitely.

---

### 3.4 `tokio::select!` Bias — Inconsistent (Resolved locally)

**Status:** Implemented in the local working tree. Cancellation and shutdown races now use `biased;` in the CLI execution race, channel wrappers, Telegram/Discord polling and retry waits, email/cron polling waits, file watcher shutdown handling, MCP health/bridge shutdown, and the shared subagent cancellation token. Fairness-sensitive normal work selects were left unchanged.

---

### 3.5 Worktree Cleanup on Panic / Forced Shutdown (Resolved locally)

**Status:** Implemented in the local working tree. `WorktreeGuard` now registers active isolated worktrees in a shutdown cleanup registry, unregisters on normal drop/deactivate, and `shutdown::trigger()` drains the registry before forced process exit. Focused tests cover normal drop unregister/cleanup and forced-shutdown cleanup before guard drop.

---

## 4. Enhancements & New Features

### 4.1 Streaming Tool Output & Progress Events (Partially Resolved)

**Files:** `src/channels/websocket/protocol.rs`, `src/agent/agent_loop/tool_execution.rs`, `src/channels/websocket/tests.rs`

**Status:** Intermediate progress streaming is now supported across WebSocket and messaging channels. Added typed `WsEvent::ToolProgress` to WebSocket wire protocol and wired `send_progress_update()` to automatically emit `tool_progress` events to connected WebUI clients with chat and turn correlation alongside external channel progress messages. Serialization verified via unit tests. Full intermediate tool token streaming remains a future extension for async generators.

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

**Status:** Implemented in the local working tree. `src/logs.rs` provides `SqliteLogLayer` tracing subscriber integration that writes structured log records directly into SQLite (`~/.openz/logs.db`), with indexed `session` and `timestamp` fields and automatic 7-day log retention purging. The `openz logs` command queries and tails `logs.db` directly with structured filtering by level, session, and search patterns. Unit tests in `src/logs.rs::tests::test_sqlite_logging_workflow` verify database record insertion and query filtering.

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
| **Enhancements** | 4.1-4.5 Streaming, per-session config, retry, live reload, SQLite logs | — | Feature gap |
| **Testing** | 5.1, 5.3 Integration tests and property tests | — | Coverage gap |
