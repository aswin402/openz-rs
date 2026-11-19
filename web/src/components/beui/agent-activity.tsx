import * as React from "react"
import {
  Activity,
  AlertTriangle,
  BookOpen,
  Brain,
  CheckCircle2,
  ChevronDown,
  FileCode2,
  GitBranch,
  Loader2,
  MessageSquare,
  Search,
  ShieldAlert,
  Sparkles,
  Terminal,
} from "lucide-react"

import {
  AgentActivity as OfficialBeuiAgentActivity,
  type AgentActivityContentType as OfficialContentType,
  type AgentActivityItem as BeuiOfficialActivityItem,
  type AgentSearchResult,
  type AgentStepStatus,
  type AgentTraceKind,
} from "@/components/agents/agent-activity"
import { ThinkingShimmer } from "@/components/agents/loading-states/thinking-shimmer"
import { FOX_SILHOUETTE_PATH } from "@/components/OpenZFoxIcon"
import {
  AicssOrb,
  ThinkingOrbsCanvas,
  type OrbPhase,
} from "@/components/registries/thinkingOrbs"
import { cn } from "@/lib/utils"
import { ThinkingLoaderBadge, type BeuiLoaderVariant } from "./loader"

export { OfficialBeuiAgentActivity }
export type { AgentSearchResult, BeuiOfficialActivityItem }

export type BeuiActivityPlacement =
  | "beui-timeline"
  | "always-inline"
  | "streaming-only"
  | "panel-only"

export type BeuiActivityVariant =
  | "orb-card"
  | "rail-timeline"
  | "compact-pills"
  | "trace-log"
  | "assistant-run-activity"
  | "assistant-tool-timeline"

export type BeuiActivityOrbStyle =
  | "dotted-orb"
  | "fox-pulse"
  | "minimal-spinner"
  | "thinking-orbs-canvas"
  | "aicss-orb-morph"
  | "aicss-orb-helix"
  | "aicss-orb-ring"
  | "aicss-orb-lens"
  | "rare-ai-status"
  | "minimal-dot"

export type BeuiActivityDensity = "comfortable" | "compact"

export type BeuiActivityMaxHeight =
  | "compact"
  | "standard"
  | "tall"
  | "unlimited"

export type BeuiActivityContentType =
  | "mixed"
  | "step"
  | "tool"
  | "search"
  | "reasoning"
  | "trace"

export type BeuiActivityPhase =
  | "idle"
  | "reasoning"
  | "search"
  | "tool"
  | "security"
  | "workflow"
  | "complete"
  | "error"

export type AgentActivityStepKind =
  | "tool"
  | "security"
  | "reasoning"
  | "notice"
  | "message"
  | "search"
  | "trace"

export type AgentActivityStepStatus =
  | "running"
  | "success"
  | "error"
  | "pending"
  | "info"

export type AgentActivityNoticeKind =
  | "workflow"
  | "skill"
  | "memory"
  | "research"
  | "self_improvement"
  | "source"
  | "system"

export interface AgentActivityOptionDescriptor<T extends string> {
  id: T
  title: string
  description: string
  shortLabel: string
  isDefault?: boolean
}

export const ACTIVITY_MAX_HEIGHT_PX: Record<BeuiActivityMaxHeight, number> = {
  compact: 176,
  standard: 264,
  tall: 384,
  unlimited: 0,
}

export const ACTIVITY_PLACEMENT_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityPlacement>
> = [
  {
    id: "beui-timeline",
    title: "Smart Adaptive Stream",
    shortLabel: "Smart",
    description:
      "Renders @beui/agent-activity inline on assistant turns and syncs live telemetry to the right-hand Activity Drawer.",
    isDefault: true,
  },
  {
    id: "always-inline",
    title: "Always Inline Stream",
    shortLabel: "Inline",
    description:
      "Embeds @beui/agent-activity inside every assistant turn that records reasoning, searches, tools, or telemetry.",
  },
  {
    id: "streaming-only",
    title: "Live Streaming Only",
    shortLabel: "Live Only",
    description:
      "Displays @beui/agent-activity while actively working, then tucks completed history into the Activity Drawer.",
  },
  {
    id: "panel-only",
    title: "Activity Drawer Only",
    shortLabel: "Drawer",
    description:
      "Keeps chat bubbles minimal and routes all turn telemetry to the dedicated Activity Drawer and workspace feeds.",
  },
]

