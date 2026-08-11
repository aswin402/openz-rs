import { create } from 'zustand';
import type {
  ConnectionStatus,
  OpenZMessage,
  OpenZSession,
  CognitiveMemoryStats,
  McpServerInfo,
  McpStats,
  LogEntry,
  ToolExecution,
  SecurityPromptInfo,
  ProviderModelOption,
  ModelRef,
  AgentDefaultsConfig,
  SlashCommand,
  AgentStatus,
  ChatAttachment,
  BackgroundServerInfo,
  SkillInfo,
  SubagentInfo,
  ChannelConfigInfo,
  JsonObject,
  JsonValue,
  OpenZConfigPatch,
  WorkspaceNotice,
} from '../types';
import { wsService } from '../services/websocket';

/** Workspace views available from the left navigation rail. */
export type WorkspaceView = 'dashboard' | 'chats' | 'agents' | 'skills' | 'knowledge';

export interface OpenZState {
  // Connection
  connectionStatus: ConnectionStatus;
  wsUrl: string;
  wsToken: string;
  setWsConfig: (url: string, token: string) => void;

  // Sessions & Active Chat
  sessions: OpenZSession[];
  activeChatId: string;
  messages: Record<string, OpenZMessage[]>;
  isStreaming: boolean;

  // Realtime config (populated from backend events — never hardcoded)
  activeModel: string;
  activeProvider: string;
  settings: AgentDefaultsConfig | null;
  providers: ProviderModelOption[];
  recentModels: ModelRef[];
  favoriteModels: ModelRef[];
  loadingModelProvider: string | null;
  slashCommands: SlashCommand[];
  status: AgentStatus | null;

  // Settings & Toggles (bound to real config via set_config)
  cavemanMode: boolean;
  streamingMode: boolean;
  toggleCavemanMode: () => void;
  toggleStreamingMode: () => void;
  setActiveModel: (model: string, provider?: string) => void;
  requestProviderModels: (provider: string) => void;
  toggleFavoriteModel: (provider: string, model: string) => void;
  updateSettings: (patch: Partial<AgentDefaultsConfig>) => void;

  // Modals & Panels
  isSidebarOpen: boolean; // mobile drawer (screen < md)
  isSidebarCollapsed: boolean; // desktop icon-rail collapse
  isActivityPanelOpen: boolean;
  activeView: WorkspaceView;
  isMemoryOpen: boolean;
  isLogsOpen: boolean;
  isMcpsOpen: boolean;
  isSettingsOpen: boolean;
  isServersOpen: boolean;
  workspaceNotice: WorkspaceNotice | null;
  setIsSidebarOpen: (open: boolean) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setIsActivityPanelOpen: (open: boolean) => void;
  toggleActivityPanel: () => void;
  setActiveView: (view: WorkspaceView) => void;
  setIsMemoryOpen: (open: boolean) => void;
  setIsLogsOpen: (open: boolean) => void;
  setIsMcpsOpen: (open: boolean) => void;
  setIsSettingsOpen: (open: boolean) => void;
  setIsServersOpen: (open: boolean) => void;
  setWorkspaceNotice: (notice: Omit<WorkspaceNotice, 'timestamp'>) => void;
  clearWorkspaceNotice: (scope?: WorkspaceNotice['scope']) => void;

  // Memory & Logs State (real event payloads)
  cognitiveStats: CognitiveMemoryStats;
  mcpServers: McpServerInfo[];
  mcpStats: McpStats;
  logs: LogEntry[];
  servers: BackgroundServerInfo[];
  skills: SkillInfo[];
  subagents: SubagentInfo[];
  channels: ChannelConfigInfo[];
  providersConfig: JsonObject;
  channelsConfig: JsonObject;

