import * as React from "react"
import {
  Activity,
  AlertTriangle,
  Brain,
  CheckCircle2,
  ChevronDown,
  Clock3,
  FileCode2,
  GitBranch,
  Loader2,
  MessageSquare,
  Search,
  ShieldAlert,
  Sparkles,
  Terminal,
} from "lucide-react"

import { FOX_SILHOUETTE_PATH } from "@/components/OpenZFoxIcon"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

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

export type BeuiActivityOrbStyle =
  | "dotted-orb"
  | "fox-pulse"
  | "minimal-spinner"

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
      "Shows @beui/agent-activity inline on multi-step or active turns plus the live right-hand Activity Drawer.",
    isDefault: true,
  },
  {
    id: "always-inline",
    title: "Always Inline Stream",
    shortLabel: "Inline",
    description:
      "Embeds @beui/agent-activity inside every assistant turn that records reasoning, tools, notices, or approvals.",
  },
  {
    id: "streaming-only",
    title: "Live Streaming Only",
    shortLabel: "Live Only",
    description:
      "Displays inline activity while a turn is actively streaming, then tucks completed history into the panel.",
  },
  {
    id: "panel-only",
    title: "Activity Drawer Only",
    shortLabel: "Drawer",
    description:
      "Keeps chat bubbles clean and routes all turn telemetry to the dedicated Activity Drawer and workspace feeds.",
  },
]

export const ACTIVITY_VARIANT_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityVariant>
> = [
  {
    id: "orb-card",
    title: "BEUI Thought-Orb Card",
    shortLabel: "Orb Card",
    description:
      "Signature beui.dev adaptive stream card with reactive 9-dot Thought Orb, live ticker, duration timer, and expandable rows.",
    isDefault: true,
  },
  {
    id: "rail-timeline",
    title: "BEUI Connected Rail",
    shortLabel: "Rail",
    description:
      "Vertical timeline connector rail with status nodes, role badges, actor tags, and structured step cards.",
  },
  {
    id: "compact-pills",
    title: "BEUI Compact Step Strip",
    shortLabel: "Pills",
    description:
      "Minimal wrapped chip stream showing status dots, step titles, and durations with click-to-inspect disclosure.",
  },
  {
    id: "trace-log",
    title: "BEUI Chronological Trace",
    shortLabel: "Trace",
    description:
      "Dense monospace execution trace with timestamps, kind tags, actors, and expandable JSON/text payloads.",
  },
]

export const ACTIVITY_ORB_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityOrbStyle>
> = [
  {
    id: "dotted-orb",
    title: "Reactive Dotted Thought Orb",
    shortLabel: "9-Dot Orb",
    description:
      "Authentic beui.dev 3×3 dot matrix whose wave geometry adapts to thinking, search, tool, security, and completion phases.",
    isDefault: true,
  },
  {
    id: "fox-pulse",
    title: "OpenZ Fox Emblem Pulse",
    shortLabel: "Fox Pulse",
    description:
      "Sculpted OpenZ Fox silhouette with phase-tinted orbital ring pulse while steps execute.",
  },
  {
    id: "minimal-spinner",
    title: "Minimal Orbital Indicator",
    shortLabel: "Minimal",
    description:
      "Clean understated status ring and crisp checkmark without decorative motion.",
  },
]

export const ACTIVITY_DENSITY_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityDensity>
> = [
  {
    id: "comfortable",
    title: "Comfortable Spacing",
    shortLabel: "Comfort",
    description: "Balanced padding, readable step excerpts, and clear metadata pills.",
    isDefault: true,
  },
  {
    id: "compact",
    title: "Compact High-Density",
    shortLabel: "Compact",
    description: "Tighter row rhythm to fit more tool, subagent, and log steps per screen.",
  },
]

export const ACTIVITY_MAX_HEIGHT_OPTIONS: ReadonlyArray<
  AgentActivityOptionDescriptor<BeuiActivityMaxHeight>
