# Agent Loop Registry Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor OpenZ's largest execution and registration paths into focused modules, fix known seccomp/session-ledger/version drift issues, and verify the result with cargo checks and targeted tests.

**Architecture:** Keep behavior unchanged while extracting responsibilities behind small Rust modules and functions. The `AgentLoop` turn runner remains the coordinator, while streaming assembly, tool execution, transcript writing, and loop/security decisions become independently testable units. Tool registration moves from one monolithic builder function into domain-specific registration functions with a single full-registration entry point.

**Tech Stack:** Rust 2021, Tokio, anyhow, serde_json, clap, reqwest, fs2, libc/seccomp, existing in-tree test style with `#[test]` and `#[tokio::test]`.

## Global Constraints

- Do not change public CLI subcommands or tool names.
- Do not remove any registered tool unless a test first proves it was unreachable because of API truncation and the new scoping behavior preserves access.
- Preserve existing session file format compatibility; older session JSON files must still load.
- Keep refactors behavior-preserving before adding security/session-ledger fixes.
- Use `cargo check` and targeted `cargo test` commands after each logical slice.
- Do not introduce new external crates unless a task explicitly calls for one; this plan uses only existing dependencies.

---

## File Structure

- Modify: `src/agent/agent_loop/run.rs`
  - Keep `handle()` as orchestration only.
  - Remove inline streaming assembly, loop checks, tool result transcript writing, and tool executor details after they are extracted.
- Create: `src/agent/agent_loop/streaming.rs`
  - Own streaming chunk assembly and partial tool-call reconstruction.
  - Export `StreamingAssembly`, `assemble_stream_response()`, and tests for chunk parsing behavior.
- Create: `src/agent/agent_loop/tool_execution.rs`
  - Own per-tool execution, timeout, cancellation, security approval calls, and user-facing progress messages.
  - Export `ToolExecutionRequest`, `ToolExecutionOutcome`, and `execute_tool_call()`.
- Create: `src/agent/agent_loop/transcript.rs`
  - Own assistant/tool message insertion and output compression.
  - Export `append_assistant_tool_calls()` and `append_tool_results()`.
- Create: `src/agent/agent_loop/loop_control.rs`
  - Own repeated text/tool-call detection and self-healing hint generation.
  - Export `count_previous_text_responses()`, `count_previous_tool_calls()`, `is_repeated_tool_call()`, and `generate_self_healing_hint()`.
- Modify: `src/agent/agent_loop/mod.rs`
  - Add the new modules.
- Create: `src/cli/tools.rs`
  - Own domain-specific tool registration functions.
  - Export `register_all_tools(registry, config, provider, session_manager)`.
- Modify: `src/cli/mod.rs`
  - Add `pub mod tools;`.
- Modify: `src/cli/builder.rs`
  - Replace inline registration list with `tools::register_all_tools(...)`.
  - Keep provider/session construction in builder.
- Modify: `src/tools/mod.rs`
  - Add non-mutating `tool_names()` and `tool_count()` helpers for tests.
  - Keep `to_openai_format()` 128-tool truncation behavior unchanged.
- Modify: `src/tools/shell.rs`
  - Disable seccomp filter on AArch64 until syscall tables are architecture-specific, or split syscall tables by arch.
- Modify: `src/session.rs`
  - Extend message hash calculation to include canonicalized `extra` metadata except `hash`.
- Modify: `README.md`, `CHANGELOG.md`, and any hardcoded displayed version docs found by `rg "0\\.0\\.36|0\\.0\\.39|version"` where relevant.
- Test-only changes: targeted tests in the same modules.

---

### Task 1: Create Agent Loop Extraction Modules Without Behavior Change

**Files:**
- Modify: `src/agent/agent_loop/mod.rs`
- Create: `src/agent/agent_loop/streaming.rs`
- Create: `src/agent/agent_loop/tool_execution.rs`
- Create: `src/agent/agent_loop/transcript.rs`
- Create: `src/agent/agent_loop/loop_control.rs`

**Interfaces:**
- Consumes: existing `Message`, `ToolCallRequest`, `ToolRegistry`, `TurnContext`, style helpers, security helpers.
- Produces:
  - `streaming::StreamingAssembly`
  - `tool_execution::ToolExecutionOutcome`
  - `transcript::append_assistant_tool_calls(messages: &mut Vec<Message>, tool_calls_json: Vec<Value>, reasoning: Option<&str>)`
  - `transcript::append_tool_results(messages: &mut Vec<Message>, config: &Config, results: Vec<ToolExecutionOutcome>)`
  - `loop_control::{count_previous_text_responses, count_previous_tool_calls, generate_self_healing_hint}`

- [ ] **Step 1: Add module declarations**

Add this to `src/agent/agent_loop/mod.rs` next to the existing module list:

```rust
pub mod streaming;
pub mod tool_execution;
pub mod transcript;
pub mod loop_control;
```

- [ ] **Step 2: Create `streaming.rs` with compilation-safe scaffolding**

