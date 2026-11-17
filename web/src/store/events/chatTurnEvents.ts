import type {
  ActivityNotice,
  OpenZMessage,
  SecurityPromptInfo,
  ToolExecution,
} from '../../types';
import { isTurnEventCurrent } from '../../types/websocket';
import { wsService } from '../../services/websocket';
import {
  applyOrchestrationEvent,
  createWorkspaceNotice,
  newMsgId,
  normalizeChatId,
  settleAssistantTurnMessages,
  settleOrchestrationRuns,
  settleStoppedOrchestrationRuns,
  type OrchestrationPayload,
} from '../chatUtils';
import type { StoreEventContext } from '../eventContext';
import {
  dispatchActivityNoticeToast,
  dispatchOrchestrationLifecycleToast,
  dispatchSecurityRequestToast,
  dispatchWorkspaceEventToast,
  type TelemetryNavigationHandlers,
} from '../useToastStore';
import { asString, isRecord } from '../../shared/lib/format';

export function buildTelemetryNavigation({
  set,
}: StoreEventContext): TelemetryNavigationHandlers {
  return {
    openSkills: () => set({ activeView: 'skills' }),
    openKnowledge: () => set({ activeView: 'knowledge' }),
    openMemory: () => set({ isMemoryOpen: true }),
    openActivity: () => set({ activeView: 'chats', isActivityPanelOpen: true }),
    openAgents: () => set({ activeView: 'agents' }),
    openLogs: () => set({ isLogsOpen: true }),
    focusChat: () => set({ activeView: 'chats', isActivityPanelOpen: true }),
  };
}

function normalizeActivityKind(value: unknown): ActivityNotice['kind'] {
  const raw = typeof value === 'string' ? value : 'system';
  if (
    raw === 'workflow' ||
    raw === 'skill' ||
    raw === 'memory' ||
    raw === 'research' ||
    raw === 'self_improvement' ||
    raw === 'source'
  ) {
    return raw;
  }
  return 'system';
}

