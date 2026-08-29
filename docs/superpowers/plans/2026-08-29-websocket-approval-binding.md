# WebSocket Security Approval Binding

## Goal

Bind every WebUI security approval to the WebSocket client and chat that initiated the agent turn. Deliver the approval prompt only to that socket, reject responses from another client or chat without consuming the pending request, and resolve pending requests when a client disconnects.

## Constraints

- Preserve the existing CLI and Telegram approval flows.
- Keep the change scoped to the gateway, approval bridge, and the existing WebUI notice surface.
- Do not run full-workspace Cargo commands. Use `just test-one <name> openz`, `just check openz`, and focused WebUI checks with at most two compiler jobs.
- Preserve unrelated worktree edits and stage only the intended hunks.

## Design

1. Replace the approval map value with a `PendingWsApproval` containing an `WsApprovalContext { client_id, chat_id }` and the oneshot sender.
2. Add a task-local approval context scope around each WebSocket agent turn. The existing `ask_approval` API remains compatible for CLI/Telegram callers and reads the context only when the current turn originated from WebUI.
3. Add targeted event delivery to one active WebSocket sender. WebUI security requests use this path when a context exists; non-WebUI fallback behavior remains unchanged.
4. Validate `client_id` and normalized `chat_id` in `resolve_ws_approval`. A mismatch or unknown request returns `false`, leaves a mismatched pending request untouched, and sends the requesting socket a `security_response_rejected` event with a safe generic explanation.
5. Cancel all pending requests for a disconnected client with `approved = false`, preventing approval futures from waiting for the timeout and preventing stale request IDs from being reused.
6. Surface rejected responses as a global WebUI error notice so users understand why a response did not take effect.

## Implementation tasks

### 1. Add failing approval-isolation tests

Files:

- `src/channels/websocket.rs`

Add focused Tokio unit tests for:

- a response from the wrong client or wrong chat being rejected while the matching response succeeds;
- disconnect cleanup resolving a pending approval as denied;
- targeted security-response rejection payload containing a generic detail and the request/chat identifiers.

Run each new test with `just test-one <test_name> openz` before implementing the helpers so the tests demonstrate the missing behavior.

### 2. Implement context-aware approval storage and delivery

Files:

- `src/channels/websocket.rs`
- `src/agent/security.rs`

Add the context/pending types, task-local scope/accessor, target sender helper, guarded resolve/cancel functions, and the rejection-event helper. Wrap the spawned WebSocket agent turn in the client/chat scope. Update the security response branch to pass the authenticated socket’s client ID and normalized chat ID, and update the WebUI approval branch to register and publish using the scoped context.

### 3. Integrate rejection feedback and checklist status

Files:

- `web/src/store/useOpenZStore.ts`
- `plan/webui-remediation-todo.md`

Listen for `security_response_rejected` and show a global error notice. Mark the approval-binding item complete after focused tests pass.

### 4. Verify and review

Run only:

- `just test-one websocket_approval_rejects_wrong_client_or_chat openz`
- `just test-one websocket_approval_cancels_on_client_disconnect openz`
- `just check openz`
- `bun test ./src/types/websocket.test.ts`
- `bun run lint`
- `bun run build`
- `git diff --check`

Review the final diff for accidental staging of unrelated WebSocket/store changes, confirm no approval payload contains secrets, and update this plan’s checklist when complete.

## Completion checklist

- [x] Pending approvals carry client and chat ownership.
- [x] Security prompts are delivered only to the initiating socket.
- [x] Wrong-client and wrong-chat responses are rejected and tested.
- [x] Disconnects deny and clean up pending approvals.
- [x] WebUI shows a useful rejection notice.
- [x] Focused Rust/WebUI verification passes without full Cargo commands.
