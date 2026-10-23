import type {
  ProviderModelOption,
} from '../../types';
import { normalizeWebUiCapabilities } from '../../types';
import { wsService } from '../../services/websocket';
import { createWorkspaceNotice } from '../chatUtils';
import type { StoreEventContext } from '../eventContext';
import { EMPTY_MCP_STATS, normalizeMcpStats } from '../slices/memorySlice';

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

function applyModelPrefsPatch(
  set: StoreEventContext['set'],
  payload: { recent_models?: unknown; favorite_models?: unknown },
) {
  if (Array.isArray(payload.recent_models)) {
    set({ recentModels: payload.recent_models });
  }
  if (Array.isArray(payload.favorite_models)) {
    set({ favoriteModels: payload.favorite_models });
  }
}

function applyCapabilitiesAndDefaultsPatch(
  { set, get }: StoreEventContext,
  payload: {
    capabilities?: unknown;
    defaults?: {
      model?: string;
      provider?: string;
      caveman_mode?: boolean;
      streaming?: boolean;
      max_tokens?: number;
      temperature?: number;
    };
  },
) {
  if (payload.capabilities && typeof payload.capabilities === 'object') {
    set({ capabilities: normalizeWebUiCapabilities(payload.capabilities as Record<string, unknown>) });
  }
  if (payload.defaults) {
    set({
      settings: payload.defaults as ReturnType<typeof get>['settings'],
      activeModel: payload.defaults.model || get().activeModel,
      activeProvider: payload.defaults.provider || get().activeProvider,
      cavemanMode: !!payload.defaults.caveman_mode,
      streamingMode: payload.defaults.streaming !== false,
    });
  }
}

function emitWorkspaceUpdateNotice(
  set: StoreEventContext['set'],
  scope: 'skills' | 'agents',
  labelPrefix: string,
  fallbackName: string,
  payload: { status?: unknown; name?: unknown },
) {
  const status = typeof payload.status === 'string' ? payload.status : 'updated';
  const name = typeof payload.name === 'string' ? payload.name : fallbackName;
  set({
    workspaceNotice: createWorkspaceNotice(
      scope,
      'success',
      `${labelPrefix} ${name} ${status}.`,
    ),
  });
}

export function registerConfigEvents(ctx: StoreEventContext) {
  const { set, get } = ctx;

  wsService.on('models_list', (payload) => {
    if (Array.isArray(payload.providers)) {
      set({
        providers: mergeProviders(get().providers, payload.providers, !!payload.partial),
        loadingModelProvider: null,
      });
    }
    applyModelPrefsPatch(set, payload);
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
    applyModelPrefsPatch(set, payload);
  });

  wsService.on('config_data', (payload) => {
    applyCapabilitiesAndDefaultsPatch(ctx, payload);
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
    emitWorkspaceUpdateNotice(set, 'skills', 'Skill', 'skills', payload);
  });

  wsService.on('subagents_updated', (payload) => {
    if (Array.isArray(payload.subagents)) {
      set({ subagents: payload.subagents });
    }
    emitWorkspaceUpdateNotice(set, 'agents', 'Subagent', 'subagent', payload);
  });

  wsService.on('config_updated', (payload) => {
    applyCapabilitiesAndDefaultsPatch(ctx, payload);
    if (payload.defaults) {
      set({
        workspaceNotice: createWorkspaceNotice(
          'settings',
          'success',
          'Settings saved and refreshed from gateway.',
        ),
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
        mcp: normalizeMcpStats(payload.mcp),
      },
    });
  });
}
