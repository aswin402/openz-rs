# OpenZ Future Updates Roadmap

This roadmap defines the path from the current `v0.0.x` line to `v0.1.0`.
The main rule is simple: stabilize the core runtime before adding large surface
areas like a desktop app or full RAG UI.

## Product Direction

The proposed direction is good.

OpenZ should not rush into a Tauri desktop app, a visual RAG system, or broad
marketplace work before the agent runtime is efficient, cancellable, resource
bounded, observable, and safe to leave running. A desktop app built on an
unstable core would only make bugs harder to diagnose. The right order is:

1. Make the core fast, stable, low-resource, and autonomous.
2. Harden tools, skills, subagents, memory, and server lifecycle.
3. Improve CLI ergonomics.
4. Build a full Ratatui TUI.
5. Add the final RAG system.
6. Ship `v0.1.0` with cross-platform install/update support.
7. Start the separate Tauri desktop project after the OpenZ core is stable.

## Research Notes

Current agent platforms point toward the same priorities:

- OpenAI's agent guidance recommends layered guardrails, human intervention
  points, and failure thresholds for high-risk actions. OpenZ should strengthen
  tool preflight checks and recovery before expanding UX.
- OpenAI Agents SDK guardrails include input, output, and tool guardrails. OpenZ
  should add explicit tool guardrails around risky tools instead of relying only
  on post-generated approvals.
- Claude Code exposes a mature CLI shape: print mode, resume/continue,
  permission modes, JSON output, and update commands. OpenZ CLI work after
  `v0.0.80` should copy that discipline, not only add more slash commands.
- Claude Code hooks show that lifecycle events such as `PreToolUse` and
  `SessionStart` are useful extension points. OpenZ should add internal hooks
  before plugin/desktop work.
- OpenHands Enterprise emphasizes control planes, access policy, reusable
  workflows, visibility, audit trails, and cost/performance tracking. OpenZ
  should prioritize local observability and managed autonomy.
- MCP registry and roadmap work shows that MCP ecosystem scale is real, but
  marketplace/discovery should come after core lifecycle and guardrail stability.

## Version Phases

### v0.0.63 to v0.0.79: Core Stability, Efficiency, Hardening

Goal: OpenZ should be safe and reliable enough to run 24/7 with low resources.

Focus areas:

- Cancellation reliability for TUI input, permission menus, long-running tools,
  subagents, browser tools, media tools, and nested workflows.
- Automatic managed server lifecycle for `npm run dev`, `bun run dev`, Vite,
  Next.js, Python HTTP servers, file viewers, browser launches, and media
  players. The agent should use managed lifecycle automatically; users should
  not need `/servers`, `/stop-server`, or `pkill` for normal cleanup.
- Resource watchdog for CPU, memory, disk, process count, runtime duration, and
  output size. Long tasks need clear budgets and graceful shutdown.
- Tool guardrails before execution, especially for shell, filesystem writes,
  network downloads, MCP servers, browser automation, and process management.
- Better error classification: distinguish model failure, tool timeout, denied
  permission, sandbox block, command failure, provider rate limit, malformed
  JSON, and user cancellation.
- Source-memory relevance hardening so local operational prompts never show
  unrelated research/source footers.
- Self-improvement quality control so workflows and skills are saved only when
  they are reusable, correct, scoped, and verified.
- Subagent isolation hardening: separate cancellation tokens, workspace scope,
  memory profile, model routing, and timeout budget.
- Low-resource mode improvements for laptops and small machines.
- Test isolation for memory, graph branches, server registry, self-improvement,
  source matching, cancellation, and approval menu behavior.

Deliverables before `v0.0.80`:

- No known TUI input slowdown after Esc/Ctrl+C.
- No known double-Enter approval bug.
- No unmanaged OpenZ-launched server left running after task completion.
- No repeated viewer/browser open loop after the user closes a file/app.
- Stable `openz_inventory` and `tool_catalog` results.
- Managed server registry survives restarts and can clean stale PIDs.
- `openz doctor` reports runtime health, disk pressure, stale servers, corrupt
  DBs, bad config, and orphaned tool outputs.
- Every bugfix has a focused regression test.

### v0.0.80 to v0.0.84: CLI Improvement

Goal: Make command-line usage predictable, scriptable, and clean.

Focus areas:

- Consistent command grammar across `agent`, `gateway`, `telegram`, `discord`,
  `whatsapp`, `email`, `subagent`, `sop`, `logs`, `doctor`, and `changelog`.
- Add machine-readable output where useful: `--json`, `--quiet`, `--verbose`,
  stable exit codes, and stable error objects.
- Promote important agent-readable tools into CLI commands only when human use
  is valuable. The agent should still prefer tools for automation.
- Improve install/update commands, including build profiles, disk-pressure
  checks, backup/rollback, and clearer failures.
- Prepare single-command installer design similar in spirit to Bun install,
  without shipping it until cross-platform release gates are met.

Deliverables before `v0.0.85`:

- CLI command naming is stable.
- Help text is clear and complete.
- Common automation paths can be scripted without parsing human TUI output.
- Runtime config inspection is reliable.

