# WebUI Configuration Write Reliability

## Goal

Ensure WebUI `set_config` changes are not silently lost while the gateway is disconnected or reconnecting, and make accepted, queued, rejected, and confirmed states visible to the Settings UI.

## Constraints

- Queue only idempotent configuration writes; chat prompts and other commands keep their current delivery semantics.
- Reuse stable WebSocket request IDs so a write can be replayed after a disconnect.
- Keep optimistic settings updates, but replace indefinite “waiting” with explicit queued/accepted/error notices.
- Use focused WebUI tests, `bun run lint`, and `bun run build`; do not run full-workspace Cargo commands.
- Preserve unrelated dirty-worktree edits and stage only intended hunks.

## Design

1. Add a reconnect queue and replay map to `OpenZWebSocketService` for `set_config` commands. Queue writes while disconnected, replay unacknowledged writes after reconnect, and remove them only after a command acknowledgement.
2. Emit a small `command_queued` event when a write is held for reconnect; retain backend `command_ack` and `config_updated` as the accepted/confirmed signals.
3. Make `updateConfig` and `sendSetConfig` use the queueable path.
4. Handle queued, accepted, rejected, and backend `config_update_rejected` events in Zustand with scoped Settings notices.
5. Add focused tests for retryable command classification and verify the WebUI build/lint gates.
6. Keep `archive/delete` session rows visible until the gateway confirms the mutation; refresh authoritative sessions and inventory after rejection.

## Completion checklist

- [x] Disconnected `set_config` writes are queued instead of dropped.
- [x] Unacknowledged config writes replay after reconnect.
- [x] Non-config commands are not silently replayed.
- [x] Settings exposes queued, accepted, rejected, and confirmed states.
- [x] Focused WebUI verification passes.
- [x] Session archive/delete mutations are confirmed before local removal and refresh on rejection.
