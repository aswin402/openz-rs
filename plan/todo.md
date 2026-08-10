# OpenZ Task Lifecycle Manager TODO

## Phase 1: Registry

- [x] Create `src/tools/task_manager.rs`.
- [x] Add `TaskKind`: `browser`, `server`, `agent`, `subagent`, `mcp`, `watcher`, `background_job`.
- [x] Add `TaskOwner`: `openz`, `external`, `user`.
- [x] Add `CleanupPolicy`: `on_turn_end`, `on_session_end`, `manual`, `keep_alive`.
- [x] Add `ManagedTask` fields: id, kind, owner, purpose, command, pid, port, session_id, started_at, last_used_at, ttl_secs, cleanup_policy.
- [x] Add `register_task`.
- [x] Add `list_tasks`.
- [x] Add `stop_tasks`.
- [x] Add `cleanup_expired_tasks`.
- [x] Add `cleanup_turn_end_tasks`.
- [x] Add unit tests for registry list, stop, and cleanup.

## Phase 2: Native Tool

- [x] Implement `ManageTasksTool`.
- [x] Support `manage_tasks {"action":"list"}`.
- [x] Support `manage_tasks {"action":"cleanup"}`.
- [x] Support `manage_tasks {"action":"stop","target":"<id|kind|purpose|all>"}`.
- [x] Register `manage_tasks` in `src/cli/tools.rs`.
- [x] Add targeted tests for tool JSON output.

## Phase 3: Existing Process Integration

- [x] Extend `shutdown.rs` process metadata into task registry.
- [x] Mark OpenZ-spawned child processes as `owner=openz`.
- [x] Keep existing `/servers` and `/stop-server` behavior compatible.
- [x] Ensure `shell.rs` dev server processes show in `manage_tasks`.
- [x] Ensure `firefox.rs` geckodriver process shows in `manage_tasks`.
- [x] Ensure `browser_common.rs` Obscura/Chrome process shows in `manage_tasks`.
- [ ] Add tests that spawned child processes appear as managed tasks.

## Phase 4: Browser Broker

- [x] Create `src/tools/browser_broker.rs`.
- [x] Add backend priority: Obscura headless -> Firefox headless -> GSD/Chrome GUI.
- [x] Add `eval_with_browser_broker`.
- [x] Add `render_with_browser_broker`.
- [x] Register browser tasks with purpose and cleanup policy.
- [x] Ensure Obscura closes tabs after use.
- [x] Ensure Firefox can close session when task ends.
- [x] Ensure GSD/Chrome GUI is only used as last fallback.
- [x] Add diagnostics for backend used, fallbacks tried, cleanup result.

## Phase 5: Search/Research Upgrade

- [x] Route `SearchXyzBrowserSearchTool` through browser broker.
- [x] Keep rendered DOM extraction first.
- [x] Keep static page-source parser as fallback.
- [x] Return `backend`, `cleanup`, `fallbacks_tried`, and `extraction_strategy`.
- [x] Keep DuckDuckGo then Bing engine fallback.
- [x] Improve all-search-failed diagnostics to show native, browser, and external attempts.
- [ ] Add targeted tests for broker-backed browser search response.

## Phase 6: Automatic UX and UI

- [x] Keep lifecycle management automatic through `manage_tasks` and cleanup hooks.
- [x] Do not add manual `/tasks` or `/stop-task` slash commands.
- [x] Keep existing `/servers` and `/stop-server` compatibility for already-supported dev server workflows.
- [ ] Add WebUI Running Resources panel later, fed by `inspect_browsers`/`manage_tasks` data.
- [ ] Add safe stop buttons in WebUI for OpenZ-owned resources only.
- [ ] Add browser health and cleanup controls in WebUI settings/dashboard.

## Phase 7: Auto Cleanup

- [x] Clean `on_turn_end` tasks after each agent turn.
- [ ] Clean `on_session_end` tasks on shutdown.
- [ ] Clean expired TTL tasks opportunistically before listing tasks.
- [x] Never auto-stop `owner=external`.
- [ ] Require approval before stopping unknown external/user resources.
- [x] Log cleanup actions with task id, kind, purpose, and result.

## Phase 8: Verification

- [ ] Run `cargo test -j 2 --lib task_registry_lists_registered_openz_owned_task -- --nocapture`.
- [ ] Run `cargo test -j 2 --lib manage_tasks_lists_registered_tasks -- --nocapture`.
- [ ] Run `cargo test -j 2 --lib browser_backend_priority_prefers_obscura_then_firefox_then_gsd -- --nocapture`.
- [ ] Run `cargo test -j 2 --lib browser_search_response_includes_backend_and_cleanup_diagnostics -- --nocapture`.
- [x] Run `git diff --check`.
- [ ] Manually verify live search uses Obscura headless first.
- [ ] Manually verify fallback to Firefox headless.
- [ ] Manually verify GSD/Chrome GUI is last fallback.
- [ ] Manually verify `manage_tasks list` after browser research.
- [ ] Manually verify cleanup after turn completion.

## Phase 9: Release

- [ ] Update `CHANGELOG.md`.
- [ ] Increment version by `0.0.1`.
- [ ] Run focused tests only.
- [ ] Commit changes.
- [ ] Push to GitHub after confirmation.

