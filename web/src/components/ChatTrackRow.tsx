import * as React from "react"
import { cn } from "@/lib/utils"
import type { ChatWidthChoice } from "@/store/useUiCustomizationStore"

export interface ChatTrackRowProps {
  chatWidth?: ChatWidthChoice
  leftGutter?: React.ReactNode
  rightGutter?: React.ReactNode
  className?: string
  centerClassName?: string
  children: React.ReactNode
}

/**
 * Shared 3-column geometry for OpenZ Chat:
 * - Column 1 (Left outer gutter): Full Red-Orange Fox icon (positioned outside to the left of ChatBox)
 * - Column 2 (Center track): Exact width shared by ChatInput, Assistant messages, User messages, and HeroWelcome
 *   so message left & right edges align pixel-for-pixel with the two ends of the ChatBox!
 * - Column 3 (Right outer gutter): User ("ours") icon (positioned outside to the right of ChatBox)
 */
export const ChatTrackRow: React.FC<ChatTrackRowProps> = ({
  chatWidth = "standard",
  leftGutter,
  rightGutter,
  className,
  centerClassName,
  children,
}) => {
  const maxWidthClass =
    chatWidth === "full"
      ? "max-w-none"
      : chatWidth === "wide"
        ? "max-w-[68rem]"
        : "max-w-[54rem]"

  return (
    <div
      className={cn(
        "mx-auto grid w-full grid-cols-[32px_minmax(0,1fr)_32px] items-start gap-2.5 px-3 sm:grid-cols-[38px_minmax(0,1fr)_38px] sm:gap-3.5 sm:px-6",
        maxWidthClass,
        className
      )}
    >
      <div className="col-start-1 flex min-h-px items-start justify-start">
        {leftGutter}
      </div>
      <div className={cn("col-start-2 min-w-0 w-full", centerClassName)}>
        {children}
      </div>
      <div className="col-start-3 flex min-h-px items-start justify-end">
        {rightGutter}
      </div>
    </div>
  )
}
