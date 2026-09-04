export const WS_STORAGE_KEYS = {
  url: 'openz_ws_url',
  token: 'openz_ws_token',
} as const;

export const UI_STORAGE_KEYS = {
  activeChatId: 'openz_active_chat_id',
  sidebarCollapsed: 'openz_sidebar_collapsed',
  activityPanelOpen: 'openz_activity_panel_open',
} as const;

export const TIMING_MS = {
  heartbeat: 15_000,
  reconnect: 3_000,
  memoryPoll: 8_000,
  inventoryPoll: 10_000,
} as const;

type ViteRuntimeEnv = { DEV?: boolean; VITE_OPENZ_WS_URL?: string; VITE_OPENZ_GATEWAY_URL?: string };

function runtimeEnv(): ViteRuntimeEnv {
  return (import.meta as ImportMeta & { env?: ViteRuntimeEnv }).env || {};
}

function gatewayWebSocketUrl(value: string): string {
  const protocol = value.startsWith('https://') ? 'wss://' : value.startsWith('http://') ? 'ws://' : '';
  const base = protocol ? protocol + value.replace(/^https?:\/\//, '') : value;
  return base.endsWith('/ws') ? base : base.replace(/\/$/, '') + '/ws';
}

export function resolveWebSocketUrl(location: Location, configured?: string): string {
  const env = runtimeEnv();
  const explicit = env.VITE_OPENZ_WS_URL?.trim();
  if (explicit) return explicit;

  const stored = configured?.trim();
  if (stored) return stored;

  if (env.DEV) {
    return gatewayWebSocketUrl(env.VITE_OPENZ_GATEWAY_URL?.trim() || 'http://127.0.0.1:8765');
  }

  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${location.host}/ws`;
}