  // Actions
  updateConfig: (data: OpenZConfigPatch) => void;
  init: () => void;
  selectSession: (chatId: string) => void;
  newSession: () => void;
  deleteSession: (chatId: string) => void;
  clearActiveSession: () => void;
  sendMessage: (content: string, attachments?: ChatAttachment[]) => void;
  stopTurn: () => void;
  handleSecurityChoice: (reqId: string, choice: 'approve' | 'deny') => void;
  requestServers: () => void;
  stopServer: (id: string) => void;
  saveSkill: (name: string, content: string) => void;
  deleteSkill: (name: string) => void;
  saveSubagent: (data: { name: string; description: string; systemPrompt: string; model?: string; fallbacks?: string[] }) => void;
  deleteSubagent: (name: string) => void;
}

const EMPTY_MEMORY: CognitiveMemoryStats = {
  entitiesCount: 0,
  relationsCount: 0,
  factsCount: 0,
  workingMemoryKeys: [],
  nodes: [],
  edges: [],
  facts: [],
};

const EMPTY_MCP_STATS: McpStats = { loaded: 0, failed: 0, total: 0 };
const DRAFT_SESSION_TITLE = 'New Session';
const ACTIVE_CHAT_STORAGE_KEY = 'openz_active_chat_id';

let msgCounter = 0;
const newMsgId = (prefix: string) => `${prefix}-${Date.now()}-${msgCounter++}`;


function mergeProviders(current: ProviderModelOption[], incoming: ProviderModelOption[], partial: boolean): ProviderModelOption[] {
  if (!partial) return incoming;
  const byName = new Map(current.map((provider) => [provider.name, provider]));
  for (const provider of incoming) {
    byName.set(provider.name, { ...(byName.get(provider.name) || {}), ...provider });
  }
  return Array.from(byName.values());
}

function withRecentModel(recent: ModelRef[], provider: string, model: string): ModelRef[] {
  const cleanProvider = provider.trim();
  const cleanModel = model.trim();
  if (!cleanProvider || !cleanModel) return recent;
  return [
    { provider: cleanProvider, model: cleanModel },
    ...recent.filter((entry) => entry.provider !== cleanProvider || entry.model !== cleanModel),
  ].slice(0, 12);
}

function savedActiveChatId(): string {
  try {
    return normalizeChatId(sessionStorage.getItem(ACTIVE_CHAT_STORAGE_KEY) || '');
  } catch {
    return '';
  }
}

function rememberActiveChatId(chatId: string) {
  const normalizedChatId = normalizeChatId(chatId);
  if (!normalizedChatId) return;
  try {
    sessionStorage.setItem(ACTIVE_CHAT_STORAGE_KEY, normalizedChatId);
  } catch {
    // Ignore storage failures; the active session still works for this page lifetime.
  }
}

function forgetActiveChatId() {
  try {
    sessionStorage.removeItem(ACTIVE_CHAT_STORAGE_KEY);
  } catch {
    // Ignore storage failures.
  }
}

function createDraftSession(chatId: string): OpenZSession {
  const now = Date.now();
  return {
    id: normalizeChatId(chatId),
    title: DRAFT_SESSION_TITLE,
    createdAt: now,
    lastMessageAt: now,
    messageCount: 0,
    isDraft: true,
  };
}

function upsertDraftSession(sessions: OpenZSession[], chatId: string): OpenZSession[] {
  const normalizedChatId = normalizeChatId(chatId);
  if (!normalizedChatId) return sessions;
  if (sessions.some((session) => session.id === normalizedChatId)) return sessions;
  return [createDraftSession(normalizedChatId), ...sessions.filter((session) => !session.isDraft)];
}

function titleFromFirstMessage(content: string): string {
  const compact = content.trim().replace(/\s+/g, ' ');
  if (!compact) return DRAFT_SESSION_TITLE;
  return compact.length > 42 ? compact.slice(0, 39) + '...' : compact;
}

/**
 * Infer the provider from a model name using prefix-based routing,
 * mirroring the backend's resolution logic.
 */