```rust
use anyhow::Result;
use crate::providers::{ChatStreamChunk, LLMResponse, ToolCallRequest};

#[derive(Debug, Default)]
pub struct StreamingAssembly {
    pub content: String,
    pub reasoning: String,
    pub finish_reason: String,
    partial_tool_calls: std::collections::HashMap<usize, PartialToolCall>,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl StreamingAssembly {
    pub fn new() -> Self {
        Self {
            finish_reason: "stop".to_string(),
            ..Self::default()
        }
    }

    pub fn push_chunk(&mut self, chunk: ChatStreamChunk) {
        match chunk {
            ChatStreamChunk::Content(text) => self.content.push_str(&text),
            ChatStreamChunk::Reasoning(text) => self.reasoning.push_str(&text),
            ChatStreamChunk::ToolCall { index, id, name, arguments } => {
                let entry = self.partial_tool_calls.entry(index).or_default();
                if let Some(id) = id {
                    entry.id = id;
                }
                if let Some(name) = name {
                    entry.name = name;
                }
                if let Some(arguments) = arguments {
                    entry.arguments.push_str(&arguments);
                }
            }
            ChatStreamChunk::Done { finish_reason } => {
                if let Some(reason) = finish_reason {
                    self.finish_reason = reason;
                }
            }
        }
    }

    pub fn into_response(self) -> LLMResponse {
        let mut keys: Vec<_> = self.partial_tool_calls.keys().copied().collect();
        keys.sort_unstable();

        let mut tool_calls = Vec::new();
        for key in keys {
            if let Some(partial) = self.partial_tool_calls.get(&key) {
                let arguments = serde_json::from_str(&partial.arguments).unwrap_or_else(|err| {
                    let repaired = partial.arguments.replace('\n', "\\n").replace('\r', "\\r");
                    serde_json::from_str(&repaired).unwrap_or_else(|_| {
                        serde_json::json!({ "parse_error": err.to_string() })
                    })
                });

                tool_calls.push(ToolCallRequest {
                    id: partial.id.clone(),
                    name: partial.name.clone(),
                    arguments,
                });
            }
        }

        LLMResponse {
            content: if self.content.is_empty() { None } else { Some(self.content) },
            tool_calls,
            finish_reason: self.finish_reason,
            reasoning_content: if self.reasoning.is_empty() { None } else { Some(self.reasoning) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_split_tool_call_arguments_in_index_order() {
        let mut assembly = StreamingAssembly::new();
        assembly.push_chunk(ChatStreamChunk::ToolCall {
            index: 1,
            id: Some("call_b".to_string()),
            name: Some("read_file".to_string()),
            arguments: Some("{\"path\":\"b".to_string()),
        });
        assembly.push_chunk(ChatStreamChunk::ToolCall {
            index: 1,
            id: None,
            name: None,
            arguments: Some(".rs\"}".to_string()),
        });
        assembly.push_chunk(ChatStreamChunk::ToolCall {
            index: 0,
            id: Some("call_a".to_string()),
            name: Some("list_dir".to_string()),
            arguments: Some("{\"path\":\".\"}".to_string()),
        });

        let response = assembly.into_response();
        assert_eq!(response.tool_calls.len(), 2);
        assert_eq!(response.tool_calls[0].id, "call_a");
        assert_eq!(response.tool_calls[1].id, "call_b");
        assert_eq!(response.tool_calls[1].arguments["path"], "b.rs");
    }
}
```

- [ ] **Step 3: Create `loop_control.rs` by moving existing helpers**

Move these existing helper functions from `src/agent/agent_loop/run.rs` into `src/agent/agent_loop/loop_control.rs` unchanged, then make them `pub(crate)`:

```rust
pub(crate) fn count_previous_text_responses(messages: &[crate::session::Message], content: &str) -> usize {
    messages
        .iter()
        .filter(|msg| msg.role == "assistant" && msg.content == content)
        .count()
}

pub(crate) fn count_previous_tool_calls(
    messages: &[crate::session::Message],
    tool_name: &str,
    arguments: &serde_json::Value,
) -> usize {
    let expected_arguments = arguments.to_string();
    messages
        .iter()
        .filter(|msg| msg.role == "assistant")
        .filter_map(|msg| msg.extra.get("tool_calls").and_then(|v| v.as_array()))
        .flatten()
        .filter(|call| {
            let name = call
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str());
            let args = call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|v| v.as_str());
            name == Some(tool_name) && args == Some(expected_arguments.as_str())
        })
        .count()
}
```

Also move the existing `generate_self_healing_hint()` function unchanged and mark it `pub(crate)`.

- [ ] **Step 4: Create `transcript.rs` with assistant/tool append helpers**

