import * as React from "react"
import {
  Bot,
  CheckCircle2,
  Circle,
  GitBranch,
  Loader2,
  XCircle,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import type { OrchestrationRunState } from "@/types"

export type BeuiSubagentCardVariant = "card" | "tree" | "compact"

export interface BeuiSubagentStep {
  id: string
  agent?: string
  status: string
  output?: string
  error?: string
}

export interface BeuiSubagentCardProps {
  agentName: string
  mode?: string
  goal?: string
  status: string
  summary?: string
  durationLabel?: string
  steps?: BeuiSubagentStep[]
  variant?: BeuiSubagentCardVariant
  className?: string
}

function StatusIcon({ status }: { status: string }) {
  if (status === "success" || status === "completed") {
    return <CheckCircle2 className="size-3.5 shrink-0 text-success" />
  }
  if (status === "failed" || status === "error" || status === "cancelled") {
    return <XCircle className="size-3.5 shrink-0 text-destructive" />
  }
  if (status === "running") {
    return <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
  }
  return <Circle className="size-3.5 shrink-0 text-muted-foreground" />
}

export function orchestrationRunToSubagentProps(
  run: OrchestrationRunState
): Omit<BeuiSubagentCardProps, "variant" | "className"> {
  return {
    agentName: run.mode || "orchestrator",
    mode: run.mode,
    goal: run.goal || run.id,
    status: run.status,
    summary: run.summary,
    steps: run.steps.map((s) => ({
      id: s.id,
      agent: s.agent,
      status: s.status,
      output: s.output,
      error: s.error,
    })),
  }
}

/**
 * @beui/subagent-card — Delegation & multi-agent orchestration run card.
 */
export const BeuiSubagentCard: React.FC<BeuiSubagentCardProps> = ({
  agentName,
  mode,
  goal,
  status,
  summary,
  durationLabel,
  steps = [],
  variant = "card",
  className,
}) => {
  if (variant === "compact") {
    return (
      <div
        className={cn(
          "my-1.5 flex items-center justify-between gap-2 rounded-lg border border-border/75 bg-card/75 px-3 py-1.5 text-xs",
          className
        )}
      >
        <div className="flex min-w-0 items-center gap-2">
          <Bot className="size-3.5 shrink-0 text-primary" />
          <span className="font-mono font-semibold text-foreground">{agentName}</span>
          {goal && (
            <span className="truncate text-[11px] text-muted-foreground">{goal}</span>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {durationLabel && (
            <span className="font-mono text-[10px] text-muted-foreground">
              {durationLabel}
            </span>
          )}
          <StatusIcon status={status} />
        </div>
      </div>
    )
  }

  return (
    <div
      className={cn(
        "my-2 rounded-xl border border-border/75 bg-card/80 p-3 text-xs shadow-2xs",
        className
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="inline-flex items-center gap-1 font-mono text-[11px] font-semibold text-primary">
              <Bot className="size-3.5" />
              <span>{agentName}</span>
            </span>
            {mode && (
              <Badge variant="outline" className="gap-1 px-1.5 py-0 font-mono text-[9.5px]">
                <GitBranch className="size-2.5" />
                <span>{mode}</span>
              </Badge>
            )}
          </div>
          {goal && (
            <div className="mt-1 line-clamp-2 font-medium text-foreground">{goal}</div>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          {durationLabel && (
            <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[10px]">
              {durationLabel}
            </Badge>
          )}
          <span className="inline-flex items-center gap-1 text-[10.5px] font-medium capitalize text-muted-foreground">
            <StatusIcon status={status} />
            <span>{status.replaceAll("_", " ")}</span>
          </span>
        </div>
      </div>

      {steps.length > 0 && (
        <div
          className={cn(
            "mt-2.5 space-y-1.5",
            variant === "tree"
              ? "ml-1.5 border-l-2 border-primary/35 pl-3"
              : "border-t border-border/50 pt-2"
          )}
        >
          {steps.map((step) => (
            <div key={step.id} className="flex min-w-0 items-start gap-2">
              <div className="mt-0.5">
                <StatusIcon status={step.status} />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <span className="truncate font-mono text-[11px] font-medium text-foreground">
                    {step.id}
                  </span>
                  {step.agent && (
                    <span className="truncate text-[10px] text-muted-foreground">
                      · {step.agent}
                    </span>
                  )}
                </div>
                {step.output && (
                  <div className="mt-0.5 line-clamp-2 text-[11px] text-muted-foreground">
                    {step.output}
                  </div>
                )}
                {step.error && (
                  <div className="mt-0.5 line-clamp-2 text-[11px] text-destructive">
                    {step.error}
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {summary && (
        <div className="mt-2 border-t border-border/45 pt-2 text-[11px] leading-relaxed text-muted-foreground">
          {summary}
        </div>
      )}
    </div>
  )
}