export const ACTIVITY_VARIANT_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityVariant>
> = [
  {
    id: "orb-card",
    title: "BEUI Adaptive Mixed Stream",
    shortLabel: "Adaptive",
    description:
      "Official @beui/agent-activity mixed stream combining reasoning paragraphs, web search results, Read/Edit/Run tool pills, and steps.",
    isDefault: true,
  },
  {
    id: "rail-timeline",
    title: "BEUI Step Checklist Stream",
    shortLabel: "Steps",
    description:
      "Official @beui/agent-activity step rows with Check, pulsing active dot, and right-aligned duration/actor metadata.",
  },
  {
    id: "compact-pills",
    title: "BEUI Compact Tool Stream",
    shortLabel: "Compact",
    description:
      "Tight @beui/agent-activity tool & search rows without verbose secondary descriptions.",
  },
  {
    id: "trace-log",
    title: "BEUI Structured Trace",
    shortLabel: "Trace",
    description:
      "Official @beui/agent-activity 3-column execution trace with kind icons, labels, and monospace detail pills.",
  },
  {
    id: "assistant-run-activity",
    title: "Assistant-UI Run Activity",
    shortLabel: "Run Activity",
    description:
      "assistant-ui/elements/run-activity — Minimal verb + target working summary with quiet collapsible receipt.",
  },
  {
    id: "assistant-tool-timeline",
    title: "Assistant-UI Tool Timeline",
    shortLabel: "Tool Timeline",
    description:
      "assistant-ui/elements/tool-timeline — Connected chronological tool steps with verb badges and millisecond timings.",
  },
]

export const ACTIVITY_ORB_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityOrbStyle>
> = [
  {
    id: "dotted-orb",
    title: "Thought Orb + Shimmer",
    shortLabel: "9-Dot Orb",
    description:
      "Pairs a reactive 3×3 Thought-Orb dot wave beside @beui/agent-activity's ThinkingShimmer.",
    isDefault: true,
  },
  {
    id: "thinking-orbs-canvas",
    title: "Thinking-Orbs 2D Canvas",
    shortLabel: "Canvas Orb",
    description:
      "thinking-orbs — Multi-harmonic 2D HTML5 Canvas wave orb reacting dynamically to reasoning, search, and tool phases.",
  },
  {
    id: "aicss-orb-morph",
    title: "AICSS Orbs Morph",
    shortLabel: "Morph Orb",
    description:
      "aicss.dev/r/orbs — Fluid organic contour morphing orb for active thinking and synthesis.",
  },
  {
    id: "aicss-orb-helix",
    title: "AICSS Orbs Helix",
    shortLabel: "Helix Orb",
    description:
      "aicss.dev/r/orbs — Counter-oscillating DNA wave particles.",
  },
  {
    id: "aicss-orb-ring",
    title: "AICSS Orbs Ring",
    shortLabel: "Ring Orb",
    description:
      "aicss.dev/r/orbs — Orbital arc ring around a breathing core nucleus.",
  },
  {
    id: "aicss-orb-lens",
    title: "AICSS Orbs Lens",
    shortLabel: "Lens Orb",
    description:
      "aicss.dev/r/orbs — Translucent focal lens sweep inside a hairline glass circle.",
  },
  {
    id: "fox-pulse",
    title: "OpenZ Fox + Shimmer",
    shortLabel: "Fox Pulse",
    description:
      "Pairs a subtle OpenZ Fox emblem pulse beside @beui/agent-activity's ThinkingShimmer.",
  },
  {
    id: "rare-ai-status",
    title: "Rare-UI Status Pulse",
    shortLabel: "Rare Pulse",
    description:
      "swamimalode07/rare-ui/ai-status — Concentric live radar ping around a crisp primary dot.",
  },
  {
    id: "minimal-dot",
    title: "Minimal Hairline Dot",
    shortLabel: "Minimal Dot",
    description:
      "Whisper-quiet static/breathing 6px status dot for distraction-free layouts.",
  },
  {
    id: "minimal-spinner",
    title: "Pure ThinkingShimmer",
    shortLabel: "Shimmer",
    description:
      "Unadorned @beui/agent-activity ThinkingShimmer text sweep without a leading icon.",
  },
]