```rust
use crate::config::schema::Config;
use crate::session::Message;

#[derive(Debug, Clone)]
pub struct ToolTranscriptResult {
    pub id: String,
    pub name: String,
    pub result: serde_json::Value,
}

pub(crate) fn append_assistant_tool_calls(
    messages: &mut Vec<Message>,
    tool_calls_json: Vec<serde_json::Value>,
    reasoning: Option<&str>,
) {
    let mut extra = serde_json::Map::new();
    extra.insert("tool_calls".to_string(), serde_json::Value::Array(tool_calls_json));
    if let Some(reasoning) = reasoning {
        extra.insert(
            "reasoning_content".to_string(),
            serde_json::Value::String(reasoning.to_string()),
        );
    }

    if let Some(last_msg) = messages.last_mut() {
        if last_msg.role == "assistant" {
            for (key, value) in extra {
                last_msg.extra.insert(key, value);
            }
            return;
        }
    }

    messages.push(Message {
        role: "assistant".to_string(),
        content: String::new(),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra,
    });
}

pub(crate) async fn append_tool_results(
    messages: &mut Vec<Message>,
    config: &Config,
    tool_results: Vec<ToolTranscriptResult>,
) {
    for tool_result in tool_results {
        let mut extra = serde_json::Map::new();
        extra.insert("tool_call_id".to_string(), serde_json::Value::String(tool_result.id));
        extra.insert("name".to_string(), serde_json::Value::String(tool_result.name.clone()));

        let content_str = tool_result.result.to_string();
        let limit = config.agents.defaults.tool_output_limit.unwrap_or(4000);
        let is_retrieve = tool_result.name == "retrieve_original"
            || tool_result.name == "headroom/retrieve_original";

        let content = if content_str.len() > limit && !is_retrieve {
            let outputs_dir = crate::config::resolve_path("~/.openz/tool_outputs");
            if let Err(err) = tokio::fs::create_dir_all(&outputs_dir).await {
                tracing::warn!("Failed to create tool outputs directory '{}': {}", outputs_dir.display(), err);
            }
            let file_name = format!("output_{}_{}.json", tool_result.name, uuid::Uuid::new_v4());
            let file_path = outputs_dir.join(file_name);
            if let Err(err) = tokio::fs::write(&file_path, &content_str).await {
                tracing::warn!("Failed to write tool output file '{}': {}", file_path.display(), err);
            }

            let compressed = crate::agent::context_compactor::compress_tool_output(
                &tool_result.name,
                &content_str,
            );
            format!(
                "{}\n\n... [TRUNCATED - Full output saved for reference at file://{}] ...",
                compressed,
                file_path.to_string_lossy()
            )
        } else {
            content_str
        };

        messages.push(Message {
            role: "tool".to_string(),
            content,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            extra,
        });
    }
}
```

- [ ] **Step 5: Create `tool_execution.rs` with request/result types only**

```rust
#[derive(Debug, Clone)]
pub(crate) struct ToolExecutionOutcome {
    pub id: String,
    pub name: String,
    pub result: serde_json::Value,
    pub assistant_tool_call: serde_json::Value,
    pub should_halt: bool,
}
```

Leave `execute_tool_call()` for Task 3 to avoid changing behavior in this setup task.

- [ ] **Step 6: Run module compile check**

Run:

```bash
cargo check --lib
```

Expected: PASS. If it fails because moved helpers are not yet used, keep functions `pub(crate)` and allow dead code only inside new modules with `#[allow(dead_code)]` for this task.

- [ ] **Step 7: Commit**

```bash
git add src/agent/agent_loop/mod.rs src/agent/agent_loop/streaming.rs src/agent/agent_loop/tool_execution.rs src/agent/agent_loop/transcript.rs src/agent/agent_loop/loop_control.rs
git commit -m "refactor: scaffold agent loop execution modules"
```

---

### Task 2: Extract Streaming Assembly From `run.rs`

**Files:**
- Modify: `src/agent/agent_loop/run.rs`
- Modify: `src/agent/agent_loop/streaming.rs`

**Interfaces:**
- Consumes: `StreamingAssembly::push_chunk()` and `StreamingAssembly::into_response()`.
- Produces: no behavior change; streaming branch still returns `LLMResponse`.

- [ ] **Step 1: Run existing streaming assembly unit test**

Run:

```bash
cargo test --lib agent::agent_loop::streaming::tests::assembles_split_tool_call_arguments_in_index_order
```

Expected: PASS.

- [ ] **Step 2: Replace inline `PartialToolCall` map in `run.rs`**

In `src/agent/agent_loop/run.rs`, inside the `if config.agents.defaults.streaming` branch:

1. Delete the local `struct PartialToolCall`.
2. Delete `let mut partial_tool_calls = std::collections::HashMap::<usize, PartialToolCall>::new();`.
3. Add:

```rust
let mut assembly = super::streaming::StreamingAssembly::new();
```

4. In each stream chunk arm, after existing UI/progress behavior, call:

```rust
assembly.push_chunk(crate::providers::ChatStreamChunk::Content(text));
```

or:

```rust
assembly.push_chunk(crate::providers::ChatStreamChunk::Reasoning(text));
```

For the `ToolCall` arm, replace the manual map update with:

```rust
assembly.push_chunk(crate::providers::ChatStreamChunk::ToolCall {
    index,
    id,
    name,
    arguments,
});
```

