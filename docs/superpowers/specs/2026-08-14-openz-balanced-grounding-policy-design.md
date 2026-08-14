# OpenZ Balanced Grounding Policy Design

## Purpose
OpenZ should reduce hallucinations without turning every turn into slow web research. The system should answer directly for stable or trivial requests, but retrieve from OpenZ memory, skills, local files, docs, or the web when the answer depends on current, external, source-specific, high-stakes, or uncertain information.

The default mode is **Balanced Grounding**: fast direct answers when safe, grounded answers when accuracy depends on outside context.

## Problem
The new orchestrator works, but a simple smoke-test step such as summarizing `hello` caused the planner to over-plan, delegate, and claim it needed web access. That shows two related issues:

- The model is not being told when to answer directly versus when to retrieve.
- Subagents and orchestrated steps can overuse delegation/research for trivial tasks.
- Evolution skill creation is still too eager for tiny demo tasks and failed/tool-limitation outputs.

Separately, normal OpenZ chat still risks model-memory answers for facts that may be stale or wrong.

## Design Goals
- Keep simple chat and simple workflow steps fast.
- Use local OpenZ knowledge first when the question is about the user, project, saved paths, docs, or previous work.
- Use web/search/fetch for current, changing, external, source-specific, high-stakes, or uncertain facts.
- Require clear caveats when source verification is missing or weak.
- Avoid nested delegation unless the task actually needs decomposition, parallelism, or specialist work.
- Suppress noisy skill evolution for smoke tests, short outputs, failed outputs, and generic one-off facts.

## Non-Goals
- Do not force web search for every factual sentence.
- Do not build a full enterprise RAG platform in this iteration.
- Do not remove model knowledge entirely; stable knowledge remains useful.
- Do not require citations for greetings, creative writing, provided-text summarization, or simple stable concepts.

## Grounding Classes
OpenZ classifies each user turn or orchestrator step into one of these classes:

| Class | Examples | Default action |
|---|---|---|
| `trivial` | greetings, simple wording, tiny demos, summarize `hello` | answer directly |
| `stable` | basic math, common programming concepts, timeless definitions | answer directly, optional memory |
| `local_project` | repo questions, file paths, saved docs, current code behavior | local files/docs/memory/tools first |
| `personal_memory` | user preferences, prior decisions, saved links, skills | memory/skills first |
| `current_external` | latest versions, news, prices, company/person status, schedules | web/search/fetch required |
| `source_specific` | user gives URL, paper, docs page, file, package docs | read/fetch that source required |
| `high_stakes` | medical, legal, financial, security-impacting advice | require sources and caveat |
| `uncertain` | model indicates doubt or answer depends on unknown current state | retrieve before final answer |

## Grounding Policy
Balanced policy decisions:

- `trivial` and `stable`: answer directly. Do not search/delegate unless the user explicitly asks for research.
- `local_project`: use `scope_context`, file search, semantic search, doc readers, or memory before answering.
- `personal_memory`: use memory/skills before answering when relevant.
- `current_external`: use web search/fetch and cite sources.
- `source_specific`: fetch/read the exact source before summarizing or comparing it.
- `high_stakes`: use sources when possible, include a limitation caveat, and avoid pretending certainty.
- `uncertain`: run retrieval or say what is unknown.

If retrieval fails, OpenZ should say verification is incomplete and provide a bounded answer only when safe.

## Main Agent Changes
Add a concise grounding instruction block to the system prompt:

- Answer directly for stable/trivial tasks.
- Use OpenZ memory, skills, saved links, local files, and docs before relying on model memory for user/project questions.
- Use web/search/fetch for current, changing, source-specific, high-stakes, or uncertain claims.
- Cite or name sources when live sources are used.
- If sources are missing or weak, say verification is incomplete.

Reuse the existing `source_ledger` caveat path for live/current answers.

## Orchestrator Changes
Update `build_step_prompt()` so every step receives execution guidance:

- Complete the step directly when possible.
- Do not delegate for trivial/general-knowledge steps.
- Do not research for trivial/stable steps unless the step explicitly asks for research or current/source-specific facts.
- Use web/docs/local tools only when the step depends on current, external, source-specific, uncertain, or high-stakes facts.
- If a tool is unavailable, provide a safe stable answer when possible instead of failing trivial tasks.
- Return only the requested deliverable plus real blockers.

Add an internal `StepExecutionPolicy` produced from the workflow goal and step goal:

```text
allow_web: bool
allow_nested_delegation: bool
require_sources: bool
suppress_evolution: bool
```

Initial implementation can be rule-based keyword classification, not an extra LLM call.

## Nested Delegation Control
Default inside orchestrator:

- `planner` and `reviewer`: nested delegation disabled unless step asks for delegation, decomposition, multi-source research, repo-wide analysis, parallel investigation, or implementation.
- `researcher`: web/research allowed when the step class is `current_external`, `source_specific`, `high_stakes`, or explicitly says research.
- Manager-style profiles retain delegation ability for manager-worker workflows.

This keeps centralized orchestrator control while preserving power for complex tasks.

## Evolution Suppression
Suppress subagent skill evolution when any condition is true:

- workflow/step class is `trivial`
- workflow goal looks like a smoke test/demo
- output is shorter than a minimum threshold
- subagent failed, timed out, or only reported missing tools
- output lacks a reusable procedural pattern

Evolution should still run for successful, substantial, reusable task patterns.

## Tooling And Sources
OpenZ already has the building blocks:

- web: `web_search`, `web_fetch`, `crawl_website`, browser tools
- local/project: filesystem tools, grep, AST search, semantic search, doc readers
- memory/skills: graph memory, extra memory, skills DB, source ledger
- orchestration: typed workflow specs and capability policies

The change is policy and prompt routing, not a new search engine.

## Error Handling
- If a required source/tool is unavailable, return a clear blocker only for tasks that truly require it.
- For trivial/stable tasks, answer directly even without web.
- For current/high-stakes/source-specific tasks, include “verification incomplete” if sources fail.
- Avoid tool loops by making policy decisions before prompting the worker where possible.

## Testing Plan
Focused low-resource tests:

1. `grounding_policy_classifies_trivial_hello`
   - `hello` summary classifies as `trivial`, `allow_web=false`, `allow_nested_delegation=false`.
2. `grounding_policy_requires_web_for_latest_version`
   - latest/current wording classifies as `current_external`, `require_sources=true`.
3. `step_prompt_instructs_direct_completion_for_trivial_steps`
   - prompt contains direct-completion/no-delegation guidance.
4. `step_prompt_allows_research_for_current_external_steps`
   - current/source-specific prompt allows research and source use.
5. `evolution_suppressed_for_smoke_test_outputs`
   - short/demo/failed outputs do not save skills.
6. Existing orchestrator tests still pass.

Validation should use `just test-one <name> openz` and `just check openz`, never full cargo test by default.

## Rollout
Phase 1: Rule-based `GroundingPolicy` and orchestrator step prompt update.
Phase 2: Nested delegation controls for orchestrated child workers.
Phase 3: Evolution suppression gates.
Phase 4: Optional WebUI/TUI indicators showing when an answer was direct, memory-grounded, local-grounded, or web-grounded.

## Success Criteria
- The `hello` workflow completes directly with a useful planner summary and reviewer feedback.
- Current/latest/source-specific questions trigger retrieval.
- No obvious hallucination-prone answer is produced for current/high-stakes questions without a source caveat.
- Smoke-test workflows do not create noisy skills.
- Low-resource focused tests pass.
