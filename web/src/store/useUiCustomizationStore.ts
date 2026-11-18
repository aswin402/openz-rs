import { create } from "zustand";
import { createJSONStorage, persist, type StateStorage } from "zustand/middleware";
import {
  normalizeApprovalStyle,
  normalizeToolResultStyle,
  type ApprovalStyleChoice,
  type ToolResultStyleChoice,
} from "../components/assistant-ui/elements";
import {
  normalizeAlertStyle,
  normalizeBeuiLoaderVariant,
  normalizeToastPosition,
  normalizeToastVariant,
  type BeuiActivityDensity,
  type BeuiActivityMaxHeight,
  type BeuiActivityOrbStyle,
  type BeuiActivityPlacement,
  type BeuiActivityVariant,
  type BeuiAgentAvatarVariant,
  type BeuiAlertStyleVariant,
  type BeuiCitationsVariant,
  type BeuiCodeBlockVariant,
  type BeuiCollapsibleVariant,
  type BeuiContextUsageVariant,
  type BeuiDataTableVariant,
  type BeuiDiffViewMode,
  type BeuiInlineNoticeStyle,
  type BeuiJsonViewerVariant,
  type BeuiLoaderVariant,
  type BeuiMessageActionsVariant,
  type BeuiModelSelectorVariant,
  type BeuiPlanChecklistVariant,
  type BeuiPromptInputVariant,
  type BeuiPromptSuggestionsVariant,
  type BeuiReasoningVariant,
  type BeuiSendButtonVariant,
  type BeuiStreamingCursorStyle,
  type BeuiSubagentCardVariant,
  type BeuiTerminalVariant,
  type BeuiToastDurationPreset,
  type BeuiToastMaxVisible,
  type BeuiToastPosition,
  type BeuiToastVariant,
  type BeuiUserAvatarVariant,
  type ScrollerBehaviorMode,
} from "../components/beui";
import {
  THEME_FAMILIES,
  resolveDualModePalette,
  resolveThemeFamily,
  type DualModeThemeFamily,
  type ResolvedThemePalette,
  type ThemeAppearanceMode,
} from "../shared/lib/colorEngine";
import type { Theme } from "./useThemeStore";

export const UI_CUSTOMIZATION_STORAGE_KEY = "openz-ui-customization";
export const UI_CUSTOMIZATION_BACKUP_KEY = "openz-ui-customization-backup";
export const LEGACY_UI_CUSTOMIZATION_KEYS = [
  UI_CUSTOMIZATION_BACKUP_KEY,
  "openz-ui-customization-v5",
  "openz-ui-customization-v4",
  "openz-ui-customization-v3",
  "openz-ui-customization-v2",
  "openz-ui-customization-v1",
] as const;

const memoryMap = new Map<string, string>();

function readRawStorageKey(key: string): string | null {
  if (typeof globalThis.localStorage !== "undefined") {
    return globalThis.localStorage.getItem(key);
  }
  return memoryMap.get(key) ?? null;
}

function writeRawStorageKey(key: string, value: string): void {
  if (typeof globalThis.localStorage !== "undefined") {
    globalThis.localStorage.setItem(key, value);
    return;
  }
  memoryMap.set(key, value);
}

const safePersistStorage: StateStorage = {
  getItem: (name) => {
    const direct = readRawStorageKey(name);
    if (direct !== null) {
      writeRawStorageKey(UI_CUSTOMIZATION_BACKUP_KEY, direct);
      return direct;
    }
    for (const legacyKey of LEGACY_UI_CUSTOMIZATION_KEYS) {
      const legacyValue = readRawStorageKey(legacyKey);
      if (legacyValue !== null) {
        writeRawStorageKey(name, legacyValue);
        writeRawStorageKey(UI_CUSTOMIZATION_BACKUP_KEY, legacyValue);
        return legacyValue;
      }
    }
    return null;
  },
  setItem: (name, value) => {
    writeRawStorageKey(name, value);
    writeRawStorageKey(UI_CUSTOMIZATION_BACKUP_KEY, value);
  },
  removeItem: (name) => {
    if (typeof globalThis.localStorage !== "undefined") {
      globalThis.localStorage.removeItem(name);
      return;
    }
    memoryMap.delete(name);
  },
};

export type ThemePalettePreset = ResolvedThemePalette;
export type ThemeFamilyPreset = DualModeThemeFamily;

/**
 * Every available OpenZ theme supports BOTH Dark and Light modes via `colorEngine`.
 */
export const PREBUILT_THEMES: DualModeThemeFamily[] = THEME_FAMILIES;

