import * as React from "react"
import {
  Activity,
  AlertTriangle,
  ArrowUpRight,
  Bell,
  BellOff,
  BookOpen,
  Bookmark,
  Bot,
  Brain,
  CheckCircle2,
  GitBranch,
  Layers,
  Search,
  ShieldAlert,
  Sparkles,
  Terminal,
  X,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Switch } from "@/components/ui/switch"
import {
  BackgroundInboxElement,
  type BackgroundInboxStyleChoice,
  type BackgroundInboxTask,
} from "@/components/assistant-ui/elements"
import {
  RareDeleteButton,
  type DeleteButtonStyleChoice,
} from "@/components/registries/rareAndCommunity"
import {
  Alert as OfficialBeuiAlert,
  AlertAction,
  AlertClose,
  AlertContent,
  AlertDescription,
  AlertIcon,
  AlertTitle,
  type AlertVariant,
} from "@/components/motion/alert"
import {
  AnimatedToastStack as OfficialMotionToastStack,
  type AnimatedToast as OfficialAnimatedToast,
  type ToastPosition as OfficialMotionToastPosition,
  type ToastStatus as OfficialMotionToastStatus,
} from "@/components/motion/animated-toast-stack"
import { cn } from "@/lib/utils"
import type { ActivityNotice } from "@/types"

export type BeuiToastCategory =
  | "workflow"
  | "skill"
  | "self_improvement"
  | "memory"
  | "research"
  | "source"
  | "security"
  | "orchestration"
  | "system"
  | "error"

export type BeuiToastStatus =
  | "info"
  | "success"
  | "warning"
  | "error"
  | "running"

export type BeuiToastVariant =
  | "beui-stack"
  | "beui-motion"
  | "beui-alert"
  | "compact-pills"
  | "dynamic-island"
  | "minimal-glass"
  | "accent-banner"
  | "terminal-hud"
  | "split-action"
  | "soft-tinted"
  | "brutalist-outline"
  | "micro-badge"
  | "rare-toast"

export type BeuiCanonicalToastPosition =
  | "top-start"
  | "top-center"
  | "top-end"
  | "bottom-start"
  | "bottom-center"
  | "bottom-end"

export type BeuiToastPosition =
  | BeuiCanonicalToastPosition
  | "bottom-right"
  | "bottom-left"
  | "top-right"

export type BeuiToastDurationPreset =
  | "fast"
  | "standard"
  | "extended"
  | "sticky"

export type BeuiToastMaxVisible = 3 | 4 | 5

export type BeuiInlineNoticeStyle =
  | "minimal-whisper"
  | "memory-chips"
  | "tool-chips"
  | "ai-status"
  | "pills"
  | "cards"
  | "toast-only"

export interface BeuiToastOptionDescriptor<T extends string | number> {
  id: T
  title: string
  shortLabel: string
  description: string
  isDefault?: boolean
}

export interface BeuiToastItem {
  id: string
  dedupeKey?: string
  category: BeuiToastCategory
  status?: BeuiToastStatus
  title: string
  detail?: string
  badge?: string
  durationMs?: number
  createdAt: number
  count?: number
  actionLabel?: string
  onAction?: () => void
}

type IconComponent = React.ComponentType<{ className?: string }>

export interface BeuiToastCategoryMeta {
  label: string
  shortTag: string
  icon: IconComponent
  iconWrapClass: string
  pillClass: string
  cardBorderClass: string
  railClass: string
  progressClass: string
  softSurfaceClass: string
  dotClass: string
  stampClass: string
  defaultActionLabel?: string
}

export const TOAST_DURATION_MS: Record<BeuiToastDurationPreset, number> = {
  fast: 3200,
  standard: 5000,
  extended: 8000,
  sticky: 0,
}

export const TOAST_CATEGORY_META: Record<
  BeuiToastCategory,
  BeuiToastCategoryMeta
> = {
  workflow: {
    label: "Workflow",
    shortTag: "WORKFLOW",
    icon: GitBranch,
    iconWrapClass: "border-chart-4/40 bg-chart-4/15 text-chart-4",
    pillClass:
      "border-chart-4/35 bg-chart-4/10 text-foreground hover:border-chart-4/60 hover:bg-chart-4/16",
    cardBorderClass: "border-chart-4/35",
    railClass: "border-l-chart-4",
    progressClass: "bg-chart-4",
    softSurfaceClass: "border-chart-4/40 bg-chart-4/12",
    dotClass: "bg-chart-4",
    stampClass: "bg-chart-4 text-primary-foreground",
    defaultActionLabel: "Inspect Activity",
  },
  skill: {
    label: "Skill",
    shortTag: "SKILL",
    icon: BookOpen,
    iconWrapClass: "border-primary/40 bg-primary/15 text-primary",
    pillClass:
      "border-primary/35 bg-primary/10 text-foreground hover:border-primary/60 hover:bg-primary/16",
    cardBorderClass: "border-primary/35",
    railClass: "border-l-primary",
    progressClass: "bg-primary",
    softSurfaceClass: "border-primary/40 bg-primary/12",
    dotClass: "bg-primary",
    stampClass: "bg-primary text-primary-foreground",
    defaultActionLabel: "Open Skills",
  },
  self_improvement: {
    label: "Self-Improvement",
    shortTag: "CURATOR",
    icon: Sparkles,
    iconWrapClass: "border-success/40 bg-success/15 text-success",
    pillClass:
      "border-success/35 bg-success/10 text-foreground hover:border-success/60 hover:bg-success/16",
    cardBorderClass: "border-success/35",
    railClass: "border-l-success",
    progressClass: "bg-success",
    softSurfaceClass: "border-success/40 bg-success/12",
    dotClass: "bg-success",
    stampClass: "bg-success text-primary-foreground",
    defaultActionLabel: "Open Skills",
  },
  memory: {
    label: "Memory",
    shortTag: "MEMORY",
    icon: Brain,
    iconWrapClass: "border-info/40 bg-info/15 text-info",
    pillClass:
      "border-info/35 bg-info/10 text-foreground hover:border-info/60 hover:bg-info/16",
    cardBorderClass: "border-info/35",
    railClass: "border-l-info",
    progressClass: "bg-info",
    softSurfaceClass: "border-info/40 bg-info/12",
    dotClass: "bg-info",
    stampClass: "bg-info text-primary-foreground",
    defaultActionLabel: "Open Memory",
  },
  research: {
    label: "Research",
    shortTag: "RESEARCH",
    icon: Search,
    iconWrapClass: "border-info/40 bg-info/15 text-info",
    pillClass:
      "border-info/35 bg-info/10 text-foreground hover:border-info/60 hover:bg-info/16",
    cardBorderClass: "border-info/35",
    railClass: "border-l-info",
    progressClass: "bg-info",
    softSurfaceClass: "border-info/40 bg-info/12",
    dotClass: "bg-info",
    stampClass: "bg-info text-primary-foreground",
    defaultActionLabel: "Open Knowledge",
  },
  source: {
    label: "Knowledge Source",
    shortTag: "SOURCE",
    icon: Bookmark,
    iconWrapClass: "border-chart-2/40 bg-chart-2/15 text-chart-2",
    pillClass:
      "border-chart-2/35 bg-chart-2/10 text-foreground hover:border-chart-2/60 hover:bg-chart-2/16",
    cardBorderClass: "border-chart-2/35",
    railClass: "border-l-chart-2",
    progressClass: "bg-chart-2",
    softSurfaceClass: "border-chart-2/40 bg-chart-2/12",
    dotClass: "bg-chart-2",
    stampClass: "bg-chart-2 text-primary-foreground",
    defaultActionLabel: "Open Knowledge",
  },
  security: {
    label: "Security Guard",
    shortTag: "APPROVAL",
    icon: ShieldAlert,
    iconWrapClass: "border-warning/45 bg-warning/15 text-warning",
    pillClass:
      "border-warning/40 bg-warning/12 text-foreground hover:border-warning/65 hover:bg-warning/18",
    cardBorderClass: "border-warning/40",
    railClass: "border-l-warning",
    progressClass: "bg-warning",
    softSurfaceClass: "border-warning/45 bg-warning/14",
    dotClass: "bg-warning",
    stampClass: "bg-warning text-background",
    defaultActionLabel: "Review Guard",
  },
  orchestration: {
    label: "Orchestration",
    shortTag: "AGENTS",
    icon: Bot,
    iconWrapClass: "border-chart-4/40 bg-chart-4/15 text-chart-4",
    pillClass:
      "border-chart-4/35 bg-chart-4/10 text-foreground hover:border-chart-4/60 hover:bg-chart-4/16",
    cardBorderClass: "border-chart-4/35",
    railClass: "border-l-chart-4",
    progressClass: "bg-chart-4",
    softSurfaceClass: "border-chart-4/40 bg-chart-4/12",
    dotClass: "bg-chart-4",
    stampClass: "bg-chart-4 text-primary-foreground",
    defaultActionLabel: "Inspect Run",
  },
  system: {
    label: "System",
    shortTag: "SYSTEM",
    icon: Activity,
    iconWrapClass: "border-border bg-muted/70 text-muted-foreground",
    pillClass:
      "border-border/75 bg-muted/45 text-foreground hover:border-border hover:bg-muted/70",
    cardBorderClass: "border-border/80",
    railClass: "border-l-muted-foreground",
    progressClass: "bg-primary",
    softSurfaceClass: "border-border/75 bg-muted/55",
    dotClass: "bg-primary",
    stampClass: "bg-foreground text-background",
  },
  error: {
    label: "Runtime Alert",
    shortTag: "ALERT",
    icon: AlertTriangle,
    iconWrapClass: "border-destructive/45 bg-destructive/15 text-destructive",
    pillClass:
      "border-destructive/40 bg-destructive/10 text-foreground hover:border-destructive/65 hover:bg-destructive/16",
    cardBorderClass: "border-destructive/40",
    railClass: "border-l-destructive",
    progressClass: "bg-destructive",
    softSurfaceClass: "border-destructive/45 bg-destructive/12",
    dotClass: "bg-destructive",
    stampClass: "bg-destructive text-destructive-foreground",
    defaultActionLabel: "Open Logs",
  },
}

