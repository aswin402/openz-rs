# WebSocket Protocol Contracts and Acknowledgements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add typed WebUI WebSocket command/event contracts and request-correlated command acknowledgements without breaking existing streaming or data events.

**Architecture:** Keep the existing JSON wire format and event names, but centralize outgoing command shapes and incoming event parsing in a TypeScript protocol module. Every command sent by the WebUI receives a monotonic `request_id`; the gateway emits a `command_ack` receipt with that ID and an accepted/rejected status. Existing command-specific events remain unchanged so current pages continue to work while the store can surface rejected commands.

**Tech Stack:** Rust/Axum WebSocket gateway, serde_json, React/TypeScript, Bun test runner, Vite.

## Global Constraints

- Preserve existing WebSocket event names and payload fields for backwards compatibility.
- Add request IDs without requiring IDs from legacy clients; commands without IDs continue to work.
- Do not run full workspace Cargo check/build/test commands; use `just check openz`, `just test-one <name> openz`, and WebUI focused Bun checks with parallelism capped by existing recipes.
- Do not add dependencies; use existing serde_json, browser WebSocket APIs, and Bun test.
- Do not change streaming turn ordering or the gateway’s existing command handlers beyond receipt acknowledgement.

---

### Task 1: Define the protocol contracts and failing tests

**Files:**
- Create: `web/src/types/websocket.ts`
- Create: `web/src/types/websocket.test.ts`

**Interfaces:**
- Produces `WebSocketCommand`, `WebSocketCommandEnvelope`, `WebSocketEventEnvelope`, `WebSocketCommandAckEvent`, `createRequestId`, `buildCommandEnvelope`, `parseWebSocketEvent`, and `isCommandAckEvent` for the WebSocket service and store.
- Consumes existing payload field names from `web/src/services/websocket.ts` and `src/channels/websocket.rs`.

- [x] **Step 1: Write the failing protocol tests**

Create tests for the required pure contract behavior:

```ts
import { describe, expect, test } from 'bun:test';
import {
  buildCommandEnvelope,
  createRequestId,
  isCommandAckEvent,
  parseWebSocketEvent,
} from './websocket';

describe('WebSocket protocol contracts', () => {
  test('creates unique request IDs and embeds them in command envelopes', () => {
    const first = createRequestId(1, 1700000000000);
    const second = createRequestId(2, 1700000000000);
    expect(first).not.toBe(second);
    expect(buildCommandEnvelope({ type: 'get_status' }, first)).toEqual({
      type: 'get_status',
      request_id: first,
    });
  });

  test('normalizes event and legacy type envelopes', () => {
    expect(parseWebSocketEvent({ event: 'ready', chat_id: 'chat-1' })).toMatchObject({
      event: 'ready',
      chat_id: 'chat-1',
    });
    expect(parseWebSocketEvent({ type: 'run_started', run_id: 'run-1' })).toMatchObject({
      event: 'run_started',
      run_id: 'run-1',
    });
    expect(parseWebSocketEvent({ detail: 'missing event' })).toBeNull();
  });

  test('recognizes only valid command acknowledgement events', () => {
    const ack = parseWebSocketEvent({
      event: 'command_ack',
      request_id: 'req-1',
      command: 'get_status',
      status: 'accepted',
    });
    expect(ack && isCommandAckEvent(ack)).toBe(true);
    expect(isCommandAckEvent(parseWebSocketEvent({ event: 'ready' }))).toBe(false);
  });
});
```

- [x] **Step 2: Run the protocol tests to verify the initial failure**

Run from `web/`:

```bash
bun test ./src/types/websocket.test.ts
```

Expected: FAIL because `websocket.ts` does not yet exist.

- [x] **Step 3: Implement the minimal typed command/event module**

Define the outgoing command union with all current WebUI command names and their existing fields:

```ts
export type WebSocketCommand =
  | { type: 'ping' }
  | { type: 'message'; chat_id: string; content: string; model?: string; provider?: string; attachments?: WebSocketAttachment[] }
  | { type: 'new_chat' }
  | { type: 'attach' | 'load_history' | 'archive_session' | 'delete_session'; chat_id: string }
  | { type: 'list_sessions' | 'get_cognitive_memory' | 'get_mcp_servers' | 'get_logs' | 'get_servers' | 'get_config' | 'get_slash_commands' | 'get_status' | 'get_runtime_inventory' }
  | { type: 'stop_server'; target: string }
  | { type: 'get_models'; provider?: string }
  | { type: 'toggle_favorite_model'; provider: string; model: string }
  | { type: 'set_config'; defaults?: Record<string, unknown>; providers?: Record<string, unknown>; channels?: Record<string, unknown> }
  | { type: 'save_skill'; name: string; content: string }
  | { type: 'delete_skill' | 'delete_subagent'; name: string }
  | { type: 'save_subagent'; name: string; description: string; systemPrompt: string; model?: string; fallbacks?: string[] }
  | { type: 'update_subagent_settings'; name: string; model?: string | null; fallbacks?: string[] | null }
  | { type: 'pause_cron_job' | 'resume_cron_job' | 'delete_cron_job'; id: string }
  | { type: 'get_cron_logs'; id?: string; limit?: number }
  | { type: 'security_response'; req_id: string; approved: boolean };

export interface WebSocketCommandEnvelope {
  type: WebSocketCommand['type'];
  request_id: string;
  [key: string]: unknown;
}

export interface WebSocketEventEnvelope {
  event: string;
  request_id?: string;
  [key: string]: unknown;
}

export interface WebSocketCommandAckEvent extends WebSocketEventEnvelope {
  event: 'command_ack';
  request_id: string;
  command: WebSocketCommand['type'];
  status: 'accepted' | 'rejected';
  detail?: string;
}
```

