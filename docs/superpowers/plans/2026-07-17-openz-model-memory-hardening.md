# OpenZ Model and Memory Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make OpenZ reliable across weak and strong models by validating model switches, separating reasoning from visible answers, injecting pinned identity/session context, and reducing noisy self-improvement races.

**Architecture:** Add a small model reliability layer around existing provider resolution instead of hardcoding a larger picker. Normalize provider outputs before `AgentLoop` display logic. Improve prompt construction with deterministic context blocks for pinned identity facts, recent session summary, and weak-model operating rules. Make self-improvement retries quiet and debounce curator work while the same session is still active.

**Tech Stack:** Rust, Tokio, serde, rusqlite, existing OpenZ provider/channel/session/memory modules.

## Global Constraints

- Preserve existing provider routing behavior in `src/providers/resolver.rs`.
- Keep weak/small model support; warn and harden, do not ban.
- Do not expose raw `<think>...</think>` blocks as final answers.
- Do not make memory retrieval depend only on the model deciding to call a tool.
- Keep TUI `/model` and channel `/switch-model` behavior consistent.
- Tests must be narrow and runnable on low-resource machines.

---

## Research Notes

- Claude Code uses persistent instruction files plus auto memory; startup context includes a concise memory entrypoint, while detailed topic files are read on demand. This supports OpenZ adding a small pinned memory block rather than dumping all memory every turn.
- Cursor Rules and Memories are injected at the start of model context, confirming that weak-model reliability improves when key rules/preferences are deterministic prompt context, not optional tool calls.
- Continue defines agents as model + rules + tools + context providers; it also supports separate model roles. This supports OpenZ adding model capability tiers and selecting stronger defaults for planning/research/tool-heavy work.
- Anthropic extended thinking treats thinking as a separate content block before text. OpenZ should mirror that separation even when OpenAI-compatible providers leak `<think>` tags into normal content.

Sources:
- https://code.claude.com/docs/en/memory
- https://docs.cursor.com/context/rules
- https://docs.continue.dev/customize/rules
- https://docs.continue.dev/customize/custom-providers
- https://platform.claude.com/docs/en/docs/build-with-claude/extended-thinking
- https://openai.github.io/openai-agents-python/agents/

---

## File Structure

- Modify `src/providers/openai.rs`: parse `<think>` blocks from non-streaming OpenAI-compatible responses.
- Modify `src/providers/mod.rs`: optionally expose shared reasoning normalization helpers if streaming and non-streaming need them.
- Modify `src/agent/agent_loop/streaming.rs`: route streamed `<think>` chunks into reasoning output instead of visible content.
- Modify `src/agent/agent_loop/run.rs`: use normalized reasoning/content and prevent reasoning-only fallback from leaking hidden thought tags.
- Modify `src/agent/agent_loop/build.rs`: add pinned identity/persona memory retrieval, recent session summary, and weak-model operating rules.
- Modify `src/agent/agent_loop/save.rs`: debounce/quiet concurrent curator aborts.
- Modify `src/channels/mod.rs`: add shared model switch validation API, model risk labels, and health result formatting.
- Modify `src/channels/cli/mod.rs`: replace duplicated TUI `/model` catalog flow with shared model switch code and validation.
- Modify `src/channels/telegram.rs`, `src/channels/discord.rs`, `src/channels/whatsapp.rs`, `src/channels/email.rs`, `src/channels/websocket.rs` if present: surface model warnings consistently.
- Create `src/model_registry.rs`: persisted model health/capability registry under `~/.openz/model_registry.json`.
- Add focused tests near modified modules.

---

## Task 1: Normalize Reasoning Tags

**Files:**
- Modify: `src/providers/openai.rs`
- Modify: `src/agent/agent_loop/streaming.rs`
- Modify: `src/agent/agent_loop/run.rs`

**Interfaces:**
- Produces: `split_think_blocks(content: &str) -> (Option<String>, Option<String>)`
- Consumes: existing `LLMResponse { content, reasoning_content, tool_calls, finish_reason }`