### v0.0.85 to v0.0.89: Full Ratatui TUI

Goal: Build the full terminal interface after the core and CLI are stable.

Keep `openz agent` lightweight. Add or evolve a full Ratatui workspace UI with
clear panes instead of turning every feature into chat text.

Target TUI panels:

- Chat/session panel.
- Tool activity panel.
- Approval/security panel.
- Managed servers panel.
- Logs/traces panel.
- Subagents panel.
- Skills/workflows panel.
- Memory summary panel, but not the full RAG UI yet.
- Provider/model status panel.

Hard requirements:

- First Enter must work in all menus.
- Esc/Ctrl+C must cancel the active layer only, then restore input cleanly.
- Long tool output must not freeze rendering.
- Streaming output must not corrupt layout.
- Remote channels must remain usable while the TUI is active.

Deliverables before `v0.0.90`:

- `openz agent` remains fast and simple.
- Full Ratatui mode is stable enough for daily use.
- Approval menus, server controls, logs, and tool state are visible without
  needing manual slash commands.

### v0.0.90 to v0.0.99: RAG System

Goal: Add the final retrieval system only after core/TUI stability.

Focus areas:

- Document ingestion for Markdown, text, PDF, DOCX, HTML, code, GitHub repos,
  and web pages.
- Chunking, deduplication, metadata extraction, freshness, citations, and
  deletion/tombstone support.
- Hybrid retrieval using lexical search, vector search, graph traversal, and
  recency scoring.
- Local-first privacy controls, including local embedding mode, cloud embedding
  mode, and cloud-only mode.
- RAG eval harness for precision, recall, hallucination risk, stale-source
  handling, and citation accuracy.
- Clear separation between memory, research briefs, code index, document index,
  and workflow skills.

Deliverables before `v0.1.0`:

- RAG answers cite exact sources.
- Stale and conflicting facts are handled correctly.
- Deletes/forget requests remove indexed data across all stores.
- Resource usage remains bounded during large ingestion.
- RAG can be used by TUI, CLI, channels, and subagents through one shared API.

### v0.1.0: Stable Core Release

Goal: First stable public release target.

Release requirements:

- Linux, macOS, and Windows builds.
- x86_64 and ARM64 support where practical.
- Single-command install/update/uninstall flow.
- Signed or checksummed binaries.
- Clear release notes and migration notes.
- Stable config schema with migration support.
- Stable runtime data layout under `~/.openz` or platform-specific equivalents.
- `openz doctor` can diagnose common install/runtime problems.
- Core docs are accurate: README, CHANGELOG, docs, onpkg docs, install guide,
  security guide, tool guide, and troubleshooting guide.
- Regression suite covers the known bug classes from `v0.0.56` through
  `v0.0.79`.

## Post-v0.1.0: Separate Tauri Desktop Project

The desktop app should be a separate project after OpenZ core stability.

Direction:

- Tauri app as GUI shell.
- OpenZ core remains the primary backend and first-priority agent.
- The app may also connect to other LLMs or external agents.
- Desktop should not duplicate core agent logic.
- Desktop features should call stable OpenZ APIs/tools instead of bypassing the
  runtime.

Reasoning:

Keeping desktop separate prevents UI work from destabilizing the agent runtime.
It also lets OpenZ remain usable as a single binary, CLI, TUI, server, and
automation backend.

## Not Now

These are explicitly deferred:

- Tauri desktop app before `v0.1.0`.
- Full visual RAG browser before the RAG core is stable.
- Public plugin marketplace before guardrails and lifecycle hooks are stable.
- Large MCP marketplace/discovery UX before resource limits and MCP security are
  hardened.
- More flashy demo features that do not improve reliability, speed, autonomy,
  or user trust.

## Engineering Gates

Every release should satisfy:

- `cargo fmt --check`
- `cargo check`
- Focused regression tests for the changed behavior.
- `cargo test --lib -- --test-threads=1` before tagging.
- `git diff --check`
- Version sync across `Cargo.toml`, `Cargo.lock`, `README.md`, `CHANGELOG.md`,
  and package metadata when a release version changes.
- No unrelated source-memory footer on local action prompts.
- No unmanaged background process left from agent-launched servers.
- No hidden destructive cleanup. Use managed lifecycle first; use `pkill` only
  as an explicit last-resort recovery path.

## Immediate Next Backlog

Recommended next work for `v0.0.63+`:

1. Add tool preflight guardrail hooks before sensitive tool execution.
2. Add a lifecycle event bus for `SessionStart`, `PreToolUse`, `PostToolUse`,
   `ToolError`, `ServerStarted`, `ServerStopped`, and `TurnCancelled`.
3. Harden managed server auto-cleanup with stale PID detection and task-scoped
   ownership.
4. Add resource watchdog metrics and local-only observability output.
5. Add approval-menu regression tests for first-Enter behavior.
6. Add cancellation regression tests for nested subagents and long-running tools.
7. Add self-improvement skill/workflow validation before saving new skills.
8. Add docs-health checks for README/docs/onpkg consistency.

