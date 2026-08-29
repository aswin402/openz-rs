# WebUI Per-Turn Cancellation

## Goal

Replace the WebUI `/stop` command’s process-wide cancellation broadcast with a cancellation token owned by one WebSocket client/chat turn. Add stable turn IDs to streamed lifecycle events and make the WebUI ignore stale events from a previously stopped turn.

## Constraints

- Preserve the existing CLI, Telegram, Discord, and WhatsApp cancellation behavior.
- Reuse the existing `CancellationToken` used by `AgentLoop`; do not introduce a second cancellation primitive.
- Keep stop authorization scoped to the authenticated WebSocket connection and its normalized chat ID.
- Do not run full-workspace Cargo commands. Use `just test-one <name> openz`, `just check openz`, and focused WebUI checks with at most two compiler jobs.
- Preserve unrelated dirty-worktree edits and stage only intended hunks.

## Design

1. Add a task-local `TurnCancellationContext { turn_id, token }` in the AgentLoop module with scope/accessor helpers. `run.rs` uses the scoped token when present, otherwise preserves its existing token creation for non-WebUI channels.
2. Add a WebSocket registry of active turns keyed by `turn_id`, storing the authenticated `client_id`, normalized `chat_id`, and token. Register before spawning the agent future and remove on completion. A stop request resolves only a turn owned by the same client/chat.
3. Change WebUI `/stop` handling to cancel the matching token and emit `stopped` with `turn_id`. It must not call the global `trigger_cli_cancel()` path. Disconnect cleanup cancels all turns owned by the socket.
4. Make in-flight provider and tool waits observe the turn token in addition to the existing global CLI/shutdown signals. Cancellation must interrupt streaming, non-streaming provider calls, and tool execution, then follow the existing save/settle path.
5. Include `turn_id` in WebUI `delta`, `reasoning_delta`, `tool_start`, `tool_end`, `error`, and `turn_end` events. Keep command responses and legacy non-WebUI events unchanged.
6. Track the active turn per chat in Zustand. Ignore deltas/tool events whose `turn_id` is not the current turn for that chat, and settle only the matching turn. Stop only clears the stopped chat’s turn state instead of the global `isStreaming` flag for every chat.

## Implementation tasks

### 1. Add failing cancellation tests

Files:

- `src/channels/websocket.rs`
- `src/agent/agent_loop/mod.rs`
- `src/tools/subagent/cancellation_token.rs`
- `web/src/types/websocket.test.ts` or a focused store test if available

Add tests for:

- a registered WebSocket turn can be cancelled by its owning client/chat;
- a different client or chat cannot cancel it;
- cancelling one turn leaves another client’s turn active;
- the AgentLoop cancellation context exposes the registered token to the turn;
- a stale turn ID is ignored by the WebUI event contract/store helper.

Run each new Rust test with `just test-one <test_name> openz` before implementation so the red phase proves the missing behavior.

### 2. Implement scoped token propagation and gateway registry

Files:

- `src/agent/agent_loop/mod.rs`
- `src/agent/agent_loop/run.rs`
- `src/agent/agent_loop/tool_execution.rs`
- `src/channels/websocket.rs`

Add the task-local context and active-turn registry. Wrap each spawned WebSocket agent run in its turn context, update `/stop` and disconnect cleanup, and add turn IDs to lifecycle events. Update provider/tool cancellation selects to observe the per-turn token.

### 3. Make WebUI state turn-aware

Files:

- `web/src/types/websocket.ts`
- `web/src/store/useOpenZStore.ts`
- `web/src/services/websocket.ts`

Carry `turn_id` on stop/message-related commands where needed, maintain `activeTurnIds` by chat, reject stale events, and settle only the matching turn. Keep compatibility for gateways that omit a turn ID by accepting the first active turn for that chat.

### 4. Verify and update roadmap

Run only:

- the focused Rust cancellation tests;
- `just check openz`;
- focused WebUI tests;
- `bun run lint`;
- `bun run build`;
- `git diff --check`.

Mark the cancellation item complete in `plan/webui-remediation-todo.md` and this plan only after the scoped checks pass. Review the staged diff to ensure no unrelated dirty-worktree changes are included.

## Completion checklist

- [ ] WebUI stop no longer triggers global CLI cancellation.
- [ ] Active turns are owned by client, chat, and turn ID.
- [ ] Wrong-client/wrong-chat stop requests are rejected.
- [ ] Provider streaming, provider requests, and tools observe turn cancellation.
- [ ] Lifecycle events carry turn IDs and stale events are ignored.
- [ ] One client’s stop does not affect another client’s active turn.
- [ ] Focused Rust/WebUI verification passes without full Cargo commands.
