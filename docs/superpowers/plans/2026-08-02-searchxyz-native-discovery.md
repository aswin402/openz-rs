# SearchXyz Native Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make OpenZ able to discover web results without mandatory Brave/SearXNG dependency by adding browser-backed discovery, local-first cascade policy, structured failure handling, and backend cooldowns.

**Architecture:** Keep SearchXyz as the stable facade. Add a native browser-search fallback that uses existing browser tools/daemons to render search result pages, extract organic result links, normalize/dedup, and hand URLs back to existing SearchXyz readers. Keep Brave/SearXNG as optional fast accelerators, not required core dependencies.

**Tech Stack:** Rust, async Tokio, `scraper` HTML parser, existing `Tool` trait, existing `gsd-browser` CLI, SearchXyz local index/cache.

## Global Constraints

- TDD required: failing test before production code for each task.
- No paid search API dependency for the new native path.
- Do not remove Brave/SearXNG support; keep them optional.
- Browser-backed discovery must return structured errors for blocked/captcha/timeout cases.
- Browser-backed discovery must enforce bounded timeouts and max result limits.
- Do not create files outside the repo except normal runtime/cache paths already used by OpenZ.
- Existing direct URL scope and one-reader-per-URL discipline must remain unchanged.

---

### Task 1: Add `searchxyz_browser_search` native tool

**Files:**
- Modify: `src/tools/searchxyz/web.rs`
- Modify: `src/tools/searchxyz/mod.rs`
- Modify: `src/cli/tools.rs`

**Interfaces:**
- Produces: `SearchXyzBrowserSearchTool` implementing `Tool`.
- Produces helper: `browser_search_url(engine: &str, query: &str) -> Result<String>`.
- Produces helper: `extract_browser_search_results(engine: &str, html: &str, max_results: usize) -> Vec<Value>`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn browser_search_builds_search_urls() {
    assert!(browser_search_url("duckduckgo", "rust async").unwrap().contains("q=rust+async"));
    assert!(browser_search_url("bing", "rust async").unwrap().contains("q=rust+async"));
}

#[test]
fn browser_search_extracts_and_dedupes_result_links() {
    let html = r#"<a href="https://example.com/a"><h2>A</h2></a><a href="/l/?uddg=https%3A%2F%2Fexample.com%2Fb">B</a>"#;
    let results = extract_browser_search_results("duckduckgo", html, 10);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["url"], "https://example.com/a");
    assert_eq!(results[1]["url"], "https://example.com/b");
}
```

Run: `cargo test tools::searchxyz::web::tests::browser_search_ --lib -- --test-threads=1`
Expected: FAIL because helpers/tool do not exist.

- [ ] **Step 2: Implement minimal helper and tool**

Add a `SearchXyzBrowserSearchTool` that:

```rust
pub struct SearchXyzBrowserSearchTool;
```

Parameters:

```json
{
  "query": "string required",
  "engine": "duckduckgo|bing default duckduckgo",
  "max_results": "integer default 5 max 20",
  "timeout_secs": "integer default 20 max 60"
}
```

Behavior:
- Build search URL.
- Use `GsdBrowserTool` to `navigate` then `page_source`.
- Parse result links.
- Return `{ status, engine, query, results, result_count }`.
- If no links and HTML contains captcha/block terms, return `{ status: "blocked", error_kind: "captcha_or_bot_block" }`.

- [ ] **Step 3: Register/export tool**

Add `SearchXyzBrowserSearchTool` to exports and registration so `tool_catalog domain=web` can expose it.

- [ ] **Step 4: Run focused and full tests**

Run:

```bash
cargo test tools::searchxyz::web::tests::browser_search_ --lib -- --test-threads=1
cargo test cli::builder::tests::test_native_tool_registration_names --lib
cargo test --lib -- --test-threads=1
```

Expected: all pass.

---

### Task 2: Add SearchXyz backend cooldown ledger

**Files:**
- Modify: `src/tools/searchxyz/web.rs`
- Modify: `src/tools/searchxyz/mod.rs`

**Interfaces:**
- Produces helper: `classify_search_failure(message: &str) -> SearchFailureKind`.
- Produces helper: `cooldown_seconds(kind: SearchFailureKind) -> u64`.

- [ ] Write tests that classify `captcha`, `403`, `429`, timeout, and empty results.
- [ ] Store ephemeral cooldowns in a process-local `OnceLock<Mutex<HashMap<String, Instant>>>`.
- [ ] Make doctor show cooled-down backends.
- [ ] Run focused tests and full lib tests.

---

### Task 3: Add search cascade policy

**Files:**
- Modify: `src/tools/web_search.rs`
- Modify: `src/tools/searchxyz/web.rs`
- Modify: `src/agent/agent_loop/build.rs`

**Interfaces:**
- Produces policy value: `native_then_browser`.
- Existing `native_only` and `native_then_external` remain.

- [ ] Write tests showing native failure falls through to browser search when policy is `native_then_browser`.
- [x] Add schema/docs for `search_policy=native_then_browser`.
- [ ] Ensure direct URL requests still do not invoke search.
- [ ] Run full lib tests.

---

### Task 4: Make browser-discovered URLs feed existing readers

**Files:**
- Modify: `src/tools/searchxyz/web.rs`

**Interfaces:**
- Produces optional `read_top_results: bool` and `save_mode` passthrough on `searchxyz_browser_search`.

- [ ] Test that `read_top_results=false` returns only URLs.
- [ ] Test that max result/read limits are clamped.
- [x] Implement optional calls into `searchxyz_read_url` for top N results.
- [ ] Run full lib tests.

---

### Task 5: Improve diagnostics and changelog

**Files:**
- Modify: `src/tools/searchxyz/web.rs`
- Modify: `CHANGELOG.md`
- Modify: `README.md`

**Interfaces:**
- Produces doctor hint: browser fallback available, no Brave/SearXNG required.

- [ ] Test doctor report includes browser fallback availability.
- [ ] Update changelog with Native Browser Discovery.
- [ ] Update README web-search section.
- [ ] Run full lib tests.

---

## Self-Review

Spec coverage:
- Browser fallback without Brave/SearXNG: Task 1 and Task 3.
- Other-agent inspiration: Crawlee/Crawl4AI/Scrapling patterns are mapped into queue/limits/browser/caching/failure classification tasks.
- Future-proofing: Task 2 cooldowns and Task 5 diagnostics.
- Existing optional providers preserved: Global constraints and Task 3.

Placeholder scan:
- No TBD placeholders. Each task has concrete files, commands, and expected behavior.

Type consistency:
- `SearchXyzBrowserSearchTool`, `browser_search_url`, and `extract_browser_search_results` are consistently named and consumed by later tasks.
