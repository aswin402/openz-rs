import type { ConnectionStatus } from '../types';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type EventListener = (data: any) => void;

export class OpenZWebSocketService {
  private ws: WebSocket | null = null;
  private url: string = 'ws://127.0.0.1:8765/ws';
  private token: string = '';
  private listeners: Map<string, Set<EventListener>> = new Map();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private status: ConnectionStatus = 'disconnected';
  private onStatusChange: ((status: ConnectionStatus) => void) | null = null;

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

  /** Send a raw envelope, ignoring failures silently (used for fire-and-forget requests). */
  private send(envelope: Record<string, unknown>) {
    if (!this.socketOpen) {
      console.warn('[ws] Dropped message, socket not connected:', envelope.type);
      return;
    }
    this.ws!.send(JSON.stringify(envelope));
  }

  public connect() {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
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
      };

      this.ws.onmessage = (event) => {
        try {
          const payload = JSON.parse(event.data);
          const eventType = payload.event || payload.type;

          if (eventType) {
            this.emit(eventType, payload);
          }
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
        this.send({ type: 'ping' });
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
    attachments?: Array<{ name: string; mime: string; size: number; data: string }>,
  ) {
    if (!this.socketOpen) {
      throw new Error('WebSocket is not connected');
    }
    const payload: Record<string, unknown> = { type: 'message', chat_id: chatId, content };
    if (model) payload.model = model;
    if (provider) payload.provider = provider;
    if (attachments?.length) payload.attachments = attachments;
    this.ws!.send(JSON.stringify(payload));
  }

  public createNewChat() {
    this.send({ type: 'new_chat' });
  }

  public attachChat(chatId: string) {
    this.send({ type: 'attach', chat_id: chatId });
  }

  public sendStop(chatId: string) {
    this.sendMessage(chatId, '/stop');
  }

  public requestSessions() {
    this.send({ type: 'list_sessions' });
  }

  public requestHistory(chatId: string) {
    this.send({ type: 'load_history', chat_id: chatId });
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
    this.send({ type: 'set_config', defaults: patch });
  }

  public sendSetConfig(data: { defaults?: Record<string, unknown>; providers?: Record<string, unknown>; channels?: Record<string, unknown> }) {
    this.send({ type: 'set_config', ...data });
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

  /** Resolve a pending security-approval request. */
  public sendSecurityResponse(reqId: string, approved: boolean) {
    this.send({ type: 'security_response', req_id: reqId, approved });
  }

  // ---- Event bus ----

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