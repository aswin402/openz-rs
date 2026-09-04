import type { ConnectionStatus } from '../types';
import { resolveWebSocketUrl, WS_STORAGE_KEYS } from '../config/runtime';
import {
  createWebSocketCommands,
  type SaveSubagentData,
  type UpdateSubagentSettingsData,
  type WebSocketCommandMethods,
  type WebSocketConfigData,
} from './websocket/commands';
import { WebSocketTransport } from './websocket/transport';
import {
  type WebSocketAttachment,
  type WebSocketCommandAckEvent,
  type WebSocketEventMap,
} from './websocket/protocol';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type EventListener = (data: any) => void;

export function defaultWebSocketUrl(): string {
  if (typeof window === 'undefined') return '';
  return resolveWebSocketUrl(window.location, localStorage.getItem(WS_STORAGE_KEYS.url) || undefined);
}

export class OpenZWebSocketService {
  private listeners: Map<string, Set<EventListener>> = new Map();
  private status: ConnectionStatus = 'disconnected';
  private onStatusChange: ((status: ConnectionStatus) => void) | null = null;
  private transport: WebSocketTransport;
  private commands: WebSocketCommandMethods;

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
    this.commands = createWebSocketCommands((command, options) => this.transport.send(command, options));
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

  // ---- Chat / session actions ----

  public sendMessage(
    chatId: string,
    content: string,
    model?: string,
    provider?: string,
    attachments?: WebSocketAttachment[],
  ) {
    return this.commands.sendMessage(chatId, content, model, provider, attachments);
  }

  public createNewChat() {
    this.commands.createNewChat();
  }

  public attachChat(chatId: string) {
    this.commands.attachChat(chatId);
  }

  public sendStop(chatId: string, turnId?: string) {
    return this.commands.sendStop(chatId, turnId);
  }

  public requestSessions(offset?: number, limit?: number) {
    this.commands.requestSessions(offset, limit);
  }

  public requestHistory(chatId: string) {
    this.commands.requestHistory(chatId);
  }

  public archiveSession(chatId: string) {
    this.commands.archiveSession(chatId);
  }

  public deleteSession(chatId: string) {
    this.commands.deleteSession(chatId);
  }

  public requestCognitiveMemory() {
    this.commands.requestCognitiveMemory();
  }

  public requestMcpServers() {
    this.commands.requestMcpServers();
  }

  public requestLogs() {
    this.commands.requestLogs();
  }

  public requestServers() {
    this.commands.requestServers();
  }

  public stopServer(target: string) {
    this.commands.stopServer(target);
  }

  // ---- Realtime data commands (replaces hardcoded frontend values) ----

  /** Fetch configured providers with a small model preview. */
  public requestModels() {
    this.commands.requestModels();
  }

  /** Fetch the full model list for one configured provider. */
  public requestProviderModels(provider: string) {
    this.commands.requestProviderModels(provider);
  }

  public toggleFavoriteModel(provider: string, model: string) {
    this.commands.toggleFavoriteModel(provider, model);
  }

  /** Fetch editable agent defaults, skills, mcp servers and version. */
  public requestConfig() {
    this.commands.requestConfig();
  }

  public updateConfig(patch: Record<string, unknown>) {
    return this.commands.updateConfig(patch);
  }

  public sendSetConfig(data: WebSocketConfigData) {
    return this.commands.sendSetConfig(data);
  }

  public saveSkill(name: string, content: string) {
    this.commands.saveSkill(name, content);
  }

  public deleteSkill(name: string) {
    this.commands.deleteSkill(name);
  }

  public saveSubagent(data: SaveSubagentData) {
    this.commands.saveSubagent(data);
  }

  public updateSubagentSettings(data: UpdateSubagentSettingsData) {
    this.commands.updateSubagentSettings(data);
  }

  public deleteSubagent(name: string) {
    this.commands.deleteSubagent(name);
  }

  /** Fetch the real slash command list from the backend. */
  public requestSlashCommands() {
    this.commands.requestSlashCommands();
  }

  /** Fetch gateway/agent status (version + MCP counts). */
  public requestStatus() {
    this.commands.requestStatus();
  }

  public requestRuntimeInventory() {
    this.commands.requestRuntimeInventory();
  }
  public pauseCronJob(id: string) {
    this.commands.pauseCronJob(id);
  }

  public resumeCronJob(id: string) {
    this.commands.resumeCronJob(id);
  }

  public deleteCronJob(id: string) {
    this.commands.deleteCronJob(id);
  }

  public requestCronLogs(id?: string, limit = 20) {
    this.commands.requestCronLogs(id, limit);
  }


  /** Resolve a pending security-approval request. */
  public sendSecurityResponse(reqId: string, approved: boolean) {
    this.commands.sendSecurityResponse(reqId, approved);
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
