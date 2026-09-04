import type {
  WebSocketAttachment,
  WebSocketCommand,
} from './protocol';

export interface WebSocketSendOptions {
  acknowledge?: boolean;
  requireConnected?: boolean;
  queueIfDisconnected?: boolean;
}

export type WebSocketCommandSender = (
  command: WebSocketCommand,
  options?: WebSocketSendOptions,
) => string | null;

export interface WebSocketConfigData {
  defaults?: Record<string, unknown>;
  providers?: Record<string, unknown>;
  channels?: Record<string, unknown>;
}

export interface SaveSubagentData {
  name: string;
  description: string;
  systemPrompt: string;
  model?: string;
  fallbacks?: string[];
}

export interface UpdateSubagentSettingsData {
  name: string;
  model?: string | null;
  fallbacks?: string[] | null;
}

export interface WebSocketCommandMethods {
  sendMessage(
    chatId: string,
    content: string,
    model?: string,
    provider?: string,
    attachments?: WebSocketAttachment[],
  ): string | null;
  createNewChat(): void;
  attachChat(chatId: string): void;
  sendStop(chatId: string, turnId?: string): string | null;
  requestSessions(offset?: number, limit?: number): void;
  requestHistory(chatId: string): void;
  archiveSession(chatId: string): void;
  deleteSession(chatId: string): void;
  requestCognitiveMemory(): void;
  requestMcpServers(): void;
  requestLogs(): void;
  requestServers(): void;
  stopServer(target: string): void;
  requestModels(): void;
  requestProviderModels(provider: string): void;
  toggleFavoriteModel(provider: string, model: string): void;
  requestConfig(): void;
  updateConfig(patch: Record<string, unknown>): string | null;
  sendSetConfig(data: WebSocketConfigData): string | null;
  saveSkill(name: string, content: string): void;
  deleteSkill(name: string): void;
  saveSubagent(data: SaveSubagentData): void;
  updateSubagentSettings(data: UpdateSubagentSettingsData): void;
  deleteSubagent(name: string): void;
  requestSlashCommands(): void;
  requestStatus(): void;
  requestRuntimeInventory(): void;
  pauseCronJob(id: string): void;
  resumeCronJob(id: string): void;
  deleteCronJob(id: string): void;
  requestCronLogs(id?: string, limit?: number): void;
  sendSecurityResponse(reqId: string, approved: boolean): void;
}

/** Build the public command API around the transport's typed send function. */
export function createWebSocketCommands(
  send: WebSocketCommandSender,
): WebSocketCommandMethods {
  return {
    sendMessage(chatId, content, model, provider, attachments) {
      const payload: Extract<WebSocketCommand, { type: 'message' }> = {
        type: 'message',
        chat_id: chatId,
        content,
      };
      if (model) payload.model = model;
      if (provider) payload.provider = provider;
      if (attachments?.length) payload.attachments = attachments;
      return send(payload, { requireConnected: true });
    },

    createNewChat() {
      send({ type: 'new_chat' });
    },

    attachChat(chatId) {
      send({ type: 'attach', chat_id: chatId });
    },

    sendStop(chatId, turnId) {
      const payload: Extract<WebSocketCommand, { type: 'message' }> = {
        type: 'message',
        chat_id: chatId,
        content: '/stop',
      };
      if (turnId) payload.turn_id = turnId;
      return send(payload, { requireConnected: true });
    },

    requestSessions(offset?: number, limit?: number) {
      const payload: Extract<WebSocketCommand, { type: 'list_sessions' }> = { type: 'list_sessions' };
      if (typeof offset === 'number') payload.offset = offset;
      if (typeof limit === 'number') payload.limit = limit;
      send(payload);
    },

    requestHistory(chatId) {
      send({ type: 'load_history', chat_id: chatId });
    },

    archiveSession(chatId) {
      send({ type: 'archive_session', chat_id: chatId });
    },

    deleteSession(chatId) {
      send({ type: 'delete_session', chat_id: chatId });
    },

    requestCognitiveMemory() {
      send({ type: 'get_cognitive_memory' });
    },

    requestMcpServers() {
      send({ type: 'get_mcp_servers' });
    },

    requestLogs() {
      send({ type: 'get_logs' });
    },

    requestServers() {
      send({ type: 'get_servers' });
    },

    stopServer(target) {
      send({ type: 'stop_server', target });
    },

    requestModels() {
      send({ type: 'get_models' });
    },

    requestProviderModels(provider) {
      send({ type: 'get_models', provider });
    },

    toggleFavoriteModel(provider, model) {
      send({ type: 'toggle_favorite_model', provider, model });
    },

    requestConfig() {
      send({ type: 'get_config' });
    },

    updateConfig(patch) {
      return send(
        { type: 'set_config', defaults: patch },
        { queueIfDisconnected: true },
      );
    },

    sendSetConfig(data) {
      return send(
        { type: 'set_config', ...data },
        { queueIfDisconnected: true },
      );
    },

    saveSkill(name, content) {
      send({ type: 'save_skill', name, content });
    },

    deleteSkill(name) {
      send({ type: 'delete_skill', name });
    },

    saveSubagent(data) {
      send({ type: 'save_subagent', ...data });
    },

    updateSubagentSettings(data) {
      send({ type: 'update_subagent_settings', ...data });
    },

    deleteSubagent(name) {
      send({ type: 'delete_subagent', name });
    },

    requestSlashCommands() {
      send({ type: 'get_slash_commands' });
    },

    requestStatus() {
      send({ type: 'get_status' });
    },

    requestRuntimeInventory() {
      send({ type: 'get_runtime_inventory' });
    },

    pauseCronJob(id) {
      send({ type: 'pause_cron_job', id });
    },

    resumeCronJob(id) {
      send({ type: 'resume_cron_job', id });
    },

    deleteCronJob(id) {
      send({ type: 'delete_cron_job', id });
    },

    requestCronLogs(id, limit = 20) {
      send({ type: 'get_cron_logs', id, limit });
    },

    sendSecurityResponse(reqId, approved) {
      send({ type: 'security_response', req_id: reqId, approved });
    },
  };
}
