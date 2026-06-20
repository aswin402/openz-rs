# OpenZ Fix Implementation Plan

**Date:** 2026-06-20
**Total Issues:** 48 (10 Critical, 13 High, 17 Medium, 8 Low)
**Strategy:** Fix by severity, then by dependency order within severity.

---

## Phase 1: CRITICAL Security Fixes (C1-C10)

| ID | Issue | File(s) | Fix Strategy | Est. Lines Changed |
|----|-------|---------|-------------|-------------------|
| C1 | SQL Injection | `tools/db_inspector.rs` | Add proper SQL blocklist with `sqlparser` crate or strict allowlist; block `ATTACH`, `PRAGMA`, `.shell`, `.import` | ~40 |
| C2 | Shell Injection | `tools/compiler_auto_heal.rs` | Validate `compile_command` against allowlist (`cargo`, `rustc`, `gcc`, etc.); add backup before overwrite | ~30 |
| C3 | SSRF | `tools/web.rs`, `tools/rust_docs.rs` | Add URL validator: block private IPs, `localhost`, `127.*`, `169.254.*`, `10.*`, `172.16-31.*`, `192.168.*` | ~50 |
| C4 | JS Injection | `tools/image_generator.rs` | Properly escape selector with JSON.stringify equivalent; use `serde_json::to_string` for JS values | ~10 |
| C5 | WhatsApp Signature | `channels/whatsapp.rs` | Verify `X-Hub-Signature-256` HMAC; change default token to random; bind `127.0.0.1` | ~40 |
| C6 | CORS | `channels/websocket.rs` | Restrict to configured origins; default to `127.0.0.1` origins only | ~15 |
| C7 | IMAP Plaintext | `channels/email.rs` | Add TLS configuration; default to port 993 with TLS | ~20 |
| C8 | Hardcoded Paths | `config/schema.rs` | Use `dirs` crate for home dir; make `AI_AGENT_TOOLS_BASE` configurable via env/config | ~25 |
| C9 | env::set_var | `cli.rs` | Replace `set_var` with passing env through `Command::env()` or using `std::sync::OnceLock` | ~10 |
| C10 | Obscura JS | `tools/obscura.rs` | Remove `--disable-web-security` and `--allow-file-access-from-files`; keep `--no-sandbox` only for local dev | ~15 |

**Verification:** `cargo check` after each fix.

---

## Phase 2: HIGH Severity Fixes (H1-H13)

| ID | Issue | File(s) | Fix Strategy | Est. Lines Changed |
|----|-------|---------|-------------|-------------------|
| H1 | UTF-8 Panics | `social_search.rs`, `agent_loop.rs`, `logs.rs`, `security.rs` | Use `.chars().take(n).collect()` or `unicode-segmentation` for safe truncation | ~30 |
| H2 | Disk Usage | `agent/agent_loop.rs` | Add cleanup on startup: delete files older than 7 days in `tool_outputs/` and `traces/` | ~40 |
| H3 | Activity Race | `agent/activity.rs` | Use `fs2::FileExt` for file locking; add atomic write via temp file + rename | ~35 |
| H4 | Port Race | `tools/mcp.rs` | Return the `TcpListener` instead of dropping it; bind is done by caller | ~20 |
| H5 | Mutex Panics | `tools/watcher.rs` | Replace `.unwrap()` with `.lock().unwrap_or_else(\|e\| e.into_inner())` | ~10 |
| H6 | NaN Panic | `tools/semantic_search.rs` | Replace `.unwrap()` with `.unwrap_or(std::cmp::Ordering::Equal)` | ~3 |
| H7 | SOP Crash | `sop/engine.rs` | Replace `.expect()` with `?` or proper error return | ~5 |
| H8 | Ollama Race | `providers/ollama_manager.rs` | Use `OnceCell` with async init; single spawn guard | ~25 |
| H9 | Rate Limiting | `channels/websocket.rs`, `channels/whatsapp.rs` | Add `tokio::sync::Semaphore` with configurable max concurrent tasks | ~30 |
| H10 | Orphan Processes | `tools/firefox.rs` | Store `Child` handle; implement `Drop` to kill process | ~20 |
| H11 | Template Recursion | `tools/template_compiler.rs` | Add `depth` parameter with max 10; return error on exceed | ~15 |
| H12 | Loose Mode | `agent/security.rs` | Block `curl.*\|.*bash` and `wget.*\|.*bash` in all modes | ~5 |
| H13 | WS Cancel | `channels/cli.rs` | Use `CancellationToken` to abort agent task on Esc/Ctrl+C | ~20 |

**Verification:** `cargo check` + `cargo test` after Phase 2.

---

## Phase 3: MEDIUM Severity Fixes (M1-M17)

