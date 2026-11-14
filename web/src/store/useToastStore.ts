import { create } from "zustand"
import {
  mapNoticeKindToToastCategory,
  type BeuiToastCategory,
  type BeuiToastItem,
  type BeuiToastStatus,
} from "../components/beui"
import type { ActivityNotice, SecurityPromptInfo, WorkspaceNoticeScope } from "../types"
import {
  useUiCustomizationStore,
  type UiCustomizationSnapshot,
} from "./useUiCustomizationStore"

const MAX_ACTIVE_TOASTS = 12
const MAX_HISTORY_ITEMS = 40
const DEDUPE_WINDOW_MS = 2800

let toastSeq = 0
function nextToastId(): string {
  toastSeq += 1
  return `toast-${Date.now()}-${toastSeq}`
}

export type ToastCategoryToggleKey =
  | "toastNotifyWorkflow"
  | "toastNotifySkill"
  | "toastNotifySelfImprovement"
  | "toastNotifyMemory"
  | "toastNotifyResearch"
  | "toastNotifyApproval"

export interface ToastCategoryToggleDescriptor {
  key: ToastCategoryToggleKey
  category: BeuiToastCategory
  title: string
  description: string
}

export const TOAST_CATEGORY_TOGGLE_OPTIONS: ReadonlyArray<ToastCategoryToggleDescriptor> =
  [
    {
      key: "toastNotifyWorkflow",
      category: "workflow",
      title: "Workflow Matched & Orchestration",
      description:
        "Reusable workflow card matches, saved procedures, and multi-agent orchestration milestones.",
    },
    {
      key: "toastNotifySkill",
      category: "skill",
      title: "Skill Used & Curated",
      description:
        "Procedural skills matched and injected into a turn or curated in ~/.openz/skills/.",
    },
    {
      key: "toastNotifySelfImprovement",
      category: "self_improvement",
      title: "Self-Improvement Curator",
      description:
        "Background curator syntheses when OpenZ refines durable session learnings or skills.",
    },
    {
      key: "toastNotifyMemory",
      category: "memory",
      title: "Memory & Source Captures",
      description:
        "Automatic research brief captures, cognitive memory stores, and bookmarked sources.",
    },
    {
      key: "toastNotifyResearch",
      category: "research",
      title: "Research Brief & Source Hits",
      description:
        "Saved research briefs and ranked knowledge sources matched before web search.",
    },
    {
      key: "toastNotifyApproval",
      category: "security",
      title: "Security Guard & Runtime Alerts",
      description:
        "Interactive tool permission prompts and actionable gateway runtime alerts.",
    },
  ]

const CATEGORY_TOGGLE_KEY_BY_CATEGORY: Record<
  BeuiToastCategory,
  ToastCategoryToggleKey
> = {
  workflow: "toastNotifyWorkflow",
  orchestration: "toastNotifyWorkflow",
  skill: "toastNotifySkill",
  self_improvement: "toastNotifySelfImprovement",
  memory: "toastNotifyMemory",
  source: "toastNotifyMemory",
  research: "toastNotifyResearch",
  security: "toastNotifyApproval",
  error: "toastNotifyApproval",
  system: "toastNotifyWorkflow",
}

export type ToastFilterSnapshot = Pick<
  UiCustomizationSnapshot,
  | "toastEnabled"
  | "toastNotifyWorkflow"
  | "toastNotifySkill"
  | "toastNotifySelfImprovement"
  | "toastNotifyMemory"
  | "toastNotifyResearch"
  | "toastNotifyApproval"
>

export function shouldEmitCategoryToast(
  category: BeuiToastCategory,
  prefs: ToastFilterSnapshot
): boolean {
  if (!prefs.toastEnabled) return false
  const toggleKey = CATEGORY_TOGGLE_KEY_BY_CATEGORY[category]
  return Boolean(prefs[toggleKey])
}

export interface PushToastInput {
  id?: string
  dedupeKey?: string
  category: BeuiToastCategory
  status?: BeuiToastStatus
  title: string
  detail?: string
  badge?: string
  durationMs?: number
  actionLabel?: string
  onAction?: () => void
  silent?: boolean
}

export interface ToastStoreState {
  toasts: BeuiToastItem[]
  history: BeuiToastItem[]
  unreadCount: number
  isCenterOpen: boolean
  pushToast: (input: PushToastInput) => BeuiToastItem
  dismissToast: (id: string) => void
  clearActiveToasts: () => void
  clearHistory: () => void
  setIsCenterOpen: (open: boolean) => void
  toggleCenter: () => void
  resetToastStore: () => void
}

