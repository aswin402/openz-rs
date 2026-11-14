import type {
  JsonValue,
  OpenZMessage,
  OpenZSession,
  ToolExecution,
} from '../../types';
import { wsService } from '../../services/websocket';
import { applySessionMutationResult } from '../sessionMutation';
import {
  createWorkspaceNotice,
  mergeAssistantFinalIntoToolTurn,
  normalizeChatId,
  rememberActiveChatId,
  upsertDraftSession,
} from '../chatUtils';
import type { StoreEventContext } from '../eventContext';
import { asString, isRecord } from '../../shared/lib/format';

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
    const currentActiveChatId = get().activeChatId;
    const preferredChatId = currentActiveChatId || readyChatId;
    if (!currentActiveChatId && preferredChatId) {
      rememberActiveChatId(preferredChatId);
      set({
        activeChatId: preferredChatId,
        isStreaming: false,
        sessions: upsertDraftSession(get().sessions, preferredChatId),
      });
    }
    if (preferredChatId && preferredChatId !== readyChatId) {
      wsService.attachChat(preferredChatId);
    }
    // Request everything real from the gateway on connect.
    wsService.requestSessions();
    if (currentActiveChatId && preferredChatId) wsService.requestHistory(preferredChatId);
    wsService.requestCognitiveMemory();
    wsService.requestMcpServers();
    wsService.requestLogs();
    wsService.requestModels();
    wsService.requestConfig();
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
      set({ sessions: exists ? realSessions : upsertDraftSession(realSessions, activeChatId) });
    }
  });

  const handleSessionLifecycleEvent = (
    payload: unknown,
    options: {
      flagKey: 'archived' | 'deleted';
      errorMessage: string;
      presentVerb: string;
      refreshSessionsOnSuccess: boolean;
    },
  ) => {
    const payloadObj = isRecord(payload) ? payload : {};
    const rawChatId = asString(payloadObj.chat_id) || asString(payloadObj.session_key) || '';
    const chatId = normalizeChatId(rawChatId);
    const status = asString(payloadObj.status);
    if (status === 'error') {
      const result = applySessionMutationResult(get().sessions, get().messages, chatId, false);
      set({
        ...result,
        workspaceNotice: createWorkspaceNotice(
          'inventory',
          'error',
          asString(payloadObj.detail) || options.errorMessage,
        ),
      });
      wsService.requestSessions();
      wsService.requestRuntimeInventory();
      return;
    }
    if (chatId) {
      const succeeded = payloadObj[options.flagKey] !== false;
      const result = applySessionMutationResult(get().sessions, get().messages, chatId, true);
      const wasActive = get().activeChatId === chatId;
      set({
        ...result,
        workspaceNotice: createWorkspaceNotice(
          'inventory',
          succeeded ? 'success' : 'info',
          succeeded
            ? `${options.presentVerb} session ${chatId}.`
            : `Session ${chatId} was already missing on disk.`,
        ),
      });
      if (wasActive) {
        if (result.sessions.length > 0) get().selectSession(result.sessions[0].id);
        else get().newSession();
      }
    }
    if (options.refreshSessionsOnSuccess) {
      wsService.requestSessions();
    }
    wsService.requestRuntimeInventory();
  };

  wsService.on('session_archived', (payload) =>
    handleSessionLifecycleEvent(payload, {
      flagKey: 'archived',
      errorMessage: 'Failed to archive session.',
      presentVerb: 'Archived',
      refreshSessionsOnSuccess: true,
    }),
  );

  wsService.on('session_deleted', (payload) =>
    handleSessionLifecycleEvent(payload, {
      flagKey: 'deleted',
      errorMessage: 'Failed to delete session.',
      presentVerb: 'Deleted',
      refreshSessionsOnSuccess: false,
    }),
  );

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
      if (chatId === get().activeChatId) {
        rememberActiveChatId(chatId);
      }

      const existingMessages = get().messages[chatId] || [];
      const lastExisting = existingMessages[existingMessages.length - 1];
      const hasInFlightTurn = Boolean(
        get().activeTurnIds[chatId] ||
          (lastExisting && lastExisting.role === 'assistant' && lastExisting.isStreaming),
      );

      if (hasInFlightTurn && existingMessages.length > 0) {
        if (normalized.length === 0 || existingMessages.length > normalized.length) {
          return;
        }
        const inFlightStartIdx = existingMessages.findIndex(
          (msg, idx) =>
            (msg.role === 'user' &&
              existingMessages[idx + 1]?.role === 'assistant' &&
              existingMessages[idx + 1]?.isStreaming) ||
            (msg.role === 'assistant' && msg.isStreaming),
        );
        const inFlightTail =
          inFlightStartIdx >= 0 ? existingMessages.slice(inFlightStartIdx) : existingMessages;
        set({
          messages: { ...get().messages, [chatId]: [...normalized, ...inFlightTail] },
        });
        return;
      }

      set({
        messages: { ...get().messages, [chatId]: normalized },
      });
    }
  });

  wsService.on('attached', (payload) => {
    if (payload.chat_id) {
      const chatId = normalizeChatId(payload.chat_id);
      rememberActiveChatId(chatId);
      const chatMessages = get().messages[chatId] || [];
      const lastMsg = chatMessages[chatMessages.length - 1];
      const isTargetStreaming = Boolean(
        get().activeTurnIds[chatId] ||
          (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming),
      );
      const nextSessions = upsertDraftSession(get().sessions, chatId);
      const matchedSession = nextSessions.find((session) => session.id === chatId);
      const isDraftSession = Boolean(matchedSession?.isDraft);
      set({
        activeChatId: chatId,
        activeView: 'chats',
        isStreaming: isTargetStreaming,
        sessions: nextSessions,
      });
      if (!isTargetStreaming && !isDraftSession) {
        wsService.requestHistory(chatId);
      }
    }
  });
}
