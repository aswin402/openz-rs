# Private Trace / Public Output Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent OpenZ from exposing private model reasoning, workflow internals, memory-capture notices, and debug traces in normal user-facing channels while improving research/browser reliability through policy-driven fallback budgets.

**Architecture:** Introduce a strict boundary between private trace events and public user output. Keep raw reasoning and detailed runtime diagnostics in logs/session metadata only when explicitly configured, while public channels render compact progress summaries. Research and browser recovery become budgeted, health-aware flows instead of repeated ad-hoc retries.

**Tech Stack:** Rust, Tokio, serde/serde_json, existing OpenZ `AgentLoop`, channel renderers, SearchXyz, browser tools, `cargo test`.

## Global Constraints

- Do not hardcode fixes for a single model, user query, website, or provider.
- Default user-facing output must not include raw reasoning, system/developer instructions, memory-policy details, or tool-planning narration.
- Keep debug visibility available through explicit local configuration and structured logs.
- Preserve existing tool-call transcript compatibility for provider continuation.
- Use TDD for behavior changes.
- Prefer config-driven policy over scattered conditionals.
- Keep changes small and independently testable.
- Run targeted tests after each task; run `cargo test --lib -- --test-threads=1` before release.

---

## File Structure

- Modify `src/config/schema.rs`: add durable visibility/research config defaults and migrate thought display defaults.
- Modify `src/config/loader.rs`: migrate legacy configs safely without overwriting explicit user choices.
- Modify `src/agent/agent_loop/run.rs`: centralize reasoning visibility decisions and stop public raw reasoning sends.
- Modify `src/agent/agent_loop/transcript.rs`: preserve raw tool output with stable refs and avoid public truncation confusion.
- Modify `src/agent/agent_loop/research_policy.rs`: add configurable research budgets and source requirements.
- Modify `src/tools/searchxyz/web.rs`: surface doctor diagnostics as structured data usable by the agent loop.
- Modify `src/tools/subagent/parallel_research.rs`: support partial success budgets.
- Modify `src/tools/browser_status.rs`, `src/tools/browser_common.rs`, `src/tools/firefox.rs`, `src/tools/gsd_browser.rs`, `src/tools/obscura.rs`: add browser preflight/facade behavior.
- Modify `src/channels/cli/mod.rs`, `src/cli/configure.rs`: rename UX from thoughts to trace visibility while preserving old commands.
- Modify docs: `README.md`, `CHANGELOG.md`, `docs/tools.md`, `futureupdates.md`.

---

## TODO

### Task 1: Hide Reasoning By Default

**Files:**
- Modify: `src/config/schema.rs`
- Modify: `src/agent/agent_loop/run.rs`
- Modify: `src/channels/cli/mod.rs`
- Modify: `src/cli/configure.rs`
- Test: `src/config/schema.rs`
- Test: `src/agent/agent_loop/run.rs`

**Interfaces:**
- Produces: `default_tui_thought_display() -> String` returns `"off"`.
- Produces: `public_reasoning_visibility(mode: &str) -> PublicReasoningVisibility`.
- Produces: `should_send_public_reasoning_progress(mode: &str) -> bool`.

- [x] **Step 1: Write failing config default test**

Add this test to `src/config/schema.rs`:

```rust
#[test]
fn default_tui_thought_display_is_hidden_for_public_safety() {
    let defaults = AgentDefaults::default();
    assert_eq!(defaults.tui_thought_display, "off");
}
```

- [x] **Step 2: Run test to verify RED**

Run: `cargo test --lib default_tui_thought_display_is_hidden_for_public_safety -- --test-threads=1`

Expected: FAIL because current default is `"full"`.

- [x] **Step 3: Implement minimal config default**

Change `default_tui_thought_display()` to return `"off"`.

- [x] **Step 4: Write public-rendering tests**

Add tests in `src/agent/agent_loop/run.rs`:

