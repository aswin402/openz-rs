# Browser Acquisition Hardening Plan

## Problem

The v0.0.82 wallpaper workflow exposed four generic agent-runtime bugs:

- Browser fill calls could fail when model output used `value` or `query` instead of `text`.
- Repeated browser observation calls such as `snapshot` could be blocked as loops even though browser agents need observe/action/observe cycles.
- Search backend failures could make the agent declare the task blocked before trying browser-backed retrieval paths.
- Research auto-capture could save unrelated web/tool noise during download/show workflows, polluting future answers.

## Implementation

1. Accept fill aliases in `gsd_browser` while keeping `text` as the preferred argument.
2. Treat read-only browser observation actions as repeat-safe for loop detection.
3. Add acquisition/open workflow discipline to the runtime prompt so OpenZ tries browser and DOM/source fallbacks before declaring blocked.
4. Skip research auto-capture for acquisition/display tasks unless the user explicitly asks for research/comparison/analysis.
5. Add focused regressions for each runtime behavior and run tests one module at a time.

## Verification

- `cargo test --lib gsd_browser::tests -- --test-threads=1`
- `cargo test --lib agent_loop::loop_control::tests -- --test-threads=1`
- `cargo test --lib shared_memory::auto_capture::tests -- --test-threads=1`
- `cargo fmt --check`
