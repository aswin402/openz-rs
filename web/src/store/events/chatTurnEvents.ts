import type {
  OpenZMessage,
  SecurityPromptInfo,
  ToolExecution,
} from '../../types';
import { isTurnEventCurrent } from '../../types/websocket';
import { wsService } from '../../services/websocket';
import {
  applyOrchestrationEvent,
  newMsgId,
  normalizeChatId,
  settleAssistantTurnMessages,
  settleOrchestrationRuns,
  type OrchestrationPayload,
} from '../chatUtils';
import type { StoreEventContext } from '../eventContext';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function normalizeActivityKind(value: unknown): 'workflow' | 'memory' | 'research' | 'self_improvement' | 'source' | 'system' {
  const raw = typeof value === 'string' ? value : 'system';
  if (raw === 'workflow' || raw === 'memory' || raw === 'research' || raw === 'self_improvement' || raw === 'source') return raw;
  return 'system';
}

function attachActivityNotice(
  { set, get }: StoreEventContext,
  chatId: string,
  payload: Record<string, unknown>,
) {
  const state = get();
  const messages = state.messages[chatId] || [];
  const notice = {
    id: newMsgId('activity'),
    kind: normalizeActivityKind(payload.kind),
    title: asString(payload.title) || 'Agent activity',
    detail: asString(payload.detail),
    timestamp: typeof payload.timestamp === 'number' ? payload.timestamp : Date.now(),
  };
  const lastMsg = messages[messages.length - 1];

  if (lastMsg && lastMsg.role === 'assistant') {
    const updated: OpenZMessage = {
      ...lastMsg,
      activityNotices: [...(lastMsg.activityNotices || []), notice],
    };
    set({
      messages: { ...state.messages, [chatId]: [...messages.slice(0, -1), updated] },
    });
    return;
  }

  const message: OpenZMessage = {
    id: newMsgId('activity-msg'),
    role: 'assistant',
    content: '',
    timestamp: notice.timestamp,
    isNotice: true,
    activityNotices: [notice],
  };
  set({
    messages: { ...state.messages, [chatId]: [...messages, message] },
  });
}

