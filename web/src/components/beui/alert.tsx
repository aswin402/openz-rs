import * as React from "react"
import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Info,
  X,
} from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export type BeuiAlertTone = "default" | "info" | "success" | "warning" | "destructive"

export interface BeuiAlertProps {
  tone?: BeuiAlertTone
  title?: React.ReactNode
  children: React.ReactNode
  icon?: React.ReactNode
  onDismiss?: () => void
  compact?: boolean
  className?: string
}

const TONE_ICON: Record<BeuiAlertTone, React.ComponentType<{ className?: string }>> = {
  default: Info,
  info: Info,
  success: CheckCircle2,
  warning: AlertTriangle,
  destructive: AlertCircle,
}

/**
 * @beui/alert — Styled alert banner built on shadcn/ui Alert with semantic tones
 * and optional inline dismiss trigger.
 */
export const BeuiAlert: React.FC<BeuiAlertProps> = ({
  tone = "default",
  title,
  children,
  icon,
  onDismiss,
  compact = false,
  className,
}) => {
  const IconComp = TONE_ICON[tone]

  return (
    <Alert
      variant={tone}
      className={cn(
        "t-msg-enter items-center",
        compact && "py-1.5 px-3 text-[11px]",
        onDismiss && "pr-9",
        className
      )}
    >
      {icon ?? <IconComp className="size-3.5 shrink-0" />}
      {title && <AlertTitle>{title}</AlertTitle>}
      <AlertDescription className={cn(compact && "text-[11px]")}>
        {children}
      </AlertDescription>
      {onDismiss && (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={onDismiss}
          className="absolute top-1.5 right-1.5 opacity-75 hover:opacity-100"
          aria-label="Dismiss notice"
        >
          <X className="size-3" />
        </Button>
      )}
    </Alert>
  )
}
