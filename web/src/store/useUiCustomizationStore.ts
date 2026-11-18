import { create } from "zustand";
import { createJSONStorage, persist, type StateStorage } from "zustand/middleware";
import {
  normalizeApprovalStyle,
  normalizeToolResultStyle,
  type AgentHandoffStyleChoice,
  type ApprovalStyleChoice,
  type ArtifactCardStyleChoice,
  type BackgroundInboxStyleChoice,
  type ErrorStateStyleChoice,
  type FileTreeStyleChoice,
  type MathBlockStyleChoice,
  type RecommendationStyleChoice,
  type ResearchReportStyleChoice,
  type RetrievalChunksStyleChoice,
  type SpecSheetStyleChoice,
  type TimelineStyleChoice,
  type ToolResultStyleChoice,
  type WebSearchStyleChoice,
} from "../components/assistant-ui/elements";
import type { GlobalBadgeStyle } from "../components/ui/badge";
import type {
  ComparisonStyleChoice,
  DeleteButtonStyleChoice,
  InlineCitationStyleChoice,
} from "../components/registries/rareAndCommunity";
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
  type ImageGenVariant,
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
export type ImageGenStyleChoice = ImageGenVariant;
export type ActivityStyleChoice = BeuiActivityPlacement;
export type AlertStyleChoice = BeuiAlertStyleVariant;
export type ThinkingLoaderChoice = BeuiLoaderVariant;

export type GatewaySettingsSectionId = "agent" | "caveman" | "providers" | "channels";

export type CustomizeUiSectionId =
  | "theme"
  | "badge-style"
  | "avatars"
  | "user-avatars"
  | "chat-width"
  | "delete-button"
  | "prompt-input"
  | "send-button"
  | "model-selector"
  | "prompt-suggestions"
  | "recommendation-card"
  | "context-usage"
  | "message-scroller"
  | "message-card"
  | "message-actions"
  | "streaming-response"
  | "thinking-loader"
  | "reasoning-text"
  | "agent-activity"
  | "activity-orb"
  | "activity-placement"
  | "plan-checklist"
  | "subagent-card"
  | "agent-handoff"
  | "timeline"
  | "animated-toast-stack"
  | "toast-placement"
  | "telemetry-badges"
  | "background-inbox"
  | "alert-collapsible"
  | "error-state"
  | "collapsible"
  | "code-block"
  | "file-diff"
  | "file-tree"
  | "artifact-card"
  | "terminal-output"
  | "tool-result"
  | "tool-approval"
  | "web-search"
  | "retrieval-chunks"
  | "research-report"
  | "inline-citations"
  | "citations"
  | "image-generation"
  | "math-block"
  | "spec-sheet"
  | "comparison-card"
  | "data-table"
  | "json-viewer";

export interface UiCustomizationSnapshot {
  activeThemeId: string;
  badgeStyle: GlobalBadgeStyle;
  deleteButtonStyle: DeleteButtonStyleChoice;
  chatWidth: ChatWidthChoice;
  promptInputStyle: BeuiPromptInputVariant;
  sendButtonStyle: BeuiSendButtonVariant;
  modelSelectorStyle: BeuiModelSelectorVariant;
  promptSuggestionsStyle: BeuiPromptSuggestionsVariant;
  recommendationStyle: RecommendationStyleChoice;
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
  agentHandoffStyle: AgentHandoffStyleChoice;
  timelineStyle: TimelineStyleChoice;
  toastEnabled: boolean;
  toastVariant: BeuiToastVariant;
  toastPosition: BeuiToastPosition;
  toastDuration: BeuiToastDurationPreset;
  toastMaxVisible: BeuiToastMaxVisible;
  inlineNoticeStyle: BeuiInlineNoticeStyle;
  backgroundInboxStyle: BackgroundInboxStyleChoice;
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
  fileTreeStyle: FileTreeStyleChoice;
  artifactCardStyle: ArtifactCardStyleChoice;
  terminalStyle: BeuiTerminalVariant;
  toolResultStyle: ToolResultStyleChoice;
  approvalStyle: ApprovalStyleChoice;
  webSearchStyle: WebSearchStyleChoice;
  retrievalChunksStyle: RetrievalChunksStyleChoice;
  researchReportStyle: ResearchReportStyleChoice;
  inlineCitationStyle: InlineCitationStyleChoice;
  imageGenStyle: ImageGenStyleChoice;
  citationsStyle: BeuiCitationsVariant;
  mathBlockStyle: MathBlockStyleChoice;
  specSheetStyle: SpecSheetStyleChoice;
  comparisonStyle: ComparisonStyleChoice;
  dataTableStyle: BeuiDataTableVariant;
  jsonViewerStyle: BeuiJsonViewerVariant;
  alertStyle: AlertStyleChoice;
  errorStateStyle: ErrorStateStyleChoice;
  collapsibleStyle: BeuiCollapsibleVariant;
}

export const DEFAULT_CUSTOMIZATION: UiCustomizationSnapshot = {
  activeThemeId: "obsidian-ember",
  badgeStyle: "minimal",
  deleteButtonStyle: "rare-confirm",
  chatWidth: "standard",
  promptInputStyle: "elevated",
  sendButtonStyle: "icon-round",
  modelSelectorStyle: "command",
  promptSuggestionsStyle: "bento",
  recommendationStyle: "assistant-rec",
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
  agentHandoffStyle: "assistant-handoff",
  timelineStyle: "assistant-timeline",
  toastEnabled: true,
  toastVariant: "beui-stack",
  toastPosition: "bottom-end",
  toastDuration: "standard",
  toastMaxVisible: 4,
  inlineNoticeStyle: "minimal-whisper",
  backgroundInboxStyle: "assistant-inbox",
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
  fileTreeStyle: "assistant-tree",
  artifactCardStyle: "assistant-artifact",
  terminalStyle: "console",
  toolResultStyle: "beui-activity",
  approvalStyle: "assistant-default",
  webSearchStyle: "assistant-web-search",
  retrievalChunksStyle: "assistant-chunks",
  researchReportStyle: "assistant-outline",
  inlineCitationStyle: "aicss-pill",
  imageGenStyle: "beui-frame",
  citationsStyle: "pills",
  mathBlockStyle: "assistant-math",
  specSheetStyle: "assistant-spec",
  comparisonStyle: "aicss-table",
  dataTableStyle: "interactive",
  jsonViewerStyle: "tree",
  alertStyle: "beui-alert",
  errorStateStyle: "assistant-quiet",
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
  const out = {} as Record<keyof UiCustomizationSnapshot, unknown>;
  for (const key of SNAPSHOT_KEYS) {
    out[key] = state[key];
  }
  return out as UiCustomizationSnapshot;
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
      version: 3,
      storage: createJSONStorage(() => safePersistStorage),
      partialize: (state) => extractSnapshot(state),
      migrate: (persistedState, version) => {
        const clean = sanitizeCustomizationSnapshot(persistedState);
        if (version < 2 && clean.toolResultStyle === "assistant-collapsed") {
          clean.toolResultStyle = "beui-activity";
        }
        if (version < 3 && clean.inlineNoticeStyle === "pills") {
          clean.inlineNoticeStyle = "minimal-whisper";
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
