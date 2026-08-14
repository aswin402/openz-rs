# OpenZ Balanced Grounding Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add balanced grounding so OpenZ answers simple/stable tasks directly, retrieves from local memory/docs/web when facts are current/source-specific/high-stakes/uncertain, and prevents orchestrated subagents from over-researching trivial steps.

**Architecture:** Add a shared rule-based `grounding` module that classifies user turns and workflow steps into grounding classes and execution policies. Inject concise guidance into the main agent system prompt and orchestrator step prompts, then use the same policy to limit nested delegation and noisy evolution skill creation for smoke-test/trivial runs.

**Tech Stack:** Rust 2021, existing OpenZ agent loop, orchestrator runtime, subagent tools, native memory/source tooling. Validation must use `just test-one <name> openz` and `just check openz` only.

## Global Constraints

- Default mode is **Balanced Grounding**: fast direct answers when safe, grounded answers when accuracy depends on outside context.
- Do not force web search for every factual sentence.
- Use local OpenZ knowledge first for user/project/saved-path/docs questions.
- Use web/search/fetch for current, changing, source-specific, high-stakes, or uncertain facts.
- Avoid nested delegation unless the task needs decomposition, parallelism, specialist work, or explicitly asks for delegation/research.
- Suppress noisy skill evolution for smoke tests, short outputs, failed outputs, and generic one-off facts.
- Use low-resource validation only: `just test-one <name> openz` and `just check openz`; do not run full cargo test/build/check.

---

## File Structure

- Create `src/grounding.rs`: shared grounding classes, rule-based classifier, execution policy, prompt guidance helpers, and tests.
- Modify `src/lib.rs`: export `pub mod grounding;`.
- Modify `src/agent/agent_loop/build.rs`: inject balanced grounding rules into main system prompt and account for prompt budget.
- Modify `src/orchestrator/runtime.rs`: use `grounding::step_execution_policy()` in `build_step_prompt()` and add direct-completion/research guidance.
- Modify `src/tools/orchestrator.rs`: apply step execution policy to worker tool exposure and nested delegation metadata.
- Modify `src/tools/subagent/mod.rs`: add task-local override for nested delegation inside orchestrated worker runs.
- Modify `src/tools/mod.rs`: honor the task-local nested-delegation override alongside existing manager-profile checks.
- Modify `src/tools/subagent/delegate_task.rs` and `src/tools/subagent/delegate_profile.rs`: skip evolution review when the grounding policy marks output as non-reusable/noisy.
- Modify `CHANGELOG.md`: add a v0.0.129 fix entry.

---

### Task 1: Shared Grounding Policy Classifier

**Files:**
- Create: `src/grounding.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `GroundingClass`, `StepExecutionPolicy`, `classify_grounding_text(text: &str) -> GroundingClass`, `step_execution_policy(workflow_goal: &str, step_goal: &str, agent: &str) -> StepExecutionPolicy`, `main_agent_grounding_rules() -> &'static str`, `should_suppress_evolution(goal: &str, context: &str, summary: &str) -> bool`.
- Consumes: no project-local dependencies beyond std string matching.

- [ ] **Step 1: Write failing tests for grounding classification**

Create `src/grounding.rs` with the tests first and minimal type declarations that intentionally fail until implementation is added:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundingClass {
    Trivial,
    Stable,
    LocalProject,
    PersonalMemory,
    CurrentExternal,
    SourceSpecific,
    HighStakes,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepExecutionPolicy {
    pub grounding_class: GroundingClass,
    pub allow_web: bool,
    pub allow_nested_delegation: bool,
    pub require_sources: bool,
    pub suppress_evolution: bool,
}

pub fn classify_grounding_text(_text: &str) -> GroundingClass {
    GroundingClass::Stable
}

pub fn step_execution_policy(_workflow_goal: &str, _step_goal: &str, _agent: &str) -> StepExecutionPolicy {
    StepExecutionPolicy {
        grounding_class: GroundingClass::Stable,
        allow_web: false,
        allow_nested_delegation: false,
        require_sources: false,
        suppress_evolution: false,
    }
}

pub fn main_agent_grounding_rules() -> &'static str {
    ""
}

