import type {
  ProviderModelOption,
} from '../../types';
import { normalizeWebUiCapabilities } from '../../types';
import { wsService } from '../../services/websocket';
import { createWorkspaceNotice } from '../chatUtils';
import type { StoreEventContext } from '../eventContext';
import { EMPTY_MCP_STATS, normalizeMcpStats } from '../slices/memorySlice';
import {
  registerThemeSyncListener,
  useThemeStore,
} from '../useThemeStore';
import {
  DEFAULT_CUSTOMIZATION,
  extractSnapshot,
  registerUiCustomizationSyncListener,
  useUiCustomizationStore,
} from '../useUiCustomizationStore';
import {
  normalizeCavemanLevel,
  normalizeCavemanStats,
} from '../../shared/lib/cavemanLevels';
import { dispatchWorkspaceEventToast } from '../useToastStore';
import { buildTelemetryNavigation } from './chatTurnEvents';

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
      theme?: string;
      theme_explicit?: string | null;
      active_theme_id?: string | null;
      ui_customization?: Record<string, unknown> | null;
      caveman_mode?: boolean;
      caveman_level?: string;
      caveman_stats?: unknown;
      streaming?: boolean;
      max_tokens?: number;
      temperature?: number;
    };
  },
  isInitialSnapshot = false,
) {
  if (payload.capabilities && typeof payload.capabilities === 'object') {
    set({ capabilities: normalizeWebUiCapabilities(payload.capabilities as Record<string, unknown>) });
  }
  if (payload.defaults) {
    const parsedCavemanStats = normalizeCavemanStats(payload.defaults.caveman_stats);
    set({
      settings: payload.defaults as ReturnType<typeof get>['settings'],
      activeModel: payload.defaults.model || get().activeModel,
      activeProvider: payload.defaults.provider || get().activeProvider,
      cavemanMode: !!payload.defaults.caveman_mode,
      cavemanLevel: normalizeCavemanLevel(
        payload.defaults.caveman_level ?? get().cavemanLevel,
      ),
      cavemanStats: parsedCavemanStats ?? get().cavemanStats,
      streamingMode: payload.defaults.streaming !== false,
    });

    const explicitTheme = payload.defaults.theme_explicit;
    const serverThemeId = payload.defaults.active_theme_id;
    const serverCustomization = payload.defaults.ui_customization;
    const hasServerUiState =
      Boolean(explicitTheme) ||
      Boolean(serverThemeId) ||
      Boolean(serverCustomization && typeof serverCustomization === 'object');

    if (
      explicitTheme === 'light' ||
      explicitTheme === 'dark' ||
      explicitTheme === 'system'
    ) {
      useThemeStore.getState().hydrateTheme(explicitTheme);
    }

    if (serverCustomization || serverThemeId) {
      useUiCustomizationStore
        .getState()
        .hydrateFromGateway(serverCustomization ?? {}, serverThemeId);
    }

    // When connecting to a gateway that does not yet have persisted WebUI preferences,
    // push local browser customizations & theme up so they are backed up on disk immediately.
    if (isInitialSnapshot && !hasServerUiState) {
      const localSnapshot = extractSnapshot(useUiCustomizationStore.getState());
      const localTheme = useThemeStore.getState().theme;
      const hasNonDefaultLocal =
        localTheme !== 'system' ||
        JSON.stringify(localSnapshot) !== JSON.stringify(DEFAULT_CUSTOMIZATION);
      if (hasNonDefaultLocal) {
        wsService.updateConfig({
          theme: localTheme,
          active_theme_id: localSnapshot.activeThemeId,
          ui_customization: localSnapshot as unknown as Record<string, unknown>,
        });
      }
    }
  }
}

function emitWorkspaceUpdateNotice(
  ctx: StoreEventContext,
  scope: 'skills' | 'agents',
  labelPrefix: string,
  fallbackName: string,
  payload: { status?: unknown; name?: unknown },
) {
  const status = typeof payload.status === 'string' ? payload.status : 'updated';
  const name = typeof payload.name === 'string' ? payload.name : fallbackName;
  const message = `${labelPrefix} ${name} ${status}.`;
  ctx.set({
    workspaceNotice: createWorkspaceNotice(
      scope,
      'success',
      message,
    ),
  });
  dispatchWorkspaceEventToast(scope, 'success', message, buildTelemetryNavigation(ctx));
}

export function registerConfigEvents(ctx: StoreEventContext) {
  const { set, get } = ctx;

  // Automatically persist every theme & UI customization change to Gateway + ~/.openz/backups
  registerThemeSyncListener((theme) => {
    const snapshot = extractSnapshot(useUiCustomizationStore.getState());
    wsService.updateConfig({
      theme,
      active_theme_id: snapshot.activeThemeId,
      ui_customization: snapshot as unknown as Record<string, unknown>,
    });
  });

  registerUiCustomizationSyncListener((snapshot) => {
    const theme = useThemeStore.getState().theme;
    wsService.updateConfig({
      theme,
      active_theme_id: snapshot.activeThemeId,
      ui_customization: snapshot as unknown as Record<string, unknown>,
    });
  });

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
    applyCapabilitiesAndDefaultsPatch(ctx, payload, true);
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
    emitWorkspaceUpdateNotice(ctx, 'skills', 'Skill', 'skills', payload);
  });

  wsService.on('subagents_updated', (payload) => {
    if (Array.isArray(payload.subagents)) {
      set({ subagents: payload.subagents });
    }
    emitWorkspaceUpdateNotice(ctx, 'agents', 'Subagent', 'subagent', payload);
  });

  wsService.on('config_updated', (payload) => {
    applyCapabilitiesAndDefaultsPatch(ctx, payload, false);
    if (payload.defaults) {
      set({
        workspaceNotice: createWorkspaceNotice(
          'settings',
          'success',
          'Settings saved and backed up to ~/.openz.',
        ),
      });
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