export const TOAST_VARIANT_OPTIONS: ReadonlyArray<
  BeuiToastOptionDescriptor<BeuiToastVariant>
> = [
  {
    id: "beui-stack",
    title: "BEUI Layered 3D Card",
    shortLabel: "Stack",
    description:
      "Signature @beui/animated-toast-stack depth-scaled card deck with category emblem, action pill, and countdown bar.",
    isDefault: true,
  },
  {
    id: "beui-motion",
    title: "Official @beui/animated-toast-stack Motion",
    shortLabel: "Motion",
    description:
      "Official @beui/animated-toast-stack spring-blur portal list with swipe-to-dismiss, icon popLayout, and pill CTA.",
  },
  {
    id: "beui-alert",
    title: "Official @beui/alert Callout Toast",
    shortLabel: "Alert",
    description:
      "Renders live notifications using @beui/alert semantic motion surfaces, animated icons, and spring close controls.",
  },
  {
    id: "compact-pills",
    title: "Compact Telemetry Capsule",
    shortLabel: "Pills",
    description:
      "Sleek single-line floating stadium capsule keeping workflow, skill, and curator hits crisp and compact.",
  },
  {
    id: "dynamic-island",
    title: "Dynamic Island HUD",
    shortLabel: "Island",
    description:
      "High-contrast obsidian HUD capsule with live semantic pulse ring and translucent quick-jump pill.",
  },
  {
    id: "minimal-glass",
    title: "Frosted Minimal Glass",
    shortLabel: "Glass",
    description:
      "Airy backdrop-blurred glassmorphism surface with subtle specular ring and inline link CTA.",
  },
  {
    id: "accent-banner",
    title: "Semantic Left-Rail Callout",
    shortLabel: "Rail",
    description:
      "Structured architectural callout card with a bold category left rail for instant visual scanning.",
  },
  {
    id: "terminal-hud",
    title: "Terminal Monospace Strip",
    shortLabel: "Terminal",
    description:
      "Developer CLI telemetry strip with monospace prompt tags, command chevron, and bracketed jump action.",
  },
  {
    id: "split-action",
    title: "Split Two-Column Action Card",
    shortLabel: "Split",
    description:
      "Structured two-column layout pairing telemetry details on the left with stacked Action / Dismiss cells on the right.",
  },
  {
    id: "soft-tinted",
    title: "Soft Semantic Surface",
    shortLabel: "Tinted",
    description:
      "Tonal category-tinted surface and border delivering warm semantic hierarchy across Light and Dark themes.",
  },
  {
    id: "brutalist-outline",
    title: "Neo-Brutalist Offset Frame",
    shortLabel: "Brutalist",
    description:
      "Sharp 2px architectural border with crisp offset hard shadow and solid category stamp badge.",
  },
  {
    id: "micro-badge",
    title: "Ultra-Minimal Status Chip",
    shortLabel: "Micro",
    description:
      "Featherweight single-line status chip with semantic indicator dot and minimal viewport footprint.",
  },
  {
    id: "rare-toast",
    title: "Rare-UI Motion Capsule Toast",
    shortLabel: "Rare UI",
    description:
      "swamimalode07/rare-ui/toast — Smooth monochrome hairline capsule with subtle primary dot and quiet dismiss.",
  },
]

const VALID_TOAST_VARIANTS: Readonly<Record<BeuiToastVariant, true>> = {
  "beui-stack": true,
  "beui-motion": true,
  "beui-alert": true,
  "compact-pills": true,
  "dynamic-island": true,
  "minimal-glass": true,
  "accent-banner": true,
  "terminal-hud": true,
  "split-action": true,
  "soft-tinted": true,
  "brutalist-outline": true,
  "micro-badge": true,
  "rare-toast": true,
}

export function normalizeToastVariant(raw: unknown): BeuiToastVariant {
  if (typeof raw === "string" && raw in VALID_TOAST_VARIANTS) {
    return raw as BeuiToastVariant
  }
  return "beui-stack"
}

export const TOAST_POSITION_OPTIONS: ReadonlyArray<
  BeuiToastOptionDescriptor<BeuiCanonicalToastPosition>
> = [
  {
    id: "top-start",
    title: "Top Start",
    shortLabel: "Top Start",
    description: "Anchored in the upper-left corner of the workspace.",
  },
  {
    id: "top-center",
    title: "Top Center",
    shortLabel: "Top Center",
    description: "Centered at the top of the viewport like a Dynamic Island HUD.",
  },
  {
    id: "top-end",
    title: "Top End",
    shortLabel: "Top End",
    description: "Anchored in the upper-right corner below workspace quick actions.",
  },
  {
    id: "bottom-start",
    title: "Bottom Start",
    shortLabel: "Bottom Start",
    description: "Anchored in the lower-left corner with clean composer clearance.",
  },
  {
    id: "bottom-center",
    title: "Bottom Center",
    shortLabel: "Bottom Center",
    description: "Centered above the prompt composer in the lower viewport.",
  },
  {
    id: "bottom-end",
    title: "Bottom End",
    shortLabel: "Bottom End",
    description: "Anchored in the lower-right corner cleanly above composer controls.",
    isDefault: true,
  },
]

const TOAST_POSITION_ALIASES: Readonly<
  Record<string, BeuiCanonicalToastPosition>
> = {
  "top-start": "top-start",
  "top-left": "top-start",
  "top-center": "top-center",
  "top-end": "top-end",
  "top-right": "top-end",
  "bottom-start": "bottom-start",
  "bottom-left": "bottom-start",
  "bottom-center": "bottom-center",
  "bottom-end": "bottom-end",
  "bottom-right": "bottom-end",
}

export function normalizeToastPosition(
  raw: unknown
): BeuiCanonicalToastPosition {
  if (typeof raw !== "string") return "bottom-end"
  const key = raw.trim().toLowerCase()
  return TOAST_POSITION_ALIASES[key] ?? "bottom-end"
}

export const TOAST_DURATION_OPTIONS: ReadonlyArray<
  BeuiToastOptionDescriptor<BeuiToastDurationPreset>
> = [
  {
    id: "fast",
    title: "Snappy (3.2s)",
    shortLabel: "3.2s",
    description: "Quick transient telemetry pulse for high-velocity runs.",
  },
  {
    id: "standard",
    title: "Balanced (5.0s)",
    shortLabel: "5.0s",
    description: "Comfortable reading window with hover-to-pause protection.",
    isDefault: true,
  },
  {
    id: "extended",
    title: "Extended (8.0s)",
    shortLabel: "8.0s",
    description: "Longer dwell time so background curator & workflow notices remain visible.",
  },
  {
    id: "sticky",
    title: "Sticky (Manual)",
    shortLabel: "Sticky",
    description: "Stays pinned in the stack until swiped away or dismissed.",
  },
]

export const TOAST_MAX_VISIBLE_OPTIONS: ReadonlyArray<
  BeuiToastOptionDescriptor<BeuiToastMaxVisible>
> = [
  {
    id: 3,
    title: "3 Cards",
    shortLabel: "3",
    description: "Tight three-card deck.",
  },
  {
    id: 4,
    title: "4 Cards",
    shortLabel: "4",
    description: "Balanced four-card stack depth.",
    isDefault: true,
  },
  {
    id: 5,
    title: "5 Cards",
    shortLabel: "5",
    description: "Deep five-card telemetry deck.",
  },
]

export const INLINE_NOTICE_STYLE_OPTIONS: ReadonlyArray<
  BeuiToastOptionDescriptor<BeuiInlineNoticeStyle>
> = [
  {
    id: "minimal-whisper",
    title: "Minimal Whisper Row (Default)",
    shortLabel: "Minimal",
    description:
      "Borderless low-noise telemetry line with a subtle primary dot and hover jump — zero heavy pill borders.",
    isDefault: true,
  },
  {
    id: "memory-chips",
    title: "Assistant-UI Memory Chips",
    shortLabel: "Memory Chips",
    description:
      "assistant-ui/elements/memory-chips — Neutral hairline chips harmonized cleanly with your active theme.",
  },
  {
    id: "tool-chips",
    title: "BeautifulUI Tool Chips",
    shortLabel: "Tool Chips",
    description:
      "beautifului.dev/r/tool-chips — Compact monospace capsule tags summarizing runtime actions.",
  },
  {
    id: "ai-status",
    title: "Rare-UI Inline AI Status",
    shortLabel: "AI Status",
    description:
      "swamimalode07/rare-ui/ai-status — Soft status capsule with subtle live dot indicator.",
  },
  {
    id: "pills",
    title: "Theme Hairline Pills",
    shortLabel: "Pills",
    description:
      "Refined rounded-full badges using your active theme surface without clashing neon borders.",
  },
  {
    id: "cards",
    title: "Structured Callout Cards",
    shortLabel: "Cards",
    description:
      "Bordered telemetry rows with category tags, details, and quick-jump buttons.",
  },
  {
    id: "toast-only",
    title: "Hidden Inline (Toast & Inbox Only)",
    shortLabel: "Toast Only",
    description:
      "Hides inline chat badges completely and routes telemetry exclusively to Toast Stack & Notifications.",
  },
]