For the `Done` arm, replace `finish_reason = r` with:

```rust
assembly.push_chunk(crate::providers::ChatStreamChunk::Done {
    finish_reason: reason,
});
```

- [ ] **Step 3: Build response from assembly**

Replace the local construction of `crate::providers::LLMResponse { ... }` at the end of the streaming branch with:

```rust
let mut assembled = assembly.into_response();
assembled.content = if full_content.is_empty() { None } else { Some(full_content) };
assembled.reasoning_content = if full_reasoning.is_empty() { None } else { Some(full_reasoning) };
assembled
```

This preserves existing UI buffers while moving tool-call parsing into `streaming.rs`.

- [ ] **Step 4: Run targeted tests**

Run:

```bash
cargo test --lib agent::agent_loop::streaming providers::openai::tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/agent/agent_loop/run.rs src/agent/agent_loop/streaming.rs
git commit -m "refactor: extract streaming tool call assembly"
```

---

### Task 3: Extract Transcript Writing and Loop Control

**Files:**
- Modify: `src/agent/agent_loop/run.rs`
- Modify: `src/agent/agent_loop/transcript.rs`
- Modify: `src/agent/agent_loop/loop_control.rs`

**Interfaces:**
- Consumes:
  - `loop_control::count_previous_text_responses`
  - `loop_control::count_previous_tool_calls`
  - `loop_control::generate_self_healing_hint`
  - `transcript::{append_assistant_tool_calls, append_tool_results, ToolTranscriptResult}`
- Produces: `run.rs` no longer owns transcript compression or repeat-count helper definitions.

- [ ] **Step 1: Replace helper calls in `run.rs`**

Replace:

```rust
count_previous_text_responses(&ctx.messages, &content)
count_previous_tool_calls(&ctx.messages, &call.name, &call.arguments)
generate_self_healing_hint(&call.name, &error_str)
```

with:

```rust
super::loop_control::count_previous_text_responses(&ctx.messages, &content)
super::loop_control::count_previous_tool_calls(&ctx.messages, &call.name, &call.arguments)
super::loop_control::generate_self_healing_hint(&call.name, &error_str)
```

- [ ] **Step 2: Replace assistant tool-call transcript block**

Replace the `if let Some(last_msg) = ctx.messages.last_mut() { ... } else { ... }` block that inserts `tool_calls` into an assistant message with:

```rust
super::transcript::append_assistant_tool_calls(
    &mut ctx.messages,
    assistant_tool_calls_json,
    resp.reasoning_content.as_deref(),
);
```

- [ ] **Step 3: Replace tool result transcript loop**

Convert:

```rust
tool_results.push((call.id.clone(), call.name.clone(), result_val));
```

to:

```rust
tool_results.push(super::transcript::ToolTranscriptResult {
    id: call.id.clone(),
    name: call.name.clone(),
    result: result_val,
});
```

Then replace the `for (id, name, result) in tool_results { ... }` compression/message append block with:

```rust
super::transcript::append_tool_results(
    &mut ctx.messages,
    &config,
    tool_results,
).await;
```

- [ ] **Step 4: Delete duplicate helper definitions from `run.rs`**

Delete the old local definitions for:

```rust
fn count_previous_text_responses(...)
fn count_previous_tool_calls(...)
fn generate_self_healing_hint(...)
```

- [ ] **Step 5: Add transcript unit test**

In `src/agent/agent_loop/transcript.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_tool_calls_to_existing_assistant_message() {
        let mut messages = vec![Message {
            role: "assistant".to_string(),
            content: "thinking".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        }];

        append_assistant_tool_calls(
            &mut messages,
            vec![serde_json::json!({
                "id": "call_1",
                "type": "function",
                "function": { "name": "read_file", "arguments": "{\"path\":\"Cargo.toml\"}" }
            })],
            Some("reasoning"),
        );

        assert_eq!(messages.len(), 1);
        assert!(messages[0].extra.get("tool_calls").is_some());
        assert_eq!(messages[0].extra["reasoning_content"], "reasoning");
    }
}
```

- [ ] **Step 6: Run targeted tests**

Run:

```bash
cargo test --lib agent::agent_loop::transcript agent::agent_loop::build
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/agent/agent_loop/run.rs src/agent/agent_loop/transcript.rs src/agent/agent_loop/loop_control.rs
git commit -m "refactor: extract agent transcript and loop control"
```

---

### Task 4: Extract Tool Execution and Security Approval

**Files:**
- Modify: `src/agent/agent_loop/run.rs`
- Modify: `src/agent/agent_loop/tool_execution.rs`

**Interfaces:**
- Consumes: `AgentLoop`, `TurnContext`, `ToolCallRequest`, cancellation token, formatted tool args.
- Produces:
  - `execute_tool_call(loop_ref, ctx, call, config, turn_cancel_clone, loop_blocked_count, is_loop, forbidden, approved)` returns `ToolExecutionOutcome`.

- [ ] **Step 1: Move `format_tool_args()` into `tool_execution.rs`**