pub fn should_suppress_evolution(_goal: &str, _context: &str, _summary: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounding_policy_classifies_trivial_hello() {
        let policy = step_execution_policy(
            "Run a smoke test workflow",
            "Summarize the word hello",
            "planner",
        );

        assert_eq!(policy.grounding_class, GroundingClass::Trivial);
        assert!(!policy.allow_web);
        assert!(!policy.allow_nested_delegation);
        assert!(!policy.require_sources);
        assert!(policy.suppress_evolution);
    }

    #[test]
    fn grounding_policy_requires_web_for_latest_version() {
        let class = classify_grounding_text("What is the latest stable Rust version today?");
        assert_eq!(class, GroundingClass::CurrentExternal);

        let policy = step_execution_policy(
            "Answer current software version question",
            "Find the latest stable Rust version today",
            "researcher",
        );
        assert!(policy.allow_web);
        assert!(policy.require_sources);
        assert!(!policy.suppress_evolution);
    }

    #[test]
    fn grounding_policy_detects_source_specific_requests() {
        assert_eq!(
            classify_grounding_text("Read https://example.com/docs and summarize it"),
            GroundingClass::SourceSpecific
        );
        assert_eq!(
            classify_grounding_text("Compare the PDF at /tmp/report.pdf"),
            GroundingClass::SourceSpecific
        );
    }

    #[test]
    fn grounding_policy_detects_project_questions() {
        assert_eq!(
            classify_grounding_text("In this repo, where is orchestrate_workflow implemented?"),
            GroundingClass::LocalProject
        );
    }

    #[test]
    fn evolution_suppressed_for_smoke_test_outputs() {
        assert!(should_suppress_evolution(
            "Run smoke test workflow",
            "demo context",
            "Planner summary accurate. No issues."
        ));
        assert!(should_suppress_evolution(
            "Review tiny output",
            "context",
            "Cannot research with available tools."
        ));
        assert!(!should_suppress_evolution(
            "Refactor tool routing",
            "Detailed coding task",
            "When adding tool schemas, keep required fields explicit and add focused regression tests before implementation."
        ));
    }
}
```

- [ ] **Step 2: Export the module**

Modify `src/lib.rs` by adding this line near the existing module exports:

```rust
pub mod grounding;
```

- [ ] **Step 3: Run failing tests**

Run each test one at a time:

```bash
just test-one grounding_policy_classifies_trivial_hello openz
just test-one grounding_policy_requires_web_for_latest_version openz
just test-one evolution_suppressed_for_smoke_test_outputs openz
```

Expected before implementation: at least the trivial/latest/evolution tests fail because the initial stub classifier returns `Stable` and never suppresses evolution.

- [ ] **Step 4: Implement minimal rule-based classifier**

Replace the initial stub implementations in `src/grounding.rs` with this logic:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundingClass {
    Trivial,
    Stable,
    LocalProject,
    PersonalMemory,
    CurrentExternal,
    SourceSpecific,
    HighStakes,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepExecutionPolicy {
    pub grounding_class: GroundingClass,
    pub allow_web: bool,
    pub allow_nested_delegation: bool,
    pub require_sources: bool,
    pub suppress_evolution: bool,
}

fn normalized(text: &str) -> String {
    text.to_ascii_lowercase()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn looks_like_url_or_path(text: &str) -> bool {
    contains_any(text, &["http://", "https://", "www.", ".pdf", ".docx", ".xlsx", "/tmp/", "~/", "./", "../"])
}

fn looks_trivial(text: &str) -> bool {
    let trimmed = text.trim();
    let words = trimmed.split_whitespace().count();
    words <= 10
        && contains_any(
            text,
            &[
                "hello", "hi", "hii", "hey", "greeting", "smoke test", "demo", "simple two-step", "summarize hello",
            ],
        )
}

pub fn classify_grounding_text(text: &str) -> GroundingClass {
    let text = normalized(text);

    if looks_like_url_or_path(&text) {
        return GroundingClass::SourceSpecific;
    }
    if contains_any(&text, &["medical", "legal", "financial", "diagnosis", "law", "security vulnerability", "exploit", "cve"]) {
        return GroundingClass::HighStakes;
    }
    if contains_any(&text, &["latest", "today", "current", "recent", "news", "price", "version", "schedule", "score", "release", "changed", "updated"] ) {
        return GroundingClass::CurrentExternal;
    }
    if contains_any(&text, &["this repo", "codebase", "file", "directory", "path", "function", "struct", "module", "implementation", "where is", "where are"] ) {
        return GroundingClass::LocalProject;
    }
    if contains_any(&text, &["remember", "memory", "preference", "saved", "skill", "what did we", "previous"] ) {
        return GroundingClass::PersonalMemory;
    }
    if contains_any(&text, &["not sure", "unknown", "verify", "check if", "doubt"] ) {
        return GroundingClass::Uncertain;
    }
    if looks_trivial(&text) {
        return GroundingClass::Trivial;
    }
    GroundingClass::Stable
}

fn asks_for_delegation_or_complex_work(text: &str) -> bool {
    contains_any(
        text,
        &[
            "delegate", "subagent", "parallel", "multi-source", "research", "scan", "repo-wide", "implement", "refactor", "debug", "compare sources",
        ],
    )
}

pub fn step_execution_policy(workflow_goal: &str, step_goal: &str, agent: &str) -> StepExecutionPolicy {
    let combined = format!("{workflow_goal}
{step_goal}");
    let class = classify_grounding_text(&combined);
    let combined_norm = normalized(&combined);
    let agent_norm = normalized(agent);
    let explicitly_complex = asks_for_delegation_or_complex_work(&combined_norm);
    let manager_agent = contains_any(&agent_norm, &["manager", "coordinator", "orchestrator"]);
    let researcher_agent = contains_any(&agent_norm, &["researcher", "research"]);

    let require_sources = matches!(
        class,
        GroundingClass::CurrentExternal
            | GroundingClass::SourceSpecific
            | GroundingClass::HighStakes
            | GroundingClass::Uncertain
    );
    let allow_web = require_sources || researcher_agent || contains_any(&combined_norm, &["web", "internet", "search", "docs"]);
    let allow_nested_delegation = manager_agent
        || explicitly_complex
        || matches!(class, GroundingClass::LocalProject | GroundingClass::SourceSpecific)
            && !matches!(class, GroundingClass::Trivial | GroundingClass::Stable);
    let suppress_evolution = matches!(class, GroundingClass::Trivial)
        || contains_any(&combined_norm, &["smoke test", "demo", "hello"]);

    StepExecutionPolicy {
        grounding_class: class,
        allow_web,
        allow_nested_delegation,
        require_sources,
        suppress_evolution,
    }
}

pub fn main_agent_grounding_rules() -> &'static str {
    "

Balanced grounding rules:
- Answer directly for stable/trivial tasks. Do not search or delegate for greetings, simple wording, creative writing, provided-text summaries, or simple stable concepts.
- For user/project questions, use OpenZ memory, skills, saved links, local files, docs, and code tools before relying on model memory.
- Use web/search/fetch for current, changing, source-specific, high-stakes, or uncertain claims.
- Cite or name sources when live sources are used. If sources are missing or weak, say verification is incomplete instead of pretending certainty."
}

pub fn should_suppress_evolution(goal: &str, context: &str, summary: &str) -> bool {
    let combined = normalized(&format!("{goal}
{context}
{summary}"));
    let class = classify_grounding_text(&combined);
    let summary_words = summary.split_whitespace().count();

    matches!(class, GroundingClass::Trivial)
        || summary_words < 18
        || contains_any(&combined, &["smoke test", "demo", "hello"])
        || contains_any(&combined, &["timed out", "failed", "cannot research", "missing tool", "tool limitation", "available tools"])
}
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
just test-one grounding_policy_classifies_trivial_hello openz
just test-one grounding_policy_requires_web_for_latest_version openz
just test-one grounding_policy_detects_source_specific_requests openz
just test-one grounding_policy_detects_project_questions openz
just test-one evolution_suppressed_for_smoke_test_outputs openz
```

