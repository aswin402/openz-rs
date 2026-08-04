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
  AgentDefaultsConfig,
  SlashCommand,
  AgentStatus,
} from '../types';
import { wsService } from '../services/websocket';

/** Workspace views available from the left navigation rail. */
export type WorkspaceView = 'dashboard' | 'chats' | 'agents' | 'skills' | 'knowledge';

interface OpenZState {
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
  settings: AgentDefaultsConfig | null;
  providers: ProviderModelOption[];
  slashCommands: SlashCommand[];
  status: AgentStatus | null;

  // Settings & Toggles (bound to real config via set_config)
  cavemanMode: boolean;
  streamingMode: boolean;
  toggleCavemanMode: () => void;
  toggleStreamingMode: () => void;
  setActiveModel: (model: string) => void;
  updateSettings: (patch: Partial<AgentDefaultsConfig>) => void;

  // Modals & Panels
  isSidebarOpen: boolean; // mobile drawer (screen < md)
  isSidebarCollapsed: boolean; // desktop icon-rail collapse
  activeView: WorkspaceView;
  isMemoryOpen: boolean;
  isLogsOpen: boolean;
  isMcpsOpen: boolean;
  isSettingsOpen: boolean;
  setIsSidebarOpen: (open: boolean) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setActiveView: (view: WorkspaceView) => void;
  setIsMemoryOpen: (open: boolean) => void;
  setIsLogsOpen: (open: boolean) => void;
  setIsMcpsOpen: (open: boolean) => void;
  setIsSettingsOpen: (open: boolean) => void;

  // Memory & Logs State (real event payloads)
  cognitiveStats: CognitiveMemoryStats;
  mcpServers: McpServerInfo[];
  mcpStats: McpStats;
  logs: LogEntry[];

  // Actions
  init: () => void;
  selectSession: (chatId: string) => void;
  newSession: () => void;
  deleteSession: (chatId: string) => void;
  clearActiveSession: () => void;
  sendMessage: (content: string) => void;
  stopTurn: () => void;
  handleSecurityChoice: (reqId: string, choice: 'approve' | 'deny') => void;
}

const EMPTY_MEMORY: CognitiveMemoryStats = {
  entitiesCount: 0,
  relationsCount: 0,
  factsCount: 0,
  workingMemoryKeys: [],
};

const EMPTY_MCP_STATS: McpStats = { loaded: 0, failed: 0, total: 0 };

let msgCounter = 0;
const newMsgId = (prefix: string) => `${prefix}-${Date.now()}-${msgCounter++}`;

// Guards against duplicate listener registration (React StrictMode double-invokes
// effects in dev, which would otherwise register every WS handler twice).
let hasInitialized = false;

