# OpenZ — Session Enhancements Summary

This document captures all the architectural upgrades, performance optimizations, and security features implemented during this workspace session.

---

## 1. Structured Logging to SQLite (`logs.db`)
* **Objective**: Replace plaintext logging with structured, queryable storage to support multi-process tracing.
* **Changes**:
  - Created a queryable SQLite database at `~/.openz/logs.db`.
  - Implemented an asynchronous background thread task (`init_db_writer`) communicating via an unbounded MPSC channel (`LOG_TX`) to execute writes in a non-blocking manner.
  - Implemented `SqliteLogLayer` (implementing `tracing_subscriber::Layer`) to extract metadata (level, target, message, session) from tracing events.
  - Upgraded CLI log viewers (`print_tail_sqlite`, `follow_sqlite`) to load logs filtered by session ID and log level.
  - Registered `logs.db` inside database tracking schemas and verified concurrency with unit tests.

## 2. Lazy Configuration Metadata Caching
* **Objective**: Avoid redundant disk read I/O and JSON parsing overhead on every configuration lookup.
* **Changes**:
  - Implemented `CONFIG_CACHE` inside `src/config/loader.rs`.
  - Configured `load_config` to compare the config file's modification time (`fs::metadata(path).modified()`), hitting the cached instance if unchanged.
  - Configured `save_config` to automatically invalidate the cache upon writes.

## 3. Tool Call Retry with Exponential Backoff
* **Objective**: Prevent tool executions from failing due to transient network anomalies, DNS drops, or HTTP 429 rate limits.
* **Changes**:
  - Implemented `is_transient_error` helper in `src/agent/agent_loop/run.rs` to detect rate-limit (HTTP 429), gateway (502/503/504), network connection, and DNS errors.
  - Modified `ToolExecutionPipeline::execute()` to retry transient errors up to 3 times, sleeping with exponential backoff (starting at 1s, doubling to 2s, 4s).

## 4. SQLite Vector Embeddings Cache (`embeddings_cache.db`)
* **Objective**: Migrate semantic search embeddings from a slow, unsafe JSON file to a transactional database.
* **Changes**:
  - Created tables `file_cache` (metadata & modification times) and `chunk_cache` (text segments & serialized binary float array BLOBs).
  - Wired cascade delete triggers (`ON DELETE CASCADE`) to automatically purge stale chunks when a file is modified or deleted.
  - Scoped SQLite connection and statement variables within block scopes to ensure they are dropped before any async `.await` boundary, resolving non-`Send` thread compilation constraints.
  - Wrapped bulk inserts inside a single database transaction (`conn.transaction()`) to guarantee high-speed operation.

## 5. SecurityGuard Whitelist Rule System
* **Objective**: Minimize repetitive approval prompts for standard developer actions while maintaining strict security boundaries.
* **Changes**:
  - Added `whitelisted_command_prefixes` and `whitelisted_paths` settings to `AgentDefaults` (`src/config/schema.rs`).
  - Implemented prefix word boundary matching (`matches_whitelisted_prefix`) and absolute/relative path check (`is_in_whitelisted_paths`) inside `SecurityGuard` (`src/agent/security.rs`).
  - Integrated whitelisting directly into `is_safe_path` (bypassing confirmation for safe paths) and `is_sensitive_with_mode` (bypassing confirmation for command prefixes).

## 6. CLI logs Auto-Detection & Interactive Selector
* **Objective**: Make logs easy to query and follow for both active terminals and independent background gateway/bot threads.
* **Changes**:
  - `openz logs` without arguments automatically queries the active session ID from `~/.openz/activity.json` or falls back to the latest log record.
  - `openz logs global` inspects the logs database to detect all active/historic sessions, resolves their type (Gateway, Telegram Bot, CLI Agent, etc.), retrieves their last log snippet, and renders a premium interactive vertical select menu (`select_menu_custom`) for easy selection.

## 7. `get_logs` Native Agent Tool
* **Objective**: Equip the LLM agent with direct log visibility to diagnose system issues.
* **Changes**:
  - Built `GetLogsTool` (`src/tools/get_logs.rs`) supporting optional parameters for `limit`, `session` ('current', 'gateway', 'all'), and `level`.
  - Registered it as a core tool, enabling the LLM agent to inspect active session logs natively.
  - Added unit test coverage validating query filters, thresholds, and limits.

## 8. `/settings` Chat TUI Slash Command
* **Objective**: View the active model, provider, security modes, and whitelists directly from the interactive TUI.
* **Changes**:
  - Registered `/settings` under `SLASH_COMMANDS` in `src/channels/cli/render.rs`.
  - Implemented the command handler in the CLI raw mode input loop (`src/channels/cli/mod.rs`), locking `self.defaults` to print a clean summary of configurations without exiting the session.