export type ChatWidthChoice = "standard" | "wide" | "full";
export type AgentAvatarChoice = BeuiAgentAvatarVariant;
export type UserAvatarChoice = BeuiUserAvatarVariant;
export type BubbleStyleChoice = "beui-card" | "beui-full" | "editorial";
export type { ApprovalStyleChoice, ToolResultStyleChoice };
export type ImageGenStyleChoice = "beui-frame" | "minimal";
export type ActivityStyleChoice = BeuiActivityPlacement;
export type AlertStyleChoice = BeuiAlertStyleVariant;
export type ThinkingLoaderChoice = BeuiLoaderVariant;

export type GatewaySettingsSectionId = "agent" | "caveman" | "providers" | "channels";

export type CustomizeUiSectionId =
  | "theme"
  | "avatars"
  | "prompt-input"
  | "model-selector"
  | "prompt-suggestions"
  | "context-usage"
  | "message-scroller"
  | "message-card"
  | "message-actions"
  | "streaming-response"
  | "reasoning-text"
  | "agent-activity"
  | "animated-toast-stack"
  | "plan-checklist"
  | "subagent-card"
  | "code-block"
  | "file-diff"
  | "terminal-output"
  | "tool-result"
  | "tool-approval"
  | "image-generation"
  | "citations"
  | "data-table"
  | "json-viewer"
  | "alert-collapsible";

export interface UiCustomizationSnapshot {
  activeThemeId: string;
  chatWidth: ChatWidthChoice;
  promptInputStyle: BeuiPromptInputVariant;
  sendButtonStyle: BeuiSendButtonVariant;
  modelSelectorStyle: BeuiModelSelectorVariant;
  promptSuggestionsStyle: BeuiPromptSuggestionsVariant;
  contextUsageStyle: BeuiContextUsageVariant;
  scrollerBehavior: ScrollerBehaviorMode;
  agentAvatar: AgentAvatarChoice;
  userAvatar: UserAvatarChoice;
  bubbleStyle: BubbleStyleChoice;
  messageActionsStyle: BeuiMessageActionsVariant;
  showMessageActions: boolean;
  streamingAnimation: BeuiStreamingCursorStyle;
  thinkingAnimation: BeuiReasoningVariant;
  thinkingLoader: ThinkingLoaderChoice;
  activityStyle: ActivityStyleChoice;
  activityVariant: BeuiActivityVariant;
  activityOrbStyle: BeuiActivityOrbStyle;
  activityDensity: BeuiActivityDensity;
  activityMaxHeight: BeuiActivityMaxHeight;
  activityAutoCollapse: boolean;
  activityShowDetails: boolean;
  showDashboardActivity: boolean;
  toastEnabled: boolean;
  toastVariant: BeuiToastVariant;
  toastPosition: BeuiToastPosition;
  toastDuration: BeuiToastDurationPreset;
  toastMaxVisible: BeuiToastMaxVisible;
  inlineNoticeStyle: BeuiInlineNoticeStyle;
  toastNotifyWorkflow: boolean;
  toastNotifySkill: boolean;
  toastNotifySelfImprovement: boolean;
  toastNotifyMemory: boolean;
  toastNotifyResearch: boolean;
  toastNotifyApproval: boolean;
  planChecklistStyle: BeuiPlanChecklistVariant;
  subagentCardStyle: BeuiSubagentCardVariant;
  codeStyle: BeuiCodeBlockVariant;
  diffLayout: BeuiDiffViewMode;
  terminalStyle: BeuiTerminalVariant;
  toolResultStyle: ToolResultStyleChoice;
  approvalStyle: ApprovalStyleChoice;
  imageGenStyle: ImageGenStyleChoice;
  citationsStyle: BeuiCitationsVariant;
  dataTableStyle: BeuiDataTableVariant;
  jsonViewerStyle: BeuiJsonViewerVariant;
  alertStyle: AlertStyleChoice;
  collapsibleStyle: BeuiCollapsibleVariant;
}

