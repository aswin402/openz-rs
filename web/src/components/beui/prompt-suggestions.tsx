import * as React from "react"
import { ArrowUpRight } from "lucide-react"

import { Card } from "@/components/ui/card"
import { cn } from "@/lib/utils"

export type BeuiPromptSuggestionsVariant = "bento" | "pills" | "list"

export interface BeuiPromptSuggestionItem {
  id: string
  title: string
  description?: string
  prompt: string
  icon?: React.ComponentType<{ className?: string }>
}

export interface BeuiPromptSuggestionsProps {
  items: BeuiPromptSuggestionItem[]
  variant?: BeuiPromptSuggestionsVariant
  onSelect: (prompt: string) => void
  className?: string
}

/**
 * @beui/prompt-suggestions — Starter prompt cards & smart suggestion chips built with shadcn/ui.
 */
export const BeuiPromptSuggestions: React.FC<BeuiPromptSuggestionsProps> = ({
  items,
  variant = "bento",
  onSelect,
  className,
}) => {
  if (items.length === 0) return null

  if (variant === "pills") {
    return (
      <div className={cn("flex flex-wrap items-center justify-center gap-2", className)}>
        {items.map((item) => {
          const Icon = item.icon
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelect(item.prompt)}
              className="group inline-flex items-center gap-1.5 rounded-full border border-border/80 bg-card/85 px-3.5 py-1.5 text-xs font-medium text-foreground transition-colors hover:border-primary/45 hover:bg-muted/60"
            >
              {Icon && <Icon className="size-3.5 text-primary" />}
              <span>{item.title}</span>
              <ArrowUpRight className="size-3 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:-translate-y-0.5 group-hover:text-primary" />
            </button>
          )
        })}
      </div>
    )
  }

  if (variant === "list") {
    return (
      <div
        className={cn(
          "w-full divide-y divide-border/60 overflow-hidden rounded-xl border border-border/80 bg-card/75 text-left",
          className
        )}
      >
        {items.map((item) => {
          const Icon = item.icon
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelect(item.prompt)}
              className="group flex w-full items-center justify-between gap-3 px-4 py-3 text-left transition-colors hover:bg-muted/45"
            >
              <div className="flex min-w-0 items-center gap-2.5">
                {Icon && <Icon className="size-4 shrink-0 text-primary" />}
                <div className="min-w-0">
                  <div className="truncate text-xs font-semibold text-foreground group-hover:text-primary">
                    {item.title}
                  </div>
                  {item.description && (
                    <div className="truncate text-[11px] text-muted-foreground">
                      {item.description}
                    </div>
                  )}
                </div>
              </div>
              <ArrowUpRight className="size-3.5 shrink-0 text-muted-foreground group-hover:text-primary" />
            </button>
          )
        })}
      </div>
    )
  }

  return (
    <div className={cn("grid w-full grid-cols-1 gap-3 sm:grid-cols-2", className)}>
      {items.map((item) => {
        const Icon = item.icon
        return (
          <Card
            key={item.id}
            onClick={() => onSelect(item.prompt)}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault()
                onSelect(item.prompt)
              }
            }}
            className="group cursor-pointer gap-1.5 bg-card/80 p-4 text-left transition-colors hover:border-primary/45 hover:bg-card"
          >
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2 text-xs font-semibold text-foreground transition-colors group-hover:text-primary">
                {Icon && <Icon className="size-4 text-primary" />}
                <span>{item.title}</span>
              </div>
              <ArrowUpRight className="size-3.5 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
            </div>
            {item.description && (
              <p className="text-xs text-muted-foreground">{item.description}</p>
            )}
          </Card>
        )
      })}
    </div>
  )
}