Expected: all pass.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/grounding.rs src/lib.rs
git commit -m "feat: add balanced grounding policy"
```

---

### Task 2: Main Agent And Orchestrator Prompt Guidance

**Files:**
- Modify: `src/agent/agent_loop/build.rs`
- Modify: `src/orchestrator/runtime.rs`

**Interfaces:**
- Consumes: `crate::grounding::main_agent_grounding_rules()` and `crate::grounding::step_execution_policy()` from Task 1.
- Produces: updated system prompt and step prompt behavior.

- [ ] **Step 1: Write failing orchestrator prompt tests**

In `src/orchestrator/runtime.rs`, add tests near the existing `builds_step_prompt_with_expected_output_and_prior_results` test:

```rust
#[test]
fn step_prompt_instructs_direct_completion_for_trivial_steps() {
    let step = WorkflowStep {
        id: "plan".to_string(),
        agent: "planner".to_string(),
        goal: "Summarize hello".to_string(),
        depends_on: vec![],
        expected_output: "short summary".to_string(),
        max_retries: 0,
    };

    let prompt = build_step_prompt(&step, "Run smoke test workflow", &[]);

    assert!(prompt.contains("Complete this step directly when possible"));
    assert!(prompt.contains("Do not delegate"));
    assert!(prompt.contains("Do not research"));
}