const VERTICAL_SIGN_BY_POSITION: Record<BeuiCanonicalToastPosition, 1 | -1> = {
  "top-start": 1,
  "top-center": 1,
  "top-end": 1,
  "bottom-start": -1,
  "bottom-center": -1,
  "bottom-end": -1,
}

const POSITION_CONTAINER_CLASSES: Record<BeuiCanonicalToastPosition, string> = {
  "top-start": "top-14 left-4 items-start",
  "top-center": "top-4 left-1/2 -translate-x-1/2 items-center",
  "top-end": "top-14 right-4 items-end md:right-14",
  "bottom-start": "bottom-20 left-4 items-start",
  "bottom-center": "bottom-24 left-1/2 -translate-x-1/2 items-center",
  "bottom-end": "bottom-20 right-4 items-end",
}

const POSITION_ANCHOR_ORIGIN_CLASSES: Record<
  BeuiCanonicalToastPosition,
  string
> = {
  "top-start": "top-0 left-0 origin-top-left",
  "top-center": "top-0 left-1/2 -translate-x-1/2 origin-top",
  "top-end": "top-0 right-0 origin-top-right",
  "bottom-start": "bottom-0 left-0 origin-bottom-left",
  "bottom-center": "bottom-0 left-1/2 -translate-x-1/2 origin-bottom",
  "bottom-end": "bottom-0 right-0 origin-bottom-right",
}

const SWIPE_DISMISS_THRESHOLD_PX = 80
const EXPANDED_STACK_GAP_PX = 8
const DEFAULT_CARD_HEIGHT_PX = 76

export interface ToastStackGeometry {
  scale: number
  translateY: number
  opacity: number
  zIndex: number
  hidden: boolean
}

export function resolveToastDurationMs(
  preset: BeuiToastDurationPreset,
  explicitDurationMs?: number
): number {
  if (typeof explicitDurationMs === "number" && explicitDurationMs >= 0) {
    return explicitDurationMs
  }
  return TOAST_DURATION_MS[preset]
}

export function computeCumulativeExpandedOffsets(
  heights: ReadonlyArray<number>,
  gapPx = EXPANDED_STACK_GAP_PX
): number[] {
  const offsets: number[] = []
  let running = 0
  for (let i = 0; i < heights.length; i += 1) {
    offsets.push(running)
    const h = heights[i] > 0 ? heights[i] : DEFAULT_CARD_HEIGHT_PX
    running += h + gapPx
  }
  return offsets
}

export function computeToastStackGeometry(params: {
  index: number
  isExpanded: boolean
  expandedOffsetPx: number
  position: BeuiToastPosition
  maxVisible: number
}): ToastStackGeometry {
  const { index, isExpanded, expandedOffsetPx, position, maxVisible } = params
  const canonical = normalizeToastPosition(position)
  const sign = VERTICAL_SIGN_BY_POSITION[canonical]
  const clampedMax = Math.max(1, maxVisible)
  const hidden = index >= clampedMax

  if (hidden) {
    return {
      scale: Number(Math.max(0.76, 1 - clampedMax * 0.048).toFixed(4)),
      translateY: sign * clampedMax * 11,
      opacity: 0,
      zIndex: Math.max(1, 60 - index),
      hidden: true,
    }
  }

  if (isExpanded) {
    return {
      scale: 1,
      translateY: sign * expandedOffsetPx,
      opacity: 1,
      zIndex: 60 - index,
      hidden: false,
    }
  }

  return {
    scale: Number(Math.max(0.84, 1 - index * 0.048).toFixed(4)),
    translateY: sign * index * 11,
    opacity: Number(Math.max(0.22, 1 - index * 0.2).toFixed(4)),
    zIndex: 60 - index,
    hidden: false,
  }
}

