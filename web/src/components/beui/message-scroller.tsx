import * as React from "react"
import { ArrowDown } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export type ScrollerBehaviorMode = "beui-smart" | "always-follow" | "manual"

export interface MessageScrollerProps {
  sessionKey?: string
  itemCount: number
  isStreaming?: boolean
  behaviorMode?: ScrollerBehaviorMode
  thresholdPx?: number
  className?: string
  contentClassName?: string
  jumpPillClassName?: string
  children: React.ReactNode
}

/**
 * @beui/message-scroller — Reader-aware chat transcript scroller.
 * Automatically follows streaming tokens at the bottom edge, immediately releases
 * scroll-lock when the user scrolls upward to read history, and provides an
 * interactive floating Jump-to-Latest button.
 */
export const MessageScroller: React.FC<MessageScrollerProps> = ({
  sessionKey,
  itemCount,
  isStreaming = false,
  behaviorMode = "beui-smart",
  thresholdPx = 140,
  className,
  contentClassName,
  jumpPillClassName,
  children,
}) => {
  const viewportRef = React.useRef<HTMLDivElement>(null)
  const contentRef = React.useRef<HTMLDivElement>(null)
  const lockedToBottomRef = React.useRef(true)
  const programmaticScrollUntilRef = React.useRef(0)
  const prevSessionRef = React.useRef(sessionKey)
  const prevCountRef = React.useRef(itemCount)
  const [showJumpPill, setShowJumpPill] = React.useState(false)

  const getDistanceFromBottom = React.useCallback((el: HTMLDivElement) => {
    return el.scrollHeight - el.scrollTop - el.clientHeight
  }, [])

  const scrollToBottom = React.useCallback((smooth: boolean) => {
    const el = viewportRef.current
    if (!el) return
    lockedToBottomRef.current = true
    setShowJumpPill(false)
    if (smooth) {
      programmaticScrollUntilRef.current = Date.now() + 450
      el.scrollTo({ top: el.scrollHeight, behavior: "smooth" })
    } else {
      programmaticScrollUntilRef.current = 0
      el.scrollTop = el.scrollHeight
    }
  }, [])

  // Session switch -> jump straight to bottom
  React.useEffect(() => {
    if (prevSessionRef.current !== sessionKey) {
      prevSessionRef.current = sessionKey
      prevCountRef.current = itemCount
      lockedToBottomRef.current = true
      setShowJumpPill(false)
      requestAnimationFrame(() => scrollToBottom(false))
    }
  }, [sessionKey, itemCount, scrollToBottom])

  // New message appended or session history hydrated
  React.useEffect(() => {
    const el = viewportRef.current
    if (!el) return
    const prevCount = prevCountRef.current
    const grew = itemCount > prevCount
    prevCountRef.current = itemCount

    if (!grew || behaviorMode === "manual") return

    if (behaviorMode === "always-follow" || lockedToBottomRef.current) {
      const isInitialHydration = prevCount === 0
      if (isInitialHydration) {
        requestAnimationFrame(() => scrollToBottom(false))
      } else {
        scrollToBottom(!isStreaming)
      }
    } else {
      setShowJumpPill(true)
    }
  }, [itemCount, isStreaming, behaviorMode, scrollToBottom])

  // ResizeObserver tracks streaming token growth inside the active turn
  React.useEffect(() => {
    const contentEl = contentRef.current
    const viewportEl = viewportRef.current
    if (!contentEl || !viewportEl || typeof ResizeObserver === "undefined") {
      return
    }

    const observer = new ResizeObserver(() => {
      if (behaviorMode === "manual") {
        const dist = getDistanceFromBottom(viewportEl)
        setShowJumpPill(dist > thresholdPx)
        return
      }
      if (behaviorMode === "always-follow" || lockedToBottomRef.current) {
        viewportEl.scrollTop = viewportEl.scrollHeight
      } else {
        const dist = getDistanceFromBottom(viewportEl)
        setShowJumpPill(dist > thresholdPx)
      }
    })

    observer.observe(contentEl)
    return () => observer.disconnect()
  }, [behaviorMode, getDistanceFromBottom, thresholdPx])

  const handleScroll = () => {
    const el = viewportRef.current
    if (!el) return
    const distance = getDistanceFromBottom(el)
    const nearBottom = distance <= thresholdPx
    if (!nearBottom && Date.now() < programmaticScrollUntilRef.current) {
      return
    }
    if (nearBottom) {
      programmaticScrollUntilRef.current = 0
    }
    lockedToBottomRef.current = nearBottom
    setShowJumpPill(!nearBottom && itemCount > 0)
  }

  return (
    <div className={cn("relative flex min-h-0 flex-1 flex-col overflow-hidden", className)}>
      <div
        ref={viewportRef}
        onScroll={handleScroll}
        className="size-full overflow-y-scroll overscroll-contain [scrollbar-gutter:stable]"
      >
        <div ref={contentRef} className={cn("w-full py-5", contentClassName)}>
          {children}
        </div>
      </div>

      {showJumpPill && (
        <div
          className={cn(
            "pointer-events-none absolute right-0 bottom-3 left-0 z-20 flex justify-center",
            jumpPillClassName
          )}
        >
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => scrollToBottom(true)}
            className="t-popover pointer-events-auto gap-1.5 rounded-full border-border bg-card/95 px-3.5 shadow-md backdrop-blur-xs hover:border-primary/50"
            aria-label="Scroll to latest message"
          >
            {isStreaming && (
              <span className="size-1.5 animate-pulse rounded-full bg-primary" />
            )}
            <ArrowDown className="size-3.5 text-primary" />
            <span className="text-xs font-medium">
              {isStreaming ? "Follow live stream" : "Jump to latest"}
            </span>
          </Button>
        </div>
      )}
    </div>
  )
}