export const DEFAULT_CUSTOMIZATION: UiCustomizationSnapshot = {
  activeThemeId: "obsidian-ember",
  chatWidth: "standard",
  promptInputStyle: "elevated",
  sendButtonStyle: "icon-round",
  modelSelectorStyle: "command",
  promptSuggestionsStyle: "bento",
  contextUsageStyle: "ring-pill",
  scrollerBehavior: "beui-smart",
  agentAvatar: "fox-classic",
  userAvatar: "user-default",
  bubbleStyle: "beui-card",
  messageActionsStyle: "floating",
  showMessageActions: true,
  streamingAnimation: "beui-pulse",
  thinkingAnimation: "card",
  thinkingLoader: "dot-matrix",
  activityStyle: "beui-timeline",
  activityVariant: "orb-card",
  activityOrbStyle: "dotted-orb",
  activityDensity: "comfortable",
  activityMaxHeight: "standard",
  activityAutoCollapse: true,
  activityShowDetails: true,
  showDashboardActivity: true,
  toastEnabled: true,
  toastVariant: "beui-stack",
  toastPosition: "bottom-end",
  toastDuration: "standard",
  toastMaxVisible: 4,
  inlineNoticeStyle: "pills",
  toastNotifyWorkflow: true,
  toastNotifySkill: true,
  toastNotifySelfImprovement: true,
  toastNotifyMemory: true,
  toastNotifyResearch: true,
  toastNotifyApproval: true,
  planChecklistStyle: "card",
  subagentCardStyle: "card",
  codeStyle: "framed",
  diffLayout: "unified",
  terminalStyle: "console",
  toolResultStyle: "beui-activity",
  approvalStyle: "assistant-default",
  imageGenStyle: "beui-frame",
  citationsStyle: "pills",
  dataTableStyle: "interactive",
  jsonViewerStyle: "tree",
  alertStyle: "beui-alert",
  collapsibleStyle: "subtle",
};

const SNAPSHOT_KEYS = Object.keys(DEFAULT_CUSTOMIZATION) as Array<keyof UiCustomizationSnapshot>;

export function sanitizeCustomizationSnapshot(
  raw: unknown,
): Partial<UiCustomizationSnapshot> {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  const record = raw as Record<string, unknown>;
  const clean: Partial<UiCustomizationSnapshot> = {};

  for (const key of SNAPSHOT_KEYS) {
    const val = record[key];
    if (val === undefined || val === null) continue;
    if (key === "activeThemeId") {
      if (typeof val === "string" && PREBUILT_THEMES.some((t) => t.id === val)) {
        clean.activeThemeId = val;
      }
      continue;
    }
    if (key === "toastMaxVisible") {
      if (val === 3 || val === 4 || val === 5) {
        clean.toastMaxVisible = val;
      }
      continue;
    }
    if (key === "toastVariant") {
      if (typeof val === "string") {
        clean.toastVariant = normalizeToastVariant(val);
      }
      continue;
    }
    if (key === "toastPosition") {
      if (typeof val === "string") {
        clean.toastPosition = normalizeToastPosition(val);
      }
      continue;
    }
    if (key === "toolResultStyle") {
      if (typeof val === "string") {
        clean.toolResultStyle = normalizeToolResultStyle(val);
      }
      continue;
    }
    if (key === "approvalStyle") {
      if (typeof val === "string") {
        clean.approvalStyle = normalizeApprovalStyle(val);
      }
      continue;
    }
    if (key === "thinkingLoader") {
      if (typeof val === "string") {
        clean.thinkingLoader = normalizeBeuiLoaderVariant(val);
      }
      continue;
    }
    if (key === "alertStyle") {
      if (typeof val === "string") {
        clean.alertStyle = normalizeAlertStyle(val);
      }
      continue;
    }
    const defaultVal = DEFAULT_CUSTOMIZATION[key];
    if (typeof val === typeof defaultVal) {
      (clean as Record<string, unknown>)[key] = val;
    }
  }
  return clean;
}

export interface UiBackupPayload {
  version: "1.0";
  exportedAt: string;
  themeMode: Theme;
  activeThemeId: string;
  uiCustomization: UiCustomizationSnapshot;
  gatewayDefaults?: Record<string, unknown>;
}

type UiCustomizationSyncListener = (snapshot: UiCustomizationSnapshot) => void;
let customizationSyncListener: UiCustomizationSyncListener | null = null;

export function registerUiCustomizationSyncListener(
  listener: UiCustomizationSyncListener | null,
): () => void {
  customizationSyncListener = listener;
  return () => {
    if (customizationSyncListener === listener) {
      customizationSyncListener = null;
    }
  };
}

function notifyCustomizationSync(snapshot: UiCustomizationSnapshot): void {
  if (customizationSyncListener) {
    customizationSyncListener(snapshot);
  }
}