function upsertWithDedupe(
  list: ReadonlyArray<BeuiToastItem>,
  incoming: BeuiToastItem,
  maxItems: number
): BeuiToastItem[] {
  const dedupeKey = incoming.dedupeKey
  const matchIdx = dedupeKey
    ? list.findIndex(
        (existing) =>
          existing.dedupeKey === dedupeKey &&
          incoming.createdAt - existing.createdAt <= DEDUPE_WINDOW_MS
      )
    : -1

  if (matchIdx >= 0) {
    const target = list[matchIdx]
    const merged: BeuiToastItem = {
      ...target,
      ...incoming,
      id: target.id,
      count: (target.count ?? 1) + 1,
    }
    const remaining = list.filter((_, idx) => idx !== matchIdx)
    return [merged, ...remaining].slice(0, maxItems)
  }

  return [incoming, ...list].slice(0, maxItems)
}

export const useToastStore = create<ToastStoreState>((set, get) => ({
  toasts: [],
  history: [],
  unreadCount: 0,
  isCenterOpen: false,

  pushToast: (input) => {
    const now = Date.now()
    const dedupeKey =
      input.dedupeKey ?? `${input.category}:${input.title}:${input.detail ?? ""}`
    const item: BeuiToastItem = {
      id: input.id ?? nextToastId(),
      dedupeKey,
      category: input.category,
      status: input.status ?? "info",
      title: input.title,
      detail: input.detail,
      badge: input.badge,
      durationMs: input.durationMs,
      createdAt: now,
      count: 1,
      actionLabel: input.actionLabel,
      onAction: input.onAction,
    }

    const state = get()
    const nextHistory = upsertWithDedupe(state.history, item, MAX_HISTORY_ITEMS)
    const nextToasts = input.silent
      ? state.toasts
      : upsertWithDedupe(state.toasts, item, MAX_ACTIVE_TOASTS)
    const nextUnread = state.isCenterOpen ? 0 : state.unreadCount + 1

    set({
      toasts: nextToasts,
      history: nextHistory,
      unreadCount: nextUnread,
    })

    return nextToasts[0] ?? item
  },

  dismissToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }))
  },

  clearActiveToasts: () => set({ toasts: [] }),

  clearHistory: () => set({ history: [], unreadCount: 0 }),

  setIsCenterOpen: (open) =>
    set((state) => ({
      isCenterOpen: open,
      unreadCount: open ? 0 : state.unreadCount,
    })),

  toggleCenter: () => {
    const next = !get().isCenterOpen
    set((state) => ({
      isCenterOpen: next,
      unreadCount: next ? 0 : state.unreadCount,
    }))
  },

  resetToastStore: () =>
    set({
      toasts: [],
      history: [],
      unreadCount: 0,
      isCenterOpen: false,
    }),
}))

export interface TelemetryNavigationHandlers {
  openSkills: () => void
  openKnowledge: () => void
  openMemory: () => void
  openActivity: () => void
  openAgents: () => void
  openLogs: () => void
  focusChat: () => void
}

type NavHandlerKey = keyof TelemetryNavigationHandlers

const NOTICE_JUMP_BY_KIND: Record<
  ActivityNotice["kind"],
  { actionLabel: string; navKey: NavHandlerKey }
> = {
  workflow: { actionLabel: "Inspect Activity", navKey: "openActivity" },
  skill: { actionLabel: "Open Skills", navKey: "openSkills" },
  self_improvement: { actionLabel: "Open Memory", navKey: "openMemory" },
  memory: { actionLabel: "Open Memory", navKey: "openMemory" },
  research: { actionLabel: "Open Knowledge", navKey: "openKnowledge" },
  source: { actionLabel: "Open Knowledge", navKey: "openKnowledge" },
  system: { actionLabel: "Inspect Activity", navKey: "openActivity" },
}

export function resolveNoticeJumpAction(
  kind: ActivityNotice["kind"],
  title: string,
  nav: TelemetryNavigationHandlers
): { actionLabel: string; onAction: () => void } {
  const isSkillCuratorUpdate =
    kind === "self_improvement" && title.toLowerCase().includes("skill")
  const effectiveKind: ActivityNotice["kind"] = isSkillCuratorUpdate
    ? "skill"
    : kind
  const descriptor = NOTICE_JUMP_BY_KIND[effectiveKind]
  return {
    actionLabel: descriptor.actionLabel,
    onAction: nav[descriptor.navKey],
  }
}

/**
 * Dispatches an incoming `ActivityNotice` (`workflow`, `skill`, `self_improvement`, `memory`,
 * `research`, `source`) to `@beui/animated-toast-stack` while respecting user customization toggles.
 */