export const ACTIVITY_DENSITY_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityDensity>
> = [
  {
    id: "comfortable",
    title: "Comfortable Spacing",
    shortLabel: "Comfort",
    description: "Standard @beui/agent-activity row rhythm and breathing room.",
    isDefault: true,
  },
  {
    id: "compact",
    title: "Compact High-Density",
    shortLabel: "Compact",
    description: "Tighter vertical spacing to fit more steps in the gliding viewport.",
  },
]

export const ACTIVITY_MAX_HEIGHT_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityMaxHeight>
> = [
  {
    id: "compact",
    title: "Compact Gliding Viewport (176px)",
    shortLabel: "176px",
    description: "Begins smooth vertical stream gliding after ~4 rows.",
  },
  {
    id: "standard",
    title: "Balanced Gliding Viewport (264px)",
    shortLabel: "264px",
    description: "Default @beui/agent-activity gliding window (~6 rows).",
    isDefault: true,
  },
  {
    id: "tall",
    title: "Tall Gliding Viewport (384px)",
    shortLabel: "384px",
    description: "Expanded inspection window for deep multi-agent and log streams.",
  },
  {
    id: "unlimited",
    title: "Unclamped Full Stream",
    shortLabel: "Full",
    description: "Shows every activity row without viewport height clamping.",
  },
]

export interface AgentActivityStep {
  id: string
  kind: AgentActivityStepKind
  contentType?: Exclude<BeuiActivityContentType, "mixed">
  noticeKind?: AgentActivityNoticeKind
  title: string
  detail?: string
  payload?: string
  meta?: string
  actor?: string
  status: AgentActivityStepStatus
  durationLabel?: string
  durationMs?: number
  timestamp?: number
  /** Native @beui/agent-activity tool action ("read" | "edit" | "write" | "run" | ...) */
  toolAction?: string
  /** Native @beui/agent-activity monospace target pill text */
  toolTarget?: string
  /** Diff additions count for edit/write tools */
  additions?: number
  /** Diff deletions count for edit tools */
  deletions?: number
  /** Native @beui/agent-activity search query */
  searchQuery?: string
  /** Parsed search result links/matches */
  searchResults?: AgentSearchResult[]
  /** Extra search hits count beyond `searchResults` */
  moreCount?: number
}

export interface AgentActivityProps {
  steps: AgentActivityStep[]
  items?: BeuiOfficialActivityItem[]
  isActive?: boolean
  mode?: "timeline" | "inline-collapsible"
  variant?: BeuiActivityVariant
  orbStyle?: BeuiActivityOrbStyle
  loaderVariant?: BeuiLoaderVariant | null
  density?: BeuiActivityDensity
  maxHeight?: BeuiActivityMaxHeight | number
  contentType?: BeuiActivityContentType
  autoCollapse?: boolean
  showDetails?: boolean
  defaultOpen?: boolean
  open?: boolean
  onOpenChange?: (open: boolean) => void
  title?: string
  subtitle?: string
  startedAt?: number
  emptyLabel?: string
  headerActions?: React.ReactNode
  onStepSelect?: (step: AgentActivityStep) => void
  className?: string
}

type IconComponent = React.ComponentType<{ className?: string }>

const NOTICE_KIND_ICONS: Record<AgentActivityNoticeKind, IconComponent> = {
  workflow: GitBranch,
  skill: BookOpen,
  memory: Brain,
  research: Search,
  self_improvement: Sparkles,
  source: Search,
  system: Activity,
}

const STEP_KIND_ICONS: Record<AgentActivityStepKind, IconComponent> = {
  tool: Terminal,
  security: ShieldAlert,
  reasoning: Brain,
  notice: Activity,
  message: MessageSquare,
  search: Search,
  trace: FileCode2,
}

const PHASE_COLOR_CLASSES: Record<BeuiActivityPhase, string> = {
  idle: "text-muted-foreground",
  reasoning: "text-primary",
  search: "text-info",
  tool: "text-primary",
  security: "text-warning",
  workflow: "text-chart-4",
  complete: "text-success",
  error: "text-destructive",
}