- [ ] **Step 1: Add failing tests**

Add tests covering:

```rust
#[test]
fn strips_single_think_block_from_visible_content() {
    let (content, reasoning) = split_think_blocks("<think>private</think>\n\nfinal");
    assert_eq!(content.as_deref(), Some("final"));
    assert_eq!(reasoning.as_deref(), Some("private"));
}

#[test]
fn strips_multiple_think_blocks_and_preserves_answer() {
    let (content, reasoning) = split_think_blocks("a <think>one</think> b <think>two</think> c");
    assert_eq!(content.as_deref(), Some("a b c"));
    assert_eq!(reasoning.as_deref(), Some("one\n\n---\n\ntwo"));
}
```

- [ ] **Step 2: Run the targeted provider tests**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib split_think -- --test-threads=1
```

Expected: FAIL because helper does not exist.

- [ ] **Step 3: Implement non-streaming normalization**

After parsing `choice.message.content`, split think blocks. Merge extracted reasoning with `choice.message.reasoning_content`. Only pass cleaned content to fallback tool-call parsing.

- [ ] **Step 4: Implement streaming normalization**

Track whether the stream is inside `<think>` tags. Emit those chunks as `ChatStreamChunk::Reasoning` and normal text as `ChatStreamChunk::Content`.

- [ ] **Step 5: Verify**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib split_think -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --lib openai -- --test-threads=1
```

Expected: PASS. No visible final content contains `<think>`.

---

## Task 2: Pinned Identity and Persona Context

**Files:**
- Modify: `src/agent/agent_loop/build.rs`

**Interfaces:**
- Produces: `retrieve_pinned_identity_memories(user_content: &str) -> String`
- Consumes: `cognitive_memory`, `semantic_metadata`, and `graph_nodes`

- [ ] **Step 1: Add failing tests**

Add tests for these cases:

```rust
#[tokio::test]
async fn identity_question_gets_identity_memory_even_without_keyword_overlap() {
    store_test_memory("Aswin's name is Aswin", 0.9).await;
    let prompt_block = retrieve_cross_session_memories("what is my name").await;
    assert!(prompt_block.contains("Aswin"));
}

#[tokio::test]
async fn persona_question_gets_agent_identity_memory() {
    store_test_memory("The assistant's name is Mivi", 0.9).await;
    let prompt_block = retrieve_cross_session_memories("what is your name").await;
    assert!(prompt_block.contains("Mivi"));
}
```

- [ ] **Step 2: Run targeted tests**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib identity_question -- --test-threads=1
```

Expected: FAIL with current keyword-overlap retrieval.

- [ ] **Step 3: Add deterministic identity trigger**

Detect identity/persona queries with normalized phrases:

- `my name`
- `who am i`
- `what do you know about me`
- `your name`
- `who are you`
- `persona`
- `remember about me`

When triggered, include top high-importance memories containing identity/persona/preference terms even if keyword overlap is zero.

- [ ] **Step 4: Add always-on pinned memory block**

Every prompt gets a tiny block limited to about 1,500 characters:

```text
[Pinned Memory]
- User identity/preference facts with importance >= 0.85
- Assistant persona facts with importance >= 0.85
```

This block must be deduplicated and placed before the larger cross-session memory block.

- [ ] **Step 5: Verify**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib retrieve_cross_session_memories -- --test-threads=1
```

Expected: PASS. Identity facts are available before the model has to call memory tools.

---

## Task 3: Recent Session Summary for Weak Models

**Files:**
- Modify: `src/agent/agent_loop/build.rs`
- Modify: `src/session.rs` only if a helper belongs there