export function dispatchActivityNoticeToast(
  notice: ActivityNotice,
  rawKind: string,
  nav: TelemetryNavigationHandlers
): void {
  // Transient streaming tool progress updates belong in the live Activity Orb/Drawer, not popup toasts.
  if (rawKind === "progress") return

  // Subtly route self-improvement "Skill updated" notices so both Skill and Self-Improvement toggles apply cleanly.
  const category = mapNoticeKindToToastCategory(notice.kind)
  const prefs = useUiCustomizationStore.getState()
  const showPopup = shouldEmitCategoryToast(category, prefs)
  const jump = resolveNoticeJumpAction(notice.kind, notice.title, nav)

  useToastStore.getState().pushToast({
    dedupeKey: `${category}:${notice.title}:${notice.detail ?? ""}`,
    category,
    status: category === "self_improvement" ? "success" : "info",
    title: notice.title,
    detail: notice.detail,
    actionLabel: jump.actionLabel,
    onAction: jump.onAction,
    silent: !showPopup,
  })
}

export function dispatchOrchestrationLifecycleToast(
  eventPayload: Record<string, unknown>,
  nav: TelemetryNavigationHandlers
): void {
  const type = typeof eventPayload.type === "string" ? eventPayload.type : ""
  const mode = typeof eventPayload.mode === "string" ? eventPayload.mode : "workflow"
  const goal = typeof eventPayload.goal === "string" ? eventPayload.goal : undefined
  const summary =
    typeof eventPayload.summary === "string" ? eventPayload.summary : undefined

  const lifecycleConfig: Record<
    string,
    { title: string; detail?: string; status: BeuiToastStatus } | undefined
  > = {
    workflow_started: {
      title: `Workflow started (${mode})`,
      detail: goal,
      status: "running",
    },
    workflow_completed: {
      title: "Workflow completed",
      detail: summary ?? goal,
      status: "success",
    },
    workflow_failed: {
      title: "Workflow failed",
      detail:
        (typeof eventPayload.error === "string" ? eventPayload.error : undefined) ??
        summary ??
        goal,
      status: "error",
    },
  }

  const descriptor = lifecycleConfig[type]
  if (!descriptor) return

  const prefs = useUiCustomizationStore.getState()
  const showPopup = shouldEmitCategoryToast("orchestration", prefs)

  useToastStore.getState().pushToast({
    category: "orchestration",
    status: descriptor.status,
    title: descriptor.title,
    detail: descriptor.detail,
    actionLabel: "Inspect Run",
    onAction: nav.openActivity,
    silent: !showPopup,
  })
}

export function dispatchSecurityRequestToast(
  prompt: SecurityPromptInfo,
  nav: TelemetryNavigationHandlers
): void {
  const prefs = useUiCustomizationStore.getState()
  const showPopup = shouldEmitCategoryToast("security", prefs)

  useToastStore.getState().pushToast({
    dedupeKey: `security:${prompt.id}`,
    category: "security",
    status: "warning",
    title: `Approval needed · ${prompt.toolName}`,
    detail: prompt.description,
    durationMs: prefs.toastDuration === "fast" ? 6500 : 9000,
    actionLabel: "Review Guard",
    onAction: nav.focusChat,
    silent: !showPopup,
  })
}

const SCOPE_CATEGORY_MAP: Record<WorkspaceNoticeScope, BeuiToastCategory> = {
  skills: "skill",
  agents: "orchestration",
  knowledge: "source",
  inventory: "system",
  settings: "system",
  global: "system",
}

const SCOPE_TITLE_MAP: Record<WorkspaceNoticeScope, string> = {
  skills: "Skills Updated",
  agents: "Subagent Profiles Updated",
  knowledge: "Knowledge Sources Updated",
  inventory: "Inventory Updated",
  settings: "Settings Updated",
  global: "Workspace Notice",
}

export function dispatchWorkspaceEventToast(
  scope: WorkspaceNoticeScope,
  type: "success" | "error" | "info",
  message: string,
  nav: TelemetryNavigationHandlers
): void {
  const category: BeuiToastCategory =
    type === "error" ? "error" : SCOPE_CATEGORY_MAP[scope]
  const prefs = useUiCustomizationStore.getState()
  const showPopup = shouldEmitCategoryToast(category, prefs)

  const actionByScope: Record<
    WorkspaceNoticeScope,
    { label: string; fn: () => void } | undefined
  > = {
    skills: { label: "Open Skills", fn: nav.openSkills },
    agents: { label: "Open Agents", fn: nav.openAgents },
    knowledge: { label: "Open Knowledge", fn: nav.openKnowledge },
    inventory: undefined,
    settings: undefined,
    global: type === "error" ? { label: "Open Logs", fn: nav.openLogs } : undefined,
  }

  const jump = actionByScope[scope]
  const resolvedTitle =
    type === "error" && scope === "global"
      ? "Gateway Alert"
      : SCOPE_TITLE_MAP[scope]

  useToastStore.getState().pushToast({
    category,
    status: type,
    title: resolvedTitle,
    detail: message,
    actionLabel: jump?.label,
    onAction: jump?.fn,
    silent: !showPopup,
  })
}
