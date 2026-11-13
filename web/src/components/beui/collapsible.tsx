import * as React from "react"
import { ChevronRight } from "lucide-react"

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { cn } from "@/lib/utils"

export type BeuiCollapsibleVariant = "bordered" | "subtle" | "ghost"

export interface BeuiCollapsibleProps {
  title: React.ReactNode
  icon?: React.ReactNode
  badge?: React.ReactNode
  actions?: React.ReactNode
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  variant?: BeuiCollapsibleVariant
  className?: string
  triggerClassName?: string
  contentClassName?: string
  children: React.ReactNode
}

/**
 * @beui/collapsible — Smooth disclosure container built on shadcn/ui Collapsible
 * with animated chevron rotation and grid-height transition.
 */
export const BeuiCollapsible: React.FC<BeuiCollapsibleProps> = ({
  title,
  icon,
  badge,
  actions,
  open,
  defaultOpen = false,
  onOpenChange,
  variant = "bordered",
  className,
  triggerClassName,
  contentClassName,
  children,
}) => {
  const [uncontrolledOpen, setUncontrolledOpen] = React.useState(defaultOpen)
  const isControlled = open !== undefined
  const isOpen = isControlled ? open : uncontrolledOpen

  const handleOpenChange = (next: boolean) => {
    if (!isControlled) {
      setUncontrolledOpen(next)
    }
    onOpenChange?.(next)
  }

  return (
    <Collapsible
      open={isOpen}
      onOpenChange={handleOpenChange}
      className={cn(
        "group/beui-collapsible w-full overflow-hidden transition-colors",
        variant === "bordered" &&
          "rounded-xl border border-border/75 bg-card/65 hover:border-border",
        variant === "subtle" &&
          "rounded-xl border border-border/45 bg-muted/25",
        variant === "ghost" && "rounded-lg",
        className
      )}
    >
      <div className="flex w-full items-center justify-between gap-2">
        <CollapsibleTrigger
          type="button"
          className={cn(
            "flex min-w-0 flex-1 items-center justify-between gap-2.5 px-3 py-2 text-left text-xs font-medium text-foreground transition-colors outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/40",
            variant === "ghost" && "px-1.5 py-1 hover:text-primary",
            triggerClassName
          )}
        >
          <span className="flex min-w-0 items-center gap-2">
            <ChevronRight
              className={cn(
                "size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 ease-out",
                isOpen && "rotate-90 text-primary"
              )}
            />
            {icon && <span className="shrink-0 text-primary">{icon}</span>}
            <span className="min-w-0 truncate">{title}</span>
          </span>
          {badge && <span className="shrink-0">{badge}</span>}
        </CollapsibleTrigger>

        {actions && (
          <div className="flex shrink-0 items-center gap-1 pr-2.5">{actions}</div>
        )}
      </div>

      <CollapsibleContent
        forceMount
        className={cn(
          "grid transition-[grid-template-rows,opacity] duration-200 ease-out",
          isOpen
            ? "grid-rows-[1fr] opacity-100"
            : "pointer-events-none grid-rows-[0fr] opacity-0"
        )}
      >
        <div className="min-h-0 overflow-hidden">
          <div
            className={cn(
              variant !== "ghost" && "border-t border-border/45 px-3 py-2.5",
              variant === "ghost" && "pt-1.5 pl-5",
              contentClassName
            )}
          >
            {children}
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}