const GRID_COORDS: ReadonlyArray<readonly [number, number]> = [
  [0, 0],
  [0, 1],
  [0, 2],
  [1, 0],
  [1, 1],
  [1, 2],
  [2, 0],
  [2, 1],
  [2, 2],
]

const SPIRAL_ORDER: ReadonlyArray<number> = [0, 1, 2, 7, 8, 3, 6, 5, 4]

const PHASE_DELAY_FORMULAS: Record<
  BeuiActivityPhase,
  (index: number, row: number, col: number) => number
> = {
  idle: () => 0,
  complete: (index) => index * 40,
  reasoning: (index) => SPIRAL_ORDER[index] * 110,
  search: (_index, row, col) => col * 180 + row * 55,
  tool: (_index, row, col) => (row + col) * 140,
  security: (_index, row, col) => (row === 1 && col === 1 ? 0 : 260),
  workflow: (_index, row, col) => row * 170 + col * 50,
  error: (_index, row, col) => ((row + col) % 2) * 220,
}

export function resolveStepContentType(
  step: AgentActivityStep
): Exclude<BeuiActivityContentType, "mixed"> {
  if (step.contentType) return step.contentType
  const byKind: Record<
    AgentActivityStepKind,
    Exclude<BeuiActivityContentType, "mixed">
  > = {
    tool: "tool",
    security: "step",
    reasoning: "reasoning",
    notice:
      step.noticeKind === "research" || step.noticeKind === "source"
        ? "search"
        : "step",
    message: "step",
    search: "search",
    trace: "trace",
  }
  return byKind[step.kind]
}

export function resolveStepIcon(step: AgentActivityStep): IconComponent {
  const resolvedContentType = resolveStepContentType(step)
  if (resolvedContentType === "search") return Search
  if (step.kind === "notice" && step.noticeKind) {
    return NOTICE_KIND_ICONS[step.noticeKind]
  }
  return STEP_KIND_ICONS[step.kind]
}

export function resolveActivityPhase(
  steps: ReadonlyArray<AgentActivityStep>,
  isActive: boolean
): BeuiActivityPhase {
  if (steps.length === 0) return isActive ? "reasoning" : "idle"

  const activeSteps = steps.filter(
    (s) => s.status === "running" || s.status === "pending"
  )
  const hasError = steps.some((s) => s.status === "error")

  if (activeSteps.some((s) => s.kind === "security" || s.status === "pending")) {
    return "security"
  }
  if (activeSteps.some((s) => resolveStepContentType(s) === "search")) {
    return "search"
  }
  if (activeSteps.some((s) => s.kind === "notice" && s.noticeKind === "workflow")) {
    return "workflow"
  }
  if (activeSteps.some((s) => s.kind === "tool")) {
    return "tool"
  }
  if (activeSteps.length > 0 || isActive) {
    return "reasoning"
  }
  if (hasError) {
    return "error"
  }
  return "complete"
}

export function resolveMaxHeightPx(
  maxHeight: BeuiActivityMaxHeight | number | undefined
): number {
  if (typeof maxHeight === "number") return Math.max(0, maxHeight)
  const key: BeuiActivityMaxHeight = maxHeight ?? "standard"
  return ACTIVITY_MAX_HEIGHT_PX[key]
}

const STEP_STATUS_MAP: Record<AgentActivityStepStatus, AgentStepStatus> = {
  running: "active",
  pending: "pending",
  success: "complete",
  error: "complete",
  info: "complete",
}

function mapTraceKind(step: AgentActivityStep): AgentTraceKind {
  if (step.kind === "reasoning") return "thinking"
  if (step.kind === "message") return "message"
  if (step.toolAction === "edit" || step.toolAction === "write") return "write"
  if (step.toolAction === "read") return "read"
  if (step.kind === "tool" || step.kind === "trace") return "run"
  return "tool"
}

function splitReasoningParagraphs(raw: string): string[] {
  const blocks = raw
    .split("\n\n")
    .map((chunk) => chunk.trim())
    .filter(Boolean)
  const hasMultipleBlocks = blocks.length > 1
  const lines = raw
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
  return hasMultipleBlocks
    ? blocks.slice(-12)
    : lines.length <= 2
      ? [lines.join(" ")].filter(Boolean)
      : lines.slice(-12)
}

