import * as React from "react"
import {
  Activity,
  AlertTriangle,
  Brain,
  CheckCircle2,
  GitBranch,
  Loader2,
  Search,
  ShieldAlert,
  Sparkles,
  Terminal,
} from "lucide-react"

import { BeuiCollapsible } from "./collapsible"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

export interface AgentActivityStep {
  id: string
  kind: "tool" | "security" | "reasoning" | "notice" | "message"
  noticeKind?:
    | "workflow"
    | "memory"
    | "research"
    | "self_improvement"
    | "source"
    | "system"
  title: string
  detail?: string
  status: "running" | "success" | "error" | "pending" | "info"
  durationLabel?: string
  timestamp?: number
}

export interface AgentActivityProps {
  steps: AgentActivityStep[]
  isActive?: boolean
  mode?: "timeline" | "inline-collapsible"
  title?: string
  className?: string
}

function resolveStepIcon(step: AgentActivityStep) {
  if (step.kind === "tool") return Terminal
  if (step.kind === "security") return ShieldAlert
  if (step.kind === "reasoning") return Brain
  if (step.kind === "notice") {
    if (step.noticeKind === "workflow") return GitBranch
    if (step.noticeKind === "memory") return Brain
    if (step.noticeKind === "research" || step.noticeKind === "source") return Search
    if (step.noticeKind === "self_improvement") return Sparkles
  }
  return Activity
}

/**
 * @beui/agent-activity — Step-by-step live agent activity timeline with pulsing
 * working indicators, completed checkmarks, role icons, and inline collapsible mode.
 */
export const AgentActivity: React.FC<AgentActivityProps> = ({
  steps,
  isActive = false,
  mode = "timeline",
  title,
  className,
}) => {
  if (steps.length === 0) return null

  const runningCount = steps.filter(
    (s) => s.status === "running" || s.status === "pending"
  ).length
  const completedCount = steps.filter((s) => s.status === "success").length

  const renderTimeline = () => (
    <div className="relative space-y-2 pl-1">
      {steps.map((step, index) => {
        const Icon = resolveStepIcon(step)
        const isLast = index === steps.length - 1

        return (
          <div key={step.id} className="relative flex items-start gap-2.5">
            {/* Vertical Timeline Connector */}
            {!isLast && (
              <span
                aria-hidden="true"
                className="absolute top-6 bottom-[-10px] left-[11px] w-px bg-border/75"
              />
            )}

            {/* Status Node */}
            <div
              className={cn(
                "relative z-10 mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full border",
                step.status === "running" &&
                  "border-primary/50 bg-primary/15 text-primary",
                step.status === "success" &&
                  "border-success/40 bg-success/12 text-success",
                step.status === "error" &&
                  "border-destructive/45 bg-destructive/12 text-destructive",
                step.status === "pending" &&
                  "border-warning/45 bg-warning/12 text-warning",
                step.status === "info" &&
                  "border-border bg-muted/60 text-muted-foreground"
              )}
            >
              {step.status === "running" ? (
                <Loader2 className="size-3 animate-spin" />
              ) : step.status === "success" ? (
                <CheckCircle2 className="size-3" />
              ) : step.status === "error" ? (
                <AlertTriangle className="size-3" />
              ) : (
                <Icon className="size-3" />
              )}
            </div>

            {/* Step Card */}
            <div className="min-w-0 flex-1 rounded-lg border border-border/60 bg-card/70 px-2.5 py-2 text-xs transition-colors hover:border-border">
              <div className="flex items-center justify-between gap-2">
                <span className="flex min-w-0 items-center gap-1.5 font-mono text-[11.5px] font-semibold text-foreground">
                  <Icon className="size-3 shrink-0 text-muted-foreground" />
                  <span className="truncate">{step.title}</span>
                </span>
                {step.durationLabel ? (
                  <Badge variant="outline" className="px-1.5 py-0 font-mono text-[9.5px]">
                    {step.durationLabel}
                  </Badge>
                ) : null}
              </div>

              {step.detail && (
                <p className="mt-1 line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">
                  {step.detail}
                </p>
              )}
            </div>
          </div>
        )
      })}
    </div>
  )

  if (mode === "inline-collapsible") {
    return (
      <BeuiCollapsible
        defaultOpen={isActive && runningCount > 0}
        variant="subtle"
        className={cn("my-2", className)}
        icon={
          isActive && runningCount > 0 ? (
            <Loader2 className="size-3.5 animate-spin text-primary" />
          ) : (
            <Activity className="size-3.5 text-primary" />
          )
        }
        title={
          title ||
          (runningCount > 0
            ? `Agent executing (${runningCount} active)`
            : `Agent completed ${completedCount || steps.length} ${steps.length === 1 ? "step" : "steps"}`)
        }
        badge={
          <Badge variant={runningCount > 0 ? "warning" : "outline"} className="text-[10px]">
            {steps.length} {steps.length === 1 ? "step" : "steps"}
          </Badge>
        }
      >
        {renderTimeline()}
      </BeuiCollapsible>
    )
  }

  return <div className={cn("w-full", className)}>{renderTimeline()}</div>
}
