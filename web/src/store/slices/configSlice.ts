import type { StateCreator } from 'zustand';
import type {
  AgentDefaultsConfig,
  AgentStatus,
  JsonObject,
  ModelRef,
  OpenZConfigPatch,
  ProviderModelOption,
  SkillInfo,
  SlashCommand,
  SubagentInfo,
  WebUiCapabilities,
} from '../../types';
import { providerKeyFromModel } from '../../config/providers';
import { wsService } from '../../services/websocket';
import type {
  SaveSubagentData,
  UpdateSubagentSettingsData,
} from '../../services/websocket/commands';
import { createWorkspaceNotice } from '../chatUtils';
import type { OpenZState } from '../useOpenZStore';

const EMPTY_CAPABILITIES: WebUiCapabilities = {
  version: 0,
  providers: [],
  securityModes: [],
  channels: [],
  browser: {
    firefoxWebdriverPort: 0,
    firefoxAttachPort: 0,
  },
  attachments: {
    maxCount: 0,
    maxFileBytes: 0,
    maxTotalBytes: 0,
    maxMessageBytes: 0,
    ttlSeconds: 0,
    allowedMimeTypes: [],
  },
};

export interface ConfigSlice {
  activeModel: string;
  activeProvider: string;
  settings: AgentDefaultsConfig | null;
  capabilities: WebUiCapabilities;
  providers: ProviderModelOption[];
  recentModels: ModelRef[];
  favoriteModels: ModelRef[];
  loadingModelProvider: string | null;
  slashCommands: SlashCommand[];
  status: AgentStatus | null;
  cavemanMode: boolean;
  streamingMode: boolean;
  providersConfig: JsonObject;
  channelsConfig: JsonObject;
  skills: SkillInfo[];
  subagents: SubagentInfo[];
  updateConfig: (data: OpenZConfigPatch) => void;
  setActiveModel: (model: string, provider?: string) => void;
  requestProviderModels: (provider: string) => void;
  toggleFavoriteModel: (provider: string, model: string) => void;
  updateSettings: (patch: Partial<AgentDefaultsConfig>) => void;
  toggleCavemanMode: () => void;
  toggleStreamingMode: () => void;
  saveSkill: (name: string, content: string) => void;
  deleteSkill: (name: string) => void;
  saveSubagent: (data: SaveSubagentData) => void;
  updateSubagentSettings: (data: UpdateSubagentSettingsData) => void;
  deleteSubagent: (name: string) => void;
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

function inferProviderFromModel(model: string, capabilities: WebUiCapabilities): string {
  return providerKeyFromModel(model, capabilities) || 'auto';
}

export const createConfigSlice: StateCreator<
  OpenZState,
  [],
  [],
  ConfigSlice
> = (set, get) => ({
  activeModel: '',
  activeProvider: '',
  settings: null,
  capabilities: EMPTY_CAPABILITIES,
  providers: [],
  recentModels: [],
  favoriteModels: [],
  loadingModelProvider: null,
  slashCommands: [],
  status: null,
  cavemanMode: false,
  streamingMode: true,
  providersConfig: {},
  channelsConfig: {},
  skills: [],
  subagents: [],

  updateConfig: (data) => {
    set({
      workspaceNotice: createWorkspaceNotice(
        'settings',
        'info',
        'Settings save requested. Waiting for gateway refresh.',
      ),
    });
    if (data.defaults) {
      const settings = get().settings;
      if (settings) {
        set({ settings: { ...settings, ...data.defaults } });
      }
    }
    if (data.providers) {
      const providersConfig = get().providersConfig;
      set({ providersConfig: { ...providersConfig, ...data.providers } });
    }
    if (data.channels) {
      const channelsConfig = get().channelsConfig;
      set({ channelsConfig: { ...channelsConfig, ...data.channels } });
    }
    wsService.sendSetConfig(data);
  },

  setActiveModel: (model, provider) => {
    const finalProvider = provider || inferProviderFromModel(model, get().capabilities);
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

  saveSkill: (name, content) => {
    set({
      workspaceNotice: createWorkspaceNotice(
        'skills',
        'info',
        'Skill save requested. Waiting for gateway refresh.',
      ),
    });
    wsService.saveSkill(name, content);
  },

  deleteSkill: (name) => {
    set({
      workspaceNotice: createWorkspaceNotice(
        'skills',
        'info',
        'Skill delete requested. Waiting for gateway refresh.',
      ),
    });
    wsService.deleteSkill(name);
  },

  saveSubagent: (data) => {
    set({
      workspaceNotice: createWorkspaceNotice(
        'agents',
        'info',
        'Subagent save requested. Waiting for gateway refresh.',
      ),
    });
    wsService.saveSubagent(data);
  },

  updateSubagentSettings: (data) => {
    set({
      workspaceNotice: createWorkspaceNotice(
        'agents',
        'info',
        'Subagent settings update requested. Waiting for gateway refresh.',
      ),
    });
    wsService.updateSubagentSettings(data);
  },

  deleteSubagent: (name) => {
    set({
      workspaceNotice: createWorkspaceNotice(
        'agents',
        'info',
        'Subagent delete requested. Waiting for gateway refresh.',
      ),
    });
    wsService.deleteSubagent(name);
  },
});