`createRequestId(sequence, now)` must produce a nonempty ID containing the timestamp and monotonically increasing sequence; `parseWebSocketEvent` must accept either `event` or legacy `type` string fields and return `null` for non-object/missing-name input. `isCommandAckEvent` must validate request ID, command, and status before narrowing.

- [x] **Step 4: Run the focused protocol tests**

Run:

```bash
bun test ./src/types/websocket.test.ts
```

Expected: all protocol tests pass.

- [x] **Step 5: Commit the protocol contract**

```bash
git add web/src/types/websocket.ts web/src/types/websocket.test.ts
git commit -m "feat: define websocket protocol contracts"
```

### Task 2: Route WebUI commands through request IDs

**Files:**
- Modify: `web/src/services/websocket.ts`

**Interfaces:**
- Consumes `WebSocketCommand`, `WebSocketCommandEnvelope`, `WebSocketEventEnvelope`, `WebSocketCommandAckEvent`, and helper functions from Task 1.
- Produces request-correlated JSON commands and a typed `command_ack` event; existing `on('delta', ...)` and data listeners retain their current behavior.

- [x] **Step 1: Add service-level request correlation behavior test**

Extend `web/src/types/websocket.test.ts` with a pure envelope assertion that covers the exact gateway field:

```ts
test('uses request_id rather than a UI-only correlation field', () => {
  const envelope = buildCommandEnvelope({ type: 'security_response', req_id: 'sec-1', approved: false }, 'req-9');
  expect(envelope.request_id).toBe('req-9');
  expect(envelope.req_id).toBe('sec-1');
});
```

Run `bun test ./src/types/websocket.test.ts`; it must fail until `buildCommandEnvelope` preserves command fields and adds `request_id`.

- [x] **Step 2: Implement typed send and parser integration**

Replace the untyped `Record<string, unknown>` send path with:

```ts
private requestSequence = 0;
private pendingCommands = new Map<string, WebSocketCommand['type']>();

private send(command: WebSocketCommand, options: { acknowledge?: boolean; requireConnected?: boolean } = {}): string | null {
  const requestId = options.acknowledge === false ? null : createRequestId(++this.requestSequence);
  if (!this.socketOpen) {
    if (options.requireConnected) throw new Error('WebSocket is not connected');
    if (requestId) this.emit('command_ack', {
      event: 'command_ack', request_id: requestId, command: command.type,
      status: 'rejected', detail: 'WebSocket is not connected',
    } satisfies WebSocketCommandAckEvent);
    return requestId;
  }
  const envelope = requestId ? buildCommandEnvelope(command, requestId) : command;
  if (requestId) this.pendingCommands.set(requestId, command.type);
  this.ws!.send(JSON.stringify(envelope));
  return requestId;
}
```

Use `acknowledge: false` only for the heartbeat ping. Route `sendMessage` through this method with `requireConnected: true`; route every other existing command method through it. In `onmessage`, parse JSON as `unknown`, call `parseWebSocketEvent`, emit only valid envelopes, and remove acknowledged IDs from `pendingCommands`. Keep the existing `event || type` compatibility through the parser.

- [x] **Step 3: Add typed acknowledgement listener overload**

Add an overload for `on('command_ack', ...)` accepting `WebSocketCommandAckEvent`, while retaining the existing broad listener overload for legacy event consumers. Do not rewrite all store listeners in this task.

- [x] **Step 4: Run WebUI checks**

Run from `web/`:

```bash
bun test ./src/types/websocket.test.ts
bun run lint
bun run build
```

Expected: tests and lint pass; Vite builds with only the existing chunk-size advisory.

- [x] **Step 5: Commit the service changes**

```bash
git add web/src/services/websocket.ts web/src/types/websocket.test.ts
git commit -m "feat: correlate websocket commands"
```

### Task 3: Emit gateway command acknowledgements