```rust
#[test]
fn public_reasoning_progress_is_hidden_by_default() {
    assert!(!should_send_public_reasoning_progress("off"));
    assert!(!should_send_public_reasoning_progress("hidden"));
    assert!(should_send_public_reasoning_progress("compact"));
    assert!(should_send_public_reasoning_progress("full"));
}

#[test]
fn compact_reasoning_summary_never_returns_full_long_reasoning() {
    let raw = "internal step ".repeat(80);
    let compact = compact_reasoning_summary(&raw);
    assert!(compact.chars().count() <= 360);
    assert_ne!(compact, raw);
}
```

- [x] **Step 5: Route public reasoning sends through helper**

In `src/agent/agent_loop/run.rs`, gate `send_progress_update()` calls for `▶ Thought` on `should_send_public_reasoning_progress(&config.agents.defaults.tui_thought_display)`.

- [x] **Step 6: Verify GREEN**

Run:

```bash
cargo test --lib default_tui_thought_display_is_hidden_for_public_safety -- --test-threads=1
cargo test --lib agent::agent_loop::run::tests::public_reasoning_progress_is_hidden_by_default -- --test-threads=1
cargo test --lib agent::agent_loop::run::tests::compact_reasoning_summary_never_returns_full_long_reasoning -- --test-threads=1
```

Expected: PASS.

### Task 2: Separate Private Trace Events From Public Output

**Files:**
- Create: `src/agent/events.rs`
- Modify: `src/agent/mod.rs`
- Modify: `src/agent/agent_loop/tool_execution.rs`
- Modify: `src/agent/agent_loop/run.rs`
- Test: `src/agent/events.rs`

**Interfaces:**
- Produces: `enum AgentEvent`.
- Produces: `impl AgentEvent { fn public_text(&self, visibility: &OutputVisibility) -> Option<String> }`.
- Produces: `struct OutputVisibility { reasoning: PublicReasoningVisibility, workflow_notices: bool, memory_notices: bool }`.

- [x] **Step 1: Write event-rendering tests**
- [x] **Step 2: Implement `AgentEvent` and `OutputVisibility`**
- [x] **Step 3: Replace direct workflow/memory progress prints with event rendering**
- [x] **Step 4: Verify public renderer drops private reasoning and trace debug**

### Task 3: Silence Auto-Capture Notices By Default

**Files:**
- Modify: `src/config/schema.rs`
- Modify: `src/agent/agent_loop/run.rs`
- Modify: `src/tools/shared_memory/auto_capture.rs`
- Test: `src/agent/agent_loop/run.rs`

**Interfaces:**
- Produces: `show_auto_capture_notices: bool` config defaulting to `false`.
- Produces: `format_auto_capture_notice(...) -> Option<String>`.

- [x] **Step 1: Add config default test**
- [x] **Step 2: Add notice formatting tests**
- [x] **Step 3: Gate notice rendering on config**
- [x] **Step 4: Verify no `◇ [Knowledge]` appears by default**

### Task 4: Add Research Budget Policy

**Files:**
- Modify: `src/config/schema.rs`
- Modify: `src/agent/agent_loop/research_policy.rs`
- Modify: `src/agent/agent_loop/run.rs`
- Test: `src/agent/agent_loop/research_policy.rs`

**Interfaces:**
- Produces: `ResearchRuntimePolicy`.
- Produces: `ResearchBudget { default_time_budget_secs, max_search_attempts, max_browser_fallbacks, require_sources_for_current_claims, stop_on_captcha }`.
- Produces: `classify_research_failure(error: &str) -> ResearchFailureKind`.

- [x] **Step 1: Test budget defaults**
- [x] **Step 2: Test CAPTCHA/browser-driver failures are terminal for browser fallback**
- [x] **Step 3: Wire policy into research tool selection hints**
- [x] **Step 4: Verify repeated backend failures stop after budget**

### Task 5: Add Source Ledger For Research Answers

**Files:**
- Create: `src/agent/source_ledger.rs`
- Modify: `src/agent/mod.rs`
- Modify: `src/tools/searchxyz/web.rs`
- Modify: `src/tools/web_search.rs`
- Test: `src/agent/source_ledger.rs`

