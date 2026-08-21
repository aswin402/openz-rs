# OpenZ Layered Tool Scope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a balanced L0-L3 agent control layer so OpenZ exposes only the tools each turn needs, forces live research only when appropriate, and keeps orchestration/subagents powerful without bloating every prompt.

**Architecture:** Keep the existing `ToolRegistry`, `ToolMetadata`, `research_policy`, and `orchestrator` modules as the foundation. Add deterministic intent classification, scoped tool packs, dynamic scope expansion, and observability around the existing `to_openai_format_for_prompt` path instead of replacing the agent loop.

**Tech Stack:** Rust 2021, serde/serde_json, existing native tool registry, existing `justfile` commands, focused `cargo test -p openz --lib <test> -j 2` through `just test-lib-one` or `just test-one`.

## Global Constraints

- Do not run full `cargo check`, full build, or full test suite unless the user explicitly approves it.
- Use `just check openz` only when a broader package check is needed.
- Use `just test-one <test_name> openz` or `just test-lib-one <test_name> openz` for focused tests.
- Keep routing deterministic first; do not add an LLM classifier in this phase.
- Existing `SecurityGuard` remains final enforcement for destructive or privileged actions.
- MCP/tool annotations are hints for routing, not permission enforcement.
- Default behavior must be backward-compatible behind config values.
- Target visible tool payload: 8-20 tools for normal turns; hard cap stays below provider tool limits.

---

## File Structure

- Modify: `src/config/schema.rs`
  - Add config for layered tool routing: enabled flag, max visible tools, always-visible core tools, and per-intent pack sizes.
- Create: `src/agent/agent_loop/intent.rs`
  - Deterministic L0 classifier. Produces `TurnIntent` and `KnowledgePolicy`.
- Create: `src/tools/scope.rs`
  - L1 `ToolScopeEngine`, `ToolPack`, `ToolScopeDecision`, and ranking rules.
- Modify: `src/tools/mod.rs`
  - Wire `ToolScopeEngine` into `ToolRegistry::route_for_prompt()` and `to_openai_format_for_prompt()`.
- Modify: `src/tools/self_management.rs`
  - Extend `tool_catalog` / `optimize_tool_scope` style output with scope decisions.
  - Add `request_tool_scope` as the model-facing escape hatch when the first scoped pack is insufficient.
- Modify: `src/agent/agent_loop/run.rs`
  - Pass user prompt into tool scoping, emit concise scope debug/progress status when configured.
- Modify: `src/agent/agent_loop/build.rs`
  - Add prompt guidance for L0-L3 behavior: answer directly for stable/simple facts, research current/external facts, request more tools when needed.
- Modify: `src/orchestrator/runtime.rs`
  - Add L3 guardrails to prevent planner/reviewer smoke tests from nested-delegating or researching trivial steps unless the step explicitly says so.
- Modify: `src/tools/subagent/delegate_task.rs` and/or `src/tools/subagent/delegate_profile.rs`
  - Respect scoped child tool policies and depth limits for nested delegation.
- Test mostly in:
  - `src/agent/agent_loop/intent.rs`
  - `src/tools/mod.rs`
  - `src/tools/scope.rs`
  - `src/tools/self_management.rs`
  - `src/orchestrator/runtime.rs`

---

### Task 1: Add L0 Turn Intent Classifier

**Files:**
- Create: `src/agent/agent_loop/intent.rs`
- Modify: `src/agent/agent_loop/mod.rs`
- Test: `src/agent/agent_loop/intent.rs`

**Interfaces:**
- Produces:
  - `pub enum TurnIntent`
  - `pub enum KnowledgePolicy`
  - `pub struct IntentDecision`
  - `pub fn classify_turn_intent(user_content: &str) -> IntentDecision`
- Consumes:
  - `super::research_policy::{has_live_research_intent, text_has_http_url}`

- [ ] **Step 1: Write failing classifier tests**