#[test]
fn step_prompt_allows_research_for_current_external_steps() {
    let step = WorkflowStep {
        id: "latest".to_string(),
        agent: "researcher".to_string(),
        goal: "Find the latest stable Rust version today".to_string(),
        depends_on: vec![],
        expected_output: "version with source".to_string(),
        max_retries: 0,
    };

    let prompt = build_step_prompt(&step, "Answer current software version question", &[]);

    assert!(prompt.contains("Use web/docs/local tools when required"));
    assert!(prompt.contains("source-specific"));
    assert!(prompt.contains("verification is incomplete"));
}
```

- [ ] **Step 2: Run failing prompt tests**

```bash
just test-one step_prompt_instructs_direct_completion_for_trivial_steps openz
just test-one step_prompt_allows_research_for_current_external_steps openz
```

Expected before implementation: fail because current prompt only says `Return only the requested deliverable plus any blockers.`

- [ ] **Step 3: Inject main grounding rules into system prompt**

Modify `src/agent/agent_loop/build.rs`:

1. After `let weak_model_rules = weak_model_operating_rules(&config.agents.defaults.model);`, add:

```rust
let grounding_rules = crate::grounding::main_agent_grounding_rules();
```

2. Include `grounding_rules.chars().count()` in `base_len`.

3. Add `grounding_rules` to the `ctx.system_prompt = format!(...)` format string immediately after `system_guidelines`.

The final format argument order should place grounding rules early:

```rust
ctx.system_prompt = format!(
    "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
    header,
    persona_priority_part,
    system_guidelines,
    grounding_rules,
    activity_part,
    summary_part,
    memory_part,
    pinned_memory,
    recent_session_part,
    brief_context,
    source_context,
    workflow_context,
    weak_model_rules,
    cross_session_memory,
    vision_instruction,
    skills_part,
    caveman_rules
);
```

- [ ] **Step 4: Update `build_step_prompt()`**

In `src/orchestrator/runtime.rs`, replace the final prompt body with policy-aware guidance:

```rust
let policy = crate::grounding::step_execution_policy(workflow_goal, &step.goal, &step.agent);
let grounding_guidance = if policy.require_sources {
    "Use web/docs/local tools when required for current, external, source-specific, uncertain, or high-stakes facts. Cite or name sources when used. If sources are unavailable or weak, say verification is incomplete."
} else {
    "Complete this step directly when possible. Do not delegate or research for trivial/general-knowledge work unless the step explicitly asks for research, current facts, source-specific facts, or multi-source analysis."
};

