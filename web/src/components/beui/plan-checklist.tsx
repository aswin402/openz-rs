import * as React from "react"
import { CheckCircle2, Circle, ListChecks, Loader2, XCircle } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

export type BeuiPlanChecklistVariant = "card" | "compact-steps" | "minimal"

export interface BeuiPlanStep {
  id: string
  title: string
  status: "pending" | "running" | "completed" | "error"
  detail?: string
}

export interface BeuiPlanChecklistProps {
  title?: string
  steps: BeuiPlanStep[]
  variant?: BeuiPlanChecklistVariant
  className?: string
}

function StepIcon({ status }: { status: BeuiPlanStep["status"] }) {
  switch (status) {
    case "completed":
      return <CheckCircle2 className="size-3.5 shrink-0 text-success" />
    case "running":
      return <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
    case "error":
      return <XCircle className="size-3.5 shrink-0 text-destructive" />
    default:
      return <Circle className="size-3.5 shrink-0 text-muted-foreground/60" />
  }
}

/**
 * @beui/plan-checklist — Step-by-step agent task checklist & progress bar.
 */
export const BeuiPlanChecklist: React.FC<BeuiPlanChecklistProps> = ({
  title = "Execution Plan",
  steps,
  variant = "card",
  className,
}) => {
  if (steps.length === 0) return null

  const completedCount = steps.filter((s) => s.status === "completed").length
  const pct = Math.round((completedCount / steps.length) * 100)

  if (variant === "compact-steps") {
    return (
      <div
        className={cn(
          "my-2 flex flex-wrap items-center gap-1.5 rounded-xl border border-border/70 bg-muted/25 px-3 py-2 text-xs",
          className
        )}
      >
        <Badge variant="secondary" className="gap-1 font-mono text-[10px]">
          <ListChecks className="size-3 text-primary" />
          <span>
            {completedCount}/{steps.length}
          </span>
        </Badge>
        {steps.map((step) => (
          <span
            key={step.id}
            className="inline-flex items-center gap-1 rounded-md border border-border/60 bg-background/70 px-2 py-0.5 text-[11px]"
          >
            <StepIcon status={step.status} />
            <span className="truncate max-w-44">{step.title}</span>
          </span>
        ))}
      </div>
    )
  }

  return (
    <div
      className={cn(
        "my-2 overflow-hidden",
        variant === "card"
          ? "rounded-xl border border-border/75 bg-card/75 p-3 shadow-2xs"
          : "border-l-2 border-primary/35 pl-3 py-1",
        className
      )}
    >
      <div className="flex items-center justify-between gap-2 text-xs">
        <div className="flex items-center gap-1.5 font-semibold text-foreground">
          <ListChecks className="size-3.5 text-primary" />
          <span>{title}</span>
        </div>
        <span className="font-mono text-[10.5px] text-muted-foreground">
          {completedCount}/{steps.length} ({pct}%)
        </span>
      </div>

      {variant === "card" && (
        <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-primary transition-all duration-300"
            style={{ width: `${pct}%` }}
          />
        </div>
      )}

      <div className="mt-2.5 space-y-1.5">
        {steps.map((step) => (
          <div key={step.id} className="flex items-start gap-2 text-xs">
            <div className="mt-0.5">
              <StepIcon status={step.status} />
            </div>
            <div className="min-w-0 flex-1">
              <div
                className={cn(
                  "truncate font-medium",
                  step.status === "completed"
                    ? "text-muted-foreground line-through decoration-border"
                    : "text-foreground"
                )}
              >
                {step.title}
              </div>
              {step.detail && (
                <div className="truncate text-[11px] text-muted-foreground">
                  {step.detail}
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
