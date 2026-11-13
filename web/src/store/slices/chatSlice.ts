import type { StateCreator } from 'zustand';
import type {
  ChatAttachment,
  OpenZMessage,
  OpenZSession,
  OrchestrationRunState,
} from '../../types';
import { wsService } from '../../services/websocket';
import {
  createWorkspaceNotice,
  forgetActiveChatId,
  newMsgId,
  normalizeChatId,
  rememberActiveChatId,
  settleAssistantTurnMessages,
  settleStoppedOrchestrationRuns,
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
  regenerateMessage: (messageId: string) => void;
  editAndResendMessage: (messageId: string, nextContent: string) => void;
  stopTurn: () => void;
  handleSecurityChoice: (reqId: string, choice: 'approve' | 'deny') => void;
}

export const createChatSlice: StateCreator<
  OpenZState,
  [],
  [],
  ChatSlice
> = (set, get) => {
  forgetActiveChatId();
  return {
  sessions: [],
  activeChatId: '',
  messages: {},
  orchestrationRuns: {},
  isStreaming: false,
  activeTurnIds: {},

  selectSession: (chatId) => {
    const normalizedChatId = normalizeChatId(chatId);
    set({ activeView: 'chats' });
    if (get().activeChatId !== normalizedChatId) {
      rememberActiveChatId(normalizedChatId);
      const chatMessages = get().messages[normalizedChatId] || [];
      const lastMsg = chatMessages[chatMessages.length - 1];
      const isTargetStreaming = Boolean(
        get().activeTurnIds[normalizedChatId] ||
          (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming),
      );
      set({ activeChatId: normalizedChatId, isStreaming: isTargetStreaming });
      wsService.attachChat(normalizedChatId);
    }
  },

  newSession: () => {
    forgetActiveChatId();
    set({ activeView: 'chats', isStreaming: false });
    wsService.createNewChat();
  },

  archiveSession: (chatId) => {
    const normalizedChatId = normalizeChatId(chatId);
    set({
      workspaceNotice: createWorkspaceNotice(
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
      workspaceNotice: createWorkspaceNotice(
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
        workspaceNotice: createWorkspaceNotice(
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
      wsService.sendMessage(chatId, content, model, provider, attachmentPayload, {
        streaming: get().streamingMode,
        cavemanMode: get().cavemanMode,
      });
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
    set({
      messages: { ...get().messages, [chatId]: settledMessages },
      orchestrationRuns: {
        ...get().orchestrationRuns,
        [chatId]: settleStoppedOrchestrationRuns(get().orchestrationRuns[chatId] || []),
      },
      activeTurnIds: activeTurns,
      isStreaming: false,
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

  regenerateMessage: (messageId) => {
    const chatId = get().activeChatId;
    if (!chatId || get().isStreaming) return;
    const chatMessages = get().messages[chatId] || [];
    const targetIdx = chatMessages.findIndex((m) => m.id === messageId);
    if (targetIdx === -1) return;
    let userIdx = targetIdx;
    while (userIdx >= 0 && chatMessages[userIdx].role !== 'user') {
      userIdx--;
    }
    if (userIdx < 0) return;
    const promptText = chatMessages[userIdx].content;
    const trimmed = chatMessages.slice(0, userIdx);
    set({ messages: { ...get().messages, [chatId]: trimmed } });
    get().sendMessage(promptText);
  },

  editAndResendMessage: (messageId, nextContent) => {
    const cleaned = nextContent.trim();
    const chatId = get().activeChatId;
    if (!chatId || !cleaned || get().isStreaming) return;
    const chatMessages = get().messages[chatId] || [];
    const targetIdx = chatMessages.findIndex((m) => m.id === messageId);
    if (targetIdx === -1) return;
    const trimmed = chatMessages.slice(0, targetIdx);
    set({ messages: { ...get().messages, [chatId]: trimmed } });
    get().sendMessage(cleaned);
  },
  };
};