**Interfaces:**
- Produces: `recent_session_context(messages: &[Message], max_chars: usize) -> String`
- Consumes: current session messages already loaded into `ctx.session.messages`

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn recent_session_context_prioritizes_latest_user_assistant_turns() {
    let messages = vec![
        user("old topic"),
        assistant("old answer"),
        user("we are testing model switching"),
        assistant("listed weak model failures"),
    ];
    let block = recent_session_context(&messages, 500);
    assert!(block.contains("testing model switching"));
    assert!(block.contains("weak model failures"));
}
```

- [ ] **Step 2: Run targeted test**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib recent_session_context -- --test-threads=1
```

Expected: FAIL because helper does not exist.

- [ ] **Step 3: Implement a deterministic recent context block**

Add a short block near the top of the prompt:

```text
[Recent Session Context]
The latest turns in this current session:
- User: ...
- Assistant: ...
```

Limit it to the last 8 non-tool messages and about 2,000 characters.

- [ ] **Step 4: Add weak-model operating rules**

When the active model is known weak, unknown, free-tier, or under about 14B parameters, add:

```text
[Small Model Operating Rules]
- Use Recent Session Context before saying you do not know what was discussed.
- For identity/persona/preference questions, use Pinned Memory first.
- For tool-heavy tasks, ask for a stronger model or delegate to configured fallback.
- For large tasks, create research -> implementation plan -> todo list before editing.
```

- [ ] **Step 5: Verify**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib recent_session_context -- --test-threads=1
```

Expected: PASS.

---

## Task 4: Model Validation and Health Registry

**Files:**
- Create: `src/model_registry.rs`
- Modify: `src/channels/mod.rs`
- Modify: `src/channels/cli/mod.rs`
- Modify: channel command handlers for `/switch-model`

**Interfaces:**
- Produces:
  - `ModelRisk { tier, reasons, requires_confirmation }`
  - `validate_model_selection(config, provider, model) -> ModelValidationReport`
  - `ModelRegistry::load/save/upsert_health`

- [ ] **Step 1: Add registry tests**

```rust
#[test]
fn unknown_free_model_is_marked_risky() {
    let risk = classify_model_risk("opencode_zen", "big-pickle", None);
    assert!(risk.requires_confirmation);
    assert!(risk.reasons.iter().any(|r| r.contains("unknown")));
}

#[test]
fn known_strong_model_is_not_risky() {
    let risk = classify_model_risk("opencode_zen", "deepseek-v4-flash-free", None);
    assert!(!risk.requires_confirmation);
}
```

- [ ] **Step 2: Run tests**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib model_risk -- --test-threads=1
```

Expected: FAIL.

- [ ] **Step 3: Implement classification**

Initial heuristic:

- Risky if model was typed manually.
- Risky if provider model fetch returned it but it is not in OpenZ curated catalog.
- Risky if name contains `free`, `preview`, `experimental`, `beta`, `hy`, `pickle`, `mimo`, or unknown vendor aliases.
- Weak if name contains `1b`, `2b`, `3b`, `6b`, `7b`, `8b`, `9b`, `small`, `mini`, `flash-lite`.
- Strong if known curated model or observed health has recent successful tool/chat validation.

- [ ] **Step 4: Add smoke test mode**

On model switch, run a cheap validation request:

```text
Reply with exactly: OPENZ_MODEL_OK
```

The switch result should record:

- success or failure
- blank response
- raw `<think>` leak
- fallback used
- latency
- provider error text category, not secrets

For risky models, show warning and require confirmation in TUI. For remote channels, require `/switch-model confirm <provider> <model>`.

- [ ] **Step 5: Unify TUI and channel model switch**

Remove the local `ProviderModels` copy in `src/channels/cli/mod.rs`. Use `provider_model_catalog()` and shared validation.

- [ ] **Step 6: Verify**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib channels:: -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --lib model_registry -- --test-threads=1
```

Expected: PASS.

---

## Task 5: Quiet and Debounced Curator

**Files:**
- Modify: `src/agent/agent_loop/save.rs`

**Interfaces:**
- Produces: `should_run_curator(session_key: &str, now: DateTime<Utc>) -> bool`

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn curator_debounces_fast_repeated_turns() {
    let first = should_run_curator_for_test("s1", 0);
    let second = should_run_curator_for_test("s1", 5);
    assert!(first);
    assert!(!second);
}
```