export const useOpenZStore = create<OpenZState>((set, get) => ({
  connectionStatus: 'disconnected',
  wsUrl: localStorage.getItem('openz_ws_url') || 'ws://127.0.0.1:8765/ws',
  wsToken: localStorage.getItem('openz_ws_token') || '',

  sessions: [],
  activeChatId: '',
  messages: {},
  isStreaming: false,

  activeModel: '',
  settings: null,
  providers: [],
  slashCommands: [],
  status: null,

  cavemanMode: false,
  streamingMode: true,

  isSidebarOpen: false,
  isSidebarCollapsed: localStorage.getItem('openz_sidebar_collapsed') === '1',
  activeView: 'chats',
  isMemoryOpen: false,
  isLogsOpen: false,
  isMcpsOpen: false,
  isSettingsOpen: false,

  cognitiveStats: EMPTY_MEMORY,
  mcpServers: [],
  mcpStats: EMPTY_MCP_STATS,
  logs: [],

  setIsSidebarOpen: (open) => set({ isSidebarOpen: open }),
  setSidebarCollapsed: (collapsed) => {
    localStorage.setItem('openz_sidebar_collapsed', collapsed ? '1' : '0');
    set({ isSidebarCollapsed: collapsed });
  },
  setActiveView: (view) => set({ activeView: view }),
  setIsMemoryOpen: (open) => set({ isMemoryOpen: open }),
  setIsLogsOpen: (open) => set({ isLogsOpen: open }),
  setIsMcpsOpen: (open) => set({ isMcpsOpen: open }),
  setIsSettingsOpen: (open) => set({ isSettingsOpen: open }),

  setWsConfig: (url, token) => {
    set({ wsUrl: url, wsToken: token });
    wsService.setConfig(url, token);
  },

  // ---- Realtime config actions (persisted through the backend) ----

  setActiveModel: (model) => {
    set({ activeModel: model });
    wsService.updateConfig({ model });
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
      const chatId = payload.chat_id || get().activeChatId;
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
      const chatId = payload.chat_id || get().activeChatId;
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

    wsService.on('tool_start', (payload) => {
      const chatId = payload.chat_id || get().activeChatId;
      const tool: ToolExecution = {
        id: payload.tool_call_id || newMsgId('tool'),
        name: payload.name || 'tool',
        args: payload.args,
        status: 'running',
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
      const chatId = payload.chat_id || get().activeChatId;
      const chatMessages = get().messages[chatId] || [];
      const lastMsg = chatMessages[chatMessages.length - 1];
      if (!lastMsg || lastMsg.role !== 'assistant') return;

      const toolCalls = lastMsg.toolCalls || [];
      const tool: ToolExecution = {
        id: payload.tool_call_id || '',
        name: payload.name || 'tool',
        status: payload.status === 'error' ? 'error' : 'success',
        output: payload.status === 'error' ? payload.output : payload.output,
        error: payload.status === 'error' ? payload.output : undefined,
      };
      const existingIdx = toolCalls.findIndex((t) => t.id === tool.id);
      const updatedMsg: OpenZMessage = {
        ...lastMsg,
        toolCalls:
          existingIdx >= 0
            ? toolCalls.map((t, i) =>
                i === existingIdx ? { ...t, ...tool, args: t.args } : t,
              )
            : [...toolCalls, tool],
      };
      set({
        messages: {
          ...get().messages,
          [chatId]: [...chatMessages.slice(0, -1), updatedMsg],
        },
      });
    });

    wsService.on('security_request', (payload) => {
      const chatId = payload.chat_id || get().activeChatId;
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
      const chatId = payload.chat_id || get().activeChatId;
      const chatMessages = get().messages[chatId] || [];
      const lastMsg = chatMessages[chatMessages.length - 1];

      if (lastMsg && lastMsg.role === 'assistant') {
        const updatedMsg = { ...lastMsg, isStreaming: false };
        set({
          messages: {
            ...get().messages,
            [chatId]: [...chatMessages.slice(0, -1), updatedMsg],
          },
          isStreaming: false,
        });
      } else {
        set({ isStreaming: false });
      }
      // Refresh the session list so titles/message counts stay in sync.
      wsService.requestSessions();
    });

    wsService.on('stopped', () => {
      set({ isStreaming: false });
    });

    // ----- Data events (replace every hardcoded value) -----

    wsService.on('ready', (payload) => {
      const chatId = payload.chat_id || get().activeChatId;
      if (!get().activeChatId) {
        set({ activeChatId: chatId });
      }
      // Request everything real from the gateway on connect.
      wsService.requestSessions();
      wsService.requestHistory(chatId);
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
        set({ sessions: payload.sessions });
        // Ensure the active chat exists in the real list; if not, prefer the
        // first real session instead of a fabricated id.
        const activeChatId = get().activeChatId;
        const exists = payload.sessions.some((s: OpenZSession) => s.id === activeChatId);
        if (!exists && payload.sessions.length > 0) {
          const first = payload.sessions[0];
          set({ activeChatId: first.id });
          wsService.requestHistory(first.id);
        }
      }
    });

    wsService.on('session_history', (payload) => {
      if (payload.chat_id && Array.isArray(payload.messages)) {
        const normalized: OpenZMessage[] = payload.messages.map((m: any) => {
          const base: OpenZMessage = {
            id: m.id || newMsgId('msg'),
            role: m.role === 'user' || m.role === 'assistant' || m.role === 'tool' || m.role === 'system' ? m.role : 'assistant',
            content: m.content || '',
            timestamp: typeof m.timestamp === 'number' ? m.timestamp : Date.now(),
          };
          if (m.role === 'tool') {
            base.toolCalls = [
              {
                id: newMsgId('tool'),
                name: 'tool',
                status: 'success',
                output: m.content || '',
              },
            ];
          }
          return base;
        });
        set({
          messages: { ...get().messages, [payload.chat_id]: normalized },
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

    wsService.on('models_list', (payload) => {
      if (Array.isArray(payload.providers)) {
        set({ providers: payload.providers });
      }
      if (payload.active_model) {
        set({ activeModel: payload.active_model });
      }
      if (payload.active_provider && get().settings) {
        const settings = get().settings!;
        set({ settings: { ...settings, provider: payload.active_provider } });
      }
    });

    wsService.on('config_data', (payload) => {
      if (payload.defaults) {
        set({
          settings: payload.defaults,
          activeModel: payload.defaults.model || get().activeModel,
          cavemanMode: !!payload.defaults.caveman_mode,
          streamingMode: payload.defaults.streaming !== false,
        });
      }
      if (payload.version && payload.version !== (get().status?.version || '')) {
        const status = get().status;
        set({ status: { version: payload.version, mcp: status?.mcp || EMPTY_MCP_STATS } });
      }
      if (Array.isArray(payload.mcp_servers)) {
        set({ mcpServers: payload.mcp_servers });
      }
    });

    wsService.on('config_updated', (payload) => {
      if (payload.defaults) {
        set({
          settings: payload.defaults,
          activeModel: payload.defaults.model || get().activeModel,
          cavemanMode: !!payload.defaults.caveman_mode,
          streamingMode: payload.defaults.streaming !== false,
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
        set({ activeChatId: payload.chat_id });
        wsService.requestHistory(payload.chat_id);
      }
    });

    wsService.on('error', (payload) => {
      set({ isStreaming: false });
      const chatId = payload.chat_id || get().activeChatId;
      const chatMessages = get().messages[chatId] || [];
      const errorMsg: OpenZMessage = {
        id: newMsgId('err'),
        role: 'assistant',
        content: `⚠️ **Error**: ${payload.detail || 'Gateway error occurred.'}`,
        timestamp: Date.now(),
        isStreaming: false,
        isNotice: true,
      };
      set({
        messages: { ...get().messages, [chatId]: [...chatMessages, errorMsg] },
      });
    });

    wsService.connect();
  },

  selectSession: (chatId) => {
    set({ activeView: 'chats' });
    if (get().activeChatId !== chatId) {
      set({ activeChatId: chatId });
      wsService.attachChat(chatId);
    }
  },

  newSession: () => {
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

  sendMessage: (content) => {
    if (!content.trim()) return;
    const chatId = get().activeChatId;
    const chatMessages = get().messages[chatId] || [];

    const userMsg: OpenZMessage = {
      id: newMsgId('msg-user'),
      role: 'user',
      content,
      timestamp: Date.now(),
    };

    const nextMessages = {
      ...get().messages,
      [chatId]: [...chatMessages, userMsg],
    };
    set({ messages: nextMessages, isStreaming: true });

    try {
      wsService.sendMessage(chatId, content);
    } catch (err: any) {
      set({ isStreaming: false });
      const errMsgs = {
        ...nextMessages,
        [chatId]: [
          ...nextMessages[chatId],
          {
            id: newMsgId('err'),
            role: 'assistant' as const,
            content: `⚠️ **Connection Error**: ${err.message || 'Gateway offline'}`,
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

// Type-only marker so TS keeps the interface honest (no runtime effect).
export interface OpenZStoreState extends OpenZState {}