Move the existing function from `run.rs` into `tool_execution.rs` and expose:

```rust
pub(crate) fn format_tool_args(name: &str, args: &serde_json::Value) -> String {
    // Move existing match/body unchanged from run.rs.
}
```

Update `run.rs` call sites:

```rust
let formatted_args = super::tool_execution::format_tool_args(&call.name, &call.arguments);
```

- [ ] **Step 2: Create execution request struct**

Add to `tool_execution.rs`:

```rust
pub(crate) struct ToolExecutionRequest<'a> {
    pub call: crate::providers::ToolCallRequest,
    pub formatted_args: String,
    pub parse_error: Option<String>,
    pub is_loop: bool,
    pub forbidden: bool,
    pub approved: bool,
    pub security_mode: &'a str,
}
```

- [ ] **Step 3: Move result-producing branch**

Move this exact branch from `run.rs` into a new function:

```rust
pub(crate) async fn execute_tool_call(
    loop_ref: &super::AgentLoop,
    ctx: &mut super::TurnContext<'_>,
    request: ToolExecutionRequest<'_>,
    turn_cancel: crate::tools::subagent::CancellationToken,
) -> ToolExecutionOutcome {
    // Move existing result_val construction branches:
    // parse_error, is_loop, forbidden, !approved, Some(tool), None.
    // Preserve send_progress_update, spinner printing, tracing, timeout, and cancellation behavior.
}
```

The function must return:

```rust
ToolExecutionOutcome {
    id: request.call.id.clone(),
    name: request.call.name.clone(),
    result: result_val,
    assistant_tool_call: serde_json::json!({
        "id": request.call.id,
        "type": "function",
        "function": {
            "name": request.call.name,
            "arguments": request.call.arguments.to_string()
        }
    }),
    should_halt: false,
}
```

For the repeated-loop branch where `loop_blocked_count >= 3` was previously handled outside, leave `should_halt` false; `run.rs` still owns global loop halt state.

- [ ] **Step 4: Simplify per-call loop in `run.rs`**

After security decisions, replace the `let result_val = if ...` block with:

```rust
let outcome = super::tool_execution::execute_tool_call(
    loop_ref,
    ctx,
    super::tool_execution::ToolExecutionRequest {
        call,
        formatted_args,
        parse_error: parse_error.map(str::to_string),
        is_loop,
        forbidden,
        approved,
        security_mode,
    },
    turn_cancel_clone.clone(),
).await;

if let Some(err_val) = outcome.result.get("error").and_then(|v| v.as_str()) {
    ctx.turn_errors.push(format!("Tool {} returned error: {}", outcome.name, err_val));
}
crate::agent::activity::update_activity(ctx.session_key, "Processing user prompt", None);
assistant_tool_calls_json.push(outcome.assistant_tool_call.clone());
tool_results.push(super::transcript::ToolTranscriptResult {
    id: outcome.id,
    name: outcome.name,
    result: outcome.result,
});
```

- [ ] **Step 5: Run tool/security tests**

Run:

```bash
cargo test --lib agent::security tools::shell tools::filesystem
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/agent/agent_loop/run.rs src/agent/agent_loop/tool_execution.rs
git commit -m "refactor: extract agent tool execution"
```

---

### Task 5: Split Tool Registration by Domain

**Files:**
- Create: `src/cli/tools.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/builder.rs`
- Modify: `src/tools/mod.rs`

**Interfaces:**
- Consumes: `Config`, `Arc<dyn LLMProvider>`, `SessionManager`, `ToolRegistry`.
- Produces:
  - `cli::tools::register_all_tools(registry, config, provider, session_manager)`
  - `ToolRegistry::tool_names() -> Vec<String>`
  - `ToolRegistry::tool_count() -> usize`

- [ ] **Step 1: Add registry inspection helpers**

In `src/tools/mod.rs`, add:

```rust
pub fn tool_names(&self) -> Vec<String> {
    let mut names: Vec<String> = self.read_tools().keys().cloned().collect();
    names.sort();
    names
}

pub fn tool_count(&self) -> usize {
    self.read_tools().len()
}
```

- [ ] **Step 2: Add `cli::tools` module**

In `src/cli/mod.rs`, add:

```rust
pub mod tools;
```

- [ ] **Step 3: Create registration entry point**

Create `src/cli/tools.rs`:

```rust
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::Arc;

pub fn register_all_tools(
    registry: &ToolRegistry,
    config: &Config,
    provider: Arc<dyn LLMProvider>,
    session_manager: SessionManager,
) -> Result<()> {
    register_core_tools(registry, config, provider.clone(), session_manager.clone());
    register_orchestration_tools(registry, config, provider.clone(), session_manager.clone());
    register_memory_tools(registry);
    register_search_tools(registry);
    register_media_tools(registry);
    register_document_tools(registry);
    Ok(())
}

fn register_core_tools(
    registry: &ToolRegistry,
    _config: &Config,
    provider: Arc<dyn LLMProvider>,
    _session_manager: SessionManager,
) {
    registry.register(Arc::new(crate::tools::filesystem::ReadFileTool));
    registry.register(Arc::new(crate::tools::filesystem::FindFilesTool));
    registry.register(Arc::new(crate::tools::filesystem::WriteFileTool));
    registry.register(Arc::new(crate::tools::filesystem::PatchFileTool));
    registry.register(Arc::new(crate::tools::filesystem::ReplaceLinesTool));
    registry.register(Arc::new(crate::tools::filesystem::ListDirTool));
    registry.register(Arc::new(crate::tools::filesystem::ZenflowEditTool { provider }));
    registry.register(Arc::new(crate::tools::shell::ExecCommandTool));
    registry.register(Arc::new(crate::tools::shell::PythonSandboxTool));
    registry.register(Arc::new(crate::tools::grep::GrepSearchTool));
    registry.register(Arc::new(crate::tools::git_manager::GitManagerTool));
    registry.register(Arc::new(crate::tools::cargo_manager::CargoManagerTool::new(provider.clone())));
    registry.register(Arc::new(crate::tools::outline::CodeOutlineTool));
    registry.register(Arc::new(crate::tools::ast_grep::AstGrepTool));
    registry.register(Arc::new(crate::tools::ast_grep::AstGrepIndexCodebaseTool));
    registry.register(Arc::new(crate::tools::db_inspector::DbInspectorTool));
    registry.register(Arc::new(crate::tools::db_inspector::DbWriteTool));
    registry.register(Arc::new(crate::tools::doc_reader::DocReaderTool));
    registry.register(Arc::new(crate::tools::wasm_sandbox::WasmSandboxTool));
    registry.register(Arc::new(crate::tools::js_format::JsFormatTool));
    registry.register(Arc::new(crate::tools::semantic_search::SemanticSearchTool));
    registry.register(Arc::new(crate::tools::rust_docs::RustDocsTool::new()));
    registry.register(Arc::new(crate::tools::clipboard::ClipboardTool));
    registry.register(Arc::new(crate::tools::open::OpenTool));
    registry.register(Arc::new(crate::tools::watcher::FileWatcherTool));
    registry.register(Arc::new(crate::tools::system_info::SystemInfoTool));
    registry.register(Arc::new(crate::tools::network::CheckPortTool));
}
```

Then move the remaining existing registration lines from `builder.rs` into the other domain functions. Do not rename any tools.

- [ ] **Step 4: Replace builder registration block**

In `src/cli/builder.rs`, after:

```rust
let registry = ToolRegistry::new_with_context(config.clone(), provider.clone(), session_manager.clone());
```

replace all inline `registry.register(...)` calls with:

```rust
crate::cli::tools::register_all_tools(
    &registry,
    &config,
    provider.clone(),
    session_manager.clone(),
)?;
```

- [ ] **Step 5: Add registration tests**

In `src/cli/tools.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_all_tools_includes_expected_domains_without_duplicates() {
        let registry = ToolRegistry::new();
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));

        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let names = registry.tool_names();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "duplicate tool names must overwrite only intentionally");
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"exec_command".to_string()));
        assert!(names.contains(&"delegate_task".to_string()));
        assert!(names.contains(&"sequentialthinking".to_string()));
        assert!(names.contains(&"scope_context".to_string()));
        assert!(names.contains(&"create_entities".to_string()));
        assert!(names.contains(&"searchxyz_search_web".to_string()));
        assert!(names.contains(&"openmedia_ping".to_string()));
        assert!(names.contains(&"opendoc_open_document".to_string()));
        assert!(registry.tool_count() > 128, "full registry should expose more tools than one OpenAI payload can carry");
    }

    #[test]
    fn openai_format_is_sorted_and_truncated_to_api_limit() {
        let registry = ToolRegistry::new();
        for i in (0..140).rev() {
            registry.register(Arc::new(TestTool(format!("tool_{i:03}"))));
        }

        let tools = registry.to_openai_format();
        assert_eq!(tools.len(), 128);
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert_eq!(names.first().unwrap(), "tool_000");
        assert_eq!(names.last().unwrap(), "tool_127");
    }

    struct TestTool(String);

    #[async_trait::async_trait]
    impl crate::tools::Tool for TestTool {
        fn name(&self) -> &str { &self.0 }
        fn description(&self) -> &str { "test" }
        fn parameters(&self) -> serde_json::Value { serde_json::json!({ "type": "object" }) }
        async fn call(&self, _arguments: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({ "ok": true }))
        }
    }
}
```

- [ ] **Step 6: Run registration tests**

Run:

```bash
cargo test --lib cli::tools cli::builder::tests::test_native_tool_registration_names
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/cli/tools.rs src/cli/mod.rs src/cli/builder.rs src/tools/mod.rs
git commit -m "refactor: split native tool registration by domain"
```

---

### Task 6: Fix AArch64 Seccomp Handling

**Files:**
- Modify: `src/tools/shell.rs`