Add this module in the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_repo_question_prefers_repo_read_not_web() {
        let decision = classify_turn_intent("Where is orchestrate_workflow implemented in this repo?");
        assert_eq!(decision.intent, TurnIntent::LocalRepoRead);
        assert_eq!(decision.knowledge_policy, KnowledgePolicy::UseLocalContext);
        assert!(decision.reasons.contains(&"local_repo_query"));
    }

    #[test]
    fn latest_external_question_requires_live_research() {
        let decision = classify_turn_intent("What is the latest Rust stable version today?");
        assert_eq!(decision.intent, TurnIntent::ExternalResearch);
        assert_eq!(decision.knowledge_policy, KnowledgePolicy::RequireLiveResearch);
        assert!(decision.reasons.contains(&"live_research_intent"));
    }

    #[test]
    fn simple_general_task_uses_model_directly() {
        let decision = classify_turn_intent("summarize hello");
        assert_eq!(decision.intent, TurnIntent::DirectAnswer);
        assert_eq!(decision.knowledge_policy, KnowledgePolicy::ModelOk);
    }

    #[test]
    fn build_request_prefers_local_execution_tools() {
        let decision = classify_turn_intent("run the focused cargo test for workflow_spec_round_trips_from_json");
        assert_eq!(decision.intent, TurnIntent::LocalExecution);
        assert_eq!(decision.knowledge_policy, KnowledgePolicy::UseLocalContext);
    }
}
```

- [ ] **Step 2: Run the failing test**

Run: `just test-lib-one local_repo_question_prefers_repo_read_not_web openz`

Expected: compile failure because `agent_loop::intent` does not exist.

- [ ] **Step 3: Implement the classifier**

Create `src/agent/agent_loop/intent.rs` with:

```rust
use super::research_policy::{has_live_research_intent, text_has_http_url};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnIntent {
    DirectAnswer,
    LocalRepoRead,
    LocalExecution,
    ExternalResearch,
    MemoryLookup,
    CronManagement,
    Orchestration,
    MediaOrDocument,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgePolicy {
    ModelOk,
    UseLocalContext,
    RequireLiveResearch,
    UseMemoryOrSkillFirst,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentDecision {
    pub intent: TurnIntent,
    pub knowledge_policy: KnowledgePolicy,
    pub reasons: Vec<&'static str>,
}

pub fn classify_turn_intent(user_content: &str) -> IntentDecision {
    let lower = user_content.to_lowercase();
    let mut reasons = Vec::new();

    if has_live_research_intent(user_content) && !is_local_repo_query(&lower) {
        reasons.push("live_research_intent");
        return IntentDecision {
            intent: TurnIntent::ExternalResearch,
            knowledge_policy: KnowledgePolicy::RequireLiveResearch,
            reasons,
        };
    }

    if is_orchestration_query(&lower) {
        reasons.push("orchestration_query");
        return IntentDecision {
            intent: TurnIntent::Orchestration,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if is_cron_query(&lower) {
        reasons.push("cron_query");
        return IntentDecision {
            intent: TurnIntent::CronManagement,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if is_local_execution_query(&lower) {
        reasons.push("local_execution_query");
        return IntentDecision {
            intent: TurnIntent::LocalExecution,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if is_local_repo_query(&lower) {
        reasons.push("local_repo_query");
        return IntentDecision {
            intent: TurnIntent::LocalRepoRead,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if is_memory_query(&lower) {
        reasons.push("memory_query");
        return IntentDecision {
            intent: TurnIntent::MemoryLookup,
            knowledge_policy: KnowledgePolicy::UseMemoryOrSkillFirst,
            reasons,
        };
    }

    if is_media_or_document_query(&lower) || text_has_http_url(user_content) {
        reasons.push("media_or_document_query");
        return IntentDecision {
            intent: TurnIntent::MediaOrDocument,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    reasons.push("direct_answer_default");
    IntentDecision {
        intent: TurnIntent::DirectAnswer,
        knowledge_policy: KnowledgePolicy::ModelOk,
        reasons,
    }
}

fn is_local_repo_query(lower: &str) -> bool {
    [
        "this repo",
        "this codebase",
        "implemented",
        "where is",
        "which file",
        "src/",
        "cargo.toml",
        "function",
        "struct",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_local_execution_query(lower: &str) -> bool {
    [
        "cargo ",
        "just ",
        "test ",
        "check ",
        "compile",
        "build",
        "run command",
        "terminal",
        "shell",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_orchestration_query(lower: &str) -> bool {
    [
        "orchestrate_workflow",
        "orchestrate workflow",
        "multi-step workflow",
        "planner",
        "reviewer",
        "subagent workflow",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_cron_query(lower: &str) -> bool {
    ["cron", "cornjob", "schedule_job", "scheduled job", "job logs"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn is_memory_query(lower: &str) -> bool {
    ["memory", "skill", "saved link", "remember", "my name"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn is_media_or_document_query(lower: &str) -> bool {
    ["image", "screenshot", "pdf", "docx", "video", "audio", "ocr"]
        .iter()
        .any(|needle| lower.contains(needle))
}
```

Update `src/agent/agent_loop/mod.rs`:

```rust
pub mod intent;
```

- [ ] **Step 4: Run focused classifier tests**

Run: `just test-lib-one local_repo_question_prefers_repo_read_not_web openz`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/agent/agent_loop/mod.rs src/agent/agent_loop/intent.rs
git commit -m "feat: classify turn intent"
```

---

### Task 2: Add L1 Tool Packs and Scope Engine

**Files:**
- Create: `src/tools/scope.rs`
- Modify: `src/tools/mod.rs`
- Test: `src/tools/scope.rs`

**Interfaces:**
- Consumes:
  - `crate::agent::agent_loop::intent::{IntentDecision, TurnIntent}`
  - `crate::tools::{ToolMetadata, ToolRisk}`
- Produces:
  - `pub enum ToolPack`
  - `pub struct ToolScopeDecision`
  - `pub struct ToolScopeEngine`
  - `pub fn pack_for_intent(intent: TurnIntent) -> &'static [ToolPack]`
  - `pub fn tool_matches_pack(name: &str, metadata: &ToolMetadata, packs: &[ToolPack]) -> bool`

- [ ] **Step 1: Write failing scope tests**

Add these tests to `src/tools/scope.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::agent_loop::intent::{IntentDecision, KnowledgePolicy, TurnIntent};

    #[test]
    fn local_repo_read_pack_includes_grep_and_read_not_web() {
        let decision = IntentDecision {
            intent: TurnIntent::LocalRepoRead,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons: vec!["local_repo_query"],
        };
        let scope = ToolScopeEngine::default().decide(&decision, 12);
        assert!(scope.allowed_names.contains(&"grep_search"));
        assert!(scope.allowed_names.contains(&"read_file"));
        assert!(!scope.allowed_names.contains(&"web_fetch"));
    }

    #[test]
    fn external_research_pack_includes_web_and_source_tools() {
        let decision = IntentDecision {
            intent: TurnIntent::ExternalResearch,
            knowledge_policy: KnowledgePolicy::RequireLiveResearch,
            reasons: vec!["live_research_intent"],
        };
        let scope = ToolScopeEngine::default().decide(&decision, 12);
        assert!(scope.allowed_names.contains(&"web_fetch"));
        assert!(scope.allowed_names.contains(&"web_search"));
        assert!(scope.allowed_names.contains(&"retrieve_original"));
    }

    #[test]
    fn direct_answer_keeps_only_escape_hatch_and_inventory() {
        let decision = IntentDecision {
            intent: TurnIntent::DirectAnswer,
            knowledge_policy: KnowledgePolicy::ModelOk,
            reasons: vec!["direct_answer_default"],
        };
        let scope = ToolScopeEngine::default().decide(&decision, 12);
        assert!(scope.allowed_names.contains(&"request_tool_scope"));
        assert!(scope.allowed_names.contains(&"openz_inventory"));
        assert!(!scope.allowed_names.contains(&"exec_command"));
    }
}
```

- [ ] **Step 2: Run the failing test**

Run: `just test-lib-one local_repo_read_pack_includes_grep_and_read_not_web openz`

Expected: compile failure because `src/tools/scope.rs` is not wired.

- [ ] **Step 3: Implement tool packs**

Create `src/tools/scope.rs`:

```rust
use crate::agent::agent_loop::intent::{IntentDecision, TurnIntent};
use crate::tools::ToolMetadata;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolPack {
    Core,
    RepoRead,
    RepoWrite,
    LocalExec,
    WebResearch,
    Memory,
    Cron,
    Subagent,
    Orchestrator,
    Media,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolScopeDecision {
    pub packs: Vec<ToolPack>,
    pub allowed_names: BTreeSet<&'static str>,
    pub max_visible_tools: usize,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct ToolScopeEngine {
    core_tools: &'static [&'static str],
}

impl Default for ToolScopeEngine {
    fn default() -> Self {
        Self {
            core_tools: &[
                "request_tool_scope",
                "tool_catalog",
                "openz_inventory",
                "retrieve_original",
            ],
        }
    }
}

impl ToolScopeEngine {
    pub fn decide(&self, decision: &IntentDecision, max_visible_tools: usize) -> ToolScopeDecision {
        let mut allowed_names: BTreeSet<&'static str> = self.core_tools.iter().copied().collect();
        let packs = pack_for_intent(decision.intent).to_vec();
        for pack in &packs {
            for name in static_names_for_pack(*pack) {
                allowed_names.insert(name);
            }
        }

        ToolScopeDecision {
            packs,
            allowed_names,
            max_visible_tools,
            reasons: decision.reasons.clone(),
        }
    }
}

pub fn pack_for_intent(intent: TurnIntent) -> &'static [ToolPack] {
    match intent {
        TurnIntent::DirectAnswer => &[ToolPack::Core],
        TurnIntent::LocalRepoRead => &[ToolPack::Core, ToolPack::RepoRead],
        TurnIntent::LocalExecution => &[ToolPack::Core, ToolPack::RepoRead, ToolPack::LocalExec],
        TurnIntent::ExternalResearch => &[ToolPack::Core, ToolPack::WebResearch],
        TurnIntent::MemoryLookup => &[ToolPack::Core, ToolPack::Memory],
        TurnIntent::CronManagement => &[ToolPack::Core, ToolPack::Cron],
        TurnIntent::Orchestration => &[ToolPack::Core, ToolPack::Orchestrator, ToolPack::Subagent],
        TurnIntent::MediaOrDocument => &[ToolPack::Core, ToolPack::Media, ToolPack::RepoRead],
        TurnIntent::Unknown => &[ToolPack::Core, ToolPack::RepoRead],
    }
}

pub fn tool_matches_pack(name: &str, metadata: &ToolMetadata, packs: &[ToolPack]) -> bool {
    packs.iter().any(|pack| match pack {
        ToolPack::Core => matches!(name, "request_tool_scope" | "tool_catalog" | "openz_inventory" | "retrieve_original"),
        ToolPack::RepoRead => matches!(metadata.domain, "filesystem" | "code" | "git") && !metadata.writes_disk,
        ToolPack::RepoWrite => matches!(metadata.domain, "filesystem" | "git") && metadata.writes_disk,
        ToolPack::LocalExec => matches!(metadata.domain, "shell" | "code") || name.starts_with("cargo_"),
        ToolPack::WebResearch => matches!(metadata.domain, "web" | "search") || name.contains("browser"),
        ToolPack::Memory => matches!(metadata.domain, "memory" | "context" | "reasoning"),
        ToolPack::Cron => metadata.domain == "cron",
        ToolPack::Subagent => metadata.domain == "subagent",
        ToolPack::Orchestrator => name == "orchestrate_workflow",
        ToolPack::Media => matches!(metadata.domain, "media" | "document"),
    })
}

fn static_names_for_pack(pack: ToolPack) -> &'static [&'static str] {
    match pack {
        ToolPack::Core => &["request_tool_scope", "tool_catalog", "openz_inventory", "retrieve_original"],
        ToolPack::RepoRead => &["list_dir", "read_file", "grep_search", "code_outline", "git_status", "git_diff"],
        ToolPack::RepoWrite => &["write_file", "apply_patch", "replace_lines", "git_add", "git_commit"],
        ToolPack::LocalExec => &["exec_command", "cargo_check", "cargo_test"],
        ToolPack::WebResearch => &["web_search", "web_fetch", "crawl_website", "searchxyz_search", "searchxyz_read_url"],
        ToolPack::Memory => &["proactive_recall", "search_memory", "read_graph", "open_nodes", "smart_store"],
        ToolPack::Cron => &["list_jobs", "get_job", "get_job_logs", "pause_job", "resume_job", "run_job_now", "remove_job"],
        ToolPack::Subagent => &["delegate_task", "parallel_research", "evaluator_optimizer_loop"],
        ToolPack::Orchestrator => &["orchestrate_workflow"],
        ToolPack::Media => &["read_document", "generate_image", "html_to_video", "openmedia_probe", "opendoc_ocr_document"],
    }
}
```

Update `src/tools/mod.rs`:

```rust
pub mod scope;
```

- [ ] **Step 4: Run focused scope tests**

Run: `just test-lib-one local_repo_read_pack_includes_grep_and_read_not_web openz`

Expected: PASS after names are adjusted to actual registered tool names if needed.

- [ ] **Step 5: Commit**

```bash
git add src/tools/mod.rs src/tools/scope.rs
git commit -m "feat: add tool scope packs"
```

---

### Task 3: Integrate Scoped Routing Into ToolRegistry

**Files:**
- Modify: `src/tools/mod.rs:1459-1630`
- Test: `src/tools/mod.rs`

**Interfaces:**
- Consumes:
  - `crate::agent::agent_loop::intent::classify_turn_intent`
  - `crate::tools::scope::{ToolScopeEngine, tool_matches_pack}`
- Produces:
  - Existing `ToolRegistry::route_for_prompt(&self, prompt: &str) -> ToolRouteAnalysis` remains public.
  - Existing `ToolRegistry::to_openai_format_for_prompt(&self, prompt: &str)` remains public.

- [ ] **Step 1: Write failing routing tests**

Add tests in `src/tools/mod.rs` tests module:

```rust
#[test]
fn simple_prompt_does_not_expose_heavy_execution_tools() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "exec_command",
        domain: "shell",
        priority: 90,
    }));
    registry.register(Arc::new(CacheTestTool {
        name: "openz_inventory",
        domain: "self_management",
        priority: 85,
    }));
    registry.register(Arc::new(CacheTestTool {
        name: "request_tool_scope",
        domain: "self_management",
        priority: 100,
    }));

    let exposed_names: Vec<String> = registry
        .to_openai_format_for_prompt("summarize hello")
        .into_iter()
        .filter_map(|value| value["function"]["name"].as_str().map(str::to_string))
        .collect();

    assert!(exposed_names.contains(&"openz_inventory".to_string()));
    assert!(exposed_names.contains(&"request_tool_scope".to_string()));
    assert!(!exposed_names.contains(&"exec_command".to_string()));
}

#[test]
fn repo_prompt_exposes_repo_read_tools() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "grep_search",
        domain: "code",
        priority: 90,
    }));
    registry.register(Arc::new(CacheTestTool {
        name: "web_fetch",
        domain: "web",
        priority: 75,
    }));

    let exposed_names: Vec<String> = registry
        .to_openai_format_for_prompt("Where is orchestrate_workflow implemented in this repo?")
        .into_iter()
        .filter_map(|value| value["function"]["name"].as_str().map(str::to_string))
        .collect();

    assert!(exposed_names.contains(&"grep_search".to_string()));
    assert!(!exposed_names.contains(&"web_fetch".to_string()));
}
```

- [ ] **Step 2: Run first failing routing test**

Run: `just test-lib-one simple_prompt_does_not_expose_heavy_execution_tools openz`

Expected: FAIL because current router still ranks many tools by broad domain/priority.

- [ ] **Step 3: Modify `route_for_prompt` selection**

In `src/tools/mod.rs`, inside `ToolRegistry::route_for_prompt`, compute intent and scope before scoring:

```rust
let intent = crate::agent::agent_loop::intent::classify_turn_intent(prompt);
let scope = crate::tools::scope::ToolScopeEngine::default().decide(&intent, 20);
```

When building each `ToolRouteEntry`, add scope filtering before final `selected_count`:

```rust
let in_scope =
    scope.allowed_names.contains(tool.name())
        || crate::tools::scope::tool_matches_pack(tool.name(), &metadata, &scope.packs);
let selected_score = if in_scope {
    tool_selection_score(tool.name(), &metadata, &selected_domains_set).saturating_add(100)
} else {
    tool_selection_score(tool.name(), &metadata, &selected_domains_set)
};
```

Replace the broad static limit:

```rust
let static_limit = scope
    .max_visible_tools
    .saturating_sub(reserved_subagents.min(scope.max_visible_tools));
```

When marking entries exposed:

```rust
let mut exposed = 0usize;
for entry in entries.iter_mut() {
    let in_scope =
        scope.allowed_names.contains(entry.name.as_str())
            || crate::tools::scope::tool_matches_pack(&entry.name, &entry.metadata, &scope.packs);
    if in_scope && exposed < static_limit {
        entry.exposed_to_model = true;
        exposed += 1;
    } else {
        entry.hidden_reason = Some(if in_scope { "scope_limit" } else { "out_of_scope" });
    }
}
let selected_count = exposed;
let dropped_count = entries.len().saturating_sub(selected_count);
```

- [ ] **Step 4: Run routing tests**

Run:

```bash
just test-lib-one simple_prompt_does_not_expose_heavy_execution_tools openz
just test-lib-one repo_prompt_exposes_repo_read_tools openz
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/mod.rs
git commit -m "feat: scope tools by turn intent"
```

---

### Task 4: Add `request_tool_scope` Escape Hatch

**Files:**
- Modify: `src/tools/self_management.rs`
- Modify: `src/tools/mod.rs`
- Test: `src/tools/self_management.rs`

**Interfaces:**
- Produces:
  - `RequestToolScopeTool`
  - Tool name: `request_tool_scope`
  - Args: `{ "reason": string, "needed_domains"?: string[], "needed_tools"?: string[] }`
  - Output: `{ "status": "scope_request_recorded", "reason": ..., "needed_domains": [...], "needed_tools": [...] }`

- [ ] **Step 1: Write failing self-management test**

Add:

```rust
#[tokio::test]
async fn request_tool_scope_returns_structured_scope_request() {
    let tool = RequestToolScopeTool;
    let result = tool
        .call(serde_json::json!({
            "reason": "need to inspect repository files",
            "needed_domains": ["code", "filesystem"],
            "needed_tools": ["grep_search", "read_file"]
        }))
        .await
        .unwrap();

    assert_eq!(result["status"], "scope_request_recorded");
    assert_eq!(result["needed_domains"][0], "code");
    assert_eq!(result["needed_tools"][0], "grep_search");
}
```

- [ ] **Step 2: Run failing test**

Run: `just test-lib-one request_tool_scope_returns_structured_scope_request openz`

Expected: compile failure because `RequestToolScopeTool` does not exist.

- [ ] **Step 3: Implement tool**

Add to `src/tools/self_management.rs`:

```rust
pub struct RequestToolScopeTool;

#[async_trait::async_trait]
impl crate::tools::Tool for RequestToolScopeTool {
    fn name(&self) -> &str {
        "request_tool_scope"
    }

    fn description(&self) -> &str {
        "Request additional tool domains or specific tools when the current scoped tool set is insufficient."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Why the current tool scope is insufficient."
                },
                "needed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tool domains needed for the next turn."
                },
                "needed_tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional exact tool names needed for the next turn."
                }
            },
            "required": ["reason"]
        })
    }

    async fn call(&self, arguments: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let reason = arguments
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("no reason provided")
            .to_string();
        let needed_domains = arguments
            .get("needed_domains")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let needed_tools = arguments
            .get("needed_tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(serde_json::json!({
            "status": "scope_request_recorded",
            "reason": reason,
            "needed_domains": needed_domains,
            "needed_tools": needed_tools,
            "message": "The next model turn can widen tool scope if the agent loop enables scope expansion."
        }))
    }
}
```

Register it in `ToolRegistry` construction path where self-management tools are registered.

- [ ] **Step 4: Run focused test**

Run: `just test-lib-one request_tool_scope_returns_structured_scope_request openz`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/self_management.rs src/tools/mod.rs
git commit -m "feat: add tool scope request tool"
```

---

### Task 5: Update Agent Prompt for Balanced Knowledge Policy

**Files:**
- Modify: `src/agent/agent_loop/build.rs`
- Test: existing prompt build tests if present, otherwise add a unit around the prompt helper function closest to the instruction block.

**Interfaces:**
- Consumes: no new Rust types required.
- Produces: system prompt rules for:
  - Direct answer for stable/simple tasks.
  - Web only for current/external/explicit research.
  - Local repo tools for local repo questions.
  - `request_tool_scope` when a needed tool is hidden.

- [ ] **Step 1: Add prompt text near existing tool/research guidance**

Use this exact wording:

```text
Layered tool use:
- L0 classify the turn: direct answer, local repo, local execution, web research, memory, cron, media/document, or orchestration.
- Complete the request directly when the answer is stable, trivial, or already available in local context.
- Do not delegate or research for trivial/general-knowledge tasks.
- Use web/search/browser tools only when the user asks for research, provides an external URL, asks for latest/current/recent info, or when accuracy depends on live external facts.
- Use local repo tools for questions about this repository, current directory, implementation locations, diffs, tests, or files.
- If a required tool is not visible, call request_tool_scope with the exact missing tool or domain instead of guessing.
```

- [ ] **Step 2: Add or update a test**

If a prompt-building test exists, assert:

```rust
assert!(prompt.contains("Do not delegate or research for trivial/general-knowledge tasks."));
assert!(prompt.contains("If a required tool is not visible, call request_tool_scope"));
```

- [ ] **Step 3: Run focused prompt test**

Run the exact test name after locating it with:

```bash
rg -n "system prompt|build.*prompt|Do not delegate" src/agent/agent_loop
```

Then run: `just test-lib-one <prompt_test_name> openz`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/agent/agent_loop/build.rs
git commit -m "docs: guide layered tool use"
```

---

### Task 6: Add L3 Orchestrator Simple-Task Guardrails

**Files:**
- Modify: `src/orchestrator/runtime.rs`
- Test: `src/orchestrator/runtime.rs`

**Interfaces:**
- Consumes:
  - `WorkflowStep.goal`
  - existing `build_step_prompt(...)`
- Produces:
  - Step prompts that say:
    - “Complete the step directly when possible.”
    - “Do not delegate or research for trivial/general-knowledge tasks.”
    - “Use web only if required by the step.”

- [ ] **Step 1: Write failing runtime prompt test**

Add:

```rust
#[test]
fn step_prompt_discourages_research_for_trivial_steps() {
    let prompt = build_step_prompt_for_test(
        "planner",
        "Summarize hello",
        &[],
    );
    assert!(prompt.contains("Complete the step directly when possible."));
    assert!(prompt.contains("Do not delegate or research for trivial/general-knowledge tasks."));
    assert!(prompt.contains("Use web only if required by the step."));
}
```

If `build_step_prompt` is private, add:

```rust
#[cfg(test)]
fn build_step_prompt_for_test(agent: &str, goal: &str, prior_outputs: &[StepRunResult]) -> String {
    build_step_prompt(agent, goal, prior_outputs)
}
```

- [ ] **Step 2: Run failing test**

Run: `just test-lib-one step_prompt_discourages_research_for_trivial_steps openz`

Expected: FAIL because current prompt lacks the exact guidance.

- [ ] **Step 3: Update step prompt**

In the existing step prompt builder, append:

```rust
prompt.push_str(
    "\nExecution policy:\n\
     - Complete the step directly when possible.\n\
     - Do not delegate or research for trivial/general-knowledge tasks.\n\
     - Use web only if required by the step.\n\
     - Delegate only when the step explicitly asks for delegation or the work clearly needs another specialist.\n",
);
```

- [ ] **Step 4: Run focused runtime test**

Run: `just test-lib-one step_prompt_discourages_research_for_trivial_steps openz`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestrator/runtime.rs
git commit -m "fix: keep trivial workflow steps direct"
```

---

### Task 7: Suppress Evolution Saves for Smoke-Test Subagents

**Files:**
- Modify: `src/tools/subagent/delegate_profile.rs`
- Modify: `src/tools/subagent/delegate_task.rs`
- Test: `src/tools/subagent/tests.rs`

**Interfaces:**
- Produces:
  - `pub fn should_skip_evolution_capture(goal: &str, output: &str) -> bool`
- Behavior:
  - Skip skill saves when goal/output is very short.
  - Skip skill saves for common smoke prompts: hello, planner/reviewer smoke tests, tool-location probes.

- [ ] **Step 1: Write failing tests**

Add to `src/tools/subagent/tests.rs`:

```rust
#[test]
fn skips_evolution_for_short_smoke_test_outputs() {
    assert!(crate::tools::subagent::should_skip_evolution_capture(
        "Summarize hello",
        "\"Hello\" means greeting."
    ));
}

#[test]
fn does_not_skip_evolution_for_substantial_new_skill_output() {
    let output = "A reliable code review workflow should inspect diffs, map risk areas, run focused tests, and report file-line findings with severity.";
    assert!(!crate::tools::subagent::should_skip_evolution_capture(
        "Design a reusable review workflow for Rust services",
        output
    ));
}
```

- [ ] **Step 2: Run failing test**

Run: `just test-lib-one skips_evolution_for_short_smoke_test_outputs openz`

Expected: compile failure because the helper does not exist.

- [ ] **Step 3: Implement helper**

In `src/tools/subagent/mod.rs`:

```rust
pub fn should_skip_evolution_capture(goal: &str, output: &str) -> bool {
    let goal_lower = goal.to_lowercase();
    let output_words = output.split_whitespace().count();
    let smoke_goal = [
        "summarize hello",
        "review planner output",
        "where is orchestrate_workflow implemented",
        "simple two-step workflow",
    ]
    .iter()
    .any(|needle| goal_lower.contains(needle));

    smoke_goal || output_words < 24
}
```

Wrap existing evolution capture calls:

```rust
if !crate::tools::subagent::should_skip_evolution_capture(&goal, &output) {
    // existing evolution skill extraction call
}
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
just test-lib-one skips_evolution_for_short_smoke_test_outputs openz
just test-lib-one does_not_skip_evolution_for_substantial_new_skill_output openz
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/subagent/mod.rs src/tools/subagent/delegate_profile.rs src/tools/subagent/delegate_task.rs src/tools/subagent/tests.rs
git commit -m "fix: skip evolution capture for smoke tasks"
```

---

### Task 8: Cap Nested Delegation Unless Explicit

**Files:**
- Modify: `src/tools/subagent/delegate_task.rs`
- Modify: `src/tools/subagent/delegate_profile.rs`
- Test: `src/tools/subagent/tests.rs`

**Interfaces:**
- Produces:
  - `pub fn step_allows_nested_delegation(goal: &str) -> bool`
- Behavior:
  - Allow nested delegation when prompt contains “delegate”, “subagent”, “parallel”, “specialist”, “orchestrate”, or workflow mode requires it.
  - Block nested delegation for simple planner/reviewer/direct steps.

- [ ] **Step 1: Write failing tests**

Add:

```rust
#[test]
fn simple_step_does_not_allow_nested_delegation() {
    assert!(!crate::tools::subagent::step_allows_nested_delegation("Summarize hello"));
}

#[test]
fn explicit_specialist_step_allows_nested_delegation() {
    assert!(crate::tools::subagent::step_allows_nested_delegation(
        "Delegate research to a specialist and summarize findings"
    ));
}
```

- [ ] **Step 2: Run failing test**

Run: `just test-lib-one simple_step_does_not_allow_nested_delegation openz`

Expected: compile failure.

- [ ] **Step 3: Implement helper and apply it to child registry**

In `src/tools/subagent/mod.rs`:

```rust
pub fn step_allows_nested_delegation(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    ["delegate", "subagent", "parallel", "specialist", "orchestrate", "workflow"]
        .iter()
        .any(|needle| lower.contains(needle))
}
```

Where child tool registries are built, exclude `delegate_task`, `parallel_research`, `evaluator_optimizer_loop`, and `orchestrate_workflow` unless `step_allows_nested_delegation(goal)` returns true.

- [ ] **Step 4: Run focused tests**

Run:

```bash
just test-lib-one simple_step_does_not_allow_nested_delegation openz
just test-lib-one explicit_specialist_step_allows_nested_delegation openz
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/subagent/mod.rs src/tools/subagent/delegate_task.rs src/tools/subagent/delegate_profile.rs src/tools/subagent/tests.rs
git commit -m "fix: cap nested delegation for simple subagent tasks"
```

---

### Task 9: Add Scope Observability for TUI/WebUI

**Files:**
- Modify: `src/tools/mod.rs`
- Modify: `src/agent/agent_loop/run.rs`
- Modify: `src/channels/websocket.rs` if the WebUI needs structured scope events.
- Test: `src/tools/mod.rs`

**Interfaces:**
- Produces:
  - `ToolRouteAnalysis.selected_domains`
  - `ToolRouteAnalysis.selected_count`
  - `ToolRouteAnalysis.dropped_count`
  - Hidden reasons: `out_of_scope`, `scope_limit`, `active_policy`, `api_limit`

- [ ] **Step 1: Add status-line test**

```rust
#[test]
fn tool_router_status_line_reports_scope() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "grep_search",
        domain: "code",
        priority: 90,
    }));

    let line = registry.tool_router_status_line("Where is this implemented in repo?");
    assert!(line.contains("Tool Router selected"));
    assert!(line.contains("code"));
}
```

- [ ] **Step 2: Run focused status test**

Run: `just test-lib-one tool_router_status_line_reports_scope openz`

Expected: PASS or FAIL only if status wording changed. Fix wording to include selected domains.

- [ ] **Step 3: Keep runtime output quiet by default**

Ensure `show_tool_router_status` remains config-gated in `src/agent/agent_loop/run.rs`:

```rust
if config.agents.defaults.show_tool_router_status
    && !crate::agent::style::spinner::is_silent()
{
    let summary = loop_ref.tools.tool_router_status_line(ctx.user_content);
    crate::tui_println!("{}◎ {}{}", AURA_PURPLE, summary, COLOR_RESET);
}
```

- [ ] **Step 4: Commit**

```bash
git add src/tools/mod.rs src/agent/agent_loop/run.rs src/channels/websocket.rs
git commit -m "chore: expose tool scope observability"
```

---

### Task 10: Add Config Knobs and Defaults

**Files:**
- Modify: `src/config/schema.rs`
- Test: `src/config/schema.rs`

**Interfaces:**
- Produces:
  - `ToolRoutingConfig`
  - `Config.agents.defaults.layered_tool_routing`
  - Defaults:
    - `enabled: true`
    - `max_visible_tools: 20`
    - `always_visible_tools: ["request_tool_scope", "tool_catalog", "openz_inventory"]`

- [ ] **Step 1: Write config default test**

```rust
#[test]
fn layered_tool_routing_defaults_are_balanced() {
    let config = Config::default();
    assert!(config.agents.defaults.layered_tool_routing.enabled);
    assert_eq!(config.agents.defaults.layered_tool_routing.max_visible_tools, 20);
    assert!(config
        .agents
        .defaults
        .layered_tool_routing
        .always_visible_tools
        .contains(&"request_tool_scope".to_string()));
}
```

- [ ] **Step 2: Run failing config test**

Run: `just test-lib-one layered_tool_routing_defaults_are_balanced openz`

Expected: compile failure until config fields exist.

- [ ] **Step 3: Add config types**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolRoutingConfig {
    pub enabled: bool,
    pub max_visible_tools: usize,
    pub always_visible_tools: Vec<String>,
}

impl Default for ToolRoutingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_visible_tools: 20,
            always_visible_tools: vec![
                "request_tool_scope".to_string(),
                "tool_catalog".to_string(),
                "openz_inventory".to_string(),
            ],
        }
    }
}
```

Add this field to agent defaults:

```rust
pub layered_tool_routing: ToolRoutingConfig,
```

- [ ] **Step 4: Run focused config test**

Run: `just test-lib-one layered_tool_routing_defaults_are_balanced openz`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config/schema.rs
git commit -m "feat: configure layered tool routing"
```

---

### Task 11: Add End-to-End Routing Regressions

**Files:**
- Modify: `src/tools/mod.rs`
- Modify: `src/agent/agent_loop/research_policy.rs`
- Test: `src/tools/mod.rs`, `src/agent/agent_loop/research_policy.rs`

**Interfaces:**
- Ensures these user-visible cases keep working:
  - “Where is orchestrate_workflow implemented in this repo?” exposes local repo tools, not web.
  - “What is latest Rust stable today?” exposes web/search.
  - “Use orchestrate_workflow … planner/reviewer … hello” exposes orchestrator.
  - “what are my cron jobs?” exposes cron inventory/log tools.
  - “summarize hello” does not expose execution/delegation by default.

- [ ] **Step 1: Add regression tests**

Add one test per case:

```rust
#[test]
fn current_external_prompt_exposes_web_research_pack() {
    let registry = registry_with_named_tools(&[
        ("web_search", "search"),
        ("web_fetch", "web"),
        ("grep_search", "code"),
    ]);
    let names = exposed_tool_names(&registry, "What is the latest Rust stable version today?");
    assert!(names.contains(&"web_search".to_string()));
    assert!(names.contains(&"web_fetch".to_string()));
}

#[test]
fn cron_prompt_exposes_cron_pack() {
    let registry = registry_with_named_tools(&[
        ("list_jobs", "cron"),
        ("get_job_logs", "cron"),
        ("web_fetch", "web"),
    ]);
    let names = exposed_tool_names(&registry, "what are my running cron jobs and logs?");
    assert!(names.contains(&"list_jobs".to_string()));
    assert!(names.contains(&"get_job_logs".to_string()));
    assert!(!names.contains(&"web_fetch".to_string()));
}
```

Add test helpers in the same test module:

```rust
fn registry_with_named_tools(tools: &[(&'static str, &'static str)]) -> ToolRegistry {
    let registry = ToolRegistry::new();
    for (name, domain) in tools {
        registry.register(Arc::new(CacheTestTool {
            name,
            domain,
            priority: 90,
        }));
    }
    registry
}

fn exposed_tool_names(registry: &ToolRegistry, prompt: &str) -> Vec<String> {
    registry
        .to_openai_format_for_prompt(prompt)
        .into_iter()
        .filter_map(|value| value["function"]["name"].as_str().map(str::to_string))
        .collect()
}
```

- [ ] **Step 2: Run each focused regression**

Run:

```bash
just test-lib-one current_external_prompt_exposes_web_research_pack openz
just test-lib-one cron_prompt_exposes_cron_pack openz
```

Expected: PASS.

- [ ] **Step 3: Run a low-resource package check**

Run: `just check openz`

Expected: PASS. Stop and inspect if compile time causes laptop pressure.

- [ ] **Step 4: Commit**

```bash
git add src/tools/mod.rs src/agent/agent_loop/research_policy.rs
git commit -m "test: cover layered tool routing cases"
```

---

### Task 12: Update Docs, Changelog, Version

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `README.md`
- Modify: `onpkg.json`

**Interfaces:**
- Version bump: increment patch version by `0.0.1` from current version.
- Changelog entry:
  - Layered L0-L3 control model.
  - Intent-based tool scoping.
  - `request_tool_scope`.
  - Better local-vs-live research behavior.
  - Orchestrator trivial-step guardrails.

- [ ] **Step 1: Read current version**

Run:

```bash
rg -n '^version =|\"version\"|OpenZ v|v0\\.0\\.' Cargo.toml Cargo.lock README.md onpkg.json CHANGELOG.md
```

Expected: identify current version consistently before editing.

- [ ] **Step 2: Update version fields**

Update each version occurrence to the next patch version. If current is `0.0.140`, update to `0.0.141`.

- [ ] **Step 3: Add changelog section**

Add:

```markdown
## v0.0.141 - Layered Tool Scope

- Added L0-L3 tool control design: intent classification, scoped tool packs, execution guardrails, and orchestrator policies.
- Added prompt-aware scoped tool exposure so simple tasks do not see heavyweight execution, web, or subagent tools by default.
- Added `request_tool_scope` so the model can ask for exact missing tools/domains instead of guessing.
- Improved local-vs-live knowledge policy: repo questions stay local, latest/current/external URL questions route to web research.
- Hardened orchestrator smoke-test behavior so trivial planner/reviewer steps answer directly without nested delegation or unnecessary research.
```

- [ ] **Step 4: Run docs/version sanity checks**

Run:

```bash
rg -n '0\\.0\\.140|0\\.0\\.141' Cargo.toml Cargo.lock README.md onpkg.json CHANGELOG.md
git diff --check
```

Expected: old version absent from release fields, `git diff --check` clean.

- [ ] **Step 5: Commit**

```bash
git add CHANGELOG.md Cargo.toml Cargo.lock README.md onpkg.json
git commit -m "chore: release openz v0.0.141"
```

---

## Final Verification

- [ ] Run focused tests touched by this plan:

```bash
just test-lib-one local_repo_question_prefers_repo_read_not_web openz
just test-lib-one local_repo_read_pack_includes_grep_and_read_not_web openz
just test-lib-one simple_prompt_does_not_expose_heavy_execution_tools openz
just test-lib-one request_tool_scope_returns_structured_scope_request openz
just test-lib-one step_prompt_discourages_research_for_trivial_steps openz
just test-lib-one skips_evolution_for_short_smoke_test_outputs openz
just test-lib-one simple_step_does_not_allow_nested_delegation openz
just test-lib-one current_external_prompt_exposes_web_research_pack openz
just test-lib-one cron_prompt_exposes_cron_pack openz
```

- [ ] Run one package check only if laptop is stable:

```bash
just check openz
```

- [ ] Manual smoke prompts in TUI:

```text
summarize hello
Where is orchestrate_workflow implemented in this repo?
What is the latest Rust stable version today?
what are my running cron jobs?
Use orchestrate_workflow to run a simple two-step workflow: planner summarizes "hello", reviewer reviews planner output.
```

Expected behavior:
- `summarize hello`: direct answer, no web, no delegation.
- repo location question: uses local grep/read/code tools, no web.
- latest Rust stable: uses web/search and cites source.
- cron question: uses cron inventory/log tools.
- simple workflow: planner and reviewer complete directly, no nested delegation or evolution skill save.

---

## Self-Review

- Spec coverage: Covers scoped tool exposure, L0-L3 layering, balanced research behavior, orchestrator prompt changes, demo/simple-task behavior, evolution suppression, and nested delegation caps.
- Placeholder scan: No placeholder sections remain; each task names files, interfaces, tests, commands, and expected behavior.
- Type consistency: `TurnIntent`, `KnowledgePolicy`, `IntentDecision`, `ToolScopeEngine`, `ToolScopeDecision`, and `request_tool_scope` names are consistent across tasks.