function formatShortTime(timestamp: number): string {
  const d = new Date(timestamp)
  if (Number.isNaN(d.getTime())) return ""
  return d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function resolveToastVariantSurfaceClass(
  variant: BeuiToastVariant,
  meta: BeuiToastCategoryMeta
): string {
  const map: Record<BeuiToastVariant, string> = {
    "beui-stack": cn(
      "w-[344px] max-w-[calc(100vw-2rem)] rounded-2xl border bg-card/96 p-3 shadow-xl shadow-foreground/10 backdrop-blur-md",
      meta.cardBorderClass
    ),
    "beui-motion":
      "w-[344px] max-w-[calc(100vw-2rem)] rounded-2xl border border-border bg-card/95 p-3 shadow-2xl backdrop-blur-xl",
    "beui-alert":
      "w-[348px] max-w-[calc(100vw-2rem)] rounded-xl bg-transparent p-0 shadow-xl backdrop-blur-xl",
    "compact-pills": cn(
      "w-[336px] max-w-[calc(100vw-2rem)] rounded-full border bg-card/96 px-3 py-2 shadow-lg shadow-foreground/10 backdrop-blur-md",
      meta.cardBorderClass
    ),
    "dynamic-island":
      "w-[344px] max-w-[calc(100vw-2rem)] rounded-[22px] border border-zinc-800 bg-zinc-950/96 px-3.5 py-2.5 text-zinc-50 shadow-2xl backdrop-blur-xl",
    "minimal-glass":
      "w-[344px] max-w-[calc(100vw-2rem)] rounded-2xl border border-border/55 bg-background/68 p-3 shadow-xl ring-1 ring-inset ring-foreground/8 backdrop-blur-2xl",
    "accent-banner": cn(
      "w-[348px] max-w-[calc(100vw-2rem)] rounded-xl border border-border/80 border-l-4 bg-card p-3 shadow-xl",
      meta.railClass
    ),
    "terminal-hud":
      "w-[348px] max-w-[calc(100vw-2rem)] rounded-lg border border-zinc-800 bg-zinc-950/96 px-3 py-2.5 font-mono text-zinc-100 shadow-xl",
    "split-action": cn(
      "w-[348px] max-w-[calc(100vw-2rem)] rounded-2xl border bg-card/98 p-0 shadow-xl backdrop-blur-md",
      meta.cardBorderClass
    ),
    "soft-tinted": cn(
      "w-[344px] max-w-[calc(100vw-2rem)] rounded-2xl border p-3 shadow-lg backdrop-blur-md",
      meta.softSurfaceClass
    ),
    "brutalist-outline":
      "w-[344px] max-w-[calc(100vw-2rem)] rounded-none border-2 border-foreground/85 bg-card p-3 shadow-[4px_4px_0px_0px_rgba(15,23,42,0.32)] dark:shadow-[4px_4px_0px_0px_rgba(248,250,252,0.22)]",
    "micro-badge": cn(
      "w-[308px] max-w-[calc(100vw-2rem)] rounded-xl border bg-card/95 px-2.5 py-1.5 shadow-md backdrop-blur-md",
      meta.cardBorderClass
    ),
    "rare-toast":
      "w-[336px] max-w-[calc(100vw-2rem)] rounded-full border border-border/65 bg-card/95 px-3.5 py-2 shadow-xl backdrop-blur-xl",
  }
  return map[variant]
}

const PROGRESS_DISABLED_VARIANTS: Readonly<Partial<Record<BeuiToastVariant, true>>> = {
  "beui-motion": true,
  "beui-alert": true,
  "compact-pills": true,
  "dynamic-island": true,
  "micro-badge": true,
  "rare-toast": true,
}

interface ToastVariantBodyProps {
  item: BeuiToastItem
  meta: BeuiToastCategoryMeta
  Icon: IconComponent
  actionLabel?: string
  timeText: string
  onTriggerAction: () => void
  onDismiss: () => void
}

const StandardCardBody: React.FC<
  ToastVariantBodyProps & {
    linkAction?: boolean
    tonalSurface?: boolean
  }
> = ({
  item,
  meta,
  Icon,
  actionLabel,
  timeText,
  onTriggerAction,
  onDismiss,
  linkAction = false,
  tonalSurface = false,
}) => (
  <div className="flex items-start gap-2.5">
    <span
      className={cn(
        "mt-0.5 inline-flex size-7 shrink-0 items-center justify-center rounded-xl border",
        tonalSurface ? "border-border/50 bg-background/75 text-foreground" : meta.iconWrapClass
      )}
    >
      <Icon className="size-3.5" />
    </span>

    <div className="min-w-0 flex-1 space-y-1">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-1.5">
          <span
            className={cn(
              "rounded px-1.5 py-0.5 font-mono text-[9px] font-semibold tracking-wider",
              tonalSurface
                ? "bg-background/70 text-foreground"
                : "bg-muted/80 text-muted-foreground"
            )}
          >
            {item.badge ?? meta.shortTag}
          </span>
          {item.count && item.count > 1 && (
            <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[9px]">
              ×{item.count}
            </Badge>
          )}
          <span className="text-[10px] text-muted-foreground/80">{timeText}</span>
        </div>

        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss notification"
          className="inline-flex size-5 shrink-0 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted/70 hover:text-foreground"
        >
          <X className="size-3" />
        </button>
      </div>

      <div className="text-xs font-semibold leading-snug text-foreground">
        {item.title}
      </div>

      {item.detail && (
        <div className="line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">
          {item.detail}
        </div>
      )}

      {item.onAction && actionLabel && (
        <div className="pt-1">
          <button
            type="button"
            onClick={onTriggerAction}
            className={
              linkAction
                ? "inline-flex items-center gap-1 text-[10.5px] font-semibold text-primary underline-offset-4 transition hover:underline"
                : cn(
                    "inline-flex items-center gap-1 rounded-lg border px-2 py-0.5 text-[10px] font-semibold transition",
                    tonalSurface
                      ? "border-border/60 bg-background/80 text-foreground hover:border-primary/45 hover:text-primary"
                      : "border-border/75 bg-muted/55 text-foreground hover:border-primary/45 hover:bg-primary/10 hover:text-primary"
                  )
            }
          >
            <span>{actionLabel}</span>
            <ArrowUpRight className="size-2.5" />
          </button>
        </div>
      )}
    </div>
  </div>
)

const CompactPillsBody: React.FC<ToastVariantBodyProps> = ({
  item,
  meta,
  Icon,
  actionLabel,
  onTriggerAction,
  onDismiss,
}) => (
  <div className="flex items-center gap-2.5">
    <span
      className={cn(
        "inline-flex size-6 shrink-0 items-center justify-center rounded-full border",
        meta.iconWrapClass
      )}
    >
      <Icon className="size-3.5" />
    </span>

    <div className="min-w-0 flex-1">
      <div className="flex items-center gap-1.5 truncate text-xs font-semibold text-foreground">
        <span className="truncate">{item.title}</span>
        {item.detail && (
          <span className="truncate font-normal text-muted-foreground">
            · {item.detail}
          </span>
        )}
      </div>
    </div>

    {item.count && item.count > 1 && (
      <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[10px]">
        ×{item.count}
      </Badge>
    )}

    {item.onAction && actionLabel && (
      <button
        type="button"
        onClick={onTriggerAction}
        className="inline-flex shrink-0 items-center gap-0.5 rounded-full bg-primary/12 px-2 py-0.5 text-[10px] font-semibold text-primary transition hover:bg-primary/20"
      >
        <span>{actionLabel}</span>
        <ArrowUpRight className="size-2.5" />
      </button>
    )}

    <button
      type="button"
      onClick={onDismiss}
      aria-label="Dismiss notification"
      className="inline-flex size-5 shrink-0 items-center justify-center rounded-full text-muted-foreground transition hover:bg-muted hover:text-foreground"
    >
      <X className="size-3" />
    </button>
  </div>
)

const DynamicIslandBody: React.FC<ToastVariantBodyProps> = ({
  item,
  meta,
  Icon,
  actionLabel,
  onTriggerAction,
  onDismiss,
}) => (
  <div className="flex items-center gap-2.5">
    <span className="relative inline-flex size-7 shrink-0 items-center justify-center rounded-full bg-zinc-900 ring-1 ring-zinc-700/80">
      <Icon className="size-3.5 text-zinc-100" />
      <span
        className={cn(
          "absolute -top-0.5 -right-0.5 size-2 rounded-full ring-2 ring-zinc-950",
          meta.dotClass
        )}
      />
    </span>

    <div className="min-w-0 flex-1">
      <div className="flex items-center gap-1.5">
        <span className="font-mono text-[9px] font-semibold uppercase tracking-widest text-zinc-400">
          {item.badge ?? meta.shortTag}
        </span>
        {item.count && item.count > 1 && (
          <span className="rounded-full bg-zinc-800 px-1.5 py-0 font-mono text-[9px] text-zinc-200">
            ×{item.count}
          </span>
        )}
      </div>
      <div className="truncate text-xs font-semibold text-zinc-50">
        {item.title}
      </div>
      {item.detail && (
        <div className="truncate text-[10.5px] text-zinc-400">{item.detail}</div>
      )}
    </div>

    {item.onAction && actionLabel && (
      <button
        type="button"
        onClick={onTriggerAction}
        className="inline-flex shrink-0 items-center gap-1 rounded-full bg-zinc-100 px-2.5 py-1 text-[10px] font-semibold text-zinc-950 transition hover:bg-white"
      >
        <span>{actionLabel}</span>
        <ArrowUpRight className="size-2.5" />
      </button>
    )}

    <button
      type="button"
      onClick={onDismiss}
      aria-label="Dismiss notification"
      className="inline-flex size-5 shrink-0 items-center justify-center rounded-full text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-100"
    >
      <X className="size-3" />
    </button>
  </div>
)

const TerminalHudBody: React.FC<ToastVariantBodyProps> = ({
  item,
  meta,
  actionLabel,
  timeText,
  onTriggerAction,
  onDismiss,
}) => (
  <div className="space-y-1">
    <div className="flex items-center justify-between gap-2 text-[10px]">
      <div className="flex items-center gap-1.5 truncate">
        <Terminal className="size-3 shrink-0 text-emerald-400" />
        <span className="font-bold text-emerald-400">
          [{item.badge ?? meta.shortTag}]
        </span>
        <span className="text-zinc-500">{timeText}</span>
        {item.count && item.count > 1 && (
          <span className="text-amber-300">×{item.count}</span>
        )}
      </div>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss notification"
        className="inline-flex size-4 shrink-0 items-center justify-center text-zinc-500 transition hover:text-zinc-100"
      >
        <X className="size-3" />
      </button>
    </div>

    <div className="flex items-start gap-1.5 text-[11px] leading-snug text-zinc-100">
      <span className="text-emerald-400">&gt;</span>
      <div className="min-w-0 flex-1">
        <span className="font-semibold">{item.title}</span>
        {item.detail && (
          <span className="block truncate text-[10.5px] text-zinc-400">
            {item.detail}
          </span>
        )}
      </div>
    </div>

    {item.onAction && actionLabel && (
      <div className="pt-0.5">
        <button
          type="button"
          onClick={onTriggerAction}
          className="inline-flex items-center gap-1 text-[10px] font-semibold text-emerald-400 transition hover:text-emerald-300 hover:underline"
        >
          <span>[{actionLabel} ↗]</span>
        </button>
      </div>
    )}
  </div>
)

const SplitActionBody: React.FC<ToastVariantBodyProps> = ({
  item,
  meta,
  Icon,
  actionLabel,
  onTriggerAction,
  onDismiss,
}) => (
  <div className="grid grid-cols-[1fr_92px] items-stretch">
    <div className="flex min-w-0 items-start gap-2.5 p-3">
      <span
        className={cn(
          "mt-0.5 inline-flex size-6 shrink-0 items-center justify-center rounded-lg border",
          meta.iconWrapClass
        )}
      >
        <Icon className="size-3.5" />
      </span>
      <div className="min-w-0 flex-1 space-y-0.5">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
            {item.badge ?? meta.shortTag}
          </span>
          {item.count && item.count > 1 && (
            <span className="font-mono text-[9px] text-primary">×{item.count}</span>
          )}
        </div>
        <div className="truncate text-xs font-semibold text-foreground">
          {item.title}
        </div>
        {item.detail && (
          <div className="line-clamp-2 text-[10.5px] leading-snug text-muted-foreground">
            {item.detail}
          </div>
        )}
      </div>
    </div>

    <div className="flex flex-col divide-y divide-border/65 border-l border-border/65 bg-muted/20">
      {item.onAction && actionLabel ? (
        <button
          type="button"
          onClick={onTriggerAction}
          className="flex flex-1 items-center justify-center px-2 py-2 text-center text-[10.5px] font-semibold text-primary transition hover:bg-primary/12"
        >
          <span className="line-clamp-1">{actionLabel}</span>
        </button>
      ) : null}
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss notification"
        className="flex flex-1 items-center justify-center px-2 py-2 text-[10.5px] font-medium text-muted-foreground transition hover:bg-muted/60 hover:text-foreground"
      >
        Dismiss
      </button>
    </div>
  </div>
)

const BrutalistOutlineBody: React.FC<ToastVariantBodyProps> = ({
  item,
  meta,
  Icon,
  actionLabel,
  timeText,
  onTriggerAction,
  onDismiss,
}) => (
  <div className="space-y-1.5">
    <div className="flex items-center justify-between gap-2">
      <div className="flex items-center gap-1.5">
        <span
          className={cn(
            "inline-flex items-center gap-1 px-1.5 py-0.5 font-mono text-[9px] font-bold uppercase tracking-wider",
            meta.stampClass
          )}
        >
          <Icon className="size-2.5" />
          <span>{item.badge ?? meta.shortTag}</span>
        </span>
        <span className="font-mono text-[9.5px] text-muted-foreground">
          {timeText}
        </span>
      </div>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss notification"
        className="inline-flex size-5 items-center justify-center border border-foreground/50 text-foreground transition hover:bg-foreground hover:text-background"
      >
        <X className="size-3" />
      </button>
    </div>

    <div className="text-xs font-bold tracking-tight text-foreground">
      {item.title}
      {item.count && item.count > 1 ? ` [×${item.count}]` : ""}
    </div>

    {item.detail && (
      <div className="line-clamp-2 text-[11px] leading-snug text-muted-foreground">
        {item.detail}
      </div>
    )}

    {item.onAction && actionLabel && (
      <div className="pt-0.5">
        <button
          type="button"
          onClick={onTriggerAction}
          className="inline-flex items-center gap-1 border border-foreground/80 bg-background px-2 py-0.5 font-mono text-[10px] font-bold uppercase tracking-wide text-foreground transition hover:bg-foreground hover:text-background"
        >
          <span>{actionLabel}</span>
          <ArrowUpRight className="size-2.5" />
        </button>
      </div>
    )}
  </div>
)

const MicroBadgeBody: React.FC<ToastVariantBodyProps> = ({
  item,
  meta,
  actionLabel,
  onTriggerAction,
  onDismiss,
}) => (
  <div className="flex items-center gap-2">
    <span className={cn("size-2 shrink-0 rounded-full", meta.dotClass)} />
    <span className="font-mono text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
      {item.badge ?? meta.shortTag}
    </span>
    <span className="min-w-0 flex-1 truncate text-[11.5px] font-semibold text-foreground">
      {item.title}
      {item.detail ? (
        <span className="font-normal text-muted-foreground"> · {item.detail}</span>
      ) : null}
    </span>
    {item.onAction && actionLabel && (
      <button
        type="button"
        onClick={onTriggerAction}
        title={actionLabel}
        className="inline-flex size-5 shrink-0 items-center justify-center rounded-md text-primary transition hover:bg-primary/12"
      >
        <ArrowUpRight className="size-3" />
      </button>
    )}
    <button
      type="button"
      onClick={onDismiss}
      aria-label="Dismiss notification"
      className="inline-flex size-5 shrink-0 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground"
    >
      <X className="size-3" />
    </button>
  </div>
)

function mapItemToAlertTone(item: BeuiToastItem): AlertVariant {
  if (item.status === "error" || item.category === "error" || item.category === "security") {
    return "destructive"
  }
  if (item.status === "warning") return "warning"
  if (item.status === "success" || item.category === "self_improvement") return "success"
  return "info"
}

const BeuiMotionPreviewBody: React.FC<ToastVariantBodyProps> = ({
  item,
  Icon,
  actionLabel,
  onTriggerAction,
  onDismiss,
}) => (
  <div className={cn("flex gap-3", item.detail || actionLabel ? "items-start" : "items-center")}>
    <span className="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-muted text-primary">
      <Icon className="h-3.5 w-3.5" />
    </span>
    <div className="min-w-0 flex-1">
      <p className="truncate text-sm font-medium leading-5 text-foreground">
        {item.title}
        {item.count && item.count > 1 ? ` (×${item.count})` : ""}
      </p>
      {item.detail && (
        <p className="mt-0.5 line-clamp-2 text-xs leading-4 text-muted-foreground">
          {item.detail}
        </p>
      )}
      {item.onAction && actionLabel && (
        <button
          type="button"
          onClick={onTriggerAction}
          className="mt-2 inline-flex h-7 items-center rounded-full bg-muted/80 px-3 text-xs font-medium text-foreground transition-colors hover:bg-muted"
        >
          {actionLabel}
        </button>
      )}
    </div>
    <button
      type="button"
      onClick={onDismiss}
      aria-label="Dismiss toast"
      className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted/80 hover:text-foreground"
    >
      <X className="h-3.5 w-3.5" />
    </button>
  </div>
)

const BeuiAlertToastBody: React.FC<ToastVariantBodyProps> = ({
  item,
  Icon,
  actionLabel,
  onTriggerAction,
  onDismiss,
}) => {
  const tone = mapItemToAlertTone(item)
  return (
    <OfficialBeuiAlert
      variant={tone}
      onOpenChange={(open) => {
        if (!open) onDismiss()
      }}
      className="border border-border/65 bg-card/92 p-3 shadow-none backdrop-blur-xl"
    >
      <AlertIcon>
        <Icon className="size-4" />
      </AlertIcon>
      <AlertContent className="space-y-0.5">
        <AlertTitle className="text-xs font-semibold">
          {item.title}
          {item.count && item.count > 1 ? ` (×${item.count})` : ""}
        </AlertTitle>
        {item.detail && (
          <AlertDescription className="line-clamp-2 text-[11px]">
            {item.detail}
          </AlertDescription>
        )}
        {item.onAction && actionLabel && (
          <AlertAction className="pt-1.5">
            <button
              type="button"
              onClick={onTriggerAction}
              className="inline-flex items-center gap-1 rounded-md border border-border/70 bg-background/75 px-2 py-0.5 text-[10px] font-semibold text-foreground transition hover:border-primary/45 hover:text-primary"
            >
              <span>{actionLabel}</span>
              <ArrowUpRight className="size-2.5" />
            </button>
          </AlertAction>
        )}
      </AlertContent>
      <AlertClose />
    </OfficialBeuiAlert>
  )
}

const OFFICIAL_MOTION_POSITION_MAP: Record<
  BeuiCanonicalToastPosition,
  OfficialMotionToastPosition
> = {
  "top-start": "top-left",
  "top-center": "top-center",
  "top-end": "top-right",
  "bottom-start": "bottom-left",
  "bottom-center": "bottom-center",
  "bottom-end": "bottom-right",
}

function mapItemToOfficialMotionStatus(item: BeuiToastItem): OfficialMotionToastStatus {
  if (item.status === "error" || item.category === "error") return "error"
  if (item.status === "success" || item.category === "self_improvement") return "success"
  if (item.status === "running") return "loading"
  return "info"
}

const TOAST_VARIANT_RENDERERS: Record<
  BeuiToastVariant,
  React.FC<ToastVariantBodyProps>
> = {
  "beui-stack": (props) => <StandardCardBody {...props} />,
  "beui-motion": BeuiMotionPreviewBody,
  "beui-alert": BeuiAlertToastBody,
  "compact-pills": CompactPillsBody,
  "dynamic-island": DynamicIslandBody,
  "minimal-glass": (props) => <StandardCardBody {...props} linkAction />,
  "accent-banner": (props) => <StandardCardBody {...props} linkAction />,
  "terminal-hud": TerminalHudBody,
  "split-action": SplitActionBody,
  "soft-tinted": (props) => <StandardCardBody {...props} tonalSurface />,
  "brutalist-outline": BrutalistOutlineBody,
  "micro-badge": MicroBadgeBody,
  "rare-toast": CompactPillsBody,
}

export interface AnimatedToastStackProps {
  toasts: ReadonlyArray<BeuiToastItem>
  onDismiss: (id: string) => void
  onClearAll?: () => void
  onOpenOptions?: () => void
  variant?: BeuiToastVariant
  position?: BeuiToastPosition
  durationPreset?: BeuiToastDurationPreset
  maxVisible?: BeuiToastMaxVisible
  previewStatic?: boolean
  className?: string
}

interface ToastCardItemProps {
  item: BeuiToastItem
  index: number
  geometry: ToastStackGeometry
  variant: BeuiToastVariant
  position: BeuiCanonicalToastPosition
  durationMs: number
  isPaused: boolean
  previewStatic: boolean
  onDismiss: (id: string) => void
  onMeasureHeight: (id: string, height: number) => void
}

const ToastCardItem: React.FC<ToastCardItemProps> = ({
  item,
  index,
  geometry,
  variant,
  position,
  durationMs,
  isPaused,
  previewStatic,
  onDismiss,
  onMeasureHeight,
}) => {
  const cardRef = React.useRef<HTMLDivElement | null>(null)
  const dragStartXRef = React.useRef<number | null>(null)
  const [dragX, setDragX] = React.useState(0)
  const [isDragging, setIsDragging] = React.useState(false)

  const resolvedVariant = normalizeToastVariant(variant)
  const meta = TOAST_CATEGORY_META[item.category] ?? TOAST_CATEGORY_META.system
  const Icon =
    item.status === "success"
      ? CheckCircle2
      : item.status === "error"
        ? AlertTriangle
        : meta.icon

  React.useLayoutEffect(() => {
    const el = cardRef.current
    if (!el || previewStatic) return undefined
    const report = () => {
      const next = el.getBoundingClientRect().height
      if (next > 0) onMeasureHeight(item.id, Math.round(next))
    }
    report()
    if (typeof ResizeObserver === "undefined") return undefined
    const observer = new ResizeObserver(report)
    observer.observe(el)
    return () => observer.disconnect()
  }, [item.id, item.title, item.detail, resolvedVariant, previewStatic, onMeasureHeight])

  React.useEffect(() => {
    if (previewStatic || durationMs <= 0 || isPaused || geometry.hidden) {
      return undefined
    }
    const timer = globalThis.setTimeout(() => {
      onDismiss(item.id)
    }, durationMs)
    return () => globalThis.clearTimeout(timer)
  }, [
    durationMs,
    geometry.hidden,
    isPaused,
    item.id,
    item.count,
    item.createdAt,
    onDismiss,
    previewStatic,
  ])

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (previewStatic) return
    const target = e.target as HTMLElement | null
    if (target?.closest("button")) return
    dragStartXRef.current = e.clientX
    setIsDragging(true)
    e.currentTarget.setPointerCapture?.(e.pointerId)
  }

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (dragStartXRef.current === null) return
    setDragX(e.clientX - dragStartXRef.current)
  }

  const handlePointerEnd = () => {
    if (dragStartXRef.current === null) return
    const finalDelta = dragX
    dragStartXRef.current = null
    setIsDragging(false)
    if (Math.abs(finalDelta) >= SWIPE_DISMISS_THRESHOLD_PX) {
      onDismiss(item.id)
      return
    }
    setDragX(0)
  }

  const dragFade = Math.max(0.25, 1 - Math.abs(dragX) / 220)
  const combinedOpacity = Number((geometry.opacity * dragFade).toFixed(4))
  const isCentered = position === "top-center" || position === "bottom-center"
  const transformValue = isCentered
    ? `translate3d(calc(-50% + ${dragX}px), ${geometry.translateY}px, 0) scale(${geometry.scale})`
    : `translate3d(${dragX}px, ${geometry.translateY}px, 0) scale(${geometry.scale})`

  const actionLabel =
    item.actionLabel ?? (item.onAction ? meta.defaultActionLabel ?? "Inspect" : undefined)
  const BodyRenderer = TOAST_VARIANT_RENDERERS[resolvedVariant]
  const showProgressBar =
    !PROGRESS_DISABLED_VARIANTS[resolvedVariant] &&
    !previewStatic &&
    durationMs > 0 &&
    index === 0

  return (
    <div
      ref={cardRef}
      role="status"
      aria-live={!previewStatic && index === 0 ? "polite" : "off"}
      aria-hidden={geometry.hidden}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerEnd}
      onPointerCancel={handlePointerEnd}
      style={
        previewStatic
          ? undefined
          : {
              transform: transformValue,
              opacity: combinedOpacity,
              zIndex: geometry.zIndex,
              transition: isDragging
                ? "none"
                : "transform 280ms var(--t-ease-spring), opacity 220ms var(--t-ease-out)",
              pointerEvents: geometry.hidden ? "none" : "auto",
            }
      }
      className={cn(
        previewStatic ? "relative" : "absolute",
        !previewStatic && POSITION_ANCHOR_ORIGIN_CLASSES[position],
        "touch-pan-y select-none overflow-hidden",
        resolveToastVariantSurfaceClass(resolvedVariant, meta)
      )}
    >
      <BodyRenderer
        item={item}
        meta={meta}
        Icon={Icon}
        actionLabel={actionLabel}
        timeText={formatShortTime(item.createdAt)}
        onTriggerAction={() => {
          item.onAction?.()
          onDismiss(item.id)
        }}
        onDismiss={() => onDismiss(item.id)}
      />

      {showProgressBar && (
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-[2px] bg-muted/40">
          <div
            key={`${item.id}-${item.count ?? 1}`}
            style={{
              animationDuration: `${durationMs}ms`,
              animationPlayState: isPaused ? "paused" : "running",
            }}
            className={cn("beui-toast-progress h-full w-full", meta.progressClass)}
          />
        </div>
      )}
    </div>
  )
}