**Interfaces:**
- Consumes: current `sandbox_command()` and `allowlist_seccomp_filter()`.
- Produces: AArch64 no longer installs an x86_64 syscall-number filter.

- [ ] **Step 1: Add architecture-specific support gate**

In `src/tools/shell.rs`, replace the architecture comments and constants near `allowlist_seccomp_filter()` with this behavior:

```rust
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn allowlist_seccomp_filter() -> Result<(), std::io::Error> {
    const AUDIT_ARCH_X86_64: u32 = 0xC000003E;
    const EXPECTED_ARCH: u32 = AUDIT_ARCH_X86_64;
    const ALLOWED_SYSCALLS: &[u32] = &[
        // keep existing x86_64 syscall list unchanged
    ];
    install_seccomp_filter(EXPECTED_ARCH, ALLOWED_SYSCALLS)
}

#[cfg(all(target_os = "linux", not(target_arch = "x86_64")))]
unsafe fn allowlist_seccomp_filter() -> Result<(), std::io::Error> {
    tracing::warn!(
        "seccomp sandbox disabled on this CPU architecture until architecture-specific syscall tables are implemented"
    );
    Ok(())
}
```

Extract the BPF installation body into:

```rust
#[cfg(target_os = "linux")]
unsafe fn install_seccomp_filter(
    expected_arch: u32,
    allowed_syscalls: &[u32],
) -> Result<(), std::io::Error> {
    // Move existing BPF construction body here.
    // Use expected_arch instead of AUDIT_ARCH_X86_64/AUDIT_ARCH_AARCH64 dual checks.
}
```

- [ ] **Step 2: Add unit test for architecture message**

Add test:

```rust
#[cfg(all(test, target_os = "linux", not(target_arch = "x86_64")))]
#[test]
fn non_x86_64_seccomp_filter_is_noop() {
    let result = unsafe { allowlist_seccomp_filter() };
    assert!(result.is_ok());
}
```

- [ ] **Step 3: Run shell tests**

Run:

```bash
cargo test --lib tools::shell
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tools/shell.rs
git commit -m "fix: disable seccomp syscall filter on unsupported architectures"
```

---

### Task 7: Extend Session Hashing to Canonicalized Extra Metadata

**Files:**
- Modify: `src/session.rs`

**Interfaces:**
- Consumes: existing `Message.extra`.
- Produces:
  - `Session::calculate_message_hash(role, content, timestamp, extra, prev_hash)`
  - backward-compatible verification for sessions whose stored hash used the old text-only scheme.

- [ ] **Step 1: Add canonical extra serializer**

In `src/session.rs`, add:

```rust
fn canonical_extra_without_hash(extra: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut filtered = serde_json::Map::new();
    let mut keys: Vec<_> = extra.keys().filter(|key| key.as_str() != "hash").collect();
    keys.sort();
    for key in keys {
        if let Some(value) = extra.get(key) {
            filtered.insert(key.clone(), value.clone());
        }
    }
    serde_json::to_string(&serde_json::Value::Object(filtered)).unwrap_or_else(|_| "{}".to_string())
}
```

- [ ] **Step 2: Update hash calculation signature**

Replace:

```rust
pub fn calculate_message_hash(role: &str, content: &str, timestamp: Option<&str>, prev_hash: &str) -> String
```

with:

```rust
pub fn calculate_message_hash(
    role: &str,
    content: &str,
    timestamp: Option<&str>,
    extra: &serde_json::Map<String, serde_json::Value>,
    prev_hash: &str,
) -> String
```

Inside the function, add before `prev_hash`:

```rust
let canonical_extra = canonical_extra_without_hash(extra);
hasher.update(canonical_extra.as_bytes());
```

- [ ] **Step 3: Preserve old session compatibility**

In `verify_hash_chain()`, compute both hashes:

```rust
let calculated = Self::calculate_message_hash(&msg.role, &msg.content, ts_ref, &msg.extra, &prev_hash);
let legacy = {
    let mut hasher = Sha256::new();
    hasher.update(msg.role.as_bytes());
    hasher.update(msg.content.as_bytes());
    if let Some(ts) = ts_ref {
        hasher.update(ts.as_bytes());
    }
    hasher.update(prev_hash.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect::<String>()
};
```

Accept `stored == calculated || stored == legacy`. Set `prev_hash = stored.to_string()` if legacy matched, otherwise `prev_hash = calculated`. This lets old sessions load and new saves rewrite hashes with metadata included.

- [ ] **Step 4: Add tamper test**

Add:

```rust
#[cfg(test)]
mod hash_tests {
    use super::*;

    #[test]
    fn hash_changes_when_extra_metadata_changes() {
        let mut extra = serde_json::Map::new();
        extra.insert("tool_call_id".to_string(), serde_json::json!("call_1"));

        let h1 = Session::calculate_message_hash("tool", "{}", Some("2026-07-06T00:00:00Z"), &extra, "");
        extra.insert("tool_call_id".to_string(), serde_json::json!("call_2"));
        let h2 = Session::calculate_message_hash("tool", "{}", Some("2026-07-06T00:00:00Z"), &extra, "");

        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_ignores_hash_field_itself() {
        let mut extra = serde_json::Map::new();
        extra.insert("name".to_string(), serde_json::json!("read_file"));
        let h1 = Session::calculate_message_hash("tool", "{}", None, &extra, "");

        extra.insert("hash".to_string(), serde_json::json!("old"));
        let h2 = Session::calculate_message_hash("tool", "{}", None, &extra, "");

        assert_eq!(h1, h2);
    }
}
```

