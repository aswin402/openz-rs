import * as React from "react"
import {
  Check,
  ChevronDown,
  Loader2,
  ShieldAlert,
  ShieldCheck,
  Terminal,
  X,
} from "lucide-react"

import { BeuiCollapsible } from "./collapsible"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card"
import { cn } from "@/lib/utils"
import type { SecurityPromptInfo } from "@/types"

export interface ToolApprovalProps {
  prompt: SecurityPromptInfo
  onChoice: (choice: "approve" | "deny") => void
  className?: string
}

export function formatApprovalArgs(args: unknown): string {
  if (!args) return ""
  if (typeof args === "string") return args
  try {
    return JSON.stringify(args, null, 2)
  } catch {
    return String(args)
  }
}

export function summarizeApprovalInline(prompt: SecurityPromptInfo): string {
  const desc = prompt.description?.trim()
  if (desc) return desc
  if (!prompt.arguments) return ""
  if (typeof prompt.arguments === "string") return prompt.arguments.trim()
  const entries = Object.entries(prompt.arguments)
  if (entries.length === 0) return ""
  const [firstKey, firstVal] = entries[0]
  const rendered =
    typeof firstVal === "string" ? firstVal : JSON.stringify(firstVal)
  return `${firstKey}: ${rendered}`
}

/**
 * @beui/tool-approval — Full interactive permission card while `pending`;
 * automatically collapses into a single-line minimal status bar once `approved` or `denied`.
 */
export const ToolApproval: React.FC<ToolApprovalProps> = ({
  prompt,
  onChoice,
  className,
}) => {
  const argsPreview = React.useMemo(
    () => formatApprovalArgs(prompt.arguments),
    [prompt.arguments]
  )
  const inlineSummary = React.useMemo(
    () => summarizeApprovalInline(prompt),
    [prompt]
  )
  const [showResolvedArgs, setShowResolvedArgs] = React.useState(false)

  if (prompt.status !== "pending") {
    const isApproved = prompt.status === "approved"
    return (
      <div
        data-approval-state={prompt.status}
        className={cn(
          "my-1.5 overflow-hidden rounded-lg border bg-card/60 text-xs transition-colors",
          isApproved ? "border-border/65" : "border-destructive/30 bg-destructive/[0.04]",
          className
        )}
      >
        <div className="flex min-h-8 items-center justify-between gap-2 px-2.5 py-1">
          <div className="flex min-w-0 items-center gap-2">
            <span
              className={cn(
                "flex size-4 shrink-0 items-center justify-center rounded-full",
                isApproved
                  ? "bg-success/15 text-success"
                  : "bg-destructive/15 text-destructive"
              )}
            >
              {isApproved ? (
                <Check className="size-2.5" />
              ) : (
                <X className="size-2.5" />
              )}
            </span>
            <span className="inline-flex shrink-0 items-center gap-1 rounded bg-muted/65 px-1.5 py-0.5 font-mono text-[10.5px] font-medium text-foreground">
              <Terminal className="size-2.5 text-primary" />
              <span>{prompt.toolName}</span>
            </span>
            {inlineSummary && (
              <span className="truncate text-[11px] text-muted-foreground">
                {inlineSummary}
              </span>
            )}
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            {argsPreview && (
              <button
                type="button"
                onClick={() => setShowResolvedArgs((open) => !open)}
                className="inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                aria-expanded={showResolvedArgs}
                title="Toggle tool arguments"
              >
                <span>Args</span>
                <ChevronDown
                  className={cn(
                    "size-3 transition-transform",
                    showResolvedArgs && "rotate-180"
                  )}
                />
              </button>
            )}
            <span
              className={cn(
                "rounded-full px-2 py-0.5 text-[10px] font-medium",
                isApproved
                  ? "bg-success/12 text-success"
                  : "bg-destructive/12 text-destructive"
              )}
            >
              {isApproved ? "Approved" : "Denied"}
            </span>
          </div>
        </div>

        {showResolvedArgs && argsPreview && (
          <div className="border-t border-border/50 bg-muted/25 px-3 py-2">
            <pre className="max-h-36 overflow-auto font-mono text-[10.5px] leading-relaxed text-foreground/85">
              {argsPreview}
            </pre>
          </div>
        )}
      </div>
    )
  }

  return (
    <Card
      data-approval-state="pending"
      className={cn(
        "my-2.5 gap-2.5 border-warning/35 bg-warning/[0.06] py-0 shadow-xs",
        className
      )}
    >
      <CardHeader className="flex flex-row items-center justify-between gap-2 border-b border-border/50 px-3.5 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <div className="flex size-7 shrink-0 items-center justify-center rounded-lg border border-warning/30 bg-warning/15 text-warning">
            <ShieldAlert className="size-3.5" />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-xs font-semibold text-foreground">
                Tool Permission Required
              </span>
              <Badge variant="outline" className="font-mono text-[10px]">
                <Terminal className="size-2.5 text-primary" />
                <span>{prompt.toolName}</span>
              </Badge>
            </div>
          </div>
        </div>

        <Badge variant="warning" className="gap-1">
          <Loader2 className="size-2.5 animate-spin" />
          <span>Paused</span>
        </Badge>
      </CardHeader>

      <CardContent className="space-y-2 px-3.5 py-1">
        {prompt.description && (
          <p className="text-xs leading-relaxed text-muted-foreground">
            {prompt.description}
          </p>
        )}

        {argsPreview && (
          <BeuiCollapsible
            title="Inspect tool arguments"
            variant="subtle"
            defaultOpen={argsPreview.length <= 280}
          >
            <pre className="max-h-44 overflow-auto font-mono text-[11px] leading-relaxed text-foreground/90">
              {argsPreview}
            </pre>
          </BeuiCollapsible>
        )}
      </CardContent>

      <CardFooter className="flex items-center justify-between gap-2 border-t border-border/45 px-3.5 py-2.5">
        <span className="text-[11px] text-muted-foreground">
          Review before allowing execution
        </span>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => onChoice("deny")}
            className="gap-1"
          >
            <X className="size-3.5" />
            <span>Deny</span>
          </Button>
          <Button
            type="button"
            variant="default"
            size="sm"
            onClick={() => onChoice("approve")}
            className="gap-1"
          >
            <ShieldCheck className="size-3.5" />
            <span>Approve</span>
          </Button>
        </div>
      </CardFooter>
    </Card>
  )
}
