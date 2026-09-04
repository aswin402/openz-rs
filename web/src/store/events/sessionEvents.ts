import type {
  JsonValue,
  OpenZMessage,
  OpenZSession,
  ToolExecution,
} from '../../types';
import { wsService } from '../../services/websocket';
import { applySessionMutationResult } from '../sessionMutation';
import {
  mergeAssistantFinalIntoToolTurn,
  normalizeChatId,
  rememberActiveChatId,
  savedActiveChatId,
  upsertDraftSession,
} from '../chatUtils';
import type { StoreEventContext } from '../eventContext';

type ToolCallPayload = {
  id?: unknown;
  name?: unknown;
  arguments?: unknown;
  function?: unknown;
};

type SessionExtra = {
  tool_calls?: ToolCallPayload[];
  reasoning_content?: unknown;
  model?: unknown;
  tool_call_id?: unknown;
  name?: unknown;
};

type SessionHistoryMessage = {
  id?: unknown;
  role?: unknown;
  content?: unknown;
  timestamp?: unknown;
  extra?: SessionExtra;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function parseToolArgs(value: unknown): ToolExecution['args'] {
  if (typeof value === 'string') {
    if (value.trim().startsWith('{')) {
      try {
        const parsed = JSON.parse(value) as JsonValue;
        return isRecord(parsed) ? parsed : value;
      } catch {
        return value;
      }
    }
    return value;
  }
  return isRecord(value) ? value : '';
}

export function registerSessionEvents({ set, get }: StoreEventContext) {
  wsService.on('ready', (payload) => {
    const readyChatId = normalizeChatId(payload.chat_id || '');
    const preferredChatId = get().activeChatId || savedActiveChatId() || readyChatId;
    if (!get().activeChatId && preferredChatId) {
      rememberActiveChatId(preferredChatId);
      set({ activeChatId: preferredChatId, sessions: upsertDraftSession(get().sessions, preferredChatId) });
    }
    if (preferredChatId && preferredChatId !== readyChatId) {
      wsService.attachChat(preferredChatId);
    }
    // Request everything real from the gateway on connect.
    wsService.requestSessions();
    if (preferredChatId) wsService.requestHistory(preferredChatId);
    wsService.requestCognitiveMemory();
    wsService.requestMcpServers();
    wsService.requestLogs();
    wsService.requestModels();
    wsService.requestConfig();
    wsService.requestSlashCommands();
    wsService.requestStatus();
    wsService.requestRuntimeInventory();
  });

  wsService.on('sessions_list', (payload) => {
    if (Array.isArray(payload.sessions)) {
      const realSessions = payload.sessions.map((session: OpenZSession) => ({
        ...session,
        id: normalizeChatId(session.id),
        isDraft: false,
      }));
      const activeChatId = get().activeChatId;
      const exists = realSessions.some((session: OpenZSession) => session.id === activeChatId);

      if (!exists && realSessions.length > 0 && (get().messages[activeChatId]?.length ?? 0) === 0) {
        const first = realSessions[0];
        rememberActiveChatId(first.id);
        set({ sessions: realSessions, activeChatId: first.id });
        wsService.requestHistory(first.id);
        return;
      }

      set({ sessions: exists ? realSessions : upsertDraftSession(realSessions, activeChatId) });
    }
  });

  wsService.on('session_archived', (payload) => {
    const payloadObj = isRecord(payload) ? payload : {};
    const rawChatId = asString(payloadObj.chat_id) || asString(payloadObj.session_key) || '';
    const chatId = normalizeChatId(rawChatId);
    const status = asString(payloadObj.status);
    if (status === 'error') {
      const result = applySessionMutationResult(get().sessions, get().messages, chatId, false);
      set({ ...result, workspaceNotice: { scope: 'inventory', type: 'error', message: asString(payloadObj.detail) || 'Failed to archive session.', timestamp: Date.now() } });
      wsService.requestSessions();
      wsService.requestRuntimeInventory();
      return;
    }
    if (chatId) {
      const archived = payloadObj.archived !== false;
      const result = applySessionMutationResult(get().sessions, get().messages, chatId, true);
      const wasActive = get().activeChatId === chatId;
      set({
        ...result,
        workspaceNotice: {
          scope: 'inventory',
          type: archived ? 'success' : 'info',
          message: archived ? `Archived session ${chatId}.` : `Session ${chatId} was already missing on disk.`,
          timestamp: Date.now(),
        },
      });
      if (wasActive) {
        if (result.sessions.length > 0) get().selectSession(result.sessions[0].id);
        else get().newSession();
      }
    }
    wsService.requestSessions();
    wsService.requestRuntimeInventory();
  });

  wsService.on('session_deleted', (payload) => {
    const payloadObj = isRecord(payload) ? payload : {};
    const rawChatId = asString(payloadObj.chat_id) || asString(payloadObj.session_key) || '';
    const chatId = normalizeChatId(rawChatId);
    const status = asString(payloadObj.status);
    if (status === 'error') {
      const result = applySessionMutationResult(get().sessions, get().messages, chatId, false);
      set({ ...result, workspaceNotice: { scope: 'inventory', type: 'error', message: asString(payloadObj.detail) || 'Failed to delete session.', timestamp: Date.now() } });
      wsService.requestSessions();
      wsService.requestRuntimeInventory();
      return;
    }
    if (chatId) {
      const deleted = payloadObj.deleted !== false;
      const result = applySessionMutationResult(get().sessions, get().messages, chatId, true);
      const wasActive = get().activeChatId === chatId;
      set({
        ...result,
        workspaceNotice: {
          scope: 'inventory',
          type: deleted ? 'success' : 'info',
          message: deleted ? `Deleted session ${chatId}.` : `Session ${chatId} was already missing on disk.`,
          timestamp: Date.now(),
        },
      });
      if (wasActive) {
        if (result.sessions.length > 0) get().selectSession(result.sessions[0].id);
        else get().newSession();
      }
    }
    wsService.requestRuntimeInventory();
  });

  wsService.on('session_history', (payload: { chat_id?: string; messages?: SessionHistoryMessage[] }) => {
    if (payload.chat_id && Array.isArray(payload.messages)) {
      const normalized: OpenZMessage[] = [];

      for (let i = 0; i < payload.messages.length; i++) {
        const message = payload.messages[i];
        const role = asString(message.role);

        if (role === 'user') {
          normalized.push({
            id: asString(message.id) || `msg-${i}`,
            role: 'user',
            content: asString(message.content) || '',
            timestamp: typeof message.timestamp === 'number' ? message.timestamp : Date.now(),
          });
        } else if (role === 'assistant') {
          const toolCalls: ToolExecution[] = [];
          if (message.extra && Array.isArray(message.extra.tool_calls)) {
            message.extra.tool_calls.forEach((toolCall) => {
              const toolFunction = isRecord(toolCall.function) ? toolCall.function : undefined;
              const toolName = asString(toolFunction?.name) || asString(toolCall.name) || 'tool';
              const toolArgs = parseToolArgs(toolFunction?.arguments ?? toolCall.arguments);

              toolCalls.push({
                id: asString(toolCall.id) || `tool-${i}-${toolName}`,
                name: toolName,
                args: toolArgs,
                status: 'success',
                output: '',
              });
            });
          }

          const reasoningContent = asString(message.extra?.reasoning_content);

          const assistantMessage: OpenZMessage = {
            id: asString(message.id) || `msg-${i}`,
            role: 'assistant',
            content: asString(message.content) || '',
            timestamp: typeof message.timestamp === 'number' ? message.timestamp : Date.now(),
            model: asString(message.extra?.model),
            reasoningContent,
            toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
          };

          if (!mergeAssistantFinalIntoToolTurn(normalized, assistantMessage)) {
            normalized.push(assistantMessage);
          }
        } else if (role === 'tool') {
          const toolCallId = asString(message.extra?.tool_call_id);
          const toolName = asString(message.extra?.name) || 'tool';

          let lastAssistant: OpenZMessage | undefined;
          for (let j = normalized.length - 1; j >= 0; j -= 1) {
            if (normalized[j].role === 'assistant') {
              lastAssistant = normalized[j];
              break;
            }
          }

          const toolContent = asString(message.content) || '';
          const toolErrored = toolContent.includes('"error"') || toolContent.toLowerCase().startsWith('error:');

          if (lastAssistant) {
            if (!lastAssistant.toolCalls) {
              lastAssistant.toolCalls = [];
            }

            const matched = lastAssistant.toolCalls.find(
              (tool) => (toolCallId && tool.id === toolCallId) || (!toolCallId && tool.name === toolName && !tool.output),
            );

            if (matched) {
              matched.output = toolContent;
              if (toolErrored) {
                matched.status = 'error';
                matched.error = toolContent;
              }
            } else {
              lastAssistant.toolCalls.push({
                id: toolCallId || `tool-${i}`,
                name: toolName,
                status: toolErrored ? 'error' : 'success',
                output: toolContent,
                error: toolErrored ? toolContent : undefined,
              });
            }
          } else {
            normalized.push({
              id: asString(message.id) || `msg-${i}`,
              role: 'system',
              content: `Tool Execution [${toolName}]: ${toolContent}`,
              timestamp: typeof message.timestamp === 'number' ? message.timestamp : Date.now(),
              isNotice: true,
            });
          }
        } else if (role === 'system') {
          normalized.push({
            id: asString(message.id) || `msg-${i}`,
            role: 'system',
            content: asString(message.content) || '',
            timestamp: typeof message.timestamp === 'number' ? message.timestamp : Date.now(),
          });
        }
      }

      const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
      rememberActiveChatId(chatId);
      set({
        messages: { ...get().messages, [chatId]: normalized },
      });
    }
  });

  wsService.on('attached', (payload) => {
    if (payload.chat_id) {
      const chatId = normalizeChatId(payload.chat_id);
      rememberActiveChatId(chatId);
      set({ activeChatId: chatId, activeView: 'chats', sessions: upsertDraftSession(get().sessions, chatId) });
      wsService.requestHistory(chatId);
    }
  });
}
