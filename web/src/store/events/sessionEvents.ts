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

function detectToolOutputError(toolContent: string): { isError: boolean; errorDetail?: string } {
  const trimmed = toolContent.trim();
  if (!trimmed) return { isError: false };

  if (trimmed.startsWith('{')) {
    try {
      const parsed = JSON.parse(trimmed) as JsonValue;
      if (isRecord(parsed)) {
        if (typeof parsed.error === 'string' && parsed.error.trim().length > 0) {
          return { isError: true, errorDetail: parsed.error };
        }
        if (isRecord(parsed.error) && typeof parsed.error.message === 'string') {
          return { isError: true, errorDetail: parsed.error.message };
        }
        if (parsed.status === 'error' || parsed.status === 'failed' || parsed.ok === false) {
          const detail =
            asString(parsed.message) ||
            asString(parsed.detail) ||
            asString(parsed.stderr) ||
            trimmed;
          return { isError: true, errorDetail: detail };
        }
        if (typeof parsed.exit_code === 'number' && parsed.exit_code !== 0) {
          const detail = asString(parsed.stderr) || `Command exited with code ${parsed.exit_code}`;
          return { isError: true, errorDetail: detail };
        }
        return { isError: false };
      }
    } catch {
      // Fall through to plain-text prefix check when JSON is truncated or non-JSON.
    }
  }

  const lower = trimmed.toLowerCase();
  if (
    lower.startsWith('error:') ||
    lower.startsWith('tool execution error:') ||
    lower.startsWith('execution denied')
  ) {
    return { isError: true, errorDetail: trimmed };
  }

  return { isError: false };
}

export function normalizeSessionHistoryMessages(
  rawMessages: ReadonlyArray<SessionHistoryMessage>,
): OpenZMessage[] {
  const normalized: OpenZMessage[] = [];
  let turnBaseTimestamp: number | undefined;
  let turnStepOffset = 1;

  for (let i = 0; i < rawMessages.length; i += 1) {
    const message = rawMessages[i];
    const role = asString(message.role);
    const msgTimestamp = typeof message.timestamp === 'number' ? message.timestamp : Date.now();

    if (role === 'user') {
      turnBaseTimestamp = undefined;
      turnStepOffset = 1;
      normalized.push({
        id: asString(message.id) || `msg-${i}`,
        role: 'user',
        content: asString(message.content) || '',
        timestamp: msgTimestamp,
      });
    } else if (role === 'assistant') {
      if (turnBaseTimestamp === undefined) {
        turnBaseTimestamp = msgTimestamp;
        turnStepOffset = 1;
      }
      const baseTs = turnBaseTimestamp;
      const toolCalls: ToolExecution[] = [];

      if (message.extra && Array.isArray(message.extra.tool_calls)) {
        message.extra.tool_calls.forEach((toolCall) => {
          const toolFunction = isRecord(toolCall.function) ? toolCall.function : undefined;
          const toolName = asString(toolFunction?.name) || asString(toolCall.name) || 'tool';
          const toolArgs = parseToolArgs(toolFunction?.arguments ?? toolCall.arguments);

          toolCalls.push({
            id: asString(toolCall.id) || `tool-${i}-${toolName}-${turnStepOffset}`,
            name: toolName,
            args: toolArgs,
            status: 'success',
            output: '',
            startedAt: baseTs + turnStepOffset,
          });
          turnStepOffset += 1;
        });
      }

      const reasoningContent = asString(message.extra?.reasoning_content);
      const rawContent = asString(message.content) || '';
      const hasToolsInRound = toolCalls.length > 0;
      const trimmedNarration = hasToolsInRound ? rawContent.trim() : '';
      const narrationTimestamp =
        toolCalls[0]?.startedAt !== undefined ? toolCalls[0].startedAt - 0.5 : baseTs + 0.5;

      const assistantMessage: OpenZMessage = {
        id: asString(message.id) || `msg-${i}`,
        role: 'assistant',
        content: hasToolsInRound ? '' : rawContent,
        timestamp: baseTs,
        model: asString(message.extra?.model),
        reasoningContent,
        narrationSegments: trimmedNarration
          ? [
              {
                id: `${asString(message.id) || `msg-${i}`}-narration`,
                content: trimmedNarration,
                timestamp: narrationTimestamp,
              },
            ]
          : undefined,
        toolCalls: hasToolsInRound ? toolCalls : undefined,
      };

      if (!mergeAssistantFinalIntoToolTurn(normalized, assistantMessage)) {
        normalized.push(assistantMessage);
      }
    } else if (role === 'tool') {
      const toolCallId = asString(message.extra?.tool_call_id);
      const toolName = asString(message.extra?.name) || 'tool';

      let lastAssistant: OpenZMessage | undefined;
      for (let j = normalized.length - 1; j >= 0; j -= 1) {
        if (normalized[j].role === 'user') break;
        if (normalized[j].role === 'assistant') {
          lastAssistant = normalized[j];
          break;
        }
      }

      const toolContent = asString(message.content) || '';
      const { isError: toolErrored, errorDetail } = detectToolOutputError(toolContent);

      if (lastAssistant) {
        if (!lastAssistant.toolCalls) {
          lastAssistant.toolCalls = [];
        }

        const matched = lastAssistant.toolCalls.find(
          (tool) =>
            (toolCallId && tool.id === toolCallId) ||
            (!toolCallId && tool.name === toolName && !tool.output),
        );

        if (matched) {
          matched.output = toolContent;
          if (toolErrored) {
            matched.status = 'error';
            matched.error = errorDetail || toolContent;
          }
        } else {
          const baseTs = turnBaseTimestamp ?? lastAssistant.timestamp;
          lastAssistant.toolCalls.push({
            id: toolCallId || `tool-${i}-${turnStepOffset}`,
            name: toolName,
            status: toolErrored ? 'error' : 'success',
            output: toolContent,
            error: toolErrored ? errorDetail || toolContent : undefined,
            startedAt: baseTs + turnStepOffset,
          });
          turnStepOffset += 1;
        }
      } else {
        normalized.push({
          id: asString(message.id) || `msg-${i}`,
          role: 'system',
          content: `Tool Execution [${toolName}]: ${toolContent}`,
          timestamp: msgTimestamp,
          isNotice: true,
        });
      }
    } else if (role === 'system') {
      normalized.push({
        id: asString(message.id) || `msg-${i}`,
        role: 'system',
        content: asString(message.content) || '',
        timestamp: msgTimestamp,
      });
    }
  }

  return normalized;
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
    wsService.requestLogs(preferredChatId);
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
      const normalized = normalizeSessionHistoryMessages(payload.messages);

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
        if (normalized.length === 0) {
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
        if (inFlightStartIdx > 0 && existingMessages.length >= normalized.length + inFlightTail.length) {
          return;
        }
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
