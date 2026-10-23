import type { ConnectionStatus } from '../types';
import { resolveWebSocketUrl, WS_STORAGE_KEYS } from '../config/runtime';
import {
  createWebSocketCommands,
  type WebSocketCommandMethods,
} from './websocket/commands';
import { WebSocketTransport } from './websocket/transport';
import {
  type WebSocketCommandAckEvent,
  type WebSocketEventMap,
} from './websocket/protocol';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type EventListener = (data: any) => void;

export function defaultWebSocketUrl(): string {
  if (typeof window === 'undefined') return '';
  return resolveWebSocketUrl(window.location, localStorage.getItem(WS_STORAGE_KEYS.url) || undefined);
}

// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
export interface OpenZWebSocketService extends WebSocketCommandMethods {}

// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
export class OpenZWebSocketService {
  private listeners: Map<string, Set<EventListener>> = new Map();
  private status: ConnectionStatus = 'disconnected';
  private onStatusChange: ((status: ConnectionStatus) => void) | null = null;
  private transport: WebSocketTransport;

  constructor() {
    const savedToken = localStorage.getItem(WS_STORAGE_KEYS.token);
    this.transport = new WebSocketTransport({
      url: defaultWebSocketUrl(),
      token: savedToken || '',
      onEvent: (payload) => {
        this.emit(payload.event, payload);
        this.emit('*', payload);
      },
      onStatusChange: (status) => this.updateStatus(status),
    });
    Object.assign(
      this,
      createWebSocketCommands((command, options) => this.transport.send(command, options)),
    );
  }

  public setConfig(url: string, token: string) {
    this.transport.setConfig(url, token);
    localStorage.setItem(WS_STORAGE_KEYS.url, url);
    localStorage.setItem(WS_STORAGE_KEYS.token, token);
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

  public connect() {
    this.transport.connect();
  }

  public disconnect() {
    this.transport.disconnect();
  }

  // ---- Event bus ----

  public on<K extends keyof WebSocketEventMap>(event: K, fn: (data: WebSocketEventMap[K]) => void): void;
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