const OFFICIAL_CONTENT_TYPE_MAP: Record<
  BeuiActivityContentType,
  OfficialContentType
> = {
  mixed: "mixed",
  step: "step",
  tool: "tool",
  search: "search",
  reasoning: "text",
  trace: "trace",
}

/**
 * Converts OpenZ `AgentActivityStep[]` into official `@beui/agent-activity`
 * `AgentActivityItem[]` (`text`, `search`, `tool`, `step`, `trace`).
 */
export function toBeuiOfficialActivityItems(
  steps: ReadonlyArray<AgentActivityStep>,
  variant: BeuiActivityVariant = "orb-card",
  showDetails = true
): BeuiOfficialActivityItem[] {
  if (variant === "trace-log" || variant === "assistant-tool-timeline") {
    return steps.map((step) => ({
      id: step.id,
      type: "trace",
      kind: mapTraceKind(step),
      label: step.actor ? `${step.title} · ${step.actor}` : step.title,
      detail: showDetails
        ? step.toolTarget ?? step.detail ?? step.durationLabel ?? step.meta
        : undefined,
    }))
  }

  if (variant === "rail-timeline" || variant === "assistant-run-activity") {
    return steps.map((step) => {
      const targetText = step.toolTarget ?? step.searchQuery ?? step.detail
      return {
        id: step.id,
        type: "step",
        status: STEP_STATUS_MAP[step.status],
        label:
          showDetails && targetText && targetText !== step.title
            ? `${step.title} — ${targetText}`
            : step.title,
        meta: showDetails ? step.durationLabel ?? step.actor ?? step.meta : undefined,
      }
    })
  }

  if (variant === "compact-pills") {
    return steps.map((step) => {
      const contentType = resolveStepContentType(step)
      if (contentType === "search") {
        return {
          id: step.id,
          type: "search",
          query: step.searchQuery ?? step.detail ?? step.title,
        }
      }
      if (step.kind === "tool") {
        return {
          id: step.id,
          type: "tool",
          action: step.toolAction ?? "run",
          target: step.toolTarget ?? step.title,
          additions: step.additions,
          deletions: step.deletions,
        }
      }
      return {
        id: step.id,
        type: "step",
        status: STEP_STATUS_MAP[step.status],
        label: step.detail && step.kind === "reasoning" ? step.detail : step.title,
        meta: step.durationLabel,
      }
    })
  }

  // Default: "orb-card" -> Official @beui/agent-activity adaptive mixed stream
  const items: BeuiOfficialActivityItem[] = []
  for (const step of steps) {
    const contentType = resolveStepContentType(step)

    if (step.kind === "reasoning" || step.kind === "message") {
      const textSource = step.payload ?? step.detail ?? step.title
      const paragraphs = splitReasoningParagraphs(textSource)
      paragraphs.forEach((paragraph, idx) => {
        items.push({
          id: paragraphs.length === 1 ? step.id : `${step.id}-p-${idx}`,
          type: "text",
          content: paragraph,
        })
      })
      continue
    }

    if (contentType === "search") {
      items.push({
        id: step.id,
        type: "search",
        query: step.searchQuery ?? step.detail ?? step.title,
        results: showDetails ? step.searchResults : undefined,
        moreCount: showDetails ? step.moreCount : undefined,
      })
      continue
    }

    if (step.kind === "tool") {
      items.push({
        id: step.id,
        type: "tool",
        action: step.toolAction ?? "run",
        target:
          step.toolTarget ??
          (showDetails && step.detail ? `${step.title} · ${step.detail}` : step.title),
        additions: showDetails ? step.additions : undefined,
        deletions: showDetails ? step.deletions : undefined,
      })
      continue
    }

    if (step.kind === "trace") {
      items.push({
        id: step.id,
        type: "trace",
        kind: mapTraceKind(step),
        label: step.title,
        detail: showDetails ? step.detail : undefined,
      })
      continue
    }

    items.push({
      id: step.id,
      type: "step",
      status: STEP_STATUS_MAP[step.status],
      label:
        showDetails && step.detail && step.detail !== step.title
          ? `${step.title} · ${step.detail}`
          : step.title,
      meta: showDetails ? step.durationLabel ?? step.actor ?? step.meta : undefined,
    })
  }

  return items
}

