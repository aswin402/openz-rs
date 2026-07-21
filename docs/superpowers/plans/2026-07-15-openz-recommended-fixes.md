# OpenZ Recommended Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Incrementally complete the remaining actionable fixes from `recommendedfix.md` without pushing the raw audit document to GitHub.

**Architecture:** Treat `recommendedfix.md` as a local audit/backlog and implement one bounded, testable fix at a time. Prefer small shared helpers and targeted tests before larger refactors so the current OpenZ release remains stable.

**Tech Stack:** Rust, Tokio, serde_json, rusqlite, cargo test/check/fmt, existing OpenZ native tool modules.

## Global Constraints

- Do not push `recommendedfix.md` unless it is intentionally converted into public docs.
- Use TDD for bug fixes and behavior changes.
- Keep each task independently testable.
- Do not revert unrelated user changes.
- Keep `Cargo.lock` unchanged unless `Cargo.toml` dependency changes require it.
- Run `cargo fmt --check`, `cargo check`, and targeted tests after each task.

---

### Task 1: Shared Subagent Timeout Resolution

**Status:** Completed on 2026-07-15.

**Files:**
- Modify: `src/tools/subagent/mod.rs`
- Modify: `src/tools/subagent/delegate_task.rs`
- Modify: `src/tools/subagent/delegate_profile.rs`
- Modify: `src/tools/subagent/parallel_research.rs`
- Test: `src/tools/subagent/tests.rs`

**Interfaces:**
- Produces: `resolve_subagent_timeout_secs(requested_timeout_secs: Option<u64>, default_timeout_secs: u64) -> u64`
- Consumes: `crate::tools::clamp_tool_timeout_secs`

- [x] **Step 1: Write failing helper test**

```rust
#[test]
fn test_resolve_subagent_timeout_uses_default_and_clamps() {
    assert_eq!(resolve_subagent_timeout_secs(None, 300), 300);
    assert_eq!(
        resolve_subagent_timeout_secs(Some(1), 300),
        crate::tools::MIN_TOOL_TIMEOUT_SECS
    );
    assert_eq!(
        resolve_subagent_timeout_secs(Some(999_999), 300),
        crate::tools::MAX_TOOL_TIMEOUT_SECS
    );
}
```

- [x] **Step 2: Verify RED**

Run: `cargo test --lib test_resolve_subagent_timeout_uses_default_and_clamps -- --test-threads=1`
Expected before implementation: compile failure because `resolve_subagent_timeout_secs` is missing.

- [x] **Step 3: Implement helper and replace duplicated clamps**

```rust
pub fn resolve_subagent_timeout_secs(
    requested_timeout_secs: Option<u64>,
    default_timeout_secs: u64,
) -> u64 {
    crate::tools::clamp_tool_timeout_secs(requested_timeout_secs.unwrap_or(default_timeout_secs))
}
```

- [x] **Step 4: Verify GREEN**

Run: `cargo test --lib tools::subagent::tests -- --test-threads=1`
Expected: all subagent tests pass.

---

### Task 2: Shared Subagent Schema Validation Retry Helper

**Status:** Completed on 2026-07-15.

**Files:**
- Create: `src/tools/subagent/schema_retry.rs`
- Modify: `src/tools/subagent/mod.rs`
- Modify: `src/tools/subagent/delegate_task.rs`
- Modify: `src/tools/subagent/delegate_profile.rs`
- Test: `src/tools/subagent/tests.rs`

**Interfaces:**
- Produces: `SchemaRetryDecision` enum with `Accepted(String)` and `Retry { prompt: String, reason: String }`.
- Produces: `evaluate_schema_retry(output: &str, schema: &serde_json::Value, attempt: usize, max_attempts: usize) -> anyhow::Result<SchemaRetryDecision>`.
- Consumes: `evaluator_optimizer::validate_schema`.

- [x] **Step 1: Write failing unit tests**

Add tests that verify:
- valid JSON matching schema returns `Accepted(clean_json)`;
- invalid JSON returns `Retry` before max attempts;
- invalid JSON returns `Err` at max attempts;
- schema mismatch returns `Retry` before max attempts.

Run: `cargo test --lib schema_retry -- --test-threads=1`
Expected: FAIL because module/helper does not exist.

- [x] **Step 2: Implement `schema_retry.rs`**

Create a pure helper that strips markdown fences, parses JSON, validates schema, and builds the retry prompt. Keep it independent from `AgentLoop` so it can be unit-tested without a provider.

- [x] **Step 3: Replace copied retry parsing logic**