**Interfaces:**
- Produces: `SourceLedger`.
- Produces: `SourceRef { url, title, fetched_at, status }`.
- Produces: `SourceLedger::confidence_for_live_claims()`.

- [x] **Step 1: Test source ledger confidence levels**
- [x] **Step 2: Capture successful source URLs from web/search tools**
- [x] **Step 3: Capture failed attempts**
- [x] **Step 4: Force final answer caveat when sources are missing for live claims**

### Task 6: Browser Preflight And Unified Fallback

**Files:**
- Modify: `src/tools/browser_status.rs`
- Modify: `src/tools/browser_common.rs`
- Modify: `src/tools/firefox.rs`
- Modify: `src/tools/gsd_browser.rs`
- Modify: `src/tools/obscura.rs`
- Test: `src/tools/browser_status.rs`

**Interfaces:**
- Produces: `BrowserHealth`.
- Produces: `BrowserBackendStatus`.
- Produces: `recommended_browser_backend(health: &BrowserHealth) -> Option<BrowserBackend>`.

- [x] **Step 1: Test missing geckodriver reports actionable preflight status**
- [x] **Step 2: Test Chrome CDP preferred when running**
- [x] **Step 3: Test CAPTCHA-like page classification**
- [x] **Step 4: Route browser tools through preflight result**

### Task 7: Parallel Research Partial Results

**Files:**
- Modify: `src/tools/subagent/parallel_research.rs`
- Test: `src/tools/subagent/tests.rs`

**Interfaces:**
- Adds optional parameters: `overall_timeout_secs`, `min_successes`, `min_success_ratio`.
- Returns: `status: "partial_success"` when enough tasks succeeded and others timed out.

- [x] **Step 1: Test partial success return shape**
- [x] **Step 2: Implement overall timeout race**
- [x] **Step 3: Preserve cancellation semantics**
- [x] **Step 4: Verify existing subagent tests still pass**

### Task 8: Stable Long Tool Output References

**Files:**
- Modify: `src/agent/agent_loop/transcript.rs`
- Modify: `src/tools/headroom/cache.rs`
- Test: `src/agent/agent_loop/transcript.rs`

**Interfaces:**
- Produces: `tool-output://<uuid>` refs.
- Produces: transcript payload with `truncated`, `original_ref`, `preview`, `bytes`.

- [x] **Step 1: Test raw saved output is never truncated**
- [x] **Step 2: Test transcript contains stable original ref**
- [x] **Step 3: Implement structured truncation metadata**
- [x] **Step 4: Verify `retrieve_original` still bypasses truncation**

### Task 9: Marketplace Intent Clarification Policy

**Files:**
- Modify: `src/agent/agent_loop/research_policy.rs`
- Modify: `src/agent/agent_loop/run.rs`
- Test: `src/agent/agent_loop/research_policy.rs`

**Interfaces:**
- Produces: `detect_marketplace_intent_ambiguity(text: &str) -> Option<ClarifyingQuestion>`.

- [x] **Step 1: Test buy/sell marketplace ambiguity**
- [x] **Step 2: Test unambiguous buy request does not clarify**
- [x] **Step 3: Prompt one concise clarifying question before research tools**

### Task 10: Docs And Release Notes

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/tools.md`
- Modify: `futureupdates.md`

- [x] **Step 1: Document default hidden reasoning**
- [x] **Step 2: Document `/tui trace hidden|summary|full` and legacy `/tui thoughts` alias**
- [x] **Step 3: Document research budget behavior**
- [x] **Step 4: Add v0.0.113 changelog section**

---

## Self-Review

- Spec coverage: Covers private reasoning leakage, public/internal event separation, search/research budgets, browser health, long-output references, auto-capture noise, source citations, and ambiguity handling.
- Placeholder scan: No `TBD`/generic placeholders are required for Task 1. Later tasks are intentionally high-level TODO checkpoints and must be expanded with concrete TDD snippets before execution.
- Type consistency: Task 1 functions use existing `tui_thought_display`; later tasks introduce new types with clear names.