function attachActivityNotice(
  ctx: StoreEventContext,
  chatId: string,
  payload: Record<string, unknown>,
) {
  const { set, get } = ctx;
  const rawKind = typeof payload.kind === 'string' ? payload.kind : 'system';
  const rawTitle = asString(payload.title) || 'Agent activity';

  // Raw CLI/Telegram progress strings carry ANSI codes and duplicate structured
  // WebSocket `reasoning_delta`, `tool_start`, and `tool_end` events.
  if (rawKind === 'progress' || rawTitle === 'Progress') {
    return;
  }

  const state = get();
  const messages = state.messages[chatId] || [];
  const notice: ActivityNotice = {
    id: newMsgId('activity'),
    kind: normalizeActivityKind(payload.kind),
    title: rawTitle,
    detail: asString(payload.detail),
    timestamp: typeof payload.timestamp === 'number' ? payload.timestamp : Date.now(),
  };

  dispatchActivityNoticeToast(notice, rawKind, buildTelemetryNavigation(ctx));

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

function isChatActivelyStreaming(
  state: ReturnType<StoreEventContext['get']>,
  chatId: string,
): boolean {
  if (!chatId) return false;
  if (state.activeTurnIds[chatId]) return true;
  const chatMessages = state.messages[chatId] || [];
  const lastMsg = chatMessages[chatMessages.length - 1];
  return Boolean(lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming);
}

function upsertStreamingAssistantMessage(
  { set, get }: StoreEventContext,
  chatId: string,
  updateExisting: (lastMsg: OpenZMessage) => OpenZMessage,
  initialPatch: Partial<OpenZMessage>,
  setStreamingOnExisting = false,
) {
  if (!isChatActivelyStreaming(get(), chatId)) return;
  const chatMessages = get().messages[chatId] || [];
  const lastMsg = chatMessages[chatMessages.length - 1];
  const isActiveChat = chatId === get().activeChatId;

  if (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming) {
    set({
      messages: {
        ...get().messages,
        [chatId]: [...chatMessages.slice(0, -1), updateExisting(lastMsg)],
      },
      ...(setStreamingOnExisting && isActiveChat ? { isStreaming: true } : {}),
    });
    return;
  }

  // Upgrade any pre-turn telemetry placeholder (`Workflow matched` / `Skill used`) into
  // the live streaming assistant turn so notices stay attached to their response bubble.
  if (
    lastMsg &&
    lastMsg.role === 'assistant' &&
    lastMsg.isNotice &&
    !lastMsg.content &&
    (lastMsg.activityNotices?.length ?? 0) > 0
  ) {
    const promoted: OpenZMessage = {
      ...lastMsg,
      isNotice: false,
      isStreaming: true,
      model: lastMsg.model || get().activeModel || undefined,
      ...initialPatch,
    };
    set({
      messages: {
        ...get().messages,
        [chatId]: [...chatMessages.slice(0, -1), promoted],
      },
      ...(isActiveChat ? { isStreaming: true } : {}),
    });
    return;
  }

  const newMsg: OpenZMessage = {
    id: newMsgId('msg'),
    role: 'assistant',
    content: '',
    timestamp: Date.now(),
    isStreaming: true,
    model: get().activeModel || undefined,
    ...initialPatch,
  };
  set({
    messages: { ...get().messages, [chatId]: [...chatMessages, newMsg] },
    ...(isActiveChat ? { isStreaming: true } : {}),
  });
}

function appendStreamingAssistantDelta(
  ctx: StoreEventContext,
  payload: { chat_id?: string; turn_id?: string; content?: string },
  field: 'content' | 'reasoningContent',
) {
  const { get } = ctx;
  const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
  if (!isTurnEventCurrent({ chat_id: chatId, turn_id: payload.turn_id }, get().activeTurnIds)) return;
  const chunk = payload.content || '';
  upsertStreamingAssistantMessage(
    ctx,
    chatId,
    (lastMsg) =>
      field === 'content'
        ? { ...lastMsg, content: lastMsg.content + chunk }
        : { ...lastMsg, reasoningContent: (lastMsg.reasoningContent || '') + chunk },
    field === 'content' ? { content: chunk } : { reasoningContent: chunk },
    true,
  );
}

function settleActiveTurn(
  { get }: StoreEventContext,
  rawChatId: unknown,
  rawTurnId: unknown,
  toolSettledReason: string,
): {
  chatId: string;
  activeTurns: Record<string, string>;
  settledMessages: OpenZMessage[];
} | null {
  const chatId = normalizeChatId(asString(rawChatId) || get().activeChatId);
  const turnId = asString(rawTurnId)?.trim();
  if (!isTurnEventCurrent({ chat_id: chatId, turn_id: turnId }, get().activeTurnIds)) return null;
  const activeTurns = { ...get().activeTurnIds };
  if (!turnId || activeTurns[chatId] === turnId) delete activeTurns[chatId];
  const settledMessages = settleAssistantTurnMessages(
    get().messages[chatId] || [],
    toolSettledReason,
  );
  return { chatId, activeTurns, settledMessages };
}

export function registerChatTurnEvents(ctx: StoreEventContext) {
  const { set, get } = ctx;

  wsService.on('turn_started', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    const turnId = asString(payload.turn_id)?.trim();
    if (!turnId) return;
    const nextActiveTurns = { ...get().activeTurnIds, [chatId]: turnId };
    set({
      activeTurnIds: nextActiveTurns,
      ...(chatId === get().activeChatId ? { isStreaming: true } : {}),
    });
  });

  wsService.on('remote_user_message', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    const content = asString(payload.content) || '';
    if (!chatId || !content.trim()) return;
    const chatMessages = get().messages[chatId] || [];
    const sender = asString(payload.sender)?.trim() || 'remote';
    const userMsg: OpenZMessage = {
      id: newMsgId('remote-user'),
      role: 'user',
      content: `[Remote · ${sender}] ${content}`,
      timestamp: typeof payload.timestamp === 'number' ? payload.timestamp : Date.now(),
    };
    set({
      messages: { ...get().messages, [chatId]: [...chatMessages, userMsg] },
    });
  });

  wsService.on('delta', (payload) => {
    appendStreamingAssistantDelta(ctx, payload, 'content');
  });

  wsService.on('reasoning_delta', (payload) => {
    appendStreamingAssistantDelta(ctx, payload, 'reasoningContent');
  });

  wsService.on('activity_notice', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    attachActivityNotice(ctx, chatId, payload as Record<string, unknown>);
  });

  wsService.on('orchestration_event', (payload) => {
    const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
    const eventPayload = isRecord(payload.payload) ? payload.payload as OrchestrationPayload : {};
    dispatchOrchestrationLifecycleToast(
      eventPayload as unknown as Record<string, unknown>,
      buildTelemetryNavigation(ctx),
    );
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
    upsertStreamingAssistantMessage(
      ctx,
      chatId,
      (lastMsg) => {
        const toolCalls = lastMsg.toolCalls || [];
        const existingIdx = toolCalls.findIndex((t) => t.id === tool.id);
        return {
          ...lastMsg,
          toolCalls:
            existingIdx >= 0
              ? toolCalls.map((t, i) => (i === existingIdx ? tool : t))
              : [...toolCalls, tool],
        };
      },
      { toolCalls: [tool] },
    );
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
    dispatchSecurityRequestToast(prompt, buildTelemetryNavigation(ctx));
    upsertStreamingAssistantMessage(
      ctx,
      chatId,
      (lastMsg) => ({
        ...lastMsg,
        securityPrompts: [...(lastMsg.securityPrompts || []), prompt],
      }),
      { securityPrompts: [prompt] },
    );
  });

  wsService.on('security_response_rejected', (payload) => {
    set({
      workspaceNotice: createWorkspaceNotice(
        'global',
        'error',
        asString(payload.detail) ||
          'Security approval was rejected because it belongs to another client or chat.',
      ),
    });
  });

  wsService.on('turn_end', (payload) => {
    const settled = settleActiveTurn(
      ctx,
      payload.chat_id,
      payload.turn_id,
      'Turn ended before this tool reported completion.',
    );
    if (!settled) return;
    const { chatId, activeTurns, settledMessages } = settled;

    const pendingRunIds = (get().orchestrationRuns[chatId] || [])
      .filter((run) => run.status === 'running' || run.status === 'awaiting_review')
      .map((run) => run.id);

    set({
      messages: { ...get().messages, [chatId]: settledMessages },
      activeTurnIds: activeTurns,
      isStreaming: Boolean(activeTurns[get().activeChatId]),
    });
    globalThis.setTimeout(() => {
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
    if (get().isLogsOpen) {
      wsService.requestLogs(chatId);
    }
  });

  wsService.on('stopped', (payload) => {
    const payloadObj = isRecord(payload) ? payload : {};
    const settled = settleActiveTurn(
      ctx,
      payloadObj.chat_id,
      payloadObj.turn_id,
      'Turn stopped before this tool completed.',
    );
    if (!settled) return;
    const { chatId, activeTurns, settledMessages } = settled;

    set({
      messages: { ...get().messages, [chatId]: settledMessages },
      orchestrationRuns: {
        ...get().orchestrationRuns,
        [chatId]: settleStoppedOrchestrationRuns(get().orchestrationRuns[chatId] || []),
      },
      activeTurnIds: activeTurns,
      isStreaming: Boolean(activeTurns[get().activeChatId]),
    });
  });

  wsService.on('error', (payload) => {
    const detail = payload.detail || 'Gateway error occurred.';
    dispatchWorkspaceEventToast(
      'global',
      'error',
      String(detail),
      buildTelemetryNavigation(ctx),
    );
    if (!payload.chat_id && (get().activeView !== 'chats' || get().isSettingsOpen)) {
      const activeView = get().activeView;
      const scope = get().isSettingsOpen
        ? 'settings'
        : activeView === 'agents' || activeView === 'skills' || activeView === 'knowledge' || activeView === 'inventory'
          ? activeView
          : 'global';
      set({ workspaceNotice: createWorkspaceNotice(scope, 'error', String(detail)) });
      return;
    }
    const settled = settleActiveTurn(
      ctx,
      payload.chat_id,
      payload.turn_id,
      'Turn errored before this tool reported completion.',
    );
    if (!settled) return;
    const { chatId, activeTurns, settledMessages: chatMessages } = settled;
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

    const isEmptyPlaceholder =
      Boolean(lastMsg) &&
      lastMsg.role === 'assistant' &&
      !lastMsg.content &&
      !lastMsg.reasoningContent &&
      !(lastMsg.toolCalls && lastMsg.toolCalls.length > 0) &&
      !(lastMsg.securityPrompts && lastMsg.securityPrompts.length > 0) &&
      !(lastMsg.activityNotices && lastMsg.activityNotices.length > 0);

    set({
      messages: {
        ...get().messages,
        [chatId]: isEmptyPlaceholder
          ? [...chatMessages.slice(0, -1), errorMsg]
          : [...chatMessages, errorMsg],
      },
      orchestrationRuns,
      activeTurnIds: activeTurns,
      isStreaming: Boolean(activeTurns[get().activeChatId]),
    });
  });
}