**Files:**
- Modify: `src/channels/websocket.rs`

**Interfaces:**
- Consumes optional incoming `request_id` values from JSON commands.
- Produces `{ event: "command_ack", request_id, command, status, detail? }` receipts on the requesting socket only.

- [x] **Step 1: Write failing Rust helper tests**

Add tests in the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn websocket_request_id_is_trimmed_and_bounded() {
    assert_eq!(ws_request_id(&serde_json::json!({"request_id": " req-7 "})), Some("req-7".to_string()));
    assert_eq!(ws_request_id(&serde_json::json!({"request_id": ""})), None);
    let oversized = "x".repeat(MAX_WS_REQUEST_ID_LEN + 1);
    assert_eq!(ws_request_id(&serde_json::json!({"request_id": oversized})), None);
}

#[test]
fn websocket_command_ack_has_stable_wire_shape() {
    let ack = command_ack_event("req-7", "get_status", "accepted", None);
    assert_eq!(ack["event"], "command_ack");
    assert_eq!(ack["request_id"], "req-7");
    assert_eq!(ack["command"], "get_status");
    assert_eq!(ack["status"], "accepted");
}
```

Run:

```bash
just test-one websocket_request_id_is_trimmed_and_bounded openz
```

Expected: FAIL because the helper functions do not yet exist.

- [x] **Step 2: Implement bounded request IDs and acknowledgement helper**

Add `MAX_WS_REQUEST_ID_LEN: usize = 128`, `ws_request_id(&Value) -> Option<String>`, and `command_ack_event(...) -> Value`. Include `detail` only when provided. Treat an absent request ID as a legacy command that receives no acknowledgement.

- [x] **Step 3: Acknowledge commands at the receive boundary**

Immediately after extracting `msg_type` in `handle_socket`, send an accepted acknowledgement when a valid request ID is present. Do not add IDs to existing event payloads. For the unknown-command arm, send a rejected acknowledgement with `detail: "Unknown WebSocket command."`. Heartbeat pings are acknowledged only when a caller explicitly supplies a request ID; the WebUI heartbeat will not supply one.

- [x] **Step 4: Run focused Rust checks**

Run only:

```bash
just test-one websocket_request_id_is_trimmed_and_bounded openz
just test-one websocket_command_ack_has_stable_wire_shape openz
just check openz
```

Expected: both tests pass and the package check succeeds with `-j 2`.

- [x] **Step 5: Commit the gateway changes**

```bash
git add src/channels/websocket.rs
git commit -m "feat: acknowledge websocket commands"
```

### Task 4: Surface rejected acknowledgements and update remediation tracking

**Files:**
- Modify: `web/src/store/useOpenZStore.ts`
- Modify: `plan/webui-remediation-todo.md`

**Interfaces:**
- Consumes typed `command_ack` events from `OpenZWebSocketService`.
- Produces a global error notice only for rejected command receipts; accepted receipts remain silent to avoid UI noise.

- [x] **Step 1: Write the store behavior test contract**

Document the reducer behavior in a focused pure assertion near the protocol tests:

```ts
test('rejected acknowledgements carry actionable detail', () => {
  const ack = parseWebSocketEvent({ event: 'command_ack', request_id: 'req-1', command: 'set_config', status: 'rejected', detail: 'WebSocket is not connected' });
  expect(isCommandAckEvent(ack)).toBe(true);
  expect(ack && isCommandAckEvent(ack) && ack.detail).toBe('WebSocket is not connected');
});
```

- [x] **Step 2: Add the acknowledgement listener**

Register one `wsService.on('command_ack', ...)` listener during store initialization:

```ts
wsService.on('command_ack', (payload) => {
  if (payload.status !== 'rejected') return;
  set({ workspaceNotice: {
    scope: 'global',
    type: 'error',
    message: payload.detail || `Gateway rejected ${payload.command}.`,
    timestamp: Date.now(),
  }});
});
```

Do not set `isStreaming` or alter chat messages for command receipts; command-specific errors still use their existing events.

- [x] **Step 3: Mark the protocol reliability item complete**

Change the unchecked item in `plan/webui-remediation-todo.md` to:

```md
- [x] Add typed WebSocket event/envelope contracts and command acknowledgement IDs.
```

- [x] **Step 4: Run final focused checks**

Run from `web/`:

```bash
bun test ./src/types/websocket.test.ts
bun run lint
bun run build
```

Run from the repository root:

```bash
git diff --check
just check openz
```

Expected: all focused checks pass; no full Cargo command is run.

- [x] **Step 5: Commit store and documentation changes**

```bash
git add web/src/store/useOpenZStore.ts plan/webui-remediation-todo.md docs/superpowers/plans/2026-08-29-websocket-protocol-acknowledgements.md
git commit -m "docs: track websocket protocol reliability"
```
