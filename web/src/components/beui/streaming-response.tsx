import * as React from "react"

import { cn } from "@/lib/utils"

export type BeuiStreamingCursorStyle =
  | "beui-pulse"
  | "smooth-caret"
  | "block-cursor"
  | "none"

export interface StreamingResponseProps {
  isStreaming?: boolean
  cursorStyle?: BeuiStreamingCursorStyle
  footer?: React.ReactNode
  className?: string
  children: React.ReactNode
}

/**
 * @beui/streaming-response — Streaming markdown container with smooth live token
 * cadence, customizable streaming cursor indicator, and response footer slot.
 */
export const StreamingResponse: React.FC<StreamingResponseProps> = ({
  isStreaming = false,
  cursorStyle = "beui-pulse",
  footer,
  className,
  children,
}) => {
  return (
    <div
      data-slot="beui-streaming-response"
      data-streaming={isStreaming ? "true" : "false"}
      className={cn("relative min-w-0 leading-relaxed", className)}
    >
      <div className="min-w-0">
        {children}

        {isStreaming && cursorStyle === "beui-pulse" && (
          <span
            aria-hidden="true"
            className="ml-1.5 inline-flex items-center gap-1 align-middle"
          >
            <span className="size-2 animate-pulse rounded-full bg-primary" />
          </span>
        )}

        {isStreaming && cursorStyle === "smooth-caret" && (
          <span
            aria-hidden="true"
            className="ml-0.5 inline-block h-4 w-[2px] animate-pulse rounded-full bg-primary align-middle"
          />
        )}

        {isStreaming && cursorStyle === "block-cursor" && (
          <span
            aria-hidden="true"
            className="ml-1 inline-block h-4 w-2 animate-pulse rounded-[2px] bg-primary align-middle"
          />
        )}
      </div>

      {footer && <div className="mt-2.5 border-t border-border/45 pt-2">{footer}</div>}
    </div>
  )
}