export interface UiCustomizationState extends UiCustomizationSnapshot {
  isCustomizeOpen: boolean;
  isCommandPaletteOpen: boolean;
  activeCustomizeSection: CustomizeUiSectionId;
  activeGatewaySection: GatewaySettingsSectionId;
  quotedReplyText: string | null;
  lastSavedAt: string | null;
  setIsCustomizeOpen: (open: boolean) => void;
  setIsCommandPaletteOpen: (open: boolean) => void;
  setActiveCustomizeSection: (section: CustomizeUiSectionId) => void;
  setActiveGatewaySection: (section: GatewaySettingsSectionId) => void;
  setQuotedReplyText: (text: string | null) => void;
  selectThemePreset: (themeId: string) => DualModeThemeFamily | undefined;
  applyCustomization: (patch: Partial<UiCustomizationSnapshot>) => void;
  resetAllCustomization: () => void;
  hydrateFromGateway: (
    patch: unknown,
    activeThemeId?: string | null,
  ) => void;
  exportBackupJson: (options?: {
    themeMode?: Theme;
    gatewayDefaults?: Record<string, unknown> | null;
  }) => string;
  importBackupJson: (rawJson: string) => {
    ok: boolean;
    themeMode?: Theme;
    error?: string;
  };
}

export function extractSnapshot(state: UiCustomizationSnapshot): UiCustomizationSnapshot {
  return {
    activeThemeId: state.activeThemeId,
    chatWidth: state.chatWidth,
    promptInputStyle: state.promptInputStyle,
    sendButtonStyle: state.sendButtonStyle,
    modelSelectorStyle: state.modelSelectorStyle,
    promptSuggestionsStyle: state.promptSuggestionsStyle,
    contextUsageStyle: state.contextUsageStyle,
    scrollerBehavior: state.scrollerBehavior,
    agentAvatar: state.agentAvatar,
    userAvatar: state.userAvatar,
    bubbleStyle: state.bubbleStyle,
    messageActionsStyle: state.messageActionsStyle,
    showMessageActions: state.showMessageActions,
    streamingAnimation: state.streamingAnimation,
    thinkingAnimation: state.thinkingAnimation,
    thinkingLoader: state.thinkingLoader,
    activityStyle: state.activityStyle,
    activityVariant: state.activityVariant,
    activityOrbStyle: state.activityOrbStyle,
    activityDensity: state.activityDensity,
    activityMaxHeight: state.activityMaxHeight,
    activityAutoCollapse: state.activityAutoCollapse,
    activityShowDetails: state.activityShowDetails,
    showDashboardActivity: state.showDashboardActivity,
    toastEnabled: state.toastEnabled,
    toastVariant: state.toastVariant,
    toastPosition: state.toastPosition,
    toastDuration: state.toastDuration,
    toastMaxVisible: state.toastMaxVisible,
    inlineNoticeStyle: state.inlineNoticeStyle,
    toastNotifyWorkflow: state.toastNotifyWorkflow,
    toastNotifySkill: state.toastNotifySkill,
    toastNotifySelfImprovement: state.toastNotifySelfImprovement,
    toastNotifyMemory: state.toastNotifyMemory,
    toastNotifyResearch: state.toastNotifyResearch,
    toastNotifyApproval: state.toastNotifyApproval,
    planChecklistStyle: state.planChecklistStyle,
    subagentCardStyle: state.subagentCardStyle,
    codeStyle: state.codeStyle,
    diffLayout: state.diffLayout,
    terminalStyle: state.terminalStyle,
    toolResultStyle: state.toolResultStyle,
    approvalStyle: state.approvalStyle,
    imageGenStyle: state.imageGenStyle,
    citationsStyle: state.citationsStyle,
    dataTableStyle: state.dataTableStyle,
    jsonViewerStyle: state.jsonViewerStyle,
    alertStyle: state.alertStyle,
    collapsibleStyle: state.collapsibleStyle,
  };
}

export function resolveActivePalette(
  themeId: string,
  mode: ThemeAppearanceMode = "dark",
): ResolvedThemePalette {
  return resolveDualModePalette(themeId, mode);
}

export { resolveThemeFamily };

