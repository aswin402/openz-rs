# OpenZ — Recommended Fixes & Improvements

> Audit based on full codebase analysis at `v0.0.50`.
> Covers architectural debt, correctness bugs, test gaps, and enhancement opportunities.

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

### 1.4 `.unwrap()` / `.expect()` Panic Risk (P1)

**Problem:** ~150+ calls across the codebase that will panic on error. High-risk patterns:

| Pattern | Count | Risk |
|---|---|---|
| `lock().unwrap()` on poisoned Mutex | ~25 | If a thread panics while holding the lock, all subsequent accesses panic |
| `serde_json::to_string_pretty(...).unwrap()` | ~15 | Panics on non-serializable values (e.g., circular refs, NaN floats) |
| `Receiver::borrow()` on watch channel | ~8 | Panics if sender is dropped while receiver holds reference |
| `String::from_utf8(...).unwrap()` | ~5 | Panics on non-UTF-8 output from subprocesses |
| `PathBuf::to_str().unwrap()` | ~10 | Panics on non-Unicode paths |

**Example (real crash path):** `src/tools/subagent/delegate_task.rs` uses `lock().unwrap()` inside `WorktreeGuard::drop()`. If a subagent panics during workspace operations, the poisoned lock causes a double-panic on cleanup.

Progress now done: provider message sanitation no longer unwraps optional tool names in `src/providers/openai.rs` or `src/providers/anthropic.rs`, multimodal markdown capture parsing in `src/providers/mod.rs` now skips malformed captures instead of unwrapping, spinner/output mutexes in `src/agent/style/spinner.rs` recover poisoned locks instead of panicking, `src/main.rs` no longer panics when log-file fallback or Unix signal registration fails, subagent worktree create/remove paths in `src/tools/subagent/delegate_task.rs` now pass native `Path` arguments instead of unwrapping UTF-8 strings, and `src/tools/docs_mcp.rs` now resolves `docs.db` through the runtime data path helper instead of panicking when the home directory is unavailable, and `src/tools/openmedia/mod.rs` now returns initialization errors from `get_server()` instead of panicking on OpenMedia server startup or OnceLock races, and subagent evolution-review fenced JSON cleanup in `src/tools/subagent/delegate_task.rs` no longer unwraps known prefixes, and graph-memory branch/database initialization in `src/tools/graph_memory/branch.rs` and `src/tools/graph_memory/db.rs` now reports errors instead of panicking on missing branch IDs or SQLite fallback failures, and `src/tools/memory_extra/facts.rs` now skips invalid regex/capture cases instead of unwrapping during fact extraction, and `src/tools/memory_extra/codebase.rs` now returns regex construction errors and skips missing call captures instead of panicking during codebase indexing.

**Recommended fix:**
- Use `?` operator or `.unwrap_or_else(|e| ...)` for all lock acquisitions
- Replace `to_string_pretty` unwraps with `.unwrap_or_else(|_| "{}".to_string())`
- Use `.to_string_lossy()` instead of `.to_str().unwrap()`
- Audit all `watch::Receiver::borrow()` usage
- Continue removing runtime `unwrap()` / `expect()` sites outside tests, especially in shutdown, spinner/output locks, graph-memory branch args, docs path resolution, and parser capture handling.

---

### 1.5 `execute_approved_tool()` Pipeline Refactor (Resolved locally)

**File:** `src/agent/agent_loop/run.rs`

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

### 3.1 Argument Naming Inconsistency

**File:** Various tool files

**Problem:** Tool call arguments have no single naming convention:
- Some tools use `serde` rename (e.g., `command_line` → `CommandLine`)
- Others use direct field names
- Mix of `snake_case`, `camelCase`

The `format_tool_args()` function in `run.rs` has ~20 explicit mappings to handle this. Every new tool requires a new entry here.

**Recommended fix:** Standardize all tool argument JSON to `snake_case` and remove `format_tool_args` overrides.

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

### 4.1 Streaming Tool Output

**Current behavior:** Tool calls are synchronous — the LLM waits for the entire tool output before continuing. For long-running tools (crawl, video generation), the user sees a spinner for minutes.

**Enhancement:** Allow tools to return streaming intermediate results. The agent loop would display partial output to the user while the tool is still running, and feed intermediate results back to the LLM for early decision-making.

**Complexity:** High — requires changes to `Tool` trait, `execute_approved_tool()`, and message transcript format.

---

### 4.2 Per-Session Memory Override

**Current behavior:** All settings are global via `~/.openz/config.json`. Different conversations (Telegram vs CLI) share the same model, timeout, and provider settings.

**Enhancement:** Allow per-session config overrides stored in session metadata. A CLI session could use `claude-3-5-sonnet` with 600s timeout, while a Telegram session uses `gpt-4o-mini` with 120s timeout.

**Complexity:** Low — session metadata already has a `serde_json::Map` field. Add a merge step in `build.rs`.

---

### 4.3 Tool Call Retry with Backoff

**Current behavior:** When a tool call times out, the error is returned to the LLM, which may or may not retry. Network flakiness (DNS failure, connection reset) causes immediate failure.

**Enhancement:** Add automatic retry with exponential backoff for transient errors (network timeouts, HTTP 429/503, MCP server restarts). Only report to LLM after all retries are exhausted.

**Complexity:** Medium — needs error classification (transient vs permanent).

---

### 4.4 Config Live Reload

**Current behavior:** Config is loaded once at startup. Changing `~/.openz/config.json` requires restarting OpenZ.

**Enhancement:** Use `notify` (already a dependency) to watch `config.json` for changes and hot-reload. Only apply safe changes (tool timeouts, model selection) without restarting channels.

**Complexity:** Medium — must avoid races with in-flight agent loops.

---

### 4.5 Structured Logging to SQLite

**Current behavior:** Logs are plain text files rotated at 10MB. Filtering by session, level, or tool requires `grep`.

**Enhancement:** Write structured logs to SQLite (`~/.openz/logs.db`) with columns: timestamp, level, session_key, module, message, tool_name, duration_ms. The `openz logs` command can then run SQL queries like `WHERE session = 'telegram_123' AND level = 'error'`.

**Complexity:** Low — tracing-subscriber already supports custom writers.

---

## 5. Testing Gaps

### 5.1 No Integration Tests

**Current coverage:** 323 unit tests, **zero integration tests**. All tests use `#[cfg(test)]` inline modules.

**Gaps:**
- No tests that spin up a real AgentLoop with mock provider
- No tests for MCP client spawn/handshake
- No tests for channel startup/shutdown sequences
- No tests for the full tool call → security approval → execution → response pipeline

**Recommended:** Add a `tests/` directory with:
- `tests/agent_loop.rs` — full turn state machine with mock provider
- `tests/tools/` — each tool's `call()` with realistic inputs
- `tests/channels/` — WS connect/send/receive/disconnect

---

### 5.2 Pre-Existing Test Failures

Status: **Resolved / stale finding.** A full sequential lib run currently passes: `cargo test --lib -- --test-threads=1` = 331 passed, 0 failed.

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
