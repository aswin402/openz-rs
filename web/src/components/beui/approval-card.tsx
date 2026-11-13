import * as React from "react"
import {
  Check,
  CheckCircle2,
  ChevronDown,
  FileWarning,
  X,
  XCircle,
} from "lucide-react"

import { summarizeApprovalInline } from "./tool-approval"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { cn } from "@/lib/utils"
import type { SecurityPromptInfo } from "@/types"

export interface ApprovalCardProps {
  prompt: SecurityPromptInfo
  onChoice: (choice: "approve" | "deny") => void
  className?: string
}

function extractStructuredFields(args: unknown): Array<{ key: string; value: string }> {
  if (!args) return []
  if (typeof args === "string") {
    try {
      const parsed = JSON.parse(args) as unknown
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return Object.entries(parsed as Record<string, unknown>).map(([k, v]) => ({
          key: k,
          value: typeof v === "string" ? v : JSON.stringify(v),
        }))
      }
    } catch {
      return [{ key: "command", value: args }]
    }
    return [{ key: "payload", value: args }]
  }
  if (typeof args === "object" && !Array.isArray(args)) {
    return Object.entries(args as Record<string, unknown>).map(([k, v]) => ({
      key: k,
      value: typeof v === "string" ? v : JSON.stringify(v),
    }))
  }
  return [{ key: "payload", value: String(args) }]
}

/**
 * @beui/approval-card — Structured human-in-the-loop review card while `pending`;
 * collapses into a minimal single-line audit pill once `approved` or `denied`.
 */
export const ApprovalCard: React.FC<ApprovalCardProps> = ({
  prompt,
  onChoice,
  className,
}) => {
  const fields = React.useMemo(
    () => extractStructuredFields(prompt.arguments),
    [prompt.arguments]
  )
  const inlineSummary = React.useMemo(
    () => summarizeApprovalInline(prompt),
    [prompt]
  )
  const [showFields, setShowFields] = React.useState(false)

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
            <span className="shrink-0 font-mono text-[10.5px] font-semibold text-primary">
              {prompt.toolName}
            </span>
            {inlineSummary && (
              <span className="truncate text-[11px] text-muted-foreground">
                {inlineSummary}
              </span>
            )}
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            {fields.length > 0 && (
              <button
                type="button"
                onClick={() => setShowFields((open) => !open)}
                className="inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                aria-expanded={showFields}
                title="Toggle action fields"
              >
                <span>Details</span>
                <ChevronDown
                  className={cn("size-3 transition-transform", showFields && "rotate-180")}
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
              {isApproved ? "Confirmed" : "Rejected"}
            </span>
          </div>
        </div>

        {showFields && fields.length > 0 && (
          <div className="divide-y divide-border/45 border-t border-border/50 bg-muted/20 px-3 py-1.5">
            {fields.slice(0, 6).map((field) => (
              <div
                key={field.key}
                className="grid grid-cols-[100px_minmax(0,1fr)] gap-2 py-1 text-[10.5px]"
              >
                <span className="truncate font-mono text-muted-foreground">
                  {field.key}
                </span>
                <span className="break-all font-mono text-foreground">{field.value}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    )
  }

  return (
    <Card
      data-approval-state="pending"
      className={cn("my-2.5 gap-0 overflow-hidden border-border/85 py-0 shadow-sm", className)}
    >
      <CardHeader className="border-b border-border/60 bg-muted/35 px-4 py-3">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <FileWarning className="size-4 text-primary" />
            <CardTitle className="text-xs font-semibold">
              Action Review · <span className="font-mono text-primary">{prompt.toolName}</span>
            </CardTitle>
          </div>
          <Badge variant="warning">Needs Review</Badge>
        </div>
        {prompt.description && (
          <p className="mt-1 text-[11px] text-muted-foreground">{prompt.description}</p>
        )}
      </CardHeader>

      {fields.length > 0 && (
        <CardContent className="divide-y divide-border/50 px-4 py-2 text-xs">
          {fields.slice(0, 6).map((field) => (
            <div key={field.key} className="grid grid-cols-[110px_minmax(0,1fr)] gap-2 py-1.5">
              <span className="truncate font-mono text-[11px] text-muted-foreground">
                {field.key}
              </span>
              <span className="break-all font-mono text-[11px] text-foreground">
                {field.value}
              </span>
            </div>
          ))}
        </CardContent>
      )}

      <CardFooter className="flex items-center justify-end gap-2 border-t border-border/60 bg-muted/20 px-4 py-2.5">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChoice("deny")}
          className="gap-1.5"
        >
          <XCircle className="size-3.5" />
          <span>Reject Action</span>
        </Button>
        <Button
          type="button"
          variant="default"
          size="sm"
          onClick={() => onChoice("approve")}
          className="gap-1.5"
        >
          <CheckCircle2 className="size-3.5" />
          <span>Confirm &amp; Run</span>
        </Button>
      </CardFooter>
    </Card>
  )
}