format!(
    "Workflow goal: {workflow_goal}

Step id: {id}
Assigned agent: {agent}
Step goal: {goal}
Expected output: {expected}

Grounding guidance: {grounding_guidance}
Nested delegation allowed: {nested}
Live sources required: {sources}

Prior step results:
{prior}

Return only the requested deliverable plus real blockers.",
    id = step.id,
    agent = step.agent,
    goal = step.goal,
    expected = step.expected_output,
    nested = policy.allow_nested_delegation,
    sources = policy.require_sources,
)
```

- [ ] **Step 5: Run focused tests**

```bash
just test-one step_prompt_instructs_direct_completion_for_trivial_steps openz
just test-one step_prompt_allows_research_for_current_external_steps openz
```

Expected: both pass.

- [ ] **Step 6: Commit Task 2**

```bash
git add src/agent/agent_loop/build.rs src/orchestrator/runtime.rs
git commit -m "feat: inject balanced grounding prompts"
```

---

### Task 3: Orchestrated Nested Delegation Control

**Files:**
- Modify: `src/tools/subagent/mod.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/orchestrator.rs`

**Interfaces:**
- Consumes: `StepExecutionPolicy.allow_nested_delegation` from Task 1.
- Produces: task-local override that prevents ordinary planner/reviewer workers from seeing nested delegation tools during trivial orchestrated steps.

- [ ] **Step 1: Write failing registry test**

In `src/tools/mod.rs` route cache tests module, add a test that proves the delegation tool is normally visible when context is available, then hidden inside the orchestrated worker scope:

```rust
#[tokio::test]
async fn orchestrated_worker_policy_blocks_nested_delegation_tools() {
    let registry = registry_with_mock_context();

    assert!(registry.get("delegate_task").is_some());

    crate::tools::subagent::ORCHESTRATED_NESTED_DELEGATION_ALLOWED
        .scope(false, async {
            assert!(registry.get("delegate_task").is_none());
            let exposed_names = registry
                .to_openai_format_for_prompt("delegate this task")
                .into_iter()
                .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
                .collect::<Vec<_>>();
            assert!(!exposed_names.iter().any(|name| name == "delegate_task"));
        })
        .await;
}
```

- [ ] **Step 2: Run failing test**

```bash
just test-one orchestrated_worker_policy_blocks_nested_delegation_tools openz
```

Expected before implementation: compile fails because `ORCHESTRATED_NESTED_DELEGATION_ALLOWED` does not exist.

- [ ] **Step 3: Add task-local override**

Modify `src/tools/subagent/mod.rs` task locals:

```rust
tokio::task_local! {
    pub static DELEGATION_DEPTH: usize;
    pub static ACTIVE_SUBAGENT: String;
    pub static ORCHESTRATED_NESTED_DELEGATION_ALLOWED: bool;
}

pub fn nested_delegation_allowed_for_active_context(profile_name: &str) -> bool {
    if let Ok(allowed) = ORCHESTRATED_NESTED_DELEGATION_ALLOWED.try_with(|allowed| *allowed) {
        return allowed;
    }
    can_spawn_nested_subagents(profile_name)
}
```

Keep existing `can_spawn_nested_subagents()` unchanged for manager-style defaults.

- [ ] **Step 4: Use override in tool registry**

Modify `src/tools/mod.rs` wherever active subagent checks call `can_spawn_nested_subagents(&active_subagent)`. Replace with:

```rust
crate::tools::subagent::nested_delegation_allowed_for_active_context(&active_subagent)
```

Apply this in:

- `ToolRegistry::get()` static nested tool gate.
- `ToolRegistry::get()` dynamic subagent lookup gate.
- `ToolRegistry::dynamic_subagent_tools()` exposure gate.

- [ ] **Step 5: Scope orchestrated step execution with policy**

In `src/tools/orchestrator.rs`, inside `SubagentStepExecutor::execute_step()`, compute policy before `delegate.call(...)`:

```rust
let step_policy = crate::grounding::step_execution_policy(&spec.goal, &step.goal, &step.agent);
```

Wrap the delegate call:

```rust
let response = crate::tools::subagent::ORCHESTRATED_NESTED_DELEGATION_ALLOWED
    .scope(step_policy.allow_nested_delegation, async {
        delegate.call(&json!({ "goal": step.goal, "context": prompt })).await
    })
    .await?;