export interface BeuiActivityOrbProps {
  phase: BeuiActivityPhase
  isActive?: boolean
  styleVariant?: BeuiActivityOrbStyle
  size?: "sm" | "md" | "lg"
  className?: string
}

const ORB_DIMENSIONS: Record<"sm" | "md" | "lg", string> = {
  sm: "size-4",
  md: "size-5",
  lg: "size-7",
}

/**
 * Reactive Thought-Orb indicator paired with `@beui/agent-activity`.
 * Inline `sm` mode is borderless so it sits cleanly inside `@beui/agent-activity`'s `h-7` trigger row;
 * `lg` mode provides the drawer header emblem.
 */
export const BeuiActivityOrb: React.FC<BeuiActivityOrbProps> = ({
  phase,
  isActive = false,
  styleVariant = "dotted-orb",
  size = "md",
  className,
}) => {
  const colorClass = PHASE_COLOR_CLASSES[phase]
  const sizeClass = ORB_DIMENSIONS[size]
  const animate = isActive || phase === "security"
  const framed = size === "lg"
  const pixelSize = size === "lg" ? 22 : size === "md" ? 18 : 15

  const mappedOrbPhase: OrbPhase =
    phase === "search"
      ? "search"
      : phase === "tool" || phase === "workflow"
        ? "tool"
        : phase === "security"
          ? "security"
          : phase === "complete"
            ? "complete"
            : phase === "error"
              ? "error"
              : "reasoning"

  if (styleVariant === "thinking-orbs-canvas") {
    return (
      <ThinkingOrbsCanvas
        phase={mappedOrbPhase}
        isActive={animate}
        size={pixelSize}
        className={cn(colorClass, className)}
      />
    )
  }

  if (
    styleVariant === "aicss-orb-morph" ||
    styleVariant === "aicss-orb-helix" ||
    styleVariant === "aicss-orb-ring" ||
    styleVariant === "aicss-orb-lens"
  ) {
    const subVariant =
      styleVariant === "aicss-orb-helix"
        ? "helix"
        : styleVariant === "aicss-orb-ring"
          ? "ring"
          : styleVariant === "aicss-orb-lens"
            ? "lens"
            : "morph"
    return (
      <AicssOrb
        variant={subVariant}
        isActive={animate}
        size={pixelSize}
        className={cn(colorClass, className)}
      />
    )
  }

  if (styleVariant === "rare-ai-status") {
    return (
      <span
        aria-hidden="true"
        className={cn(
          "relative inline-flex shrink-0 items-center justify-center",
          sizeClass,
          colorClass,
          className
        )}
      >
        {animate && (
          <span className="absolute size-3 animate-ping rounded-full bg-current opacity-45" />
        )}
        <span className="relative size-2 rounded-full bg-current" />
      </span>
    )
  }

  if (styleVariant === "minimal-dot") {
    return (
      <span
        aria-hidden="true"
        className={cn(
          "inline-flex shrink-0 items-center justify-center",
          sizeClass,
          colorClass,
          className
        )}
      >
        <span
          className={cn(
            "size-1.5 rounded-full bg-current",
            animate && "animate-pulse"
          )}
        />
      </span>
    )
  }

  if (styleVariant === "fox-pulse") {
    return (
      <span
        aria-hidden="true"
        className={cn(
          "relative inline-flex shrink-0 items-center justify-center",
          framed && "rounded-lg border border-border/60 bg-card/80",
          sizeClass,
          colorClass,
          className
        )}
      >
        <svg viewBox="0 0 100 100" className={framed ? "size-[76%]" : "size-full"}>
          {animate && (
            <circle
              cx="50"
              cy="50"
              r="45"
              fill="none"
              stroke="currentColor"
              strokeWidth="5"
              className="beui-orb-ring-active"
            />
          )}
          <path d={FOX_SILHOUETTE_PATH} fill="currentColor" />
        </svg>
      </span>
    )
  }

  if (styleVariant === "minimal-spinner") {
    return (
      <span
        aria-hidden="true"
        className={cn(
          "inline-flex shrink-0 items-center justify-center",
          framed && "rounded-lg border border-border/60 bg-muted/40",
          sizeClass,
          colorClass,
          className
        )}
      >
        {animate ? (
          <Loader2 className="size-3.5 animate-spin" />
        ) : phase === "error" ? (
          <AlertTriangle className="size-3.5" />
        ) : phase === "complete" ? (
          <CheckCircle2 className="size-3.5" />
        ) : (
          <Activity className="size-3.5" />
        )}
      </span>
    )
  }

  const delayFn = PHASE_DELAY_FORMULAS[phase]

  return (
    <span
      aria-hidden="true"
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center",
        framed && "rounded-lg border border-border/60 bg-muted/45",
        sizeClass,
        colorClass,
        className
      )}
    >
      <svg viewBox="0 0 24 24" className={framed ? "size-[78%]" : "size-full"}>
        {GRID_COORDS.map(([row, col], idx) => {
          const cx = 5 + col * 7
          const cy = 5 + row * 7
          const delayMs = delayFn(idx, row, col)
          const restingOpacity =
            phase === "complete"
              ? 0.9
              : phase === "idle"
                ? row === 1 && col === 1
                  ? 0.85
                  : 0.32
                : 0.72
          return (
            <circle
              key={`${row}-${col}`}
              cx={cx}
              cy={cy}
              r={1.9}
              fill="currentColor"
              className={cn(animate && "beui-orb-dot-active")}
              style={
                animate
                  ? { animationDelay: `${delayMs}ms` }
                  : { opacity: restingOpacity }
              }
            />
          )
        })}
      </svg>
    </span>
  )
}

