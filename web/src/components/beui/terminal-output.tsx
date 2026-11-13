import * as React from "react"
import { Check, ChevronDown, ChevronUp, Copy, Terminal } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export type BeuiTerminalVariant = "console" | "minimal" | "inline"

export interface BeuiTerminalOutputProps {
  command?: string
  output: string
  exitCode?: number | null
  isRunning?: boolean
  variant?: BeuiTerminalVariant
  maxLines?: number
  className?: string
}

/**
 * @beui/terminal-output — Dedicated streaming console output card for shell & build commands.
 */
export const BeuiTerminalOutput: React.FC<BeuiTerminalOutputProps> = ({
  command,
  output,
  exitCode,
  isRunning = false,
  variant = "console",
  maxLines = 18,
  className,
}) => {
  const [copied, setCopied] = React.useState(false)
  const [expanded, setExpanded] = React.useState(false)

  const lines = React.useMemo(() => {
    const clean = output.endsWith("\n") ? output.slice(0, -1) : output
    return clean ? clean.split("\n") : []
  }, [output])

  const truncated = lines.length > maxLines && !expanded
  const shownLines = truncated ? lines.slice(-maxLines) : lines

  const handleCopy = () => {
    navigator.clipboard?.writeText(output).catch(() => {})
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1600)
  }

  if (variant === "inline") {
    return (
      <div
        className={cn(
          "rounded-lg border border-border/65 bg-muted/45 px-3 py-2 font-mono text-[11px] text-foreground",
          className
        )}
      >
        {command && (
          <div className="mb-1 flex items-center gap-1.5 text-primary">
            <span>$</span>
            <span className="truncate">{command}</span>
          </div>
        )}
        <pre className="max-h-40 overflow-auto whitespace-pre-wrap text-foreground/90">
          {shownLines.join("\n") || (isRunning ? "Running command…" : "No stdout")}
        </pre>
      </div>
    )
  }

  return (
    <div
      className={cn(
        "overflow-hidden rounded-xl border font-mono text-xs",
        variant === "console"
          ? "border-border/90 bg-card text-foreground shadow-2xs"
          : "border-border/75 bg-card/80 text-foreground",
        className
      )}
    >
      <div
        className={cn(
          "flex items-center justify-between gap-2 border-b px-3 py-1.5 text-[11px]",
          variant === "console"
            ? "border-border/80 bg-muted/55 text-muted-foreground"
            : "border-border/60 bg-muted/35 text-muted-foreground"
        )}
      >
        <div className="flex min-w-0 items-center gap-2">
          <Terminal className="size-3.5 shrink-0 text-primary" />
          <span className="truncate font-medium text-foreground">
            {command ? `$ ${command}` : "terminal"}
          </span>
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          {typeof exitCode === "number" && (
            <Badge
              variant={exitCode === 0 ? "success" : "destructive"}
              className="px-1.5 py-0 font-mono text-[9.5px]"
            >
              exit {exitCode}
            </Badge>
          )}
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={handleCopy}
            aria-label="Copy terminal output"
            className="text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            {copied ? (
              <Check className="size-3 text-success" />
            ) : (
              <Copy className="size-3" />
            )}
          </Button>
        </div>
      </div>

      <pre className="max-h-64 overflow-auto p-3 text-[11px] leading-relaxed whitespace-pre-wrap">
        {shownLines.length > 0
          ? shownLines.join("\n")
          : isRunning
            ? "Streaming process output…"
            : "(empty output)"}
      </pre>

      {lines.length > maxLines && (
        <button
          type="button"
          onClick={() => setExpanded((prev) => !prev)}
          className="flex w-full items-center justify-center gap-1 border-t border-border/80 bg-muted/45 py-1 text-[10.5px] text-muted-foreground hover:text-foreground"
        >
          {expanded ? (
            <>
              <ChevronUp className="size-3" />
              <span>Collapse output</span>
            </>
          ) : (
            <>
              <ChevronDown className="size-3" />
              <span>Show all {lines.length} lines</span>
            </>
          )}
        </button>
      )}
    </div>
  )
}