function inferProviderFromModel(model: string): string {
  const m = model.toLowerCase();
  // Explicit provider/model format (e.g. "openai/gpt-4o")
  if (m.includes('/')) {
    const provider = m.split('/')[0];
    // If it looks like an explicit provider prefix (not a model family), return it
    if (['openai', 'anthropic', 'google', 'google_ai_studio', 'deepseek', 'groq', 'mistral', 'openrouter'].includes(provider)) {
      return provider;
    }
  }
  if (m.startsWith('claude')) return 'anthropic';
  if (m.startsWith('gpt') || m.startsWith('o1') || m.startsWith('o3') || m.startsWith('o4')) return 'openai';
  if (m.startsWith('deepseek')) return 'deepseek';
  if (m.startsWith('groq/') || m.startsWith('llama') || m.startsWith('mixtral')) return 'groq';
  if (m.startsWith('gemini') || m.startsWith('gemma')) return 'google_ai_studio';
  return 'auto';
}

/**
 * Normalize the chat ID to have the 'ws:' prefix if it doesn't already
 * contain a channel prefix.
 */
function normalizeChatId(chatId: string): string {
  if (!chatId) return chatId;
  if (chatId.includes(':')) return chatId;

  const channelPrefixes = ['ws_', 'cli_', 'telegram_', 'subagent_'];
  const matchedPrefix = channelPrefixes.find((prefix) => chatId.startsWith(prefix));
  if (matchedPrefix) {
    return `${matchedPrefix.slice(0, -1)}:${chatId.slice(matchedPrefix.length)}`;
  }

  return `ws:${chatId}`;
}

// Guards against duplicate listener registration (React StrictMode double-invokes
// effects in dev, which would otherwise register every WS handler twice).
let hasInitialized = false;

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


function normalizeActivityKind(value: unknown): 'workflow' | 'memory' | 'research' | 'self_improvement' | 'source' | 'system' {
  const raw = typeof value === 'string' ? value : 'system';
  if (raw === 'workflow' || raw === 'memory' || raw === 'research' || raw === 'self_improvement' || raw === 'source') return raw;
  return 'system';
}

