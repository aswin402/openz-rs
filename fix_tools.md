# OpenZ Tools Feature & Integration Fix Plan

**Date:** 2026-06-20
**Scope:** Fix all bugs, broken features, integration issues, dead code, and documentation gaps found in full feature audit

---

## Summary

| Priority | Total | Status |
|----------|-------|--------|
| P0 — Security | 3 | ✅ Fixed |
| P1 — Broken Features | 4 | ✅ Fixed |
| P2 — Integration Bugs | 7 | ✅ Fixed |
| P3 — Dead/Unused Code | 3 | ✅ Fixed |
| P4 — Documentation Gaps | 4 | ✅ Fixed |
| P5 — Silent Failures | 5 | ✅ Fixed |
| **Total** | **26** | **26 Fixed** |

---

## P0 — Security (3)

### P0-1. Remove `--disable-web-security` from image_generator.rs and html_video.rs ✅ FIXED
- **Files:** `src/tools/image_generator.rs:82-83`, `src/tools/html_video.rs:64-65`
- **Fix:** Remove `--allow-file-access-from-files` and `--disable-web-security` flags from Chrome args
- **Note:** `obscura.rs` already has this fix; now consistent

### P0-2. Redact API keys from debug logs ✅ FIXED
- **File:** `src/providers/openai.rs:285`
- **Fix:** Remove or truncate `Raw: {}` portion from the `tracing::warn!` that logs raw tool call arguments

### P0-3. Add confirmation for MCP server removal ✅ FIXED
- **File:** `src/tools/mcp_manager.rs`
- **Fix:** Add a confirmation step or rate-limit to prevent bulk removal of MCP servers in a single turn

---

## P1 — Broken Features (4)

### P1-4. Fix Twitter search (Nitter dead) ✅ FIXED
- **File:** `src/tools/social_search.rs:73`
- **Fix:** Replace dead `nitter.privacydev.net` with working alternative or remove Twitter search path, return clear error message

### P1-5. Fix YouTube search (Invidious dead) ✅ FIXED
- **File:** `src/tools/social_search.rs:127`
- **Fix:** Replace dead `invidious.io.lol` with working alternative or rely on the existing direct YouTube scraping fallback, return clear error message

### P1-6. Add Reddit rate-limit handling ✅ FIXED
- **File:** `src/tools/social_search.rs:30`
- **Fix:** Add retry logic with exponential backoff for HTTP 429 responses, or add User-Agent header

### P1-7. Add DB corruption recovery for shared_memory ✅ FIXED
- **File:** `src/tools/shared_memory.rs:37-81`
- **Fix:** Add `PRAGMA journal_mode=WAL`, integrity check on connection failure, log warning with recovery suggestion

---

## P2 — Integration Bugs (7)

### P2-8. Fix cron jobs stuck in "Running" after crash ✅ FIXED
- **File:** `src/cron/scheduler.rs`
- **Fix:** On startup, reset all `Running` status jobs back to `Pending` so they can re-execute

### P2-9. Fix cron `last_run` timing ✅ FIXED
- **File:** `src/cron/scheduler.rs:51`
- **Fix:** Set `last_run` after execution completes, not before. Calculate `next_run` from `last_run + schedule`

### P2-10. Fix MCP dual cache waste ✅ FIXED
- **File:** `src/tools/mcp.rs`
- **Fix:** Consolidate `SPAWNED_MCP_CLIENTS` and `LAZY_MCP_CLIENTS` into a single cache, or have lazy cache reference the spawned cache

### P2-11. Fix MCP OnceLock stale client ✅ FIXED
- **File:** `src/tools/mcp.rs:12`
- **Fix:** Replace `OnceLock` with `Mutex<Option<McpClient>>` to allow reconnection on crash

### P2-12. Fix obscura Chrome tab leak ✅ FIXED
- **File:** `src/tools/obscura.rs:239-310`
- **Fix:** Use a guard pattern to ensure tab close runs even on error (like `ServerGuard` in image_generator.rs)

### P2-13. Add CDP timeout to obscura ✅ FIXED
- **File:** `src/tools/obscura.rs:116-144`
- **Fix:** Add `tokio::time::timeout` around the CDP read loop

### P2-14. Deduplicate browser code ✅ FIXED
- **Files:** `src/tools/obscura.rs`, `src/tools/image_generator.rs`, `src/tools/html_video.rs`
- **Fix:** Extracted shared `ensure_browser_running()`, `kill_browser_on_port_9222()`, `send_cdp_cmd()` into shared `browser_common.rs` module
- **New file:** `src/tools/browser_common.rs`

---

## P3 — Dead/Unused Code (3)

### P3-15. Remove unused `api_type` config field ✅ FIXED
- **File:** `src/config/schema.rs`
- **Fix:** Removed `api_type` from `ProviderConfig`, cleaned 3 call sites in `cli.rs` and `websocket.rs`

### P3-16. Remove unused `subagent_timeout_secs` config field ✅ FIXED
- **File:** `src/config/schema.rs`
- **Fix:** Removed field and default from `AgentDefaults`

### P3-17. Remove unused `extra` config field ✅ FIXED
- **File:** `src/config/schema.rs`
- **Fix:** Removed `#[serde(flatten)] extra` catch-all from top-level `Config`

---

## P4 — Documentation Gaps (4)

### P4-18. Add missing tools to README.md and CHANGELOG.md ✅ FIXED
- **Tools:** `find_files`, `replace_lines`, `rust_docs`, `social_search`, `check_port`, `store_memory`, `recall_memory`, `clear_memory`, `archive_research`, `search_research`, `index_notes`, `parallel_research`, `evaluator_optimizer_loop`, `compile_template`, `git_manager`, `python_sandbox`, `open_path`

### P4-19. Document all 38 subagent profiles ✅ FIXED
- **File:** `docs/subagents.md`
- **Fix:** Replaced "15+" mention with complete 38-profile table

### P4-20. Document `check_port` localhost restriction ✅ FIXED
- **File:** `docs/tools.md`
- **Fix:** Added note that check_port is restricted to localhost

### P4-21. Document `zenflow_edit` git requirement ✅ FIXED
- **File:** `docs/tools.md`
- **Fix:** Added note that zenflow_edit requires a git repository

---

## P5 — Silent Failures (5)

### P5-22. social_search dead backends return empty with no error
- **File:** `src/tools/social_search.rs`
- **Fix:** Return error messages explaining which backends are offline

### P5-23. social_search Reddit 429 swallowed
- **File:** `src/tools/social_search.rs`
- **Fix:** Propagate rate-limit errors or add retry logic

### P5-24. web_search DDG failure silent
- **File:** `src/tools/web_search.rs`
- **Fix:** Log warning when DDG scraping returns empty before falling back

### P5-25. crawl silent failure
- **File:** `src/tools/crawl.rs`
- **Fix:** Check crawl result and handle errors properly instead of `let _ =`

### P5-26. shared_memory DB corruption silent
- **File:** `src/tools/shared_memory.rs`
- **Fix:** Log clear warning when DB is corrupted (covered by P1-7)

---

## Execution Order

1. P0 fixes first (security)
2. P1 fixes (broken features)
3. P2 fixes (integration bugs)
4. P5 fixes (silent failures — often overlaps with P1/P2)
5. P3 cleanup (dead code)
6. P4 documentation
7. Final test pass