const DEFAULT_STYLE_PREVIEW_ITEM: BeuiToastItem = {
  id: "style-preview-card",
  category: "skill",
  status: "info",
  title: "Skill used",
  detail: "caveman, openz_fullstack_playbook",
  createdAt: 1_763_150_144_000,
  actionLabel: "Open Skills",
  onAction: () => {},
}

export interface BeuiToastStylePreviewProps {
  variant: BeuiToastVariant
  item?: BeuiToastItem
  className?: string
}

/**
 * Renders a live single-card component specimen for any of the 10 `BeuiToastVariant` styles.
 */
export const BeuiToastStylePreview: React.FC<BeuiToastStylePreviewProps> = ({
  variant,
  item = DEFAULT_STYLE_PREVIEW_ITEM,
  className,
}) => (
  <div
    className={cn(
      "pointer-events-none flex w-full items-center justify-center py-1",
      className
    )}
  >
    <ToastCardItem
      item={item}
      index={0}
      geometry={{
        scale: 1,
        translateY: 0,
        opacity: 1,
        zIndex: 1,
        hidden: false,
      }}
      variant={variant}
      position="bottom-end"
      durationMs={0}
      isPaused
      previewStatic
      onDismiss={() => {}}
      onMeasureHeight={() => {}}
    />
  </div>
)