- [ ] **Step 5: Run session tests**

Run:

```bash
cargo test --lib session
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/session.rs
git commit -m "fix: include message metadata in session hash chain"
```

---

### Task 8: Sync Versions in README and Changelog

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Inspect: `Cargo.toml`

**Interfaces:**
- Consumes: package version from `Cargo.toml`.
- Produces: docs match package version `0.0.39`.

- [ ] **Step 1: Confirm package version**

Run:

```bash
sed -n '1,8p' Cargo.toml
```

Expected: line contains `version = "0.0.39"`.

- [ ] **Step 2: Find stale version strings**

Run:

```bash
rg -n "v0\\.0\\.36|0\\.0\\.36|v0\\.0\\.39|0\\.0\\.39" README.md CHANGELOG.md docs onpkg_docs src Cargo.toml
```

Expected: identify all places that intentionally or accidentally mention versions.

- [ ] **Step 3: Update README title**

Change:

```markdown
# OpenZ 🦊 `v0.0.36`
```

to:

```markdown
# OpenZ 🦊 `v0.0.39`
```

- [ ] **Step 4: Ensure CHANGELOG has v0.0.39 entry**

If `CHANGELOG.md` lacks `### v0.0.39`, add this at the top of the release history section:

```markdown
### v0.0.39

- Refactored agent execution internals into focused streaming, transcript, loop-control, and tool-execution modules.
- Split native tool registration by domain and added registry/truncation tests.
- Disabled seccomp syscall filtering on unsupported CPU architectures until architecture-specific syscall tables are available.
- Extended session hash-chain coverage to include canonicalized message metadata.
- Synchronized project documentation version references.
```

- [ ] **Step 5: Run docs search again**

Run:

```bash
rg -n "v0\\.0\\.36|0\\.0\\.36" README.md CHANGELOG.md docs onpkg_docs src
```

Expected: no stale `0.0.36` references unless they are historical changelog entries under older releases.

- [ ] **Step 6: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: sync OpenZ version references"
```

---

### Task 9: Final Verification

**Files:**
- No planned source edits unless verification exposes failures.

**Interfaces:**
- Consumes: completed Tasks 1-8.
- Produces: passing checks or a concrete failure list.

- [ ] **Step 1: Run cargo check**

Run:

```bash
cargo check
```

Expected: PASS.

- [ ] **Step 2: Run tool registration test**

Run:

```bash
cargo test --lib cli::builder::tests::test_native_tool_registration_names
```

Expected: PASS.

- [ ] **Step 3: Run new CLI tools tests**

Run:

```bash
cargo test --lib cli::tools
```

Expected: PASS.

- [ ] **Step 4: Run security tests**

Run:

```bash
cargo test --lib agent::security tools::shell
```

Expected: PASS.

- [ ] **Step 5: Run provider resolver tests**

Run:

```bash
cargo test --lib providers::resolver
```

Expected: PASS.

- [ ] **Step 6: Run session hash tests**

Run:

```bash
cargo test --lib session
```

Expected: PASS.

- [ ] **Step 7: Run formatting**

Run:

```bash
cargo fmt
```

Expected: command exits successfully and only intended Rust files are formatted.

- [ ] **Step 8: Inspect final diff**

Run:

```bash
git diff --stat
git diff -- src/agent/agent_loop src/cli src/tools/mod.rs src/tools/shell.rs src/session.rs README.md CHANGELOG.md
```

Expected: diff only covers planned files.

- [ ] **Step 9: Commit verification fixes if needed**

If verification required any small fixes:

```bash
git add src README.md CHANGELOG.md
git commit -m "test: verify agent loop registry hardening"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review

**Spec coverage:**
- Split `agent_loop/run.rs`: Tasks 1-4.
- Split `cli/builder.rs` registration and add count/order/truncation tests: Task 5.
- Fix or disable AArch64 seccomp: Task 6.
- Extend session hashing to canonicalized extra metadata: Task 7.
- Sync README/changelog/package versions: Task 8.
- Run `cargo check` and targeted tests: Task 9.

**Placeholder scan:** No `TBD`, `TODO`, or "implement later" placeholders are required for execution. The only "move existing body unchanged" instructions identify exact source files and functions to move.

**Type consistency:** New module function names and structs are defined before use. `ToolTranscriptResult` and `ToolExecutionOutcome` are distinct: transcript receives only the result fields it needs, while tool execution can carry assistant-call JSON for the runner.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-06-agent-loop-registry-hardening.md`. Two execution options:

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints.