export function registerChatTurnEvents({ set, get }: StoreEventContext) {
  wsService.on('turn_started', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    const turnId = asString(payload.turn_id)?.trim();
    if (!turnId) return;
    const activeTurns = get().activeTurnIds;
    if (activeTurns[chatId] && activeTurns[chatId] !== turnId) return;
    set({
      activeTurnIds: { ...activeTurns, [chatId]: turnId },
      isStreaming: true,
    });
  });

  wsService.on('delta', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: payload.turn_id }, get().activeTurnIds)) return;
    const content = payload.content || '';
    const chatMessages = get().messages[chatId] || [];
    const lastMsg = chatMessages[chatMessages.length - 1];

    if (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming) {
      const updatedMsg: OpenZMessage = { ...lastMsg, content: lastMsg.content + content };
      set({
        messages: {
          ...get().messages,
          [chatId]: [...chatMessages.slice(0, -1), updatedMsg],
        },
        isStreaming: true,
      });
    } else {
      const newMsg: OpenZMessage = {
        id: newMsgId('msg'),
        role: 'assistant',
        content,
        timestamp: Date.now(),
        isStreaming: true,
        model: get().activeModel || undefined,
      };
      set({
        messages: { ...get().messages, [chatId]: [...chatMessages, newMsg] },
        isStreaming: true,
      });
    }
  });

  wsService.on('reasoning_delta', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: payload.turn_id }, get().activeTurnIds)) return;
    const content = payload.content || '';
    const chatMessages = get().messages[chatId] || [];
    const lastMsg = chatMessages[chatMessages.length - 1];

    if (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming) {
      const updatedMsg: OpenZMessage = {
        ...lastMsg,
        reasoningContent: (lastMsg.reasoningContent || '') + content,
      };
      set({
        messages: {
          ...get().messages,
          [chatId]: [...chatMessages.slice(0, -1), updatedMsg],
        },
      });
    } else {
      // Reasoning can arrive before any content delta — open a streaming message.
      const newMsg: OpenZMessage = {
        id: newMsgId('msg'),
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
        isStreaming: true,
        reasoningContent: content,
        model: get().activeModel || undefined,
      };
      set({
        messages: { ...get().messages, [chatId]: [...chatMessages, newMsg] },
        isStreaming: true,
      });
    }
  });

  wsService.on('activity_notice', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    attachActivityNotice({ set, get }, chatId, payload as Record<string, unknown>);
  });

  wsService.on('orchestration_event', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    const eventPayload = isRecord(payload.payload) ? payload.payload as OrchestrationPayload : {};
    set({
      orchestrationRuns: {
        ...get().orchestrationRuns,
        [chatId]: applyOrchestrationEvent(get().orchestrationRuns[chatId] || [], eventPayload),
      },
    });
  });

  wsService.on('tool_start', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: payload.turn_id }, get().activeTurnIds)) return;
    const tool: ToolExecution = {
      id: payload.tool_call_id || newMsgId('tool'),
      name: payload.name || 'tool',
      args: payload.args,
      status: 'running',
      startedAt: Date.now(),
    };
    const chatMessages = get().messages[chatId] || [];
    const lastMsg = chatMessages[chatMessages.length - 1];

    if (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming) {
      const toolCalls = lastMsg.toolCalls || [];
      const existingIdx = toolCalls.findIndex((t) => t.id === tool.id);
      const updatedMsg: OpenZMessage = {
        ...lastMsg,
        toolCalls:
          existingIdx >= 0
            ? toolCalls.map((t, i) => (i === existingIdx ? tool : t))
            : [...toolCalls, tool],
      };
      set({
        messages: {
          ...get().messages,
          [chatId]: [...chatMessages.slice(0, -1), updatedMsg],
        },
      });
    } else {
      const newMsg: OpenZMessage = {
        id: newMsgId('msg'),
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
        isStreaming: true,
        toolCalls: [tool],
        model: get().activeModel || undefined,
      };
      set({
        messages: { ...get().messages, [chatId]: [...chatMessages, newMsg] },
        isStreaming: true,
      });
    }
  });

  wsService.on('tool_end', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: payload.turn_id }, get().activeTurnIds)) return;
    const chatMessages = get().messages[chatId] || [];
    const toolId = payload.tool_call_id || '';
    const endedAt = Date.now();
    const status: ToolExecution['status'] = payload.status === 'error' ? 'error' : 'success';
    let matched = false;

    const updatedMessages = chatMessages.map((message) => {
      if (message.role !== 'assistant' || !message.toolCalls || !toolId) return message;

      const existingIdx = message.toolCalls.findIndex((tool) => tool.id === toolId);
      if (existingIdx < 0) return message;

      matched = true;
      return {
        ...message,
        toolCalls: message.toolCalls.map((tool, index) => {
          if (index !== existingIdx) return tool;
          const startedAt = tool.startedAt;
          return {
            ...tool,
            id: toolId,
            name: payload.name || tool.name || 'tool',
            status,
            output: payload.output,
            error: payload.status === 'error' ? payload.output : undefined,
            startedAt,
            endedAt,
            durationMs: startedAt ? endedAt - startedAt : undefined,
          };
        }),
      };
    });

    if (matched) {
      set({ messages: { ...get().messages, [chatId]: updatedMessages } });
      return;
    }

    const lastMsg = chatMessages[chatMessages.length - 1];
    if (!lastMsg || lastMsg.role !== 'assistant') return;

    const tool: ToolExecution = {
      id: toolId,
      name: payload.name || 'tool',
      status,
      output: payload.output,
      error: payload.status === 'error' ? payload.output : undefined,
      endedAt,
    };
    const updatedMsg: OpenZMessage = {
      ...lastMsg,
      toolCalls: [...(lastMsg.toolCalls || []), tool],
    };
    set({
      messages: {
        ...get().messages,
        [chatId]: [...chatMessages.slice(0, -1), updatedMsg],
      },
    });
  });

  wsService.on('security_request', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: payload.turn_id }, get().activeTurnIds)) return;
    const prompt: SecurityPromptInfo = {
      id: payload.req_id || newMsgId('sec'),
      toolName: payload.tool_name || 'exec_command',
      description: payload.description || 'Sensitive action requested',
      arguments: payload.arguments,
      status: 'pending',
    };
    const chatMessages = get().messages[chatId] || [];
    const lastMsg = chatMessages[chatMessages.length - 1];

    if (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming) {
      const updatedMsg: OpenZMessage = {
        ...lastMsg,
        securityPrompts: [...(lastMsg.securityPrompts || []), prompt],
      };
      set({
        messages: {
          ...get().messages,
          [chatId]: [...chatMessages.slice(0, -1), updatedMsg],
        },
      });
    } else {
      const newMsg: OpenZMessage = {
        id: newMsgId('msg'),
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
        isStreaming: true,
        securityPrompts: [prompt],
        model: get().activeModel || undefined,
      };
      set({
        messages: { ...get().messages, [chatId]: [...chatMessages, newMsg] },
        isStreaming: true,
      });
    }
  });

  wsService.on('security_response_rejected', (payload) => {
    set({
      workspaceNotice: {
        scope: 'global',
        type: 'error',
        message: asString(payload.detail) || 'Security approval was rejected because it belongs to another client or chat.',
        timestamp: Date.now(),
      },
    });
  });

  wsService.on('turn_end', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    const turnId = asString(payload.turn_id)?.trim();
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: turnId }, get().activeTurnIds)) return;
    const activeTurns = { ...get().activeTurnIds };
    if (!turnId || activeTurns[chatId] === turnId) delete activeTurns[chatId];
    const chatMessages = get().messages[chatId] || [];
    const settledMessages = settleAssistantTurnMessages(
      chatMessages,
      'Turn ended before this tool reported completion.',
    );

    const pendingRunIds = (get().orchestrationRuns[chatId] || [])
      .filter((run) => run.status === 'running' || run.status === 'awaiting_review')
      .map((run) => run.id);

    set({
      messages: { ...get().messages, [chatId]: settledMessages },
      activeTurnIds: activeTurns,
      isStreaming: Object.keys(activeTurns).length > 0,
    });
    window.setTimeout(() => {
      if (pendingRunIds.length === 0) return;
      set({
        orchestrationRuns: {
          ...get().orchestrationRuns,
          [chatId]: settleOrchestrationRuns(
            get().orchestrationRuns[chatId] || [],
            'failed',
            'Turn ended before this orchestration run reported completion.',
            Date.now(),
            false,
            pendingRunIds,
          ),
        },
      });
    }, 750);
    // Refresh the session list so titles/message counts stay in sync.
    wsService.requestSessions();
  });

  wsService.on('stopped', (payload) => {
    const payloadObj = isRecord(payload) ? payload : {};
    const chatId = normalizeChatId(asString(payloadObj.chat_id) || get().activeChatId);
    const turnId = asString(payloadObj.turn_id)?.trim();
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: turnId }, get().activeTurnIds)) return;
    const activeTurns = { ...get().activeTurnIds };
    if (!turnId || activeTurns[chatId] === turnId) delete activeTurns[chatId];
    const chatMessages = get().messages[chatId] || [];
    const settledMessages = settleAssistantTurnMessages(
      chatMessages,
      'Turn stopped before this tool completed.',
    );

    const stoppableRunIds = (get().orchestrationRuns[chatId] || [])
      .filter((run) => run.status === 'running' || run.status === 'awaiting_review' || run.provisionalFailure)
      .map((run) => run.id);

    set({
      messages: { ...get().messages, [chatId]: settledMessages },
      orchestrationRuns: {
        ...get().orchestrationRuns,
        [chatId]: settleOrchestrationRuns(
          get().orchestrationRuns[chatId] || [],
          'cancelled',
          'Turn stopped before this orchestration run completed.',
          Date.now(),
          true,
          stoppableRunIds,
        ),
      },
      activeTurnIds: activeTurns,
      isStreaming: Object.keys(activeTurns).length > 0,
    });
  });

  wsService.on('error', (payload) => {
    const detail = payload.detail || 'Gateway error occurred.';
    if (!payload.chat_id && (get().activeView !== 'chats' || get().isSettingsOpen)) {
      const activeView = get().activeView;
      const scope = get().isSettingsOpen
        ? 'settings'
        : activeView === 'agents' || activeView === 'skills' || activeView === 'knowledge' || activeView === 'inventory'
          ? activeView
          : 'global';
      set({ workspaceNotice: { scope, type: 'error', message: String(detail), timestamp: Date.now() } });
      return;
    }
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    const turnId = asString(payload.turn_id)?.trim();
    if (!isTurnEventCurrent({ chat_id: chatId, turn_id: turnId }, get().activeTurnIds)) return;
    const activeTurns = { ...get().activeTurnIds };
    if (!turnId || activeTurns[chatId] === turnId) delete activeTurns[chatId];
    const chatMessages = settleAssistantTurnMessages(
      get().messages[chatId] || [],
      'Turn errored before this tool reported completion.',
    );
    const orchestrationRuns = {
      ...get().orchestrationRuns,
      [chatId]: settleOrchestrationRuns(
        get().orchestrationRuns[chatId] || [],
        'failed',
        'Turn errored before this orchestration run completed.',
      ),
    };
    const lastMsg = chatMessages[chatMessages.length - 1];
    const errorMsg: OpenZMessage = {
      id: newMsgId('err'),
      role: 'assistant',
      content: `⚠️ **Error**: ${detail}`,
      timestamp: Date.now(),
      isStreaming: false,
      isNotice: true,
    };

    if (
      lastMsg
      && lastMsg.role === 'assistant'
      && !lastMsg.content
      && !lastMsg.reasoningContent
      && !(lastMsg.toolCalls && lastMsg.toolCalls.length > 0)
      && !(lastMsg.securityPrompts && lastMsg.securityPrompts.length > 0)
      && !(lastMsg.activityNotices && lastMsg.activityNotices.length > 0)
    ) {
      // Replace empty placeholder with error.
      set({
        messages: {
          ...get().messages,
          [chatId]: [...chatMessages.slice(0, -1), errorMsg],
        },
        orchestrationRuns,
        activeTurnIds: activeTurns,
        isStreaming: Object.keys(activeTurns).length > 0,
      });
    } else {
      set({
        messages: { ...get().messages, [chatId]: [...chatMessages, errorMsg] },
        orchestrationRuns,
        activeTurnIds: activeTurns,
        isStreaming: Object.keys(activeTurns).length > 0,
      });
    }
  });
}