/**
 * `@beui/animated-toast-stack` — Layered spring notification deck with 10 distinct UI component styles,
 * 6 viewport anchor placements, hover fan-out, swipe-to-dismiss, and actionable agent telemetry CTAs.
 */
export const AnimatedToastStack: React.FC<AnimatedToastStackProps> = ({
  toasts,
  onDismiss,
  onClearAll,
  onOpenOptions,
  variant = "beui-stack",
  position = "bottom-end",
  durationPreset = "standard",
  maxVisible = 4,
  previewStatic = false,
  className,
}) => {
  const [isHovered, setIsHovered] = React.useState(false)
  const [measuredHeights, setMeasuredHeights] = React.useState<Record<string, number>>({})

  const canonicalPosition = normalizeToastPosition(position)
  const resolvedVariant = normalizeToastVariant(variant)

  const handleMeasureHeight = React.useCallback((id: string, height: number) => {
    setMeasuredHeights((prev) =>
      prev[id] === height ? prev : { ...prev, [id]: height }
    )
  }, [])

  const visibleSlice = React.useMemo(
    () => toasts.slice(0, maxVisible + 1),
    [toasts, maxVisible]
  )

  const expandedOffsets = React.useMemo(() => {
    const heights = visibleSlice.map((t) => measuredHeights[t.id] ?? DEFAULT_CARD_HEIGHT_PX)
    return computeCumulativeExpandedOffsets(heights, EXPANDED_STACK_GAP_PX)
  }, [visibleSlice, measuredHeights])

  const totalExpandedHeight = React.useMemo(() => {
    const count = Math.min(toasts.length, maxVisible)
    if (count === 0) return 0
    const lastIdx = count - 1
    const lastOffset = expandedOffsets[lastIdx] ?? 0
    const lastHeight =
      measuredHeights[visibleSlice[lastIdx]?.id ?? ""] ?? DEFAULT_CARD_HEIGHT_PX
    return lastOffset + lastHeight
  }, [toasts.length, maxVisible, expandedOffsets, measuredHeights, visibleSlice])

  if (toasts.length === 0) return null

  if (resolvedVariant === "beui-motion" && !previewStatic) {
    const durationMs = resolveToastDurationMs(durationPreset)
    const mappedToasts: OfficialAnimatedToast[] = [...toasts]
      .reverse()
      .map((item) => {
        const meta = TOAST_CATEGORY_META[item.category] ?? TOAST_CATEGORY_META.system
        const Icon = meta.icon
        const actionLabel =
          item.actionLabel ??
          (item.onAction ? meta.defaultActionLabel ?? "Inspect" : undefined)
        return {
          id: item.id,
          title:
            item.count && item.count > 1
              ? `${item.title} (×${item.count})`
              : item.title,
          description: item.detail,
          status: mapItemToOfficialMotionStatus(item),
          icon: <Icon className="h-3.5 w-3.5" />,
          duration: durationMs,
          dismissible: true,
          createdAt: item.createdAt,
          action:
            item.onAction && actionLabel
              ? {
                  label: actionLabel,
                  onClick: () => {
                    item.onAction?.()
                    onDismiss(item.id)
                  },
                }
              : undefined,
        }
      })
    return (
      <OfficialMotionToastStack
        toasts={mappedToasts}
        onDismiss={onDismiss}
        position={OFFICIAL_MOTION_POSITION_MAP[canonicalPosition]}
        fixed
        maxVisible={maxVisible}
        className={className}
      />
    )
  }

  if (previewStatic) {
    return (
      <div
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        className={cn(
          "relative flex min-h-[168px] w-full items-end justify-center py-2",
          className
        )}
      >
        <div
          className="relative flex w-[348px] max-w-full justify-center"
          style={{
            height: isHovered
              ? `${Math.max(150, totalExpandedHeight)}px`
              : `${(measuredHeights[visibleSlice[0]?.id ?? ""] ?? DEFAULT_CARD_HEIGHT_PX) + Math.min(visibleSlice.length - 1, 2) * 14}px`,
            transition: "height 260ms var(--t-ease-spring)",
          }}
        >
          {visibleSlice.map((item, idx) => {
            const geometry = computeToastStackGeometry({
              index: idx,
              isExpanded: isHovered,
              expandedOffsetPx: expandedOffsets[idx] ?? idx * 84,
              position: "bottom-end",
              maxVisible,
            })
            return (
              <ToastCardItem
                key={item.id}
                item={item}
                index={idx}
                geometry={geometry}
                variant={resolvedVariant}
                position="bottom-end"
                durationMs={0}
                isPaused
                previewStatic={false}
                onDismiss={onDismiss}
                onMeasureHeight={handleMeasureHeight}
              />
            )
          })}
        </div>
      </div>
    )
  }

  const isTop =
    canonicalPosition === "top-start" ||
    canonicalPosition === "top-center" ||
    canonicalPosition === "top-end"
  const frontHeight =
    measuredHeights[visibleSlice[0]?.id ?? ""] ?? DEFAULT_CARD_HEIGHT_PX
  const collapsedHeight =
    frontHeight + Math.min(Math.max(0, toasts.length - 1), maxVisible - 1) * 12

  return (
    <section
      aria-label="Agent notifications"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onFocusCapture={() => setIsHovered(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          setIsHovered(false)
        }
      }}
      className={cn(
        "pointer-events-none fixed z-[70] flex flex-col gap-2",
        POSITION_CONTAINER_CLASSES[canonicalPosition],
        className
      )}
    >
      {isTop && isHovered && toasts.length >= 2 && (
        <div className="pointer-events-auto flex items-center gap-1.5 rounded-full border border-border/75 bg-card/95 px-2.5 py-1 text-[10px] font-medium text-muted-foreground shadow-md backdrop-blur-md">
          <Layers className="size-3 text-primary" />
          <span>{toasts.length} notifications</span>
          {onOpenOptions && (
            <button
              type="button"
              onClick={onOpenOptions}
              className="ml-1 rounded px-1.5 py-0.5 font-semibold text-foreground transition hover:bg-muted"
            >
              Options
            </button>
          )}
          {onClearAll && (
            <button
              type="button"
              onClick={onClearAll}
              className="rounded px-1.5 py-0.5 font-semibold text-primary transition hover:bg-primary/12"
            >
              Clear all
            </button>
          )}
        </div>
      )}

      <div
        className="relative w-[348px] max-w-[calc(100vw-2rem)]"
        style={{
          height: `${isHovered ? Math.max(collapsedHeight, totalExpandedHeight) : collapsedHeight}px`,
          transition: "height 260ms var(--t-ease-spring)",
        }}
      >
        {visibleSlice.map((item, idx) => {
          const geometry = computeToastStackGeometry({
            index: idx,
            isExpanded: isHovered,
            expandedOffsetPx: expandedOffsets[idx] ?? idx * 84,
            position: canonicalPosition,
            maxVisible,
          })
          const durationMs = resolveToastDurationMs(durationPreset, item.durationMs)
          return (
            <ToastCardItem
              key={item.id}
              item={item}
              index={idx}
              geometry={geometry}
              variant={resolvedVariant}
              position={canonicalPosition}
              durationMs={durationMs}
              isPaused={isHovered}
              previewStatic={false}
              onDismiss={onDismiss}
              onMeasureHeight={handleMeasureHeight}
            />
          )
        })}
      </div>

      {!isTop && isHovered && toasts.length >= 2 && (
        <div className="pointer-events-auto flex items-center gap-1.5 rounded-full border border-border/75 bg-card/95 px-2.5 py-1 text-[10px] font-medium text-muted-foreground shadow-md backdrop-blur-md">
          <Layers className="size-3 text-primary" />
          <span>{toasts.length} notifications</span>
          {onOpenOptions && (
            <button
              type="button"
              onClick={onOpenOptions}
              className="ml-1 rounded px-1.5 py-0.5 font-semibold text-foreground transition hover:bg-muted"
            >
              Options
            </button>
          )}
          {onClearAll && (
            <button
              type="button"
              onClick={onClearAll}
              className="rounded px-1.5 py-0.5 font-semibold text-primary transition hover:bg-primary/12"
            >
              Clear all
            </button>
          )}
        </div>
      )}
    </section>
  )
}

