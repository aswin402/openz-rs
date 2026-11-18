import * as React from "react"
import {
  Alert as OfficialBeuiAlert,
  AlertAction,
  AlertClose,
  AlertContent,
  AlertDescription,
  AlertIcon,
  AlertTitle,
  alertMessages,
  useAlert,
  type AlertMessages,
  type AlertProps as OfficialBeuiAlertProps,
  type AlertState,
  type AlertVariant,
} from "@/components/motion/alert"
import { cn } from "@/lib/utils"

export {
  OfficialBeuiAlert,
  AlertAction as BeuiAlertAction,
  AlertClose as BeuiAlertClose,
  AlertContent as BeuiAlertContent,
  AlertDescription as BeuiAlertDescription,
  AlertIcon as BeuiAlertIcon,
  AlertTitle as BeuiAlertTitle,
  alertMessages,
  useAlert,
  type AlertMessages,
  type OfficialBeuiAlertProps,
  type AlertState,
  type AlertVariant,
}

export type BeuiAlertTone = AlertVariant

export type BeuiAlertStyleVariant =
  | "beui-alert"
  | "beui-bordered"
  | "beui-accent"
  | "compact-pill"

export interface BeuiAlertStyleDescriptor {
  id: BeuiAlertStyleVariant
  title: string
  shortLabel: string
  description: string
  isDefault?: boolean
}

export const BEUI_ALERT_STYLE_OPTIONS: ReadonlyArray<BeuiAlertStyleDescriptor> = [
  {
    id: "beui-alert",
    title: "Official @beui/alert Motion Surface",
    shortLabel: "Motion",
    description:
      "Official @beui/alert soft semantic tint with spring layout physics, icon morphing, and animated dismiss.",
    isDefault: true,
  },
  {
    id: "beui-bordered",
    title: "@beui/alert Frosted Glass Frame",
    shortLabel: "Bordered",
    description:
      "Pairs @beui/alert spring transitions with a crisp translucent border and frosted card surface.",
  },
  {
    id: "beui-accent",
    title: "@beui/alert Left Accent Callout",
    shortLabel: "Accent Rail",
    description:
      "Highlights semantic severity with a bold left accent rail while preserving @beui/alert motion.",
  },
  {
    id: "compact-pill",
    title: "@beui/alert Compact Single-Row Pill",
    shortLabel: "Compact",
    description:
      "Slim single-line @beui/alert status banner keeping inline chat and system notices tight.",
  },
]

const VALID_ALERT_STYLES: Readonly<Record<BeuiAlertStyleVariant, true>> = {
  "beui-alert": true,
  "beui-bordered": true,
  "beui-accent": true,
  "compact-pill": true,
}

export function normalizeAlertStyle(raw: unknown): BeuiAlertStyleVariant {
  if (typeof raw === "string" && raw in VALID_ALERT_STYLES) {
    return raw as BeuiAlertStyleVariant
  }
  return "beui-alert"
}

const ACCENT_RAIL_BY_TONE: Record<BeuiAlertTone, string> = {
  default: "border-l-4 border-l-foreground/45 border border-border/60 bg-card/80",
  info: "border-l-4 border-l-primary border border-primary/20",
  success: "border-l-4 border-l-emerald-500 border border-emerald-500/20",
  warning: "border-l-4 border-l-amber-500 border border-amber-500/20",
  destructive: "border-l-4 border-l-destructive border border-destructive/20",
}

const BORDERED_BY_TONE: Record<BeuiAlertTone, string> = {
  default: "border border-border/80 bg-card/80 backdrop-blur-md",
  info: "border border-primary/30 bg-primary/8 backdrop-blur-md",
  success: "border border-emerald-500/30 bg-emerald-500/8 backdrop-blur-md",
  warning: "border border-amber-500/30 bg-amber-500/8 backdrop-blur-md",
  destructive: "border border-destructive/30 bg-destructive/8 backdrop-blur-md",
}

export interface BeuiAlertProps {
  tone?: BeuiAlertTone
  styleVariant?: BeuiAlertStyleVariant
  title?: React.ReactNode
  children: React.ReactNode
  action?: React.ReactNode
  icon?: React.ReactNode
  onDismiss?: () => void
  compact?: boolean
  className?: string
}

/**
 * `@beui/alert` — Powered directly by the official registry component (`@/components/motion/alert`)
 * installed via `bunx --bun shadcn add @beui/alert`.
 */
export const BeuiAlert: React.FC<BeuiAlertProps> = ({
  tone = "default",
  styleVariant,
  title,
  children,
  action,
  icon,
  onDismiss,
  compact = false,
  className,
}) => {
  const resolvedStyle: BeuiAlertStyleVariant =
    styleVariant ?? (compact ? "compact-pill" : "beui-alert")
  const isCompact = resolvedStyle === "compact-pill" || compact

  return (
    <OfficialBeuiAlert
      variant={tone}
      onOpenChange={(open) => {
        if (!open) onDismiss?.()
      }}
      contentClassName={cn(isCompact && "items-center gap-2")}
      className={cn(
        resolvedStyle === "beui-alert" && "border border-border/45",
        resolvedStyle === "beui-bordered" && BORDERED_BY_TONE[tone],
        resolvedStyle === "beui-accent" && ACCENT_RAIL_BY_TONE[tone],
        isCompact && "rounded-lg border border-border/55 px-3 py-2 text-xs",
        className
      )}
    >
      <AlertIcon className={cn(isCompact && "mt-0 size-3.5")}>{icon}</AlertIcon>
      <AlertContent className={cn(isCompact && "space-y-0")}>
        {title && !isCompact && <AlertTitle>{title}</AlertTitle>}
        <AlertDescription
          className={cn(
            isCompact && "text-[11.5px] leading-snug text-foreground/90"
          )}
        >
          {title && isCompact ? (
            <>
              <span className="mr-1.5 font-semibold text-foreground">{title}</span>
              <span>{children}</span>
            </>
          ) : (
            children
          )}
        </AlertDescription>
        {action && <AlertAction>{action}</AlertAction>}
      </AlertContent>
      {onDismiss && <AlertClose />}
    </OfficialBeuiAlert>
  )
}