export const useUiCustomizationStore = create<UiCustomizationState>()(
  persist(
    (set, get) => ({
      ...DEFAULT_CUSTOMIZATION,
      isCustomizeOpen: false,
      isCommandPaletteOpen: false,
      activeCustomizeSection: "theme",
      activeGatewaySection: "agent",
      quotedReplyText: null,
      lastSavedAt: null,

      setIsCustomizeOpen: (open) => set({ isCustomizeOpen: open }),
      setIsCommandPaletteOpen: (open) => set({ isCommandPaletteOpen: open }),
      setActiveCustomizeSection: (section) => set({ activeCustomizeSection: section }),
      setActiveGatewaySection: (section) => set({ activeGatewaySection: section }),
      setQuotedReplyText: (text) => set({ quotedReplyText: text }),

      selectThemePreset: (themeId) => {
        const family = PREBUILT_THEMES.find((t) => t.id === themeId);
        if (!family) return undefined;
        const now = new Date().toISOString();
        set({ activeThemeId: family.id, lastSavedAt: now });
        notifyCustomizationSync(extractSnapshot(get()));
        return family;
      },

      applyCustomization: (patch) => {
        const clean = sanitizeCustomizationSnapshot(patch);
        const now = new Date().toISOString();
        set((prev) => ({ ...prev, ...clean, lastSavedAt: now }));
        notifyCustomizationSync(extractSnapshot(get()));
      },

      resetAllCustomization: () => {
        const now = new Date().toISOString();
        set({ ...DEFAULT_CUSTOMIZATION, quotedReplyText: null, lastSavedAt: now });
        notifyCustomizationSync(extractSnapshot(get()));
      },

      hydrateFromGateway: (patch, activeThemeId) => {
        const clean = sanitizeCustomizationSnapshot(patch);
        if (
          typeof activeThemeId === "string" &&
          PREBUILT_THEMES.some((t) => t.id === activeThemeId)
        ) {
          clean.activeThemeId = activeThemeId;
        }
        if (Object.keys(clean).length === 0) return;
        const current = extractSnapshot(get());
        const nextSnapshot: UiCustomizationSnapshot = { ...current, ...clean };
        if (JSON.stringify(current) === JSON.stringify(nextSnapshot)) return;
        set({ ...clean, lastSavedAt: new Date().toISOString() });
      },

      exportBackupJson: (options) => {
        const snapshot = extractSnapshot(get());
        const payload: UiBackupPayload = {
          version: "1.0",
          exportedAt: new Date().toISOString(),
          themeMode: options?.themeMode ?? "dark",
          activeThemeId: snapshot.activeThemeId,
          uiCustomization: snapshot,
          ...(options?.gatewayDefaults ? { gatewayDefaults: options.gatewayDefaults } : {}),
        };
        return JSON.stringify(payload, null, 2);
      },

      importBackupJson: (rawJson) => {
        try {
          const parsed = JSON.parse(rawJson) as Record<string, unknown>;
          if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
            return { ok: false, error: "Backup file must contain a JSON object." };
          }
          const candidateSection =
            parsed.uiCustomization ??
            parsed.ui_customization ??
            (parsed.webui && typeof parsed.webui === "object"
              ? (parsed.webui as Record<string, unknown>).uiCustomization ??
                (parsed.webui as Record<string, unknown>).ui_customization
              : undefined) ??
            (parsed.state && typeof parsed.state === "object" ? parsed.state : parsed);

          const clean = sanitizeCustomizationSnapshot(candidateSection);
          const explicitThemeId =
            typeof parsed.activeThemeId === "string"
              ? parsed.activeThemeId
              : typeof parsed.active_theme_id === "string"
                ? parsed.active_theme_id
                : undefined;
          if (
            explicitThemeId &&
            PREBUILT_THEMES.some((t) => t.id === explicitThemeId)
          ) {
            clean.activeThemeId = explicitThemeId;
          }

          if (Object.keys(clean).length === 0) {
            return { ok: false, error: "No valid OpenZ UI customization fields found in backup." };
          }

          set((prev) => ({
            ...prev,
            ...clean,
            lastSavedAt: new Date().toISOString(),
          }));
          notifyCustomizationSync(extractSnapshot(get()));

          const rawMode = parsed.themeMode ?? parsed.theme;
          const themeMode: Theme | undefined =
            rawMode === "light" || rawMode === "dark" || rawMode === "system"
              ? rawMode
              : undefined;

          return { ok: true, themeMode };
        } catch {
          return { ok: false, error: "Failed to parse backup JSON file." };
        }
      },
    }),
    {
      name: UI_CUSTOMIZATION_STORAGE_KEY,
      version: 2,
      storage: createJSONStorage(() => safePersistStorage),
      partialize: (state) => extractSnapshot(state),
      migrate: (persistedState, version) => {
        const clean = sanitizeCustomizationSnapshot(persistedState);
        if (version < 2 && clean.toolResultStyle === "assistant-collapsed") {
          clean.toolResultStyle = "beui-activity";
        }
        return { ...DEFAULT_CUSTOMIZATION, ...clean };
      },
      merge: (persistedState, currentState) => {
        const clean = sanitizeCustomizationSnapshot(persistedState);
        return { ...currentState, ...clean };
      },
    },
  ),
);
