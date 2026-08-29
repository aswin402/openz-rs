export interface WebSocketAttachment {
  name: string;
  mime: string;
  size: number;
  data: string;
}

export type WebSocketCommand =
  | { type: 'ping' }
  | { type: 'message'; chat_id: string; content: string; model?: string; provider?: string; turn_id?: string; attachments?: WebSocketAttachment[] }
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

export type WebSocketCommandName = WebSocketCommand['type'];

export interface WebSocketCommandEnvelope {
  type: WebSocketCommandName;
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
  command: WebSocketCommandName;
  status: 'accepted' | 'rejected';
  detail?: string;
}

const COMMAND_NAMES = new Set<WebSocketCommandName>([
  'ping',
  'message',
  'new_chat',
  'attach',
  'load_history',
  'archive_session',
  'delete_session',
  'list_sessions',
  'get_cognitive_memory',
  'get_mcp_servers',
  'get_logs',
  'get_servers',
  'get_config',
  'get_slash_commands',
  'get_status',
  'get_runtime_inventory',
  'stop_server',
  'get_models',
  'toggle_favorite_model',
  'set_config',
  'save_skill',
  'delete_skill',
  'delete_subagent',
  'save_subagent',
  'update_subagent_settings',
  'pause_cron_job',
  'resume_cron_job',
  'delete_cron_job',
  'get_cron_logs',
  'security_response',
]);

export function createRequestId(sequence: number, now = Date.now()): string {
  return `req-${now.toString(36)}-${sequence.toString(36)}`;
}

export function buildCommandEnvelope(command: WebSocketCommand, requestId: string): WebSocketCommandEnvelope {
  if (!requestId.trim()) throw new Error('WebSocket request ID cannot be empty');
  return { ...command, request_id: requestId } as WebSocketCommandEnvelope;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function parseWebSocketEvent(value: unknown): WebSocketEventEnvelope | null {
  if (!isRecord(value)) return null;
  const event = typeof value.event === 'string' ? value.event : typeof value.type === 'string' ? value.type : '';
  if (!event.trim()) return null;
  return { ...value, event };
}

export function isCommandAckEvent(value: WebSocketEventEnvelope | null): value is WebSocketCommandAckEvent {
  if (!value || value.event !== 'command_ack') return false;
  return (
    typeof value.request_id === 'string' &&
    value.request_id.trim().length > 0 &&
    typeof value.command === 'string' &&
    COMMAND_NAMES.has(value.command as WebSocketCommandName) &&
    (value.status === 'accepted' || value.status === 'rejected')
  );
}


/** Return false when an event belongs to an older turn for the same chat. */
export function isTurnEventCurrent(
  value: { chat_id?: unknown; turn_id?: unknown },
  activeTurns: Record<string, string>,
): boolean {
  const turnId = typeof value.turn_id === 'string' ? value.turn_id.trim() : '';
  if (!turnId) return true;
  const chatId = typeof value.chat_id === 'string' ? value.chat_id.trim() : '';
  if (!chatId) return true;
  return activeTurns[chatId] === turnId;
}

/** Configuration patches are idempotent and safe to replay after reconnect. */
export function isRetryableWebSocketCommand(command: WebSocketCommand): boolean {
  return command.type === 'set_config';
}