- [ ] **Step 2: Run targeted tests**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib curator_debounce -- --test-threads=1
```

Expected: FAIL.

- [ ] **Step 3: Implement debounce**

If the same session had a curator spawn less than 20 seconds ago, skip this spawn. If a session changed concurrently, log at debug level and do not send a CLI notification.

- [ ] **Step 4: Verify**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib curator -- --test-threads=1
```

Expected: PASS and no user-facing concurrency warning for normal rapid chat.

---

## Task 6: Big Task Research and Planning Gate

**Files:**
- Modify: `src/agent/agent_loop/build.rs`
- Consider create: `src/agent/task_intent.rs`

**Interfaces:**
- Produces: `classify_task_intent(user_content: &str) -> TaskIntent`

- [ ] **Step 1: Add tests**

```rust
#[test]
fn implementation_request_is_large_task() {
    assert_eq!(classify_task_intent("research this and make implementation plan then todo list"), TaskIntent::LargeResearchPlan);
}

#[test]
fn greeting_is_simple_chat() {
    assert_eq!(classify_task_intent("hey"), TaskIntent::SimpleChat);
}
```

- [ ] **Step 2: Implement rules**

Large tasks include requests containing combinations of:

- `research`
- `implement`
- `fix`
- `architecture`
- `plan`
- `todo`
- `push`
- `release`
- `client`
- `workflow`

For large tasks, inject a deterministic rule:

```text
[Large Task Rule]
For broad implementation/design tasks: first inspect/research, then produce an implementation plan and todo list before editing. Do not jump directly into changes unless user explicitly says "implement now".
```

- [ ] **Step 3: Verify**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib task_intent -- --test-threads=1
```

Expected: PASS.

---

## Task 7: End-to-End Regression Script

**Files:**
- Create: `tests/model_hardening_regression.rs` if integration tests are acceptable, otherwise add unit tests near modules.

**Scenarios:**

- Model output with `<think>` tags never displays raw tags.
- Identity question includes pinned memory before model response.
- Recent-session question includes latest turns.
- Unknown/free model switch requires warning/confirmation.
- Curator rapid-turn conflict does not notify the user.

- [ ] **Step 1: Add tests**
- [ ] **Step 2: Run narrow test suite**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --lib model_hardening -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo check
```

Expected: PASS.

---

## Todo List

- [ ] Normalize `<think>` tags into reasoning channel.
- [ ] Add streaming think-tag state machine.
- [ ] Add pinned identity/persona memory block.
- [ ] Add identity query trigger for cross-session memory retrieval.
- [ ] Add recent session context block.
- [ ] Add weak-model operating rules.
- [ ] Add model risk classification.
- [ ] Add model switch smoke test.
- [ ] Add persisted dynamic model health registry.
- [ ] Unify TUI `/model` with shared channel `/switch-model` catalog.
- [ ] Require confirmation for risky unknown/free models.
- [ ] Quiet expected curator concurrency aborts.
- [ ] Debounce curator on rapid repeated turns.
- [ ] Add large-task research/plan/todo rule.
- [ ] Add regression tests for all reproduced failures.

---

## Recommended Execution Order

1. Task 1 first because it fixes visible reasoning leaks and makes provider output safe.
2. Task 2 and Task 3 next because they improve weak model reliability immediately.
3. Task 5 next because it removes user-facing noise and race warnings.
4. Task 4 after that because model validation touches every channel and needs careful UX.
5. Task 6 last because it changes agent behavior policy.
6. Task 7 closes the loop with regression coverage.

## Self-Review

- Spec coverage: covers all five reported bugs plus dynamic CRUD/model-health and large-task planning behavior.
- Placeholder scan: no TBD or deferred sections.
- Type consistency: proposed helper/function names are consistent across tasks.
- Scope: single hardening epic, split into independently testable tasks.
