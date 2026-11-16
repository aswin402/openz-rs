import * as React from "react"
import { Gauge } from "lucide-react"

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

export type BeuiContextUsageVariant = "ring-pill" | "bar" | "hidden"

export interface BeuiContextUsageProps {
  usedTokens: number
  maxTokens?: number
  messageCount?: number
  variant?: BeuiContextUsageVariant
  className?: string
}

function formatCompactTokens(n: number): string {
  if (n >= 1_000_000) {
    const m = n / 1_000_000
    return Number.isInteger(m) ? `${m}M` : `${m.toFixed(1)}M`
  }
  if (n >= 1000) {
    const k = n / 1000
    return n >= 10_000 ? `${Math.round(k)}k` : `${k.toFixed(1)}k`
  }
  return String(n)
}

/**
 * @beui/context-usage — Live context window & token meter for the composer bar.
 */
export const BeuiContextUsage: React.FC<BeuiContextUsageProps> = ({
  usedTokens,
  maxTokens = 128000,
  messageCount = 0,
  variant = "ring-pill",
  className,
}) => {
  if (variant === "hidden") return null

  const ratio = Math.min(1, Math.max(0, usedTokens / Math.max(1, maxTokens)))
  const pct = Math.round(ratio * 100)
  const circumference = 2 * Math.PI * 6
  const dashOffset = circumference * (1 - ratio)

  const toneCls =
    pct >= 85
      ? "text-destructive"
      : pct >= 60
        ? "text-warning"
        : "text-primary"

  if (variant === "bar") {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            className={cn(
              "inline-flex items-center gap-2 rounded-md border border-border/65 bg-muted/35 px-2 py-1 text-[10px] text-muted-foreground",
              className
            )}
          >
            <Gauge className={cn("size-3 shrink-0", toneCls)} />
            <div className="h-1.5 w-14 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary transition-all duration-300"
                style={{ width: `${Math.max(4, pct)}%` }}
              />
            </div>
            <span className="font-mono">{formatCompactTokens(usedTokens)}</span>
          </div>
        </TooltipTrigger>
        <TooltipContent side="top" className="text-[11px]">
          ~{usedTokens.toLocaleString()} / {maxTokens.toLocaleString()} tokens ({pct}%) ·{" "}
          {messageCount} msgs
        </TooltipContent>
      </Tooltip>
    )
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          className={cn(
            "inline-flex select-none items-center gap-1.5 rounded-md border border-border/65 bg-muted/35 px-2 py-0.5 font-mono text-[10px] text-muted-foreground",
            className
          )}
        >
          <svg width="14" height="14" viewBox="0 0 16 16" className="-rotate-90">
            <circle
              cx="8"
              cy="8"
              r="6"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="opacity-20"
            />
            <circle
              cx="8"
              cy="8"
              r="6"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeDasharray={circumference}
              strokeDashoffset={dashOffset}
              strokeLinecap="round"
              className={toneCls}
            />
          </svg>
          <span>
            {formatCompactTokens(usedTokens)} / {formatCompactTokens(maxTokens)}
          </span>
        </div>
      </TooltipTrigger>
      <TooltipContent side="top" className="text-[11px]">
        Context: ~{usedTokens.toLocaleString()} / {maxTokens.toLocaleString()} tokens ({pct}%)
        · Z-Context active
      </TooltipContent>
    </Tooltip>
  )
}
