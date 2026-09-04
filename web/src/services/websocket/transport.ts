import type { ConnectionStatus } from '../../types';
import { TIMING_MS } from '../../config/runtime';
import {
  buildCommandEnvelope,
  createRequestId,
  isCommandAckEvent,
  isRetryableWebSocketCommand,
  parseWebSocketEvent,
  type WebSocketCommand,
  type WebSocketCommandAckEvent,
  type WebSocketEventEnvelope,
} from './protocol';
import type { WebSocketSendOptions } from './commands';

type QueuedCommand = {
  command: WebSocketCommand;
  requestId: string | null;
};

export interface WebSocketTransportOptions {
  url: string;
  token?: string;
  onEvent: (payload: WebSocketEventEnvelope) => void;
  onStatusChange: (status: ConnectionStatus) => void;
}

/** Owns the browser socket lifecycle while keeping domain commands transport-agnostic. */
export class WebSocketTransport {
  private readonly options: WebSocketTransportOptions;
  private ws: WebSocket | null = null;
  private url: string;
  private token: string;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private status: ConnectionStatus = 'disconnected';
  private requestSequence = 0;
  private pendingCommands = new Map<string, WebSocketCommand['type']>();
  private queuedCommands: QueuedCommand[] = [];
  private replayableCommands = new Map<string, QueuedCommand>();

  constructor(options: WebSocketTransportOptions) {
    this.options = options;
    this.url = options.url;
    this.token = options.token || '';
  }

  public setConfig(url: string, token: string) {
    this.url = url;
    this.token = token;
  }

  public getStatus(): ConnectionStatus {
    return this.status;
  }

  private updateStatus(status: ConnectionStatus) {
    this.status = status;
    this.options.onStatusChange(status);
  }

  private get socketOpen(): boolean {
    return !!this.ws && this.ws.readyState === WebSocket.OPEN;
  }

  /** Send a typed command and attach a request ID unless acknowledgements are disabled. */
  public send(
    command: WebSocketCommand,
    options: WebSocketSendOptions = {},
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
        this.options.onEvent({
          event: 'command_queued',
          request_id: requestId,
          command: command.type,
        });
        return requestId;
      }
      console.warn('[ws] Dropped command, socket not connected:', command.type);
      if (requestId) {
        this.options.onEvent({
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
          this.options.onEvent(payload);
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
    }, TIMING_MS.heartbeat);
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
      }, TIMING_MS.reconnect);
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
}
