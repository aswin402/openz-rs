import type { ConnectionStatus } from '../types';
import {
  buildCommandEnvelope,
  createRequestId,
  isCommandAckEvent,
  isRetryableWebSocketCommand,
  parseWebSocketEvent,
  type WebSocketCommand,
  type WebSocketCommandAckEvent,
  type WebSocketAttachment,
} from '../types/websocket';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type EventListener = (data: any) => void;
type QueuedCommand = { command: WebSocketCommand; requestId: string | null };

export function defaultWebSocketUrl(): string {
  if (typeof window === 'undefined') return '';
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws`;
}

export class OpenZWebSocketService {
  private ws: WebSocket | null = null;
  private url: string = defaultWebSocketUrl();
  private token: string = '';
  private listeners: Map<string, Set<EventListener>> = new Map();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private status: ConnectionStatus = 'disconnected';
  private onStatusChange: ((status: ConnectionStatus) => void) | null = null;
  private requestSequence = 0;
  private pendingCommands = new Map<string, WebSocketCommand['type']>();
  private queuedCommands: QueuedCommand[] = [];
  private replayableCommands = new Map<string, QueuedCommand>();

  constructor() {
    const savedUrl = localStorage.getItem('openz_ws_url');
    const savedToken = localStorage.getItem('openz_ws_token');
    if (savedUrl) this.url = savedUrl;
    if (savedToken) this.token = savedToken;
  }

  public setConfig(url: string, token: string) {
    this.url = url;
    this.token = token;
    localStorage.setItem('openz_ws_url', url);
    localStorage.setItem('openz_ws_token', token);
    this.connect();
  }

  public setStatusCallback(cb: (status: ConnectionStatus) => void) {
    this.onStatusChange = cb;
  }

  private updateStatus(status: ConnectionStatus) {
    this.status = status;
    if (this.onStatusChange) {
      this.onStatusChange(status);
    }
  }

  public getStatus(): ConnectionStatus {
    return this.status;
  }

  private get socketOpen(): boolean {
    return !!this.ws && this.ws.readyState === WebSocket.OPEN;
  }

  /** Send a typed command and attach a request ID unless acknowledgements are disabled. */
  private send(
    command: WebSocketCommand,
    options: { acknowledge?: boolean; requireConnected?: boolean; queueIfDisconnected?: boolean } = {},
  ): string | null {
    const requestId = options.acknowledge === false ? null : createRequestId(++this.requestSequence);
    if (!this.socketOpen) {
      if (options.requireConnected) {
        throw new Error('WebSocket is not connected');
      }
      if (options.queueIfDisconnected && isRetryableWebSocketCommand(command)) {
        const queued = { command, requestId };
        this.queuedCommands.push(queued);
        if (requestId) {
          this.pendingCommands.set(requestId, command.type);
          this.replayableCommands.set(requestId, queued);
        }
        this.emit('command_queued', {
          event: 'command_queued',
          request_id: requestId,
          command: command.type,
        });
        return requestId;
      }
      console.warn('[ws] Dropped command, socket not connected:', command.type);
      if (requestId) {
        this.emit('command_ack', {
          event: 'command_ack',
          request_id: requestId,
          command: command.type,
          status: 'rejected',
          detail: 'WebSocket is not connected',
        } satisfies WebSocketCommandAckEvent);
      }
      return requestId;
    }

    const envelope = requestId ? buildCommandEnvelope(command, requestId) : command;
    if (requestId) {
      this.pendingCommands.set(requestId, command.type);
      if (isRetryableWebSocketCommand(command)) {
        this.replayableCommands.set(requestId, { command, requestId });
      }
    }
    this.ws!.send(JSON.stringify(envelope));
    return requestId;
  }

  private flushQueuedCommands() {
    if (!this.socketOpen || this.queuedCommands.length === 0) return;
    const queued = this.queuedCommands.splice(0);
    for (let index = 0; index < queued.length; index += 1) {
      if (!this.socketOpen) {
        this.queuedCommands.unshift(...queued.slice(index));
        return;
      }
      const entry = queued[index];
      const envelope = entry.requestId
        ? buildCommandEnvelope(entry.command, entry.requestId)
        : entry.command;
      try {
        this.ws!.send(JSON.stringify(envelope));
      } catch {
        this.queuedCommands.unshift(...queued.slice(index));
        return;
      }
    }
  }

  private requeueUnacknowledgedConfigWrites() {
    for (const entry of this.replayableCommands.values()) {
      if (!this.queuedCommands.some((queued) => queued.requestId === entry.requestId)) {
        this.queuedCommands.push(entry);
      }
    }
  }

  public connect() {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    if (!this.url.trim()) {
      this.updateStatus('error');
      return;
    }

    this.updateStatus('connecting');

    try {
      let fullUrl = this.url;
      if (this.token) {
        const separator = fullUrl.includes('?') ? '&' : '?';
        fullUrl += `${separator}token=${encodeURIComponent(this.token)}`;
      }

      this.ws = new WebSocket(fullUrl);

      this.ws.onopen = () => {
        this.updateStatus('connected');
        if (this.reconnectTimer) {
          clearTimeout(this.reconnectTimer);
          this.reconnectTimer = null;
        }
        this.startHeartbeat();
        this.flushQueuedCommands();
      };

      this.ws.onmessage = (event) => {
        try {
          const payload = parseWebSocketEvent(JSON.parse(event.data) as unknown);
          if (!payload) {
            console.error('Invalid WebSocket event envelope:', event.data);
            return;
          }
          if (isCommandAckEvent(payload)) {
            this.pendingCommands.delete(payload.request_id);
            this.replayableCommands.delete(payload.request_id);
          }
          this.emit(payload.event, payload);
          this.emit('*', payload);
        } catch (err) {
          console.error('Failed to parse WebSocket message:', err, event.data);
        }
      };

      this.ws.onerror = (err) => {
        console.error('WebSocket error:', err);
        if (this.status !== 'unauthorized') {
          this.updateStatus('error');
        }
      };

      this.ws.onclose = (event) => {
        this.stopHeartbeat();
        this.requeueUnacknowledgedConfigWrites();
        if (event.code === 4001 || event.reason === 'Unauthorized') {
          this.updateStatus('unauthorized');
        } else {
          this.updateStatus('disconnected');
          this.scheduleReconnect();
        }
      };
    } catch (err) {
      console.error('Failed to initiate WebSocket connection:', err);
      this.updateStatus('error');
      this.scheduleReconnect();
    }
  }

  private startHeartbeat() {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      if (this.socketOpen) {
        this.send({ type: 'ping' }, { acknowledge: false });
      }
    }, 15000);
  }

  private stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private scheduleReconnect() {
    if (!this.reconnectTimer) {
      this.reconnectTimer = setTimeout(() => {
        this.reconnectTimer = null;
        this.connect();
      }, 3000);
    }
  }

  public disconnect() {
    this.stopHeartbeat();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.updateStatus('disconnected');
  }

  // ---- Chat / session actions ----

  public sendMessage(
    chatId: string,
    content: string,
    model?: string,
    provider?: string,
    attachments?: WebSocketAttachment[],
  ) {
    const payload: Extract<WebSocketCommand, { type: 'message' }> = { type: 'message', chat_id: chatId, content };
    if (model) payload.model = model;
    if (provider) payload.provider = provider;
    if (attachments?.length) payload.attachments = attachments;
    return this.send(payload, { requireConnected: true });
  }

  public createNewChat() {
    this.send({ type: 'new_chat' });
  }

  public attachChat(chatId: string) {
    this.send({ type: 'attach', chat_id: chatId });
  }

  public sendStop(chatId: string, turnId?: string) {
    const payload: Extract<WebSocketCommand, { type: 'message' }> = {
      type: 'message',
      chat_id: chatId,
      content: '/stop',
    };
    if (turnId) payload.turn_id = turnId;
    return this.send(payload, { requireConnected: true });
  }

  public requestSessions() {
    this.send({ type: 'list_sessions' });
  }

  public requestHistory(chatId: string) {
    this.send({ type: 'load_history', chat_id: chatId });
  }

  public archiveSession(chatId: string) {
    this.send({ type: 'archive_session', chat_id: chatId });
  }

  public deleteSession(chatId: string) {
    this.send({ type: 'delete_session', chat_id: chatId });
  }

  public requestCognitiveMemory() {
    this.send({ type: 'get_cognitive_memory' });
  }

  public requestMcpServers() {
    this.send({ type: 'get_mcp_servers' });
  }

  public requestLogs() {
    this.send({ type: 'get_logs' });
  }

  public requestServers() {
    this.send({ type: 'get_servers' });
  }

  public stopServer(target: string) {
    this.send({ type: 'stop_server', target });
  }

  // ---- Realtime data commands (replaces hardcoded frontend values) ----

  /** Fetch configured providers with a small model preview. */
  public requestModels() {
    this.send({ type: 'get_models' });
  }

  /** Fetch the full model list for one configured provider. */
  public requestProviderModels(provider: string) {
    this.send({ type: 'get_models', provider });
  }

  public toggleFavoriteModel(provider: string, model: string) {
    this.send({ type: 'toggle_favorite_model', provider, model });
  }

  /** Fetch editable agent defaults, skills, mcp servers and version. */
  public requestConfig() {
    this.send({ type: 'get_config' });
  }

  public updateConfig(patch: Record<string, unknown>) {
    return this.send(
      { type: 'set_config', defaults: patch },
      { queueIfDisconnected: true },
    );
  }

  public sendSetConfig(data: { defaults?: Record<string, unknown>; providers?: Record<string, unknown>; channels?: Record<string, unknown> }) {
    return this.send(
      { type: 'set_config', ...data },
      { queueIfDisconnected: true },
    );
  }

  public saveSkill(name: string, content: string) {
    this.send({ type: 'save_skill', name, content });
  }

  public deleteSkill(name: string) {
    this.send({ type: 'delete_skill', name });
  }

  public saveSubagent(data: { name: string; description: string; systemPrompt: string; model?: string; fallbacks?: string[] }) {
    this.send({ type: 'save_subagent', ...data });
  }

  public updateSubagentSettings(data: { name: string; model?: string | null; fallbacks?: string[] | null }) {
    this.send({ type: 'update_subagent_settings', ...data });
  }

  public deleteSubagent(name: string) {
    this.send({ type: 'delete_subagent', name });
  }

  /** Fetch the real slash command list from the backend. */
  public requestSlashCommands() {
    this.send({ type: 'get_slash_commands' });
  }

  /** Fetch gateway/agent status (version + MCP counts). */
  public requestStatus() {
    this.send({ type: 'get_status' });
  }

  public requestRuntimeInventory() {
    this.send({ type: 'get_runtime_inventory' });
  }
  public pauseCronJob(id: string) {
    this.send({ type: 'pause_cron_job', id });
  }

  public resumeCronJob(id: string) {
    this.send({ type: 'resume_cron_job', id });
  }

  public deleteCronJob(id: string) {
    this.send({ type: 'delete_cron_job', id });
  }

  public requestCronLogs(id?: string, limit = 20) {
    this.send({ type: 'get_cron_logs', id, limit });
  }


  /** Resolve a pending security-approval request. */
  public sendSecurityResponse(reqId: string, approved: boolean) {
    this.send({ type: 'security_response', req_id: reqId, approved });
  }

  // ---- Event bus ----

  public on(event: 'command_ack', fn: (data: WebSocketCommandAckEvent) => void): void;
  public on(event: string, fn: EventListener): void;
  public on(event: string, fn: EventListener) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(fn);
  }

  public off(event: string, fn: EventListener) {
    if (this.listeners.has(event)) {
      this.listeners.get(event)!.delete(fn);
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private emit(event: string, data: any) {
    if (this.listeners.has(event)) {
      this.listeners.get(event)!.forEach((fn) => fn(data));
    }
  }
}

export const wsService = new OpenZWebSocketService();