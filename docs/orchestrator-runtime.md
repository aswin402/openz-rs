# OpenZ Orchestrator Runtime

`orchestrate_workflow` runs typed multi-agent workflows through existing OpenZ subagent profiles. It is intended for bounded, observable coordination inside one OpenZ turn: the parent model submits a workflow spec, the runtime validates it, delegates each step to the named subagent profile, and returns structured step results.

## Minimal Sequential Workflow

```json
{
  "goal": "Research and review a Rust crate",
  "mode": "sequential",
  "agents": [{ "name": "researcher" }, { "name": "reviewer" }],
  "steps": [
    {
      "id": "research",
      "agent": "researcher",
      "goal": "Read the crate docs and summarize usage",
      "expected_output": "summary with links"
    },
    {
      "id": "review",
      "agent": "reviewer",
      "goal": "Check whether the summary is actionable",
      "depends_on": ["research"],
      "expected_output": "approve or list gaps"
    }
  ],
  "termination": { "max_rounds": 4 }
}
```

## Review Loop

```json
{
  "goal": "Implement a bug fix with review",
  "mode": "review_loop",
  "agents": [{ "name": "coder" }, { "name": "reviewer" }],
  "steps": [
    {
      "id": "implement",
      "agent": "coder",
      "goal": "Make the smallest safe code change",
      "expected_output": "patch summary and tests"
    },
    {
      "id": "review",
      "agent": "reviewer",
      "goal": "Review the change",
      "depends_on": ["implement"],
      "expected_output": "APPROVE or concrete fixes"
    }
  ],
  "termination": { "max_rounds": 3, "success_keyword": "APPROVE" },
  "review": { "mode": "required", "reviewer": "reviewer" }
}
```

## Modes

- `sequential`: executes dependency-ready steps in a deterministic order.
- `parallel`: executes independent steps concurrently with an internal cap of four running steps.
- `review_loop`: repeats implementation and review steps until the success keyword appears or the round limit is reached.
- `selector`: chooses the next speaker deterministically from the configured group while avoiding immediate repeat speakers when possible.
- `manager_worker` and `graph`: accepted workflow modes for future expansion; current execution follows validated step dependencies.

## Capability Policy

Workflow specs can include a `capabilities` object to restrict the tools exposed to delegated subagents. The runtime combines inherited and workflow policies, then filters static tools and dynamic subagent tools before execution. Policies can allow explicit tool names, deny explicit tool names, deny shell/process tools, and deny filesystem write tools.

## WebUI Events

The runtime emits orchestration lifecycle events to the WebUI when a workflow runs from a WebSocket chat. The WebUI stores recent `OrchestrationRunState` records per chat and renders them in the Agent Activity panel. Running runs are settled on stop, error, or turn-end fallback so stale running UI state does not persist.

## Notes

- Subagent cancellation inherits the parent turn cancellation token.
- Parallel workflow execution is deliberately capped to avoid local resource spikes.
- Step IDs, agent names, dependencies, review settings, and termination settings are validated before execution.
- The runtime is not a replacement for durable SOP workflows; SOP remains the user-defined process engine.

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
