import type { StateCreator } from 'zustand';
import type {
  ChatAttachment,
  OpenZMessage,
  OpenZSession,
  OrchestrationRunState,
  WorkspaceNotice,
} from '../../types';
import { wsService } from '../../services/websocket';
import {
  forgetActiveChatId,
  newMsgId,
  normalizeChatId,
  rememberActiveChatId,
  savedActiveChatId,
  settleAssistantTurnMessages,
  settleOrchestrationRuns,
  titleFromFirstMessage,
} from '../chatUtils';
import type { OpenZState } from '../useOpenZStore';

export interface ChatSlice {
  sessions: OpenZSession[];
  activeChatId: string;
  messages: Record<string, OpenZMessage[]>;
  orchestrationRuns: Record<string, OrchestrationRunState[]>;
  isStreaming: boolean;
  activeTurnIds: Record<string, string>;
  selectSession: (chatId: string) => void;
  newSession: () => void;
  archiveSession: (chatId: string) => void;
  deleteSession: (chatId: string) => void;
  clearActiveSession: () => void;
  sendMessage: (content: string, attachments?: ChatAttachment[]) => void;
  stopTurn: () => void;
  handleSecurityChoice: (reqId: string, choice: 'approve' | 'deny') => void;
}

function notice(
  scope: WorkspaceNotice['scope'],
  type: WorkspaceNotice['type'],
  message: string,
): WorkspaceNotice {
  return { scope, type, message, timestamp: Date.now() };
}

export const createChatSlice: StateCreator<
  OpenZState,
  [],
  [],
  ChatSlice
> = (set, get) => ({
  sessions: [],
  activeChatId: savedActiveChatId(),
  messages: {},
  orchestrationRuns: {},
  isStreaming: false,
  activeTurnIds: {},

  selectSession: (chatId) => {
    const normalizedChatId = normalizeChatId(chatId);
    set({ activeView: 'chats' });
    if (get().activeChatId !== normalizedChatId) {
      rememberActiveChatId(normalizedChatId);
      set({ activeChatId: normalizedChatId });
      wsService.attachChat(normalizedChatId);
    }
  },

  newSession: () => {
    forgetActiveChatId();
    set({ activeView: 'chats' });
    wsService.createNewChat();
  },

  archiveSession: (chatId) => {
    const normalizedChatId = normalizeChatId(chatId);
    set({
      workspaceNotice: notice(
        'inventory',
        'info',
        `Archiving session ${normalizedChatId}.`,
      ),
    });
    wsService.archiveSession(normalizedChatId);
  },

  deleteSession: (chatId) => {
    const normalizedChatId = normalizeChatId(chatId);
    set({
      workspaceNotice: notice(
        'inventory',
        'info',
        `Deleting session ${normalizedChatId}.`,
      ),
    });
    wsService.deleteSession(normalizedChatId);
  },

  clearActiveSession: () => {
    const chatId = get().activeChatId;
    if (!chatId) return;
    set({ messages: { ...get().messages, [chatId]: [] } });
    wsService.sendMessage(chatId, '/clear');
  },

  sendMessage: (content, attachments = []) => {
    if (!content.trim() && attachments.length === 0) return;
    const chatId = get().activeChatId;
    const chatMessages = get().messages[chatId] || [];

    if (!chatId) {
      set({
        workspaceNotice: notice(
          'global',
          'error',
          'No active chat session. Reconnect the gateway and try again.',
        ),
      });
      return;
    }

    const userMsg: OpenZMessage = {
      id: newMsgId('msg-user'),
      role: 'user',
      content,
      timestamp: Date.now(),
      attachments: attachments.length
        ? attachments.map((attachment) => ({
            id: attachment.id,
            name: attachment.name,
            mime: attachment.mime,
            size: attachment.size,
            previewUrl: attachment.previewUrl,
          }))
        : undefined,
    };

    const assistantPlaceholder: OpenZMessage = {
      id: newMsgId('msg-assistant'),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
      isStreaming: true,
      model: get().activeModel || undefined,
    };

    const nextMessages = {
      ...get().messages,
      [chatId]: [...chatMessages, userMsg, assistantPlaceholder],
    };
    const nextSessions = get().sessions.map((session) =>
      session.id === chatId && session.isDraft
        ? {
            ...session,
            title: titleFromFirstMessage(content),
            lastMessageAt: Date.now(),
            messageCount: 1,
          }
        : session,
    );
    set({ messages: nextMessages, sessions: nextSessions, isStreaming: true });

    try {
      const model = get().activeModel || undefined;
      const provider = get().activeProvider || undefined;
      const attachmentPayload = attachments.flatMap((attachment) =>
        attachment.data
          ? [{ name: attachment.name, mime: attachment.mime, size: attachment.size, data: attachment.data }]
          : [],
      );
      wsService.sendMessage(chatId, content, model, provider, attachmentPayload);
    } catch (err) {
      set({ isStreaming: false });
      const errorMessage = err instanceof Error ? err.message : 'Gateway offline';
      const errMsgs = {
        ...get().messages,
        [chatId]: [
          ...chatMessages,
          userMsg,
          {
            id: newMsgId('err'),
            role: 'assistant' as const,
            content: `⚠️ **Connection Error**: ${errorMessage}`,
            timestamp: Date.now(),
            isNotice: true,
          },
        ],
      };
      set({ messages: errMsgs });
    }
  },

  stopTurn: () => {
    const chatId = get().activeChatId;
    if (!chatId) return;
    const activeTurnId = get().activeTurnIds[chatId];
    wsService.sendStop(chatId, activeTurnId);
    const activeTurns = { ...get().activeTurnIds };
    if (activeTurnId) delete activeTurns[chatId];
    const settledMessages = settleAssistantTurnMessages(
      get().messages[chatId] || [],
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
  },

  handleSecurityChoice: (reqId, choice) => {
    const chatId = get().activeChatId;
    const chatMessages = get().messages[chatId] || [];

    const updated = chatMessages.map((msg) => {
      if (msg.securityPrompts && msg.securityPrompts.some((prompt) => prompt.id === reqId)) {
        return {
          ...msg,
          securityPrompts: msg.securityPrompts.map((prompt) =>
            prompt.id === reqId
              ? { ...prompt, status: choice === 'approve' ? ('approved' as const) : ('denied' as const) }
              : prompt,
          ),
        };
      }
      return msg;
    });

    set({ messages: { ...get().messages, [chatId]: updated } });
    wsService.sendSecurityResponse(reqId, choice === 'approve');
  },
});