> = [
  {
    id: "compact",
    title: "Compact Viewport (176px)",
    shortLabel: "176px",
    description: "Clamps long step streams early with smooth internal scrolling.",
  },
  {
    id: "standard",
    title: "Balanced Viewport (264px)",
    shortLabel: "264px",
    description: "Default scrollable window showing ~5–6 steps before scrolling.",
    isDefault: true,
  },
  {
    id: "tall",
    title: "Tall Viewport (384px)",
    shortLabel: "384px",
    description: "Expanded inspection window for deep multi-agent and log streams.",
  },
  {
    id: "unlimited",
    title: "Unclamped Full Stream",
    shortLabel: "Full",
    description: "Renders every activity step inline without internal max-height clamping.",
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
}

export interface AgentActivityProps {
  steps: AgentActivityStep[]
  isActive?: boolean
  mode?: "timeline" | "inline-collapsible"
  variant?: BeuiActivityVariant
  orbStyle?: BeuiActivityOrbStyle
  density?: BeuiActivityDensity
  maxHeight?: BeuiActivityMaxHeight | number
  contentType?: BeuiActivityContentType
  autoCollapse?: boolean
  showDetails?: boolean
  defaultOpen?: boolean
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

const STEP_KIND_SHORT_TAG: Record<AgentActivityStepKind, string> = {
  tool: "TOOL",
  security: "GUARD",
  reasoning: "THINK",
  notice: "NOTE",
  message: "MSG",
  search: "SEARCH",
  trace: "TRACE",
}

const STATUS_NODE_CLASSES: Record<AgentActivityStepStatus, string> = {
  running: "border-primary/50 bg-primary/15 text-primary",
  success: "border-success/40 bg-success/12 text-success",
  error: "border-destructive/45 bg-destructive/12 text-destructive",
  pending: "border-warning/45 bg-warning/12 text-warning",
  info: "border-border bg-muted/60 text-muted-foreground",
}

const STATUS_DOT_CLASSES: Record<AgentActivityStepStatus, string> = {
  running: "bg-primary animate-pulse",
  success: "bg-success",
  error: "bg-destructive",
  pending: "bg-warning animate-pulse",
  info: "bg-muted-foreground/70",
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

const PHASE_LABELS: Record<BeuiActivityPhase, string> = {
  idle: "Ready",
  reasoning: "Thinking",
  search: "Searching",
  tool: "Executing tool",
  security: "Awaiting approval",
  workflow: "Orchestrating",
  complete: "Completed",
  error: "Attention needed",
}

/**
 * 3×3 coordinate grid (row, col in 0..2) mapped to phase-specific delay multipliers (ms).
 * Produces distinct organic wave patterns without branching logic:
 * - reasoning: spiral orbit around center
 * - search: horizontal radar sweep (col-major)
 * - tool: diagonal cascade (row + col)
 * - security: perimeter vs core pulse
 * - workflow: vertical stream (row-major)
 */
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

function formatClockTime(timestamp?: number): string {
  if (!timestamp) return "--:--:--"
  const d = new Date(timestamp)
  if (Number.isNaN(d.getTime())) return "--:--:--"
  return d.toLocaleTimeString([], {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function formatElapsedMs(ms: number): string {
  const clamped = Math.max(0, ms)
  if (clamped < 1000) return `${Math.round(clamped)}ms`
  const secs = clamped / 1000
  if (secs < 60) {
    return secs < 10 ? `${secs.toFixed(1)}s` : `${Math.round(secs)}s`
  }
  const mins = Math.floor(secs / 60)
  const remSecs = Math.round(secs % 60)
  return `${mins}m ${String(remSecs).padStart(2, "0")}s`
}

function computeEarliestTimestamp(
  steps: ReadonlyArray<AgentActivityStep>,
  startedAt?: number
): number | undefined {
  if (startedAt && startedAt > 0) return startedAt
  const valid = steps
    .map((s) => s.timestamp ?? 0)
    .filter((ts) => ts > 0)
  return valid.length > 0 ? Math.min(...valid) : undefined
}

function useLiveDurationLabel(
  steps: ReadonlyArray<AgentActivityStep>,
  isActive: boolean,
  startedAt?: number
): string | undefined {
  const anchorTs = React.useMemo(
    () => computeEarliestTimestamp(steps, startedAt),
    [steps, startedAt]
  )
  const summedMs = React.useMemo(
    () => steps.reduce((acc, s) => acc + (s.durationMs ?? 0), 0),
    [steps]
  )
  const [now, setNow] = React.useState<number>(() => Date.now())

  React.useEffect(() => {
    if (!isActive || !anchorTs) return undefined
    setNow(Date.now())
    const timer = window.setInterval(() => setNow(Date.now()), 400)
    return () => window.clearInterval(timer)
  }, [isActive, anchorTs])

  if (isActive && anchorTs) {
    return formatElapsedMs(Math.max(100, now - anchorTs))
  }
  if (summedMs > 0) {
    return formatElapsedMs(summedMs)
  }
  const explicit = steps.find((s) => Boolean(s.durationLabel))?.durationLabel
  return explicit
}

export interface BeuiActivityOrbProps {
  phase: BeuiActivityPhase
  isActive?: boolean
  styleVariant?: BeuiActivityOrbStyle
  size?: "sm" | "md" | "lg"
  className?: string
}

const ORB_DIMENSIONS: Record<"sm" | "md" | "lg", string> = {
  sm: "size-5",
  md: "size-6",
  lg: "size-8",
}

/**
 * Reactive Thought-Orb indicator modeled on `beui.dev/components/agents/agent-activity`.
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

  if (styleVariant === "fox-pulse") {
    return (
      <span
        aria-hidden="true"
        className={cn(
          "relative inline-flex shrink-0 items-center justify-center rounded-lg border border-border/60 bg-card/80",
          sizeClass,
          colorClass,
          className
        )}
      >
        <svg viewBox="0 0 100 100" className="size-[76%]">
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
        "relative inline-flex shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/45",
        sizeClass,
        colorClass,
        className
      )}
    >
      <svg viewBox="0 0 24 24" className="size-[78%]">
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
              r={1.85}
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

function renderStatusGlyph(
  status: AgentActivityStepStatus,
  Icon: IconComponent,
  sizeClass = "size-3"
) {
  if (status === "running") return <Loader2 className={cn(sizeClass, "animate-spin")} />
  if (status === "success") return <CheckCircle2 className={sizeClass} />
  if (status === "error") return <AlertTriangle className={sizeClass} />
  return <Icon className={sizeClass} />
}

/**
 * @beui/agent-activity — Adaptive live agent activity stream component
 * inspired by https://beui.dev/components/agents/agent-activity with reactive
 * Thought Orb, visual variants, contentType filtering, maxHeight viewport,
 * auto-collapse on completion, and interactive step payload disclosure.
 */
export const AgentActivity: React.FC<AgentActivityProps> = ({
  steps,
  isActive = false,
  mode = "timeline",
  variant = "orb-card",
  orbStyle = "dotted-orb",
  density = "comfortable",
  maxHeight = "standard",
  contentType = "mixed",
  autoCollapse = false,
  showDetails = true,
  defaultOpen,
  title,
  subtitle,
  startedAt,
  emptyLabel,
  headerActions,
  onStepSelect,
  className,
}) => {
  const visibleSteps = React.useMemo(() => {
    if (contentType === "mixed") return steps
    return steps.filter((s) => resolveStepContentType(s) === contentType)
  }, [steps, contentType])

  const runningCount = visibleSteps.filter(
    (s) => s.status === "running" || s.status === "pending"
  ).length
  const completedCount = visibleSteps.filter((s) => s.status === "success").length
  const errorCount = visibleSteps.filter((s) => s.status === "error").length

  const phase = React.useMemo(
    () => resolveActivityPhase(visibleSteps, isActive),
    [visibleSteps, isActive]
  )

  const liveDuration = useLiveDurationLabel(
    visibleSteps,
    isActive || runningCount > 0,
    startedAt
  )

  const initialOpen =
    defaultOpen ??
    (mode === "timeline"
      ? true
      : isActive || runningCount > 0 || !autoCollapse)

  const [isOpen, setIsOpen] = React.useState<boolean>(initialOpen)
  const [expandedStepIds, setExpandedStepIds] = React.useState<Record<string, boolean>>({})
  const wasActiveRef = React.useRef<boolean>(isActive || runningCount > 0)

  React.useEffect(() => {
    const currentlyWorking = isActive || runningCount > 0
    if (currentlyWorking && !wasActiveRef.current && mode === "inline-collapsible") {
      setIsOpen(true)
    }
    if (!currentlyWorking && wasActiveRef.current && autoCollapse && errorCount === 0) {
      setIsOpen(false)
    }
    wasActiveRef.current = currentlyWorking
  }, [isActive, runningCount, autoCollapse, errorCount, mode])

  if (visibleSteps.length === 0) {
    if (!emptyLabel) return null
    return (
      <div
        className={cn(
          "flex items-center gap-2.5 rounded-xl border border-dashed border-border/60 bg-card/40 px-3.5 py-3 text-xs text-muted-foreground",
          className
        )}
      >
        <BeuiActivityOrb phase="idle" styleVariant={orbStyle} size="sm" />
        <span>{emptyLabel}</span>
      </div>
    )
  }

  const maxHeightPx = resolveMaxHeightPx(maxHeight)
  const scrollStyle: React.CSSProperties | undefined =
    maxHeightPx > 0 ? { maxHeight: `${maxHeightPx}px` } : undefined

  const isCompact = density === "compact"

  const activeHeadlineStep =
    visibleSteps.find((s) => s.status === "running" || s.status === "pending") ??
    visibleSteps[0]

  const headerTitle =
    title ??
    (runningCount > 0
      ? `${PHASE_LABELS[phase]} · ${activeHeadlineStep?.title ?? "Working"}`
      : errorCount > 0
        ? `Activity recorded (${errorCount} issue${errorCount === 1 ? "" : "s"})`
        : `Completed ${completedCount || visibleSteps.length} ${
            visibleSteps.length === 1 ? "step" : "steps"
          }`)

  const headerSubtitle =
    subtitle ??
    (runningCount > 0 && activeHeadlineStep?.detail
      ? activeHeadlineStep.detail
      : !isOpen && activeHeadlineStep
        ? activeHeadlineStep.title
        : undefined)

  const toggleStepExpand = (step: AgentActivityStep) => {
    onStepSelect?.(step)
    const hasExpandable = Boolean(step.payload || (step.detail && step.detail.length > 72))
    if (!hasExpandable) return
    setExpandedStepIds((prev) => ({
      ...prev,
      [step.id]: !prev[step.id],
    }))
  }

  const renderStepPayload = (step: AgentActivityStep) => {
    if (!expandedStepIds[step.id]) return null
    const content = step.payload ?? step.detail
    if (!content) return null
    return (
      <pre className="mt-1.5 max-h-44 overflow-auto rounded-md border border-border/60 bg-muted/45 p-2 font-mono text-[10.5px] leading-relaxed whitespace-pre-wrap break-words text-foreground/90">
        {content}
      </pre>
    )
  }

  const renderOrbStreamRows = () => (
    <div
      style={scrollStyle}
      className={cn(
        " divide-y divide-border/45",
        maxHeightPx > 0 && "overflow-y-auto overscroll-contain"
      )}
    >
      {visibleSteps.map((step) => {
        const Icon = resolveStepIcon(step)
        const isExpanded = Boolean(expandedStepIds[step.id])
        const canExpand = Boolean(
          step.payload || (step.detail && step.detail.length > 72) || onStepSelect
        )

        return (
          <div
            key={step.id}
            onClick={canExpand ? () => toggleStepExpand(step) : undefined}
            role={canExpand ? "button" : undefined}
            tabIndex={canExpand ? 0 : undefined}
            onKeyDown={
              canExpand
                ? (e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault()
                      toggleStepExpand(step)
                    }
                  }
                : undefined
            }
            className={cn(
              "group flex items-start gap-2.5 transition-colors",
              isCompact ? "px-2.5 py-1.5" : "px-3 py-2.5",
              canExpand && "cursor-pointer hover:bg-muted/35"
            )}
          >
            <span
              className={cn(
                "mt-0.5 flex shrink-0 items-center justify-center rounded-md border",
                isCompact ? "size-5" : "size-5.5",
                STATUS_NODE_CLASSES[step.status]
              )}
            >
              {renderStatusGlyph(step.status, Icon, "size-3")}
            </span>

            <div className="min-w-0 flex-1">
              <div className="flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-1.5">
                  <span className="truncate font-mono text-[11.5px] font-semibold text-foreground">
                    {step.title}
                  </span>
                  {step.actor && (
                    <Badge
                      variant="secondary"
                      className="px-1.5 py-0 font-mono text-[9px]"
                    >
                      {step.actor}
                    </Badge>
                  )}
                  {step.meta && (
                    <span className="hidden truncate text-[10px] text-muted-foreground sm:inline">
                      {step.meta}
                    </span>
                  )}
                </div>

                <div className="flex shrink-0 items-center gap-1.5">
                  {showDetails && step.durationLabel && (
                    <Badge
                      variant="outline"
                      className="px-1.5 py-0 font-mono text-[9.5px]"
                    >
                      {step.durationLabel}
                    </Badge>
                  )}
                  {canExpand && (step.payload || (step.detail && step.detail.length > 72)) && (
                    <ChevronDown
                      className={cn(
                        "size-3 text-muted-foreground/70 transition-transform duration-150",
                        isExpanded && "rotate-180 text-foreground"
                      )}
                    />
                  )}
                </div>
              </div>

              {showDetails && step.detail && !isExpanded && (
                <p
                  className={cn(
                    "mt-0.5 text-[11px] leading-relaxed text-muted-foreground",
                    isCompact ? "line-clamp-1" : "line-clamp-2"
                  )}
                >
                  {step.detail}
                </p>
              )}

              {renderStepPayload(step)}
            </div>
          </div>
        )
      })}
    </div>
  )

  const renderRailTimeline = () => (
    <div
      style={scrollStyle}
      className={cn(
        "relative pl-1",
        isCompact ? "space-y-1.5" : "space-y-2",
        maxHeightPx > 0 && "overflow-y-auto overscroll-contain pr-1"
      )}
    >
      {visibleSteps.map((step, index) => {
        const Icon = resolveStepIcon(step)
        const isLast = index === visibleSteps.length - 1
        const canExpand = Boolean(
          step.payload || (step.detail && step.detail.length > 72) || onStepSelect
        )

        return (
          <div key={step.id} className="relative flex items-start gap-2.5">
            {!isLast && (
              <span
                aria-hidden="true"
                className="absolute top-6 bottom-[-10px] left-[11px] w-px bg-border/75"
              />
            )}

            <div
              className={cn(
                "relative z-10 mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full border",
                STATUS_NODE_CLASSES[step.status]
              )}
            >
              {renderStatusGlyph(step.status, Icon, "size-3")}
            </div>

            <div
              onClick={canExpand ? () => toggleStepExpand(step) : undefined}
              className={cn(
                "min-w-0 flex-1 rounded-lg border border-border/60 bg-card/70 text-xs transition-colors hover:border-border",
                isCompact ? "px-2 py-1.5" : "px-2.5 py-2",
                canExpand && "cursor-pointer"
              )}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11.5px] font-semibold text-foreground">
                  <Icon className="size-3 shrink-0 text-muted-foreground" />
                  <span className="truncate">{step.title}</span>
                  {step.actor && (
                    <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[9px]">
                      {step.actor}
                    </Badge>
                  )}
                </span>
                {showDetails && step.durationLabel ? (
                  <Badge variant="outline" className="px-1.5 py-0 font-mono text-[9.5px]">
                    {step.durationLabel}
                  </Badge>
                ) : null}
              </div>

              {showDetails && step.detail && !expandedStepIds[step.id] && (
                <p
                  className={cn(
                    "mt-1 text-[11px] leading-relaxed text-muted-foreground",
                    isCompact ? "line-clamp-1" : "line-clamp-2"
                  )}
                >
                  {step.detail}
                </p>
              )}

              {renderStepPayload(step)}
            </div>
          </div>
        )
      })}
    </div>
  )

  const renderCompactPills = () => {
    const selectedStep = visibleSteps.find((s) => Boolean(expandedStepIds[s.id]))
    return (
      <div className="space-y-2">
        <div
          style={scrollStyle}
          className={cn(
            "flex flex-wrap items-center gap-1.5",
            maxHeightPx > 0 && "overflow-y-auto overscroll-contain"
          )}
        >
          {visibleSteps.map((step) => {
            const Icon = resolveStepIcon(step)
            const isSelected = Boolean(expandedStepIds[step.id])
            return (
              <button
                key={step.id}
                type="button"
                onClick={() => {
                  onStepSelect?.(step)
                  setExpandedStepIds((prev) => ({
                    [step.id]: !prev[step.id],
                  }))
                }}
                className={cn(
                  "inline-flex max-w-full items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] transition-colors",
                  isSelected
                    ? "border-primary/50 bg-primary/12 text-foreground"
                    : "border-border/65 bg-card/75 text-foreground/90 hover:border-primary/35 hover:bg-muted/40"
                )}
              >
                <span
                  className={cn(
                    "size-1.5 shrink-0 rounded-full",
                    STATUS_DOT_CLASSES[step.status]
                  )}
                />
                <Icon className="size-3 shrink-0 text-muted-foreground" />
                <span className="max-w-[180px] truncate font-mono font-medium">
                  {step.title}
                </span>
                {showDetails && step.durationLabel && (
                  <span className="font-mono text-[9.5px] text-muted-foreground">
                    {step.durationLabel}
                  </span>
                )}
              </button>
            )
          })}
        </div>

        {selectedStep && (selectedStep.detail || selectedStep.payload) && (
          <div className="rounded-lg border border-border/65 bg-card/80 p-2.5 text-xs">
            <div className="mb-1 flex items-center justify-between gap-2 font-mono text-[11px] font-semibold text-foreground">
              <span>{selectedStep.title}</span>
              {selectedStep.meta && (
                <span className="text-[10px] font-normal text-muted-foreground">
                  {selectedStep.meta}
                </span>
              )}
            </div>
            <pre className="max-h-40 overflow-auto font-mono text-[10.5px] leading-relaxed whitespace-pre-wrap break-words text-muted-foreground">
              {selectedStep.payload ?? selectedStep.detail}
            </pre>
          </div>
        )}
      </div>
    )
  }

  const renderTraceLog = () => (
    <div
      style={scrollStyle}
      className={cn(
        "divide-y divide-border/40 rounded-lg border border-border/65 bg-card/75 font-mono text-[11px]",
        maxHeightPx > 0 && "overflow-y-auto overscroll-contain"
      )}
    >
      {visibleSteps.map((step) => {
        const tag = STEP_KIND_SHORT_TAG[step.kind]
        const canExpand = Boolean(
          step.payload || (step.detail && step.detail.length > 72) || onStepSelect
        )

        return (
          <div
            key={step.id}
            onClick={canExpand ? () => toggleStepExpand(step) : undefined}
            className={cn(
              "px-2.5 transition-colors",
              isCompact ? "py-1" : "py-1.5",
              canExpand && "cursor-pointer hover:bg-muted/40"
            )}
          >
            <div className="flex items-center gap-2">
              <span
                className={cn(
                  "size-1.5 shrink-0 rounded-full",
                  STATUS_DOT_CLASSES[step.status]
                )}
              />
              <span className="shrink-0 text-[10px] text-muted-foreground/75">
                {formatClockTime(step.timestamp)}
              </span>
              <span className="shrink-0 rounded bg-muted px-1 py-0.2 text-[9px] font-bold text-muted-foreground">
                {tag}
              </span>
              {step.actor && (
                <span className="shrink-0 text-[10px] text-primary/85">
                  @{step.actor}
                </span>
              )}
              <span className="min-w-0 flex-1 truncate font-semibold text-foreground">
                {step.title}
              </span>
              {showDetails && step.durationLabel && (
                <span className="shrink-0 text-[10px] text-muted-foreground">
                  {step.durationLabel}
                </span>
              )}
            </div>

            {showDetails && step.detail && !expandedStepIds[step.id] && (
              <div className="mt-0.5 truncate pl-3.5 text-[10.5px] text-muted-foreground">
                {step.detail}
              </div>
            )}

            {renderStepPayload(step)}
          </div>
        )
      })}
    </div>
  )

  const variantBodyRenderers: Record<BeuiActivityVariant, () => React.ReactNode> = {
    "orb-card": renderOrbStreamRows,
    "rail-timeline": renderRailTimeline,
    "compact-pills": renderCompactPills,
    "trace-log": renderTraceLog,
  }

  const bodyContent = variantBodyRenderers[variant]()

  // In standalone timeline mode with non-orb-card variants, render clean headerless stream unless title/headerActions provided
  if (mode === "timeline" && variant !== "orb-card" && !title && !headerActions) {
    return <div className={cn("w-full", className)}>{bodyContent}</div>
  }

  return (
    <div
      className={cn(
        "my-2 w-full overflow-hidden rounded-xl border border-border/70 bg-card/65 shadow-2xs transition-colors",
        (isActive || runningCount > 0) && "border-primary/35",
        className
      )}
    >
      <div
        className={cn(
          "flex items-center justify-between gap-2 px-3",
          isCompact ? "py-2" : "py-2.5",
          isOpen && "border-b border-border/50"
        )}
      >
        <button
          type="button"
          onClick={() => setIsOpen((prev) => !prev)}
          aria-expanded={isOpen}
          className="flex min-w-0 flex-1 items-center gap-2.5 text-left"
        >
          <BeuiActivityOrb
            phase={phase}
            isActive={isActive || runningCount > 0}
            styleVariant={orbStyle}
            size={isCompact ? "sm" : "md"}
          />

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="truncate text-xs font-semibold text-foreground">
                {headerTitle}
              </span>
            </div>
            {headerSubtitle && (
              <p className="truncate text-[11px] text-muted-foreground">
                {headerSubtitle}
              </p>
            )}
          </div>
        </button>

        <div className="flex shrink-0 items-center gap-1.5">
          {headerActions}
          {showDetails && liveDuration && (
            <Badge
              variant="secondary"
              className="gap-1 px-1.5 py-0 font-mono text-[10px]"
            >
              <Clock3 className="size-2.5" />
              <span>{liveDuration}</span>
            </Badge>
          )}
          <Badge
            variant={runningCount > 0 ? "warning" : errorCount > 0 ? "destructive" : "outline"}
            className="px-1.5 py-0 font-mono text-[10px]"
          >
            {runningCount > 0
              ? `${completedCount}/${visibleSteps.length}`
              : `${visibleSteps.length} ${visibleSteps.length === 1 ? "step" : "steps"}`}
          </Badge>
          <button
            type="button"
            onClick={() => setIsOpen((prev) => !prev)}
            aria-label={isOpen ? "Collapse activity stream" : "Expand activity stream"}
            className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted/60 hover:text-foreground"
          >
            <ChevronDown
              className={cn(
                "size-3.5 transition-transform duration-150",
                isOpen && "rotate-180"
              )}
            />
          </button>
        </div>
      </div>

      {isOpen && (
        <div
          className={cn(
            variant === "orb-card" ? "p-0" : isCompact ? "p-2" : "p-2.5"
          )}
        >
          {bodyContent}
        </div>
      )}
    </div>
  )
}