| ID | Issue | File(s) | Fix Strategy | Est. Lines Changed |
|----|-------|---------|-------------|-------------------|
| M1 | Silent Errors | Multiple | Replace `let _ =` with `if let Err(e) = ... { tracing::warn!(...) }` | ~60 |
| M2 | Empty API Key | `providers/resolver.rs` | Return `Err` if key is empty for non-local providers | ~10 |
| M3 | TLS Fallback | `providers/openai.rs`, `anthropic.rs` | Propagate TLS build error instead of `unwrap_or_default()` | ~10 |
| M4 | Blocking Sleep | `providers/ollama_manager.rs` | Replace `std::thread::sleep` with `tokio::time::sleep` | ~5 |
| M5 | reqwest Client | `providers/mod.rs` | Use `OnceLock<reqwest::Client>` for shared client | ~10 |
| M6 | Regex Recompile | `cli.rs`, `web.rs`, `context_compactor.rs` | Use `OnceLock<Regex>` for static patterns | ~30 |
| M7 | SVG Injection | `tools/svg_animator.rs` | Apply `escape_xml()` to all attribute values | ~5 |
| M8 | find_free_port | `tools/mcp.rs` | Return error if no free port found | ~5 |
| M9 | CSS Injection | `tools/image_generator.rs` | Escape backslash, backtick, dollar in template literals | ~5 |
| M10 | Vision Check | `providers/mod.rs` | Use word-boundary matching instead of `contains` | ~5 |
| M11 | MockProvider | `providers/mock.rs` | Use `fetch_sub` for atomic decrement | ~5 |
| M12 | Python Timeout | `tools/shell.rs` | Add configurable timeout (default 60s) to `PythonSandboxTool` | ~20 |
| M13 | Crawl Params | `tools/crawl.rs` | Clamp `limit` to 1000, `depth` to 10, `delay` to minimum 50ms | ~10 |
| M14 | Trailing Newline | `tools/filesystem.rs` | Preserve original file's trailing newline in output | ~5 |
| M15 | Code Duplication | `tools/` | Extract shared browser utils to `tools/browser_utils.rs` | ~100 (extract) |
| M16 | Hardcoded Instances | `tools/social_search.rs` | Make Nitter/Invidious instances configurable with fallback list | ~20 |
| M17 | Config Reload | `channels/mod.rs` | Cache config in memory; invalidate on config change | ~10 |

**Verification:** `cargo check` + `cargo test` after Phase 3.

---

## Phase 4: LOW Severity / Cleanup (L1-L11)

| ID | Issue | File(s) | Fix Strategy | Est. Lines Changed |
|----|-------|---------|-------------|-------------------|
| L1 | Dead TurnState | `agent/agent_loop.rs` | Remove `Respond` state; go directly from `Save` to `Done` | ~10 |
| L2 | AgentError | `agent/error.rs` | Remove unused error type and `From` impls | ~30 |
| L3 | Hash Perf | `session.rs` | Incremental hashing: only rehash new messages | ~20 |
| L4 | Prompt Rebuild | `agent/agent_loop.rs` | Cache system prompt; rebuild only on config/skill change | ~30 |
| L5 | Cron Shutdown | `cron/scheduler.rs` | Return `JoinHandle`; add `CancellationToken` | ~15 |
| L6 | Discord Reconnect | `channels/discord.rs` | Add max retry count (e.g., 5) with exponential backoff | ~15 |
| L7 | WASM Exit Code | `tools/wasm_sandbox.rs` | Extract actual exit code from WASI error | ~10 |
| L8 | Empty Embeddings | `tools/shared_memory.rs` | Skip storing entries with empty embeddings | ~5 |
| L9 | Cron Log None | `cron/scheduler.rs` | Handle `Option<String>` properly in log formatting | ~3 |
| L10 | Empty Model Name | `subagents/mod.rs` | Skip fallback if model name is empty | ~5 |
| L11 | Biased Random | `channels/mod.rs` | Use `fastrand` or iterate all UUID bytes | ~5 |

**Verification:** `cargo check` + `cargo test` + `cargo clippy` final pass.

---

## Execution Order

```
Phase 1 (Critical) → cargo check
Phase 2 (High)     → cargo check + cargo test
Phase 3 (Medium)   → cargo check + cargo test
Phase 4 (Low)      → cargo check + cargo test + cargo clippy
```

## Files Modified (Expected)

- `src/config/schema.rs` (C8)
- `src/channels/websocket.rs` (C6, H9)
- `src/channels/whatsapp.rs` (C5, H9)
- `src/channels/email.rs` (C7)
- `src/channels/cli.rs` (C9, H13, M6)
- `src/channels/mod.rs` (M1, M17, L11)
- `src/channels/discord.rs` (L6)
- `src/channels/telegram.rs` (M1)
- `src/agent/agent_loop.rs` (H1, H2, L1, L4)
- `src/agent/activity.rs` (H3)
- `src/agent/security.rs` (H12)
- `src/agent/context_compactor.rs` (M6)
- `src/agent/skills.rs` (M1)
- `src/agent/error.rs` (L2)
- `src/agent/style/mod.rs` (H1)
- `src/providers/mod.rs` (M5, M10)
- `src/providers/openai.rs` (M3)
- `src/providers/anthropic.rs` (M3)
- `src/providers/ollama_manager.rs` (H8, M4)
- `src/providers/mock.rs` (M11)
- `src/providers/resolver.rs` (M2)
- `src/tools/db_inspector.rs` (C1)
- `src/tools/compiler_auto_heal.rs` (C2)
- `src/tools/web.rs` (C3, M6)
- `src/tools/rust_docs.rs` (C3)
- `src/tools/image_generator.rs` (C4, M9)
- `src/tools/obscura.rs` (C10)
- `src/tools/social_search.rs` (H1, M16)
- `src/tools/mcp.rs` (H4, M8)
- `src/tools/watcher.rs` (H5)
- `src/tools/semantic_search.rs` (H6)
- `src/tools/firefox.rs` (H10)
- `src/tools/template_compiler.rs` (H11)
- `src/tools/svg_animator.rs` (M7)
- `src/tools/shell.rs` (M12)
- `src/tools/crawl.rs` (M13)
- `src/tools/filesystem.rs` (M14)
- `src/sop/engine.rs` (H7)
- `src/cron/scheduler.rs` (L5, L9)
- `src/subagents/mod.rs` (L10)
- `src/session.rs` (L3)

**New file:**
- `src/tools/browser_utils.rs` (M15 — extracted shared code)