const NOTICE_KIND_TO_CATEGORY: Record<ActivityNotice["kind"], BeuiToastCategory> = {
  workflow: "workflow",
  skill: "skill",
  self_improvement: "self_improvement",
  memory: "memory",
  research: "research",
  source: "source",
  system: "system",
}

export function mapNoticeKindToToastCategory(
  kind: ActivityNotice["kind"]
): BeuiToastCategory {
  return NOTICE_KIND_TO_CATEGORY[kind] ?? "system"
}

export interface BeuiTelemetryNoticeStripProps {
  notices: ReadonlyArray<ActivityNotice>
  styleVariant?: BeuiInlineNoticeStyle
  onInspectNotice?: (notice: ActivityNotice) => void
  className?: string
}

/**
 * Renders structured `ActivityNotice[]` (`Workflow matched`, `Skill used`, `Self-improvement`,
 * `Memory stored`, `Research brief matched`) directly inside the chat conversation track.
 */
export const BeuiTelemetryNoticeStrip: React.FC<BeuiTelemetryNoticeStripProps> = ({
  notices,
  styleVariant = "minimal-whisper",
  onInspectNotice,
  className,
}) => {
  const dedupedNotices = React.useMemo(() => {
    const map = new Map<
      string,
      { notice: ActivityNotice; count: number }
    >()
    for (const notice of notices) {
      const key = `${notice.kind}:${notice.title}:${notice.detail ?? ""}`
      const existing = map.get(key)
      if (existing) {
        existing.count += 1
      } else {
        map.set(key, { notice, count: 1 })
      }
    }
    return Array.from(map.values())
  }, [notices])

  if (styleVariant === "toast-only" || dedupedNotices.length === 0) {
    return null
  }

  if (styleVariant === "minimal-whisper") {
    return (
      <div className={cn("my-1 flex flex-wrap items-center gap-x-3 gap-y-1 not-prose", className)}>
        {dedupedNotices.map(({ notice, count }) => {
          const category = mapNoticeKindToToastCategory(notice.kind)
          const meta = TOAST_CATEGORY_META[category]
          const Icon = meta.icon
          const interactive = Boolean(onInspectNotice)
          const Tag = interactive ? "button" : "div"
          return (
            <Tag
              key={notice.id}
              type={interactive ? "button" : undefined}
              onClick={interactive ? () => onInspectNotice?.(notice) : undefined}
              className={cn(
                "group inline-flex max-w-full items-center gap-1.5 text-[11.5px] text-muted-foreground transition-colors",
                interactive && "cursor-pointer hover:text-foreground"
              )}
            >
              <Icon className="size-3 shrink-0 text-primary/80" />
              <span className="font-medium text-foreground/85">{notice.title}</span>
              {notice.detail && (
                <span className="max-w-[240px] truncate text-muted-foreground">
                  · {notice.detail}
                </span>
              )}
              {count > 1 && (
                <span className="font-mono text-[10px] text-muted-foreground">
                  ×{count}
                </span>
              )}
              {interactive && (
                <ArrowUpRight className="size-2.5 shrink-0 opacity-0 transition-opacity group-hover:opacity-75" />
              )}
            </Tag>
          )
        })}
      </div>
    )
  }

  if (styleVariant === "memory-chips") {
    return (
      <div className={cn("my-1 flex flex-wrap items-center gap-1.5 not-prose", className)}>
        {dedupedNotices.map(({ notice, count }) => {
          const category = mapNoticeKindToToastCategory(notice.kind)
          const meta = TOAST_CATEGORY_META[category]
          const Icon = meta.icon
          const interactive = Boolean(onInspectNotice)
          const Tag = interactive ? "button" : "div"
          return (
            <Tag
              key={notice.id}
              type={interactive ? "button" : undefined}
              onClick={interactive ? () => onInspectNotice?.(notice) : undefined}
              className={cn(
                "inline-flex max-w-full items-center gap-1.5 rounded-lg border border-border/55 bg-foreground/[0.038] px-2 py-0.5 text-[11px] text-foreground/85 transition-colors",
                interactive && "cursor-pointer hover:border-border hover:bg-foreground/[0.065]"
              )}
            >
              <Icon className="size-3 shrink-0 text-primary/80" />
              <span className="font-medium">{notice.title}</span>
              {notice.detail && (
                <span className="max-w-[200px] truncate text-muted-foreground">
                  · {notice.detail}
                </span>
              )}
              {count > 1 && (
                <span className="font-mono text-[9.5px] text-muted-foreground">
                  ×{count}
                </span>
              )}
            </Tag>
          )
        })}
      </div>
    )
  }

  if (styleVariant === "tool-chips") {
    return (
      <div className={cn("my-1 flex flex-wrap items-center gap-1.5 not-prose", className)}>
        {dedupedNotices.map(({ notice, count }) => {
          const interactive = Boolean(onInspectNotice)
          const Tag = interactive ? "button" : "div"
          return (
            <Tag
              key={notice.id}
              type={interactive ? "button" : undefined}
              onClick={interactive ? () => onInspectNotice?.(notice) : undefined}
              className={cn(
                "inline-flex max-w-full items-center gap-1.5 rounded-md border border-border/55 bg-card/70 px-2 py-0.5 font-mono text-[10.5px] text-foreground/85 transition-colors",
                interactive && "cursor-pointer hover:border-primary/40 hover:text-primary"
              )}
            >
              <span className="size-1.5 shrink-0 rounded-full bg-primary/80" />
              <span className="truncate">{notice.detail || notice.title}</span>
              {count > 1 && <span className="text-muted-foreground">×{count}</span>}
            </Tag>
          )
        })}
      </div>
    )
  }

  if (styleVariant === "ai-status") {
    return (
      <div className={cn("my-1 flex flex-wrap items-center gap-1.5 not-prose", className)}>
        {dedupedNotices.map(({ notice, count }) => {
          const interactive = Boolean(onInspectNotice)
          const Tag = interactive ? "button" : "div"
          return (
            <Tag
              key={notice.id}
              type={interactive ? "button" : undefined}
              onClick={interactive ? () => onInspectNotice?.(notice) : undefined}
              className={cn(
                "inline-flex max-w-full items-center gap-2 rounded-full border border-border/55 bg-foreground/[0.03] px-2.5 py-0.5 text-[11px] text-foreground/85 transition-colors",
                interactive && "cursor-pointer hover:border-border"
              )}
            >
              <span className="size-1.5 shrink-0 rounded-full bg-primary" />
              <span className="font-medium">{notice.title}</span>
              {notice.detail && (
                <span className="max-w-[200px] truncate text-muted-foreground">
                  · {notice.detail}
                </span>
              )}
              {count > 1 && (
                <span className="font-mono text-[9.5px] text-muted-foreground">×{count}</span>
              )}
            </Tag>
          )
        })}
      </div>
    )
  }

  if (styleVariant === "cards") {
    return (
      <div className={cn("my-1.5 flex w-full flex-col gap-1.5 not-prose", className)}>
        {dedupedNotices.map(({ notice, count }) => {
          const category = mapNoticeKindToToastCategory(notice.kind)
          const meta = TOAST_CATEGORY_META[category]
          const Icon = meta.icon
          const interactive = Boolean(onInspectNotice)
          return (
            <div
              key={notice.id}
              onClick={interactive ? () => onInspectNotice?.(notice) : undefined}
              role={interactive ? "button" : undefined}
              tabIndex={interactive ? 0 : undefined}
              onKeyDown={
                interactive
                  ? (e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault()
                        onInspectNotice?.(notice)
                      }
                    }
                  : undefined
              }
              className={cn(
                "flex items-center gap-2.5 rounded-xl border border-border/65 bg-card/75 px-3 py-2 text-xs transition",
                interactive && "cursor-pointer hover:border-border hover:bg-muted/35"
              )}
            >
              <span className="inline-flex size-6 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-foreground/[0.04] text-primary">
                <Icon className="size-3.5" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="rounded bg-foreground/[0.05] px-1.5 py-0 font-mono text-[9px] font-medium tracking-wider text-muted-foreground">
                    {meta.shortTag}
                  </span>
                  <span className="font-medium text-foreground">{notice.title}</span>
                  {count > 1 && (
                    <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[9px]">
                      ×{count}
                    </Badge>
                  )}
                </div>
                {notice.detail && (
                  <div className="mt-0.5 truncate text-[11px] text-muted-foreground">
                    {notice.detail}
                  </div>
                )}
              </div>
              {interactive && (
                <span className="inline-flex shrink-0 items-center gap-1 text-[10px] font-medium text-muted-foreground">
                  <span>{meta.defaultActionLabel ?? "Inspect"}</span>
                  <ArrowUpRight className="size-3" />
                </span>
              )}
            </div>
          )
        })}
      </div>
    )
  }

  return (
    <div className={cn("my-1 flex flex-wrap items-center gap-1.5 not-prose", className)}>
      {dedupedNotices.map(({ notice, count }) => {
        const category = mapNoticeKindToToastCategory(notice.kind)
        const meta = TOAST_CATEGORY_META[category]
        const Icon = meta.icon
        const interactive = Boolean(onInspectNotice)
        const Tag = interactive ? "button" : "div"
        return (
          <Tag
            key={notice.id}
            type={interactive ? "button" : undefined}
            onClick={interactive ? () => onInspectNotice?.(notice) : undefined}
            className={cn(
              "inline-flex max-w-full items-center gap-1.5 rounded-full border border-border/60 bg-foreground/[0.035] px-2.5 py-0.5 text-[11px] text-foreground/85 transition-colors",
              interactive && "cursor-pointer hover:border-border hover:bg-foreground/[0.065]"
            )}
          >
            <Icon className="size-3 shrink-0 text-primary/85" />
            <span className="font-medium">{notice.title}</span>
            {notice.detail && (
              <span className="max-w-[220px] truncate text-muted-foreground">
                · {notice.detail}
              </span>
            )}
            {count > 1 && (
              <span className="rounded-full bg-foreground/[0.06] px-1.5 py-0 font-mono text-[9.5px] text-muted-foreground">
                ×{count}
              </span>
            )}
            {interactive && <ArrowUpRight className="size-2.5 shrink-0 opacity-60" />}
          </Tag>
        )
      })}
    </div>
  )
}

