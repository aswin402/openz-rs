import type {
  ProviderModelOption,
} from '../../types';
import { normalizeWebUiCapabilities } from '../../types';
import { wsService } from '../../services/websocket';
import type { StoreEventContext } from '../eventContext';
import { EMPTY_MCP_STATS } from '../slices/memorySlice';

function mergeProviders(
  current: ProviderModelOption[],
  incoming: ProviderModelOption[],
  partial: boolean,
): ProviderModelOption[] {
  if (!partial) return incoming;
  const byName = new Map(current.map((provider) => [provider.name, provider]));
  for (const provider of incoming) {
    byName.set(provider.name, { ...(byName.get(provider.name) || {}), ...provider });
  }
  return Array.from(byName.values());
}

export function registerConfigEvents({ set, get }: StoreEventContext) {
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
    if (payload.capabilities && typeof payload.capabilities === 'object') {
      set({ capabilities: normalizeWebUiCapabilities(payload.capabilities) });
    }
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
    if (payload.capabilities && typeof payload.capabilities === 'object') {
      set({ capabilities: normalizeWebUiCapabilities(payload.capabilities) });
    }
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
}