function computeEarliestTimestamp(
  steps: ReadonlyArray<AgentActivityStep>,
  startedAt?: number
): number | undefined {
  if (startedAt && startedAt > 0) return startedAt
  const valid = steps.map((s) => s.timestamp ?? 0).filter((ts) => ts > 0)
  return valid.length > 0 ? Math.min(...valid) : undefined
}

function useElapsedSeconds(
  steps: ReadonlyArray<AgentActivityStep>,
  working: boolean,
  startedAt?: number
): number {
  const anchorTs = React.useMemo(
    () => computeEarliestTimestamp(steps, startedAt),
    [steps, startedAt]
  )
  const summedSeconds = React.useMemo(() => {
    const totalMs = steps.reduce((acc, s) => acc + (s.durationMs ?? 0), 0)
    if (totalMs > 0) return Math.max(1, Math.round(totalMs / 1000))
    const reasoningChars = steps
      .filter((s) => s.kind === "reasoning")
      .reduce((acc, s) => acc + (s.payload?.length ?? s.detail?.length ?? 0), 0)
    if (reasoningChars > 0) return Math.max(1, Math.round(reasoningChars / 180))
    return steps.length > 0 ? 1 : 0
  }, [steps])

  const frozenLiveSecRef = React.useRef<number>(0)
  const [liveSec, setLiveSec] = React.useState<number>(0)

  React.useEffect(() => {
    if (!working || !anchorTs) return undefined
    const tick = () => {
      const next = Math.max(1, Math.round((Date.now() - anchorTs) / 1000))
      frozenLiveSecRef.current = next
      setLiveSec(next)
    }
    tick()
    const timer = window.setInterval(tick, 1000)
    return () => window.clearInterval(timer)
  }, [working, anchorTs])

  if (working) {
    return liveSec || summedSeconds || 1
  }
  if (frozenLiveSecRef.current > 0) {
    return frozenLiveSecRef.current
  }
  return summedSeconds
}

/**
 * `@beui/agent-activity` — Powered directly by the official registry component
 * installed via `bunx --bun shadcn add @beui/agent-activity` (`@/components/agents/agent-activity`).
 */