export interface BeuiNotificationCenterPopoverProps {
  open: boolean
  onClose: () => void
  history: ReadonlyArray<BeuiToastItem>
  backgroundTasks?: ReadonlyArray<BackgroundInboxTask>
  backgroundInboxStyle?: BackgroundInboxStyleChoice
  deleteButtonStyle?: DeleteButtonStyleChoice
  toastEnabled: boolean
  toastVariant?: BeuiToastVariant
  toastPosition?: BeuiToastPosition
  onToggleEnabled: (enabled: boolean) => void
  onSelectVariant?: (variant: BeuiToastVariant) => void
  onSelectPosition?: (position: BeuiCanonicalToastPosition) => void
  onClearHistory: () => void
  onOpenStudioOptions?: () => void
}

/**
 * Minimal Workspace Notification History Popover anchored from the top-right rail.
 */
export const BeuiNotificationCenterPopover: React.FC<
  BeuiNotificationCenterPopoverProps
> = ({
  open,
  onClose,
  history,
  backgroundTasks = [],
  backgroundInboxStyle = "assistant-inbox",
  deleteButtonStyle = "rare-confirm",
  toastEnabled,
  onToggleEnabled,
  onClearHistory,
}) => {
  if (!open) return null

  return (
    <>
      <button
        type="button"
        aria-label="Close notifications popover"
        onClick={onClose}
        className="fixed inset-0 z-40 bg-transparent"
      />
      <div
        role="dialog"
        aria-label="Notifications"
        className="t-popover fixed top-14 right-14 z-50 flex max-h-[min(480px,calc(100vh-5rem))] w-[340px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-2xl border border-border/75 bg-popover/90 text-popover-foreground shadow-2xl backdrop-blur-2xl"
      >
        <div className="flex items-center justify-between gap-2 border-b border-border/55 px-3.5 py-2.5">
          <div className="flex min-w-0 items-center gap-2">
            <span className="inline-flex size-6 shrink-0 items-center justify-center rounded-lg border border-primary/30 bg-primary/12 text-primary">
              {toastEnabled ? (
                <Bell className="size-3.5" />
              ) : (
                <BellOff className="size-3.5" />
              )}
            </span>
            <span className="truncate text-xs font-semibold text-foreground">
              Notifications
            </span>
            {history.length > 0 && (
              <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[10px]">
                {history.length}
              </Badge>
            )}
          </div>

          <div className="flex shrink-0 items-center gap-2">
            {history.length > 0 && (
              <RareDeleteButton
                variant={deleteButtonStyle}
                iconOnly={false}
                label="Clear"
                onDelete={onClearHistory}
                ariaLabel="Clear notifications"
              />
            )}
            <Switch
              checked={toastEnabled}
              onCheckedChange={onToggleEnabled}
              aria-label="Enable live notifications"
            />
          </div>
        </div>

        <div className="flex-1 space-y-2 overflow-y-auto p-2.5">
          {backgroundTasks.length > 0 && (
            <BackgroundInboxElement
              tasks={backgroundTasks}
              variant={backgroundInboxStyle}
            />
          )}
          {history.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-1.5 py-7 text-center">
              <span className="inline-flex size-8 items-center justify-center rounded-xl border border-border/65 bg-muted/35 text-muted-foreground">
                <Bell className="size-3.5" />
              </span>
              <div className="text-xs font-medium text-foreground">
                No notifications yet
              </div>
              <p className="max-w-[220px] text-[11px] text-muted-foreground">
                Skill updates, workflow runs, and self-improvement events appear here.
              </p>
            </div>
          ) : (
            history.map((item) => {
              const meta =
                TOAST_CATEGORY_META[item.category] ?? TOAST_CATEGORY_META.system
              const Icon = meta.icon
              return (
                <div
                  key={item.id}
                  className="group flex items-start gap-2.5 rounded-xl border border-border/60 bg-card/70 p-2.5 text-xs transition hover:border-border hover:bg-card"
                >
                  <span
                    className={cn(
                      "mt-0.5 inline-flex size-6 shrink-0 items-center justify-center rounded-lg border",
                      meta.iconWrapClass
                    )}
                  >
                    <Icon className="size-3.5" />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between gap-1.5">
                      <span className="font-mono text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                        {meta.shortTag}
                      </span>
                      <span className="text-[10px] text-muted-foreground/75">
                        {formatShortTime(item.createdAt)}
                      </span>
                    </div>
                    <div className="mt-0.5 font-semibold text-foreground">
                      {item.title}
                      {item.count && item.count > 1 ? ` (×${item.count})` : ""}
                    </div>
                    {item.detail && (
                      <div className="mt-0.5 line-clamp-2 text-[11px] text-muted-foreground">
                        {item.detail}
                      </div>
                    )}
                    {item.onAction && (
                      <button
                        type="button"
                        onClick={() => {
                          item.onAction?.()
                          onClose()
                        }}
                        className="mt-1.5 inline-flex items-center gap-1 text-[10px] font-semibold text-primary hover:underline"
                      >
                        <span>
                          {item.actionLabel ?? meta.defaultActionLabel ?? "Inspect"}
                        </span>
                        <ArrowUpRight className="size-2.5" />
                      </button>
                    )}
                  </div>
                </div>
              )
            })
          )}
        </div>
      </div>
    </>
  )
}