In `delegate_task.rs` and `delegate_profile.rs`, replace only the duplicated parse/validate decision logic with the helper. Keep the existing actual retry execution in each file for now.

- [x] **Step 4: Verify**

Run:
- `cargo test --lib tools::subagent::tests -- --test-threads=1`
- `cargo check`
- `cargo fmt --check`

---

### Task 3: Timeout Lifecycle Status Duration

**Status:** Completed on 2026-07-15.

**Files:**
- Modify: `src/tools/subagent/lifecycle.rs`
- Modify: `src/tools/subagent/tests.rs`
- Modify: `src/tools/subagent/delegate_task.rs`
- Modify: `src/tools/subagent/delegate_profile.rs`
- Modify: `src/tools/subagent/parallel_research.rs`

**Interfaces:**
- Changes: `SubagentRunStatus::TimedOut` to carry `duration_secs: Option<u64>` or adds a new constructor/helper that preserves backwards-compatible labels.

- [x] **Step 1: Write failing lifecycle tests**

Add a test that classifies `Subagent execution timed out after 900s` and expects the status label to include `timed out after 900s` or metadata to contain `durationSecs: 900`.

- [x] **Step 2: Implement duration parsing/status**

Parse timeout duration from known timeout strings in `classify_subagent_error`, but keep fallback behavior for old strings.

- [x] **Step 3: Wire status JSON**

Ensure status JSON and compact lifecycle output can surface duration without breaking existing TUI stable-label tests.

- [x] **Step 4: Verify**

Run:
- `cargo test --lib tools::subagent::tests -- --test-threads=1`
- `cargo check`
- `cargo fmt --check`

---

### Task 4: Provider Missing API Key Diagnostics

**Status:** Completed on 2026-07-15.

**Files:**
- Modify: `src/providers/resolver.rs`
- Test: provider resolver tests in `src/providers/resolver.rs`

**Interfaces:**
- Preserve `ollama` no-key behavior.
- For cloud providers, return actionable errors when the selected provider lacks a key and no fallback is configured.

- [x] **Step 1: Write failing tests**

Add tests for missing `openai`/`anthropic` key error text containing the provider name, env var name, and `openz configure`.

- [x] **Step 2: Implement actionable errors**

Update resolver error construction at the final selected-provider path, avoiding false positives for Ollama/local providers.

- [x] **Step 3: Verify fallback behavior**

Run provider resolver tests and ensure existing fallback tests still pass.

---

### Task 5: Session Lock Stale/Backoff Hardening

**Status:** Completed on 2026-07-15.

**Files:**
- Modify: `src/session.rs`
- Test: session tests in `src/session.rs`

**Interfaces:**
- Add stale lock threshold constant, likely `SESSION_LOCK_STALE_SECS: u64 = 60`.
- Add bounded backoff in async lock acquisition.

- [x] **Step 1: Write tests for stale lock cleanup**

Create a stale `.lock` file in a temp session dir and verify lock acquisition removes or ignores it safely.

- [x] **Step 2: Write tests for non-stale lock preservation**

Create a fresh `.lock` file and verify code does not delete it blindly.

- [x] **Step 3: Implement stale check and backoff**

Use file metadata modified time and exponential backoff capped at a small duration.

- [x] **Step 4: Verify**

Run:
- `cargo test --lib session -- --test-threads=1`
- `cargo check`
- `cargo fmt --check`

---

### Task 6: Tool Metadata Decentralization First Pass

**Status:** Completed on 2026-07-15.

**Files:**
- Modify: `src/tools/mod.rs`
- Modify targeted high-value tools first: subagent, browser/media, shell.
- Test: `src/cli/tools.rs` and `src/tools/resource_policy.rs` tests.

**Interfaces:**
- Keep default `Tool::metadata()` inference for untouched tools.
- Add explicit metadata overrides only where the current match bloat caused real drift: timeout hints, process/network flags, domain.

- [x] **Step 1: Write tests for explicit metadata override**

Create or use a mock tool with custom metadata and verify registry/export honors it.

- [ ] **Step 2: Add helper constructors/builders for metadata**

Avoid macros for now; use simple Rust constructors to keep this low-risk.

- [ ] **Step 3: Move timeout metadata for subagent/media/browser tools**

Override metadata on those tool structs and reduce `tool_recommended_timeout()` reliance where safe.

- [ ] **Step 4: Verify**

Run:
- `cargo test --lib cli::tools tools::resource_policy -- --test-threads=1`
- `cargo check`
- `cargo fmt --check`

---

## Local Backlog Policy

`recommendedfix.md` remains local until all actionable items are either implemented or converted into polished public documentation. Do not include it in release commits by default.
