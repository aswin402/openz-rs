import * as React from "react"
import { Brain } from "lucide-react"

import { BeuiCollapsible } from "./collapsible"
import {
  ThinkingLoaderBadge,
  type BeuiLoaderVariant,
} from "./loader"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

export type BeuiReasoningVariant = "card" | "compact" | "timeline"

export interface ReasoningTextProps {
  text?: string
  isStreaming?: boolean
  label?: string
  defaultOpen?: boolean
  variant?: BeuiReasoningVariant
  loaderVariant?: BeuiLoaderVariant
  compact?: boolean
  className?: string
}

/**
 * @beui/reasoning-text — Shimmering status & collapsible chain-of-thought block
 * paired with `@beui/loader` thinking animations.
 */
export const ReasoningText: React.FC<ReasoningTextProps> = ({
  text = "",
  isStreaming = false,
  label,
  defaultOpen,
  variant = "card",
  loaderVariant = "dot-matrix",
  compact = false,
  className,
}) => {
  const effectiveVariant: BeuiReasoningVariant = compact ? "compact" : variant

  const [manualOpen, setManualOpen] = React.useState<boolean | null>(
    defaultOpen ?? null
  )

  const lines = React.useMemo(
    () =>
      text
        .split("\n")
        .map((l) => l.trim())
        .filter(Boolean),
    [text]
  )

  const latestThoughtLine = lines[lines.length - 1] ?? ""
  const hideSeparateLabel =
    loaderVariant === "scramble" || loaderVariant === "percent"

  if (!text) {
    return (
      <div
        className={cn(
          "inline-flex select-none items-center gap-2 rounded-lg border border-border/55 bg-muted/30 px-2.5 py-1.5 text-xs text-primary",
          className
        )}
      >
        <ThinkingLoaderBadge variant={loaderVariant} size={15} />
        {!hideSeparateLabel && (
          <span className="animate-pulse font-medium text-foreground/90">
            {label || "Thinking…"}
          </span>
        )}
      </div>
    )
  }

  if (effectiveVariant === "compact") {
    return (
      <div
        className={cn(
          "flex items-center gap-2 rounded-lg border border-border/55 bg-muted/25 px-2.5 py-1.5 text-xs text-muted-foreground",
          className
        )}
      >
        {isStreaming ? (
          <ThinkingLoaderBadge
            variant={loaderVariant}
            size={14}
            className="shrink-0 text-primary"
          />
        ) : (
          <Brain className="size-3.5 shrink-0 text-primary" />
        )}
        <span className="truncate">
          {isStreaming
            ? latestThoughtLine || label || "Reasoning…"
            : `Reasoned across ${lines.length} ${lines.length === 1 ? "step" : "steps"}`}
        </span>
      </div>
    )
  }

  const resolvedOpen = manualOpen ?? isStreaming

  return (
    <BeuiCollapsible
      open={resolvedOpen}
      onOpenChange={(next) => setManualOpen(next)}
      variant={effectiveVariant === "timeline" ? "ghost" : "subtle"}
      className={cn("my-1.5", className)}
      icon={
        isStreaming ? (
          <ThinkingLoaderBadge
            variant={loaderVariant}
            size={14}
            className="text-primary"
          />
        ) : (
          <Brain className="size-3.5" />
        )
      }
      title={
        <span className="flex min-w-0 items-center gap-2">
          <span
            className={cn(
              "font-medium",
              isStreaming ? "animate-pulse text-primary" : "text-foreground/90"
            )}
          >
            {label || (isStreaming ? "Reasoning…" : "Thought Process")}
          </span>
          {!resolvedOpen && latestThoughtLine && (
            <span className="hidden truncate text-[11px] font-normal text-muted-foreground sm:inline">
              — {latestThoughtLine}
            </span>
          )}
        </span>
      }
      badge={
        <Badge variant="outline" className="px-1.5 py-0 font-mono text-[10px]">
          {lines.length} {lines.length === 1 ? "step" : "steps"}
        </Badge>
      }
    >
      {effectiveVariant === "timeline" ? (
        <ol className="max-h-60 space-y-1.5 overflow-y-auto border-l-2 border-primary/35 pl-3 text-[11.5px] text-muted-foreground">
          {lines.map((line, i) => (
            <li key={i} className="relative leading-relaxed">
              <span className="mr-1.5 font-mono text-[10px] text-primary">
                {i + 1}.
              </span>
              <span>{line}</span>
            </li>
          ))}
        </ol>
      ) : (
        <div className="max-h-60 overflow-y-auto border-l-2 border-primary/35 pl-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-muted-foreground">
          {text}
        </div>
      )}
    </BeuiCollapsible>
  )
}
