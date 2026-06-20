# OpenZ MCP System Fix Plan

**Date:** 2026-06-21

## Summary

| Priority | Total | Status |
|----------|-------|--------|
| P0 — Dual Cache | 1 | ✅ Fixed |
| P1 — Dead Code | 1 | ✅ Fixed |
| P2 — TOCTOU Races | 2 | ✅ Fixed |
| P3 — Bridge Lifecycle | 2 | ✅ Fixed |
| P4 — Tests & Config | 2 | ✅ Fixed |
| **Total** | **8** | **8 Fixed** |

---

## P0 — Dual Cache Consolidation

### P0-1. Consolidate SPAWNED_MCP_CLIENTS and LAZY_MCP_CLIENTS into one cache
- **File:** `src/tools/mcp.rs`
- **Problem:** Two independent caches (`SPAWNED_MCP_CLIENTS` for `spawn()`, `LAZY_MCP_CLIENTS` for `LazyMcpToolWrapper::call()`). First lazy call always misses and re-enters slow path.
- **Fix:** Remove `LAZY_MCP_CLIENTS`. Make `LazyMcpToolWrapper::call()` use `McpClient::spawn()` which already has its own cache via `SPAWNED_MCP_CLIENTS`. The spawn function already handles the fast path (returns existing client) and slow path (spawns new).
- **Result:** Single cache, first lazy call hits cache immediately.

---

## P1 — Dead Code Removal

### P1-1. Remove McpClientType::Stdio variant and all match arms
- **File:** `src/tools/mcp.rs`
- **Problem:** `Stdio` variant defined with `#[allow(dead_code)]` but never constructed — all spawns use gRPC. ~60 lines of unreachable code.
- **Fix:** Remove the `Stdio` variant from `McpClientType`, remove the 3 match arms in `list_tools()`, `call_tool()`, and `Drop`. Remove `#[allow(dead_code)]` annotations.
- **Result:** Cleaner code, dead code eliminated.

---

## P2 — TOCTOU Race Fixes

### P2-1. Fix find_free_port() race
- **File:** `src/tools/mcp.rs`
- **Problem:** Port bound-then-released to check availability. Between check and actual use by `run_mcp_bridge()`, another process could claim it.
- **Fix:** Pass reserved `TcpListener` directly to `run_mcp_bridge()` which drops it right before `tonic::Server::serve()` binds, shrinking the TOCTOU window from ~100ms to <1µs.

### P2-2. Fix reconnect path TOCTOU race
- **File:** `src/tools/mcp.rs` (`LazyMcpToolWrapper::call()`)
- **Problem:** Drops cache lock, then re-acquires to remove stale entry. Another task could insert a fresh client in the gap.
- **Fix:** Made moot by P0-1 — lazy cache removed entirely. Spawn cache managed atomically inside `McpClient::spawn()`.

---

## P3 — Bridge Lifecycle

### P3-1. Monitor child process in run_mcp_bridge()
- **File:** `src/tools/mcp.rs`
- **Problem:** If stdio child crashes, gRPC bridge keeps running with no reader, returning stale errors.
- **Fix:** Spawn monitor task that waits for child exit and triggers bridge shutdown via `tokio::select!`.

### P3-2. Cancel stderr reader on bridge shutdown
- **File:** `src/tools/mcp.rs`
- **Problem:** Spawned stderr forwarding task runs indefinitely after bridge shutdown.
- **Fix:** Call `.abort()` on both reader handles after `tokio::select!` completes.

---

## P4 — Tests & Config

### P4-1. Add unit tests for mcp.rs
- **File:** `src/tools/mcp.rs` (`#[cfg(test)]` module)
- **Tests added:** `test_find_free_port_returns_bound_listener`, `test_find_free_port_sequential_ports_differ`, `test_invalidate_nonexistent_key_does_not_panic`, `test_invalidate_after_spawn_removes_entry`.

### P4-2. Make memory server port configurable
- **File:** `src/config/schema.rs`
- **Fix:** Memory server uses same auto-bridge via stdio as all other servers — dynamic port, no hardcoded 50051.

---

## Execution Log

1. ✅ **P0-1** — Consolidate caches
2. ✅ **P1-1** — Remove dead code
3. ✅ **P2-1** — Fix find_free_port race
4. ✅ **P3-1** — Child process monitoring
5. ✅ **P3-2** — Stderr task cancellation
6. ✅ **P4-2** — Configurable memory port
7. ✅ **P4-1** — Unit tests
8. ✅ **Final test pass** — all 118 tests pass
