# Phase 40: WebSocket Monolith Test Suite Decomposition Plan

## Context & Objectives
In `src/channels/websocket/`, `tests.rs` remained as an 855-line monolithic test suite containing 32 disparate unit tests covering attachments, turns, approvals, events, handlers, sockets, protocols, and commands. This violates the 1:1 sibling test modularization standard established across `openz` in Phases 38 and 39 (`memory_extra` and `self_management`).

This phase decomposes `src/channels/websocket/tests.rs` into focused, component-local sibling test modules:
1. `src/channels/websocket/attachments_tests.rs` (1 test) -> linked from `attachments.rs`
2. `src/channels/websocket/turns_tests.rs` (2 tests) -> linked from `turns.rs`
3. `src/channels/websocket/approvals_tests.rs` (4 tests) -> linked from `approvals.rs`
4. `src/channels/websocket/auth_tests.rs` (expanded from 1 to 5 tests) -> linked from `auth.rs`
5. `src/channels/websocket/events_tests.rs` (3 tests) -> linked from `events.rs`
6. `src/channels/websocket/handlers_tests.rs` (4 tests) -> linked from `handlers.rs`
7. `src/channels/websocket/socket_tests.rs` (2 tests) -> linked from `socket.rs`
8. `src/channels/websocket/protocol_tests.rs` (3 tests) -> linked from `protocol.rs`
9. `src/channels/websocket/commands/cron_tests.rs` (1 test) -> linked from `commands/cron.rs`
10. `src/channels/websocket/commands/sessions_tests.rs` (2 tests) -> linked from `commands/sessions.rs`
11. `src/channels/websocket/commands/config_tests.rs` (6 tests) -> linked from `commands/config.rs`

And eliminates `src/channels/websocket/tests.rs` entirely.

## Execution Tasks

### Task 1: Decompose Attachments, Turns, Approvals, and Auth Tests
- Create `src/channels/websocket/attachments_tests.rs` (1 test).
- Create `src/channels/websocket/turns_tests.rs` (2 tests).
- Create `src/channels/websocket/approvals_tests.rs` (4 tests).
- Expand `src/channels/websocket/auth_tests.rs` with the 4 auth tests from `tests.rs` (total 5 tests).
- Wire up test modules in `attachments.rs`, `turns.rs`, `approvals.rs`, and `auth.rs`.
- Verify with `CARGO_INCREMENTAL=0 cargo test -p openz --lib channels::websocket -j 1`.
- Commit.

### Task 2: Decompose Events, Handlers, Socket, and Protocol Tests
- Create `src/channels/websocket/events_tests.rs` (3 tests).
- Create `src/channels/websocket/handlers_tests.rs` (4 tests).
- Create `src/channels/websocket/socket_tests.rs` (2 tests).
- Create `src/channels/websocket/protocol_tests.rs` (3 tests).
- Wire up test modules in `events.rs`, `handlers.rs`, `socket.rs`, and `protocol.rs`.
- Verify with `CARGO_INCREMENTAL=0 cargo test -p openz --lib channels::websocket -j 1`.
- Commit.

### Task 3: Decompose Commands Tests (Cron, Sessions, Config) & Delete Monolith `tests.rs`
- Create `src/channels/websocket/commands/cron_tests.rs` (1 test).
- Create `src/channels/websocket/commands/sessions_tests.rs` (2 tests).
- Create `src/channels/websocket/commands/config_tests.rs` (6 tests).
- Wire up test modules in `commands/cron.rs`, `commands/sessions.rs`, and `commands/config.rs`.
- Remove `#[cfg(test)] mod tests;` from `src/channels/websocket/mod.rs`.
- Delete `src/channels/websocket/tests.rs`.
- Verify full websocket test suite passes: `CARGO_INCREMENTAL=0 cargo test -p openz --lib channels::websocket -j 1`.
- Commit.

### Task 4: Invariant & Linter Verification
- Run native tool registration names test: `CARGO_INCREMENTAL=0 cargo test -p openz --lib test_native_tool_registration_names -j 1` (assert exact 260 registered tools).
- Run clippy check: `cargo clippy -p openz --lib -j 2` (assert 0 warnings).

### Task 5: Release Bump (v0.0.188), Documentation & Artifact
- Bump version to `0.0.188` across `Cargo.toml`, `onpkg.json`, and `README.md`.
- Add release notes for `v0.0.188` in `CHANGELOG.md` with Ideas, Inspirations, Sources & References, Subsystem Modularization Details, and Verification.
- Add Section 4.52 in `recommendedfix.md` and update summary table.
- Verify release versions match: `CARGO_INCREMENTAL=0 cargo test -p openz --lib release_version_surfaces_match_cargo_package_version -j 1`.
- Generate Phase 40 artifact.
- Commit release.
