/**
 * Service-facing protocol boundary.
 *
 * The structural definitions remain in `types/` so non-transport consumers can
 * validate gateway payloads without importing the WebSocket service. This
 * module gathers the command and event contracts used by the transport layer.
 */
export {
  buildCommandEnvelope,
  createRequestId,
  isCommandAckEvent,
  isRetryableWebSocketCommand,
  parseWebSocketEvent,
} from '../../types/websocket';
export type {
  WebSocketAttachment,
  WebSocketCommand,
  WebSocketCommandAckEvent,
  WebSocketCommandEnvelope,
  WebSocketCommandName,
  WebSocketEventEnvelope,
} from '../../types/websocket';

export {
  normalizeCronRuns,
  normalizeRuntimeInventory,
} from '../../types/protocol';
export type { WebSocketEventMap } from '../../types/protocol';