```

- [ ] **Step 6: Run focused tests**

```bash
just test-one orchestrated_worker_policy_blocks_nested_delegation_tools openz
just test-one ordinary_subagent_cannot_access_orchestrator_tool openz
```

Expected: both pass.

- [ ] **Step 7: Commit Task 3**

```bash
git add src/tools/subagent/mod.rs src/tools/mod.rs src/tools/orchestrator.rs
git commit -m "feat: limit nested delegation in orchestrated steps"
```

---

### Task 4: Evolution Suppression Gates

**Files:**
- Modify: `src/tools/subagent/delegate_task.rs`
- Modify: `src/tools/subagent/delegate_profile.rs`
- Test: existing `src/tools/subagent/tests.rs` or local tests in modified modules if easier.

**Interfaces:**
- Consumes: `crate::grounding::should_suppress_evolution(goal, context, summary) -> bool` from Task 1.
- Produces: evolution review only runs for substantial successful reusable outputs.

- [ ] **Step 1: Write focused tests for suppression helper**

If Task 1 tests already cover `should_suppress_evolution`, add no new helper tests. Instead add call-site tests only if the call site has testable helper extraction. Extract this helper in `src/tools/subagent/delegate_task.rs`:

```rust
fn should_run_evolution_review(goal: &str, context: &str, summary: &str, filesystem_write_denied: bool) -> bool {
    !filesystem_write_denied && !crate::grounding::should_suppress_evolution(goal, context, summary)
}
```

Then add tests in the same file under its test module or create one:

```rust
#[cfg(test)]
mod evolution_gate_tests {
    use super::should_run_evolution_review;

    #[test]
    fn evolution_gate_blocks_smoke_test_summary() {
        assert!(!should_run_evolution_review(
            "Run smoke test workflow",
            "demo",
            "Planner summary accurate. No issues.",
            false,
        ));
    }

    #[test]
    fn evolution_gate_blocks_when_filesystem_writes_denied() {
        assert!(!should_run_evolution_review(
            "Refactor routing",
            "coding task",
            "When changing routing, add a focused regression test and verify the exposed tool list.",
            true,
        ));
    }

    #[test]
    fn evolution_gate_allows_substantial_reusable_guidance() {
        assert!(should_run_evolution_review(
            "Refactor routing",
            "coding task",
            "When changing routing, add a focused regression test, inspect model-facing tool exposure, and verify the runtime lookup path uses the same policy.",
            false,
        ));
    }
}
```

- [ ] **Step 2: Run failing tests**

```bash
just test-one evolution_gate_blocks_smoke_test_summary openz
just test-one evolution_gate_allows_substantial_reusable_guidance openz
```

Expected before implementation: fail to compile if helper is not added yet.

- [ ] **Step 3: Use helper in `DelegateTaskTool`**

In `src/tools/subagent/delegate_task.rs`, replace:

```rust
if !filesystem_write_denied {
    let _ = run_evolution_review(&self.parent_provider, "subagent", &clean_goal, &clean_context, &res.content).await;
}
```

with:

```rust
if should_run_evolution_review(&clean_goal, &clean_context, &res.content, filesystem_write_denied) {
    let _ = run_evolution_review(&self.parent_provider, "subagent", &clean_goal, &clean_context, &res.content).await;
}
```

- [ ] **Step 4: Reuse helper in `DelegateProfileTool`**

Move `should_run_evolution_review` to `src/tools/subagent/mod.rs` if `delegate_profile.rs` cannot access the private helper cleanly. Preferred shared function:

```rust
pub fn should_run_evolution_review(goal: &str, context: &str, summary: &str, filesystem_write_denied: bool) -> bool {
    !filesystem_write_denied && !crate::grounding::should_suppress_evolution(goal, context, summary)
}
```

Then update both call sites:

```rust
if super::should_run_evolution_review(&clean_goal, &clean_context, &run_res.content, filesystem_write_denied) {
    let _ = run_evolution_review(&self.parent_provider, &self.profile.name, &clean_goal, &clean_context, &run_res.content).await;
}
```

and

```rust
if super::should_run_evolution_review(&clean_goal, &clean_context, &res.content, filesystem_write_denied) {
    let _ = run_evolution_review(&self.parent_provider, "subagent", &clean_goal, &clean_context, &res.content).await;
}
```

- [ ] **Step 5: Run focused tests**

```bash
just test-one evolution_gate_blocks_smoke_test_summary openz
just test-one evolution_gate_allows_substantial_reusable_guidance openz
just test-one evolution_suppressed_for_smoke_test_outputs openz
```

Expected: all pass.

- [ ] **Step 6: Commit Task 4**

```bash
git add src/tools/subagent/mod.rs src/tools/subagent/delegate_task.rs src/tools/subagent/delegate_profile.rs
git commit -m "feat: suppress noisy subagent evolution"
```

---

### Task 5: Documentation, Changelog, And Final Validation

**Files:**
- Modify: `docs/orchestrator-runtime.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: all previous task behavior.
- Produces: user-facing docs and release notes for balanced grounding.