function attachActivityNotice(chatId: string, payload: Record<string, unknown>) {
  const state = useOpenZStore.getState();
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
    useOpenZStore.setState({
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
  useOpenZStore.setState({
    messages: { ...state.messages, [chatId]: [...messages, message] },
  });
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


function mergeAssistantFinalIntoToolTurn(messages: OpenZMessage[], nextMessage: OpenZMessage): boolean {
  if (nextMessage.role !== 'assistant') return false;
  if (nextMessage.toolCalls && nextMessage.toolCalls.length > 0) return false;
  if (nextMessage.activityNotices && nextMessage.activityNotices.length > 0) return false;

  for (let i = messages.length - 1; i >= 0; i--) {
    const candidate = messages[i];
    if (candidate.role === 'user') return false;
    if (candidate.role !== 'assistant') continue;
    if (!candidate.toolCalls || candidate.toolCalls.length === 0) return false;
    if (candidate.content.trim().length > 0) return false;

    messages[i] = {
      ...candidate,
      content: nextMessage.content,
      timestamp: nextMessage.timestamp || candidate.timestamp,
      model: nextMessage.model || candidate.model,
      reasoningContent: nextMessage.reasoningContent || candidate.reasoningContent,
    };
    return true;
  }

  return false;
}


function settleAssistantTurnMessages(messages: OpenZMessage[], reason: string, now = Date.now()): OpenZMessage[] {
  let changed = false;

  const settled = messages.map((message) => {
    if (message.role !== 'assistant') return message;

    const toolCalls = message.toolCalls?.map((tool) => {
      if (tool.status !== 'running') return tool;

      changed = true;
      return {
        ...tool,
        status: 'error' as const,
        output: tool.output || reason,
        error: tool.error || reason,
        endedAt: tool.endedAt || now,
        durationMs: tool.durationMs ?? (tool.startedAt ? now - tool.startedAt : undefined),
      };
    });

    if (message.isStreaming) changed = true;

    return {
      ...message,
      isStreaming: false,
      toolCalls,
    };
  });

  return changed ? settled : messages;
}

export const useOpenZStore = create<OpenZState>((set, get) => ({
  connectionStatus: 'disconnected',
  wsUrl: localStorage.getItem('openz_ws_url') || 'ws://127.0.0.1:8765/ws',
  wsToken: localStorage.getItem('openz_ws_token') || '',

  sessions: [],
  activeChatId: savedActiveChatId(),
  messages: {},
  isStreaming: false,

  activeModel: '',
  activeProvider: '',
  settings: null,
  providers: [],
  recentModels: [],
  favoriteModels: [],
  loadingModelProvider: null,
  slashCommands: [],
  status: null,

  cavemanMode: false,
  streamingMode: true,

  isSidebarOpen: false,
  isSidebarCollapsed: localStorage.getItem('openz_sidebar_collapsed') === '1',
  isActivityPanelOpen: localStorage.getItem('openz_activity_panel_open') !== '0',
  activeView: 'chats',
  isMemoryOpen: false,
  isLogsOpen: false,
  isMcpsOpen: false,
  isSettingsOpen: false,
  isServersOpen: false,
  workspaceNotice: null,

  cognitiveStats: EMPTY_MEMORY,
  mcpServers: [],
  mcpStats: EMPTY_MCP_STATS,
  logs: [],
  servers: [],
  skills: [],
  subagents: [],
  channels: [],
  providersConfig: {},
  channelsConfig: {},

  setIsSidebarOpen: (open) => set({ isSidebarOpen: open }),
  setSidebarCollapsed: (collapsed) => {
    localStorage.setItem('openz_sidebar_collapsed', collapsed ? '1' : '0');
    set({ isSidebarCollapsed: collapsed });
  },
  setIsActivityPanelOpen: (open) => {
    localStorage.setItem('openz_activity_panel_open', open ? '1' : '0');
    set({ isActivityPanelOpen: open });
  },
  toggleActivityPanel: () => {
    const open = !get().isActivityPanelOpen;
    localStorage.setItem('openz_activity_panel_open', open ? '1' : '0');
    set({ isActivityPanelOpen: open });
  },
  setActiveView: (view) => set({ activeView: view, workspaceNotice: null }),
  setIsMemoryOpen: (open) => set({ isMemoryOpen: open }),
  setIsLogsOpen: (open) => set({ isLogsOpen: open }),
  setIsMcpsOpen: (open) => set({ isMcpsOpen: open }),
  setIsSettingsOpen: (open) => set({ isSettingsOpen: open, workspaceNotice: open ? null : get().workspaceNotice }),
  setIsServersOpen: (open) => set({ isServersOpen: open }),
  setWorkspaceNotice: (notice) => set({ workspaceNotice: { ...notice, timestamp: Date.now() } }),
  clearWorkspaceNotice: (scope) => {
    const current = get().workspaceNotice;
    if (!scope || current?.scope === scope) set({ workspaceNotice: null });
  },

  setWsConfig: (url, token) => {
    set({ wsUrl: url, wsToken: token });
    wsService.setConfig(url, token);
  },

  // ---- Realtime config actions (persisted through the backend) ----

  updateConfig: (data) => {
    set({ workspaceNotice: { scope: 'settings', type: 'info', message: 'Settings save requested. Waiting for gateway refresh.', timestamp: Date.now() } });
    if (data.defaults) {
      const settings = get().settings;
      if (settings) {
        set({ settings: { ...settings, ...data.defaults } });
      }
    }
    if (data.providers) {
      const pc = get().providersConfig;
      set({ providersConfig: { ...pc, ...data.providers } });
    }
    if (data.channels) {
      const cc = get().channelsConfig;
      set({ channelsConfig: { ...cc, ...data.channels } });
    }
    wsService.sendSetConfig(data);
  },

  setActiveModel: (model, provider) => {
    const finalProvider = provider || inferProviderFromModel(model);
    set({
      activeModel: model,
      activeProvider: finalProvider,
      recentModels: withRecentModel(get().recentModels, finalProvider, model),
    });
    wsService.updateConfig({ model, provider: finalProvider });
  },

  requestProviderModels: (provider) => {
    set({ loadingModelProvider: provider });
    wsService.requestProviderModels(provider);
  },

  toggleFavoriteModel: (provider, model) => {
    wsService.toggleFavoriteModel(provider, model);
  },

  updateSettings: (patch) => {
    const settings = get().settings;
    if (settings) {
      set({ settings: { ...settings, ...patch } });
    }
    // Mirror convenience toggles into state immediately for optimistic UI.
    if (patch.caveman_mode !== undefined) set({ cavemanMode: patch.caveman_mode });
    if (patch.streaming !== undefined) set({ streamingMode: patch.streaming });
    wsService.updateConfig(patch);
  },

  toggleCavemanMode: () => {
    get().updateSettings({ caveman_mode: !get().cavemanMode });
  },

  toggleStreamingMode: () => {
    get().updateSettings({ streaming: !get().streamingMode });
  },

  // ---- Helpers applied to the active chat ----

  init: () => {
    if (hasInitialized) return;
    hasInitialized = true;

    wsService.setStatusCallback((status) => {
      set({ connectionStatus: status });
    });

    // ----- Realtime turn events (streamed from the agent loop) -----

    wsService.on('delta', (payload) => {
      const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
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
      attachActivityNotice(chatId, payload as Record<string, unknown>);
    });

    wsService.on('tool_start', (payload) => {
      const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
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

    wsService.on('turn_end', (payload) => {
      const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
      const chatMessages = get().messages[chatId] || [];
      const settledMessages = settleAssistantTurnMessages(
        chatMessages,
        'Turn ended before this tool reported completion.',
      );

      set({
        messages: { ...get().messages, [chatId]: settledMessages },
        isStreaming: false,
      });
      // Refresh the session list so titles/message counts stay in sync.
      wsService.requestSessions();
    });

    wsService.on('stopped', (payload) => {
      const payloadObj = isRecord(payload) ? payload : {};
      const chatId = normalizeChatId(asString(payloadObj.chat_id) || get().activeChatId);
      const chatMessages = get().messages[chatId] || [];
      const settledMessages = settleAssistantTurnMessages(
        chatMessages,
        'Turn stopped before this tool completed.',
      );

      set({
        messages: { ...get().messages, [chatId]: settledMessages },
        isStreaming: false,
      });
    });

    // ----- Data events (replace every hardcoded value) -----

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
    });

    wsService.on('sessions_list', (payload) => {
      if (Array.isArray(payload.sessions)) {
        const realSessions = payload.sessions.map((session: OpenZSession) => ({
          ...session,
          id: normalizeChatId(session.id),
          isDraft: false,
        }));
        const activeChatId = get().activeChatId;
        const exists = realSessions.some((s: OpenZSession) => s.id === activeChatId);

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

    wsService.on('session_history', (payload: { chat_id?: string; messages?: SessionHistoryMessage[] }) => {
      if (payload.chat_id && Array.isArray(payload.messages)) {
        const normalized: OpenZMessage[] = [];

        for (let i = 0; i < payload.messages.length; i++) {
          const m = payload.messages[i];
          const role = asString(m.role);

          if (role === 'user') {
            normalized.push({
              id: asString(m.id) || `msg-${i}`,
              role: 'user',
              content: asString(m.content) || '',
              timestamp: typeof m.timestamp === 'number' ? m.timestamp : Date.now(),
            });
          } else if (role === 'assistant') {
            const toolCalls: ToolExecution[] = [];
            if (m.extra && Array.isArray(m.extra.tool_calls)) {
              m.extra.tool_calls.forEach((tc) => {
                const tcFunction = isRecord(tc.function) ? tc.function : undefined;
                const tcName = asString(tcFunction?.name) || asString(tc.name) || 'tool';
                const tcArgs = parseToolArgs(tcFunction?.arguments ?? tc.arguments);

                toolCalls.push({
                  id: asString(tc.id) || `tool-${i}-${tcName}`,
                  name: tcName,
                  args: tcArgs,
                  status: 'success',
                  output: '',
                });
              });
            }

            const reasoningContent = asString(m.extra?.reasoning_content);

            const assistantMsg: OpenZMessage = {
              id: asString(m.id) || `msg-${i}`,
              role: 'assistant',
              content: asString(m.content) || '',
              timestamp: typeof m.timestamp === 'number' ? m.timestamp : Date.now(),
              model: asString(m.extra?.model),
              reasoningContent,
              toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
            };

            if (!mergeAssistantFinalIntoToolTurn(normalized, assistantMsg)) {
              normalized.push(assistantMsg);
            }
          } else if (role === 'tool') {
            const toolCallId = asString(m.extra?.tool_call_id);
            const toolName = asString(m.extra?.name) || 'tool';

            let lastAssistant: OpenZMessage | undefined;
            for (let j = normalized.length - 1; j >= 0; j--) {
              if (normalized[j].role === 'assistant') {
                lastAssistant = normalized[j];
                break;
              }
            }

            const toolContent = asString(m.content) || '';
            const toolErrored = toolContent.includes('"error"') || toolContent.toLowerCase().startsWith('error:');

            if (lastAssistant) {
              if (!lastAssistant.toolCalls) {
                lastAssistant.toolCalls = [];
              }

              const matched = lastAssistant.toolCalls.find(
                (tc) => (toolCallId && tc.id === toolCallId) || (!toolCallId && tc.name === toolName && !tc.output)
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
                id: asString(m.id) || `msg-${i}`,
                role: 'system',
                content: `Tool Execution [${toolName}]: ${toolContent}`,
                timestamp: typeof m.timestamp === 'number' ? m.timestamp : Date.now(),
                isNotice: true,
              });
            }
          } else if (role === 'system') {
            normalized.push({
              id: asString(m.id) || `msg-${i}`,
              role: 'system',
              content: asString(m.content) || '',
              timestamp: typeof m.timestamp === 'number' ? m.timestamp : Date.now(),
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

    wsService.on('cognitive_memory', (payload) => {
      if (payload.stats) {
        set({
          cognitiveStats: {
            entitiesCount: payload.stats.entitiesCount ?? 0,
            relationsCount: payload.stats.relationsCount ?? 0,
            factsCount: payload.stats.factsCount ?? 0,
            workingMemoryKeys: Array.isArray(payload.stats.workingMemoryKeys)
              ? payload.stats.workingMemoryKeys
              : [],
            nodes: Array.isArray(payload.nodes) ? payload.nodes : [],
            edges: Array.isArray(payload.edges) ? payload.edges : [],
            facts: Array.isArray(payload.facts) ? payload.facts : [],
          },
        });
      }
    });

    wsService.on('mcp_servers', (payload) => {
      if (Array.isArray(payload.servers)) {
        set({ mcpServers: payload.servers });
      }
      if (payload.stats) {
        set({
          mcpStats: {
            loaded: payload.stats.loaded ?? 0,
            failed: payload.stats.failed ?? 0,
            total: payload.stats.total ?? 0,
          },
        });
      }
    });

    wsService.on('logs_data', (payload) => {
      if (Array.isArray(payload.logs)) {
        set({ logs: payload.logs });
      }
    });

    wsService.on('servers_list', (payload) => {
      if (Array.isArray(payload.servers)) {
        set({ servers: payload.servers });
      }
      if (Array.isArray(payload.channels)) {
        set({ channels: payload.channels });
      }
    });

    wsService.on('server_stopped', () => {
      wsService.requestServers();
    });

    wsService.on('models_list', (payload) => {
      if (Array.isArray(payload.providers)) {
        set({
          providers: mergeProviders(get().providers, payload.providers, !!payload.partial),
          loadingModelProvider: null,
        });
      }
      if (Array.isArray(payload.recent_models)) {
        set({ recentModels: payload.recent_models });
      }
      if (Array.isArray(payload.favorite_models)) {
        set({ favoriteModels: payload.favorite_models });
      }
      if (payload.active_model) {
        set({ activeModel: payload.active_model });
      }
      if (payload.active_provider) {
        set({ activeProvider: payload.active_provider });
        if (get().settings) {
          const settings = get().settings!;
          set({ settings: { ...settings, provider: payload.active_provider } });
        }
      }
    });

    wsService.on('model_prefs', (payload) => {
      if (Array.isArray(payload.recent_models)) {
        set({ recentModels: payload.recent_models });
      }
      if (Array.isArray(payload.favorite_models)) {
        set({ favoriteModels: payload.favorite_models });
      }
    });

    wsService.on('config_data', (payload) => {
      if (payload.defaults) {
        set({
          settings: payload.defaults,
          activeModel: payload.defaults.model || get().activeModel,
          activeProvider: payload.defaults.provider || get().activeProvider,
          cavemanMode: !!payload.defaults.caveman_mode,
          streamingMode: payload.defaults.streaming !== false,
        });
      }
      if (payload.providers) {
        set({ providersConfig: payload.providers });
      }
      if (payload.channels) {
        set({ channelsConfig: payload.channels });
      }
      if (payload.version && payload.version !== (get().status?.version || '')) {
        const status = get().status;
        set({ status: { version: payload.version, mcp: status?.mcp || EMPTY_MCP_STATS } });
      }
      if (Array.isArray(payload.mcp_servers)) {
        set({ mcpServers: payload.mcp_servers });
      }
      if (Array.isArray(payload.skills)) {
        set({ skills: payload.skills });
      }
      if (Array.isArray(payload.subagents)) {
        set({ subagents: payload.subagents });
      }
    });

    wsService.on('skills_updated', (payload) => {
      if (Array.isArray(payload.skills)) {
        set({ skills: payload.skills });
      }
      const status = typeof payload.status === 'string' ? payload.status : 'updated';
      const name = typeof payload.name === 'string' ? payload.name : 'skills';
      set({ workspaceNotice: { scope: 'skills', type: 'success', message: `Skill ${name} ${status}.`, timestamp: Date.now() } });
    });

    wsService.on('subagents_updated', (payload) => {
      if (Array.isArray(payload.subagents)) {
        set({ subagents: payload.subagents });
      }
      const status = typeof payload.status === 'string' ? payload.status : 'updated';
      const name = typeof payload.name === 'string' ? payload.name : 'subagent';
      set({ workspaceNotice: { scope: 'agents', type: 'success', message: `Subagent ${name} ${status}.`, timestamp: Date.now() } });
    });

    wsService.on('config_updated', (payload) => {
      if (payload.defaults) {
        set({
          settings: payload.defaults,
          activeModel: payload.defaults.model || get().activeModel,
          activeProvider: payload.defaults.provider || get().activeProvider,
          cavemanMode: !!payload.defaults.caveman_mode,
          streamingMode: payload.defaults.streaming !== false,
          workspaceNotice: { scope: 'settings', type: 'success', message: 'Settings saved and refreshed from gateway.', timestamp: Date.now() },
        });
      }
    });

    wsService.on('slash_commands', (payload) => {
      if (Array.isArray(payload.commands)) {
        set({ slashCommands: payload.commands });
      }
    });

    wsService.on('status', (payload) => {
      set({
        status: {
          version: payload.version || '',
          mcp: {
            loaded: payload.mcp?.loaded ?? 0,
            failed: payload.mcp?.failed ?? 0,
            total: payload.mcp?.total ?? 0,
          },
        },
      });
    });

    wsService.on('attached', (payload) => {
      if (payload.chat_id) {
        const chatId = normalizeChatId(payload.chat_id);
        rememberActiveChatId(chatId);
        set({ activeChatId: chatId, activeView: 'chats', sessions: upsertDraftSession(get().sessions, chatId) });
        wsService.requestHistory(chatId);
      }
    });

    wsService.on('error', (payload) => {
      set({ isStreaming: false });
      const detail = payload.detail || 'Gateway error occurred.';
      if (!payload.chat_id && (get().activeView !== 'chats' || get().isSettingsOpen)) {
        const activeView = get().activeView;
        const scope = get().isSettingsOpen
          ? 'settings'
          : activeView === 'agents' || activeView === 'skills' || activeView === 'knowledge'
            ? activeView
            : 'global';
        set({ workspaceNotice: { scope, type: 'error', message: String(detail), timestamp: Date.now() } });
        return;
      }
      const chatId = normalizeChatId(payload.chat_id || get().activeChatId);
      const chatMessages = settleAssistantTurnMessages(
        get().messages[chatId] || [],
        'Turn errored before this tool reported completion.',
      );
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
        });
      } else {
        set({
          messages: { ...get().messages, [chatId]: [...chatMessages, errorMsg] },
        });
      }
    });

    wsService.connect();
  },

  requestServers: () => {
    wsService.requestServers();
  },

  stopServer: (id) => {
    wsService.stopServer(id);
  },

  saveSkill: (name, content) => {
    set({ workspaceNotice: { scope: 'skills', type: 'info', message: 'Skill save requested. Waiting for gateway refresh.', timestamp: Date.now() } });
    wsService.saveSkill(name, content);
  },

  deleteSkill: (name) => {
    set({ workspaceNotice: { scope: 'skills', type: 'info', message: 'Skill delete requested. Waiting for gateway refresh.', timestamp: Date.now() } });
    wsService.deleteSkill(name);
  },

  saveSubagent: (data) => {
    set({ workspaceNotice: { scope: 'agents', type: 'info', message: 'Subagent save requested. Waiting for gateway refresh.', timestamp: Date.now() } });
    wsService.saveSubagent(data);
  },

  deleteSubagent: (name) => {
    set({ workspaceNotice: { scope: 'agents', type: 'info', message: 'Subagent delete requested. Waiting for gateway refresh.', timestamp: Date.now() } });
    wsService.deleteSubagent(name);
  },

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

  deleteSession: (chatId) => {
    const nextSessions = get().sessions.filter((s) => s.id !== chatId);
    const nextMessages = { ...get().messages };
    delete nextMessages[chatId];
    set({ sessions: nextSessions, messages: nextMessages });

    if (get().activeChatId === chatId) {
      if (nextSessions.length > 0) {
        get().selectSession(nextSessions[0].id);
      } else {
        get().newSession();
      }
    }
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
      set({ workspaceNotice: { scope: 'global', type: 'error', message: 'No active chat session. Reconnect the gateway and try again.', timestamp: Date.now() } });
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
    wsService.sendStop(chatId);
    set({ isStreaming: false });
  },

  handleSecurityChoice: (reqId, choice) => {
    const chatId = get().activeChatId;
    const chatMessages = get().messages[chatId] || [];

    const updated = chatMessages.map((msg) => {
      if (msg.securityPrompts && msg.securityPrompts.some((p) => p.id === reqId)) {
        return {
          ...msg,
          securityPrompts: msg.securityPrompts.map((p) =>
            p.id === reqId
              ? { ...p, status: choice === 'approve' ? ('approved' as const) : ('denied' as const) }
              : p,
          ),
        };
      }
      return msg;
    });

    set({ messages: { ...get().messages, [chatId]: updated } });
    wsService.sendSecurityResponse(reqId, choice === 'approve');
  },
}));

// Type-only marker so TS keeps the store shape exportable without runtime effect.
export type OpenZStoreState = OpenZState;