export const AgentActivity: React.FC<AgentActivityProps> = ({
  steps,
  items: explicitItems,
  isActive = false,
  mode = "inline-collapsible",
  variant = "orb-card",
  orbStyle = "dotted-orb",
  loaderVariant = null,
  density = "comfortable",
  maxHeight = "standard",
  contentType = "mixed",
  autoCollapse = true,
  showDetails = true,
  defaultOpen,
  open,
  onOpenChange,
  title,
  startedAt,
  emptyLabel,
  headerActions,
  className,
}) => {
  const visibleSteps = React.useMemo(() => {
    if (contentType === "mixed") return steps
    return steps.filter((s) => resolveStepContentType(s) === contentType)
  }, [steps, contentType])

  const runningCount = visibleSteps.filter(
    (s) => s.status === "running" || s.status === "pending"
  ).length
  const working = Boolean(isActive || runningCount > 0)

  const phase = React.useMemo(
    () => resolveActivityPhase(visibleSteps, working),
    [visibleSteps, working]
  )

  const durationSeconds = useElapsedSeconds(visibleSteps, working, startedAt)

  const officialItems = React.useMemo(
    () =>
      explicitItems ??
      toBeuiOfficialActivityItems(visibleSteps, variant, showDetails),
    [explicitItems, visibleSteps, variant, showDetails]
  )

  if (!working && officialItems.length === 0) {
    if (title && onOpenChange) {
      const isExpanded = Boolean(open)
      return (
        <div className={cn("w-full text-sm", className)}>
          <button
            type="button"
            aria-expanded={isExpanded}
            onClick={() => onOpenChange(!isExpanded)}
            className="group flex h-7 min-w-0 items-center gap-1.5 rounded-md text-left font-medium text-muted-foreground outline-none transition-colors hover:text-foreground"
          >
            <span className="truncate">{title}</span>
            <ChevronDown
              className={cn(
                "size-3.5 shrink-0 text-muted-foreground/70 transition-transform duration-200 group-hover:text-foreground",
                isExpanded && "rotate-180"
              )}
            />
          </button>
        </div>
      )
    }
    if (!emptyLabel) return null
    return (
      <div
        className={cn(
          "flex items-center gap-2 py-2 text-xs text-muted-foreground",
          className
        )}
      >
        <BeuiActivityOrb phase="idle" styleVariant={orbStyle} size="sm" />
        <span>{emptyLabel}</span>
      </div>
    )
  }

  const maxHeightPx = resolveMaxHeightPx(maxHeight)
  const effectiveMaxHeight = maxHeightPx > 0 ? maxHeightPx : 2400
  const resolvedDefaultOpen =
    defaultOpen ?? (mode === "timeline" ? true : !autoCollapse)
  const resolvedCollapseOnComplete = mode === "timeline" ? false : autoCollapse

  const renderWorkingStatus = loaderVariant
    ? ({ label }: { label: React.ReactNode; duration: number }) => (
        <span className="inline-flex min-w-0 items-center gap-2">
          <ThinkingLoaderBadge
            variant={loaderVariant}
            size={15}
            className="shrink-0 text-primary"
          />
          {loaderVariant !== "scramble" && loaderVariant !== "percent" && (
            <ThinkingShimmer>{label}</ThinkingShimmer>
          )}
        </span>
      )
    : orbStyle === "minimal-spinner"
      ? undefined
      : ({ label }: { label: React.ReactNode; duration: number }) => (
          <span className="inline-flex min-w-0 items-center gap-2">
            <BeuiActivityOrb
              phase={phase}
              isActive
              styleVariant={orbStyle}
              size="sm"
            />
            <ThinkingShimmer>{label}</ThinkingShimmer>
          </span>
        )

  return (
    <div className={cn("w-full", className)}>
      {headerActions && (
        <div className="mb-1 flex items-center justify-end gap-1.5">
          {headerActions}
        </div>
      )}
      <OfficialBeuiAgentActivity
        items={officialItems}
        contentType={OFFICIAL_CONTENT_TYPE_MAP[contentType]}
        status={working ? "working" : "complete"}
        duration={durationSeconds}
        open={open}
        defaultOpen={resolvedDefaultOpen}
        onOpenChange={onOpenChange}
        collapseOnComplete={resolvedCollapseOnComplete}
        activeLabel={working && title ? title : undefined}
        summary={!working && title ? title : undefined}
        renderWorkingStatus={renderWorkingStatus}
        maxHeight={effectiveMaxHeight}
        contentClassName={
          density === "compact" ? "space-y-0 py-1 text-xs" : undefined
        }
      />
    </div>
  )
}