- [ ] **Step 1: Update orchestrator docs**

Append this section to `docs/orchestrator-runtime.md`:

````markdown
## Balanced Grounding In Workflow Steps

OpenZ uses balanced grounding inside orchestrated steps. Workers answer directly for trivial or stable tasks, such as small demos and simple definitions. Workers use local files, memory, docs, or web tools when the step depends on current, source-specific, high-stakes, project-local, or uncertain facts.

For simple smoke tests, prefer direct goals:

```json
{
  "goal": "Summarize hello and review it",
  "mode": "sequential",
  "agents": [{ "name": "planner" }, { "name": "reviewer" }],
  "steps": [
    { "id": "plan", "agent": "planner", "goal": "Summarize hello" },
    { "id": "review", "agent": "reviewer", "goal": "Review planner output", "depends_on": ["plan"] }
  ]
}
```

Use explicit research wording only when sources are required:

```json
{
  "id": "latest",
  "agent": "researcher",
  "goal": "Find the latest stable Rust version today and cite sources"
}
```
````

- [ ] **Step 2: Update changelog**

In `CHANGELOG.md` under `v0.0.129 (Latest Release)`, add:

```markdown
- **Fix:** Added balanced grounding policy for orchestrated steps and main-agent guidance so trivial tasks answer directly, current/source-specific facts retrieve sources, nested delegation is capped, and smoke-test outputs do not create noisy skills.
```

- [ ] **Step 3: Run final focused validations**

Run only these commands, one at a time:

```bash
just test-one grounding_policy_classifies_trivial_hello openz
just test-one grounding_policy_requires_web_for_latest_version openz
just test-one step_prompt_instructs_direct_completion_for_trivial_steps openz
just test-one step_prompt_allows_research_for_current_external_steps openz
just test-one orchestrated_worker_policy_blocks_nested_delegation_tools openz
just test-one evolution_gate_blocks_smoke_test_summary openz
just check openz
```

Expected: all pass. If any compile takes too long, stop it and report which command was stopped.

- [ ] **Step 4: Commit Task 5**

```bash
git add docs/orchestrator-runtime.md CHANGELOG.md
git commit -m "docs: document balanced grounding policy"
```

- [ ] **Step 5: Manual smoke test**

After commits, restart `openz agent` and ask:

```text
Use orchestrate_workflow to run a simple two-step workflow: planner summarizes "hello", reviewer reviews planner output.
```

Expected behavior:

- First `orchestrate_workflow` call succeeds.
- Planner answers directly with a useful short summary of `hello`.
- Planner does not claim it needs web access.
- Planner does not call nested `delegate_task` for this trivial step.
- Reviewer reviews the summary directly.
- No noisy skill is saved for this smoke-test run.

---

## Self-Review Notes

- Spec coverage: covered classifier, main prompt, orchestrator prompt, nested delegation, evolution suppression, docs, changelog, and low-resource validation.
- Scan note: no unresolved task markers or undefined task references remain.
- Type consistency: `GroundingClass`, `StepExecutionPolicy`, `step_execution_policy`, `main_agent_grounding_rules`, and `should_suppress_evolution` are defined in Task 1 and reused by later tasks with matching names.
