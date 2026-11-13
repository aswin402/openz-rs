import * as React from "react"
import {
  Check,
  Copy,
  CornerDownRight,
  GitBranch,
  Pencil,
  RotateCcw,
  ThumbsDown,
  ThumbsUp,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

export type BeuiMessageActionsVariant = "floating" | "ghost" | "compact"

export interface BeuiMessageActionsProps {
  text: string
  isUser?: boolean
  variant?: BeuiMessageActionsVariant
  onRegenerate?: () => void
  onEdit?: () => void
  onReply?: () => void
  onFork?: () => void
  className?: string
}

/**
 * @beui/message-actions — Clean shadcn/ui + BEUI action toolbar for assistant & user turns.
 */
export const BeuiMessageActions: React.FC<BeuiMessageActionsProps> = ({
  text,
  isUser = false,
  variant = "floating",
  onRegenerate,
  onEdit,
  onReply,
  onFork,
  className,
}) => {
  const [copied, setCopied] = React.useState(false)
  const [vote, setVote] = React.useState<"up" | "down" | null>(null)

  const handleCopy = () => {
    navigator.clipboard?.writeText(text).catch(() => {})
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1600)
  }

  const containerCls =
    variant === "floating"
      ? "inline-flex items-center gap-0.5 rounded-lg border border-border/75 bg-card/95 p-0.5 shadow-2xs"
      : variant === "compact"
        ? "inline-flex items-center gap-0.5 rounded-md bg-muted/45 px-1 py-0.5"
        : "inline-flex items-center gap-0.5"

  const btnSize = variant === "compact" ? "icon-xs" : "icon-xs"

  const renderAction = (
    key: string,
    label: string,
    icon: React.ReactNode,
    onClick: () => void,
    active = false
  ) => (
    <Tooltip key={key}>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size={btnSize}
          onClick={onClick}
          aria-label={label}
          className={cn(
            "size-6 rounded-md text-muted-foreground hover:bg-muted hover:text-foreground",
            active && "bg-primary/15 text-primary hover:text-primary"
          )}
        >
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="text-[11px]">
        {label}
      </TooltipContent>
    </Tooltip>
  )

  return (
    <div className={cn(containerCls, className)}>
      {renderAction(
        "copy",
        copied ? "Copied" : "Copy message",
        copied ? <Check className="size-3 text-success" /> : <Copy className="size-3" />,
        handleCopy,
        copied
      )}

      {isUser && onEdit
        ? renderAction("edit", "Edit & resend", <Pencil className="size-3" />, onEdit)
        : null}

      {!isUser && onRegenerate
        ? renderAction("regen", "Regenerate", <RotateCcw className="size-3" />, onRegenerate)
        : null}

      {onReply
        ? renderAction("reply", "Quote reply", <CornerDownRight className="size-3" />, onReply)
        : null}

      {!isUser && variant !== "compact" && (
        <>
          {renderAction(
            "up",
            "Helpful",
            <ThumbsUp className="size-3" />,
            () => setVote((v) => (v === "up" ? null : "up")),
            vote === "up"
          )}
          {renderAction(
            "down",
            "Needs improvement",
            <ThumbsDown className="size-3" />,
            () => setVote((v) => (v === "down" ? null : "down")),
            vote === "down"
          )}
        </>
      )}

      {!isUser && onFork && variant !== "compact"
        ? renderAction("fork", "New session", <GitBranch className="size-3" />, onFork)
        : null}
    </div>
  )
}
