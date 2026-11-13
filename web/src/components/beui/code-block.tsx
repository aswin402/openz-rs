import * as React from "react"
import {
  Check,
  ChevronDown,
  ChevronUp,
  Copy,
  Download,
  FileCode2,
  WrapText,
} from "lucide-react"

import { tokenizeDiffLine, type DiffToken } from "./parsers"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export type BeuiCodeBlockVariant = "framed" | "minimal" | "compact"

export interface BeuiCodeBlockProps {
  code: string
  language?: string
  filename?: string
  variant?: BeuiCodeBlockVariant
  showLineNumbers?: boolean
  defaultWrap?: boolean
  collapsibleAfterLines?: number
  className?: string
}

const TOKEN_COLOR: Record<DiffToken["t"], string> = {
  kw: "text-syntax-kw font-medium",
  fn: "text-syntax-fn",
  str: "text-syntax-str",
  num: "text-syntax-num",
  cm: "text-syntax-cm italic",
  txt: "text-foreground/95",
}

/**
 * @beui/code-block — Syntax-highlighted code viewer built with shadcn/ui primitives.
 */
export const BeuiCodeBlock: React.FC<BeuiCodeBlockProps> = ({
  code,
  language = "text",
  filename,
  variant = "framed",
  showLineNumbers = true,
  defaultWrap = false,
  collapsibleAfterLines = 28,
  className,
}) => {
  const [copied, setCopied] = React.useState(false)
  const [wrap, setWrap] = React.useState(defaultWrap)
  const [expanded, setExpanded] = React.useState(false)

  const trimmedCode = React.useMemo(
    () => (code.endsWith("\n") ? code.slice(0, -1) : code),
    [code]
  )
  const lines = React.useMemo(() => trimmedCode.split("\n"), [trimmedCode])
  const isCollapsible = lines.length > collapsibleAfterLines
  const visibleLines =
    isCollapsible && !expanded ? lines.slice(0, collapsibleAfterLines) : lines

  const effectiveLineNumbers = variant === "compact" ? false : showLineNumbers

  const handleCopy = () => {
    navigator.clipboard?.writeText(trimmedCode).catch(() => {})
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1800)
  }

  const handleDownload = () => {
    const ext = language && language !== "text" ? language : "txt"
    const blob = new Blob([trimmedCode], { type: "text/plain;charset=utf-8" })
    const url = URL.createObjectURL(blob)
    const link = document.createElement("a")
    link.href = url
    link.download = filename || `snippet.${ext}`
    link.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div
      className={cn(
        "my-3 w-full overflow-hidden rounded-xl font-mono text-xs text-foreground",
        variant === "minimal"
          ? "border border-border/75 bg-card/85"
          : "border border-border/90 bg-card shadow-xs",
        className
      )}
    >
      <div
        className={cn(
          "flex select-none items-center justify-between gap-2 px-3",
          variant === "compact"
            ? "border-b border-border/60 bg-card py-1"
            : variant === "minimal"
              ? "border-b border-border/60 bg-muted/35 py-1.5"
              : "border-b border-border/80 bg-muted/55 py-1.5"
        )}
      >
        <div className="flex min-w-0 items-center gap-2">
          {variant === "framed" && <FileCode2 className="size-3.5 shrink-0 text-primary" />}
          {filename ? (
            <span className="truncate text-[11px] font-medium text-foreground">
              {filename}
            </span>
          ) : null}
          <Badge
            variant="outline"
            className="border-border/80 bg-muted/70 px-1.5 py-0 font-mono text-[10px] uppercase tracking-wider text-muted-foreground"
          >
            {language || "code"}
          </Badge>
          {variant !== "compact" && (
            <span className="text-[10px] text-muted-foreground">
              {lines.length} {lines.length === 1 ? "line" : "lines"}
            </span>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {variant !== "compact" && (
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={() => setWrap((w) => !w)}
              className={cn(
                "h-6 px-2 text-[10px] text-muted-foreground hover:bg-muted hover:text-foreground",
                wrap && "bg-muted text-primary"
              )}
              title="Toggle line wrap"
            >
              <WrapText className="size-3" />
              <span className="hidden sm:inline">Wrap</span>
            </Button>
          )}
          {variant === "framed" && (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={handleDownload}
              className="text-muted-foreground hover:bg-muted hover:text-foreground"
              title="Download snippet"
              aria-label="Download snippet"
            >
              <Download className="size-3" />
            </Button>
          )}
          <Button
            type="button"
            variant="ghost"
            size="xs"
            onClick={handleCopy}
            className="h-6 px-2 text-[10px] text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            {copied ? (
              <>
                <Check className="size-3 text-success" />
                <span className="text-success">Copied</span>
              </>
            ) : (
              <>
                <Copy className="size-3" />
                <span>Copy</span>
              </>
            )}
          </Button>
        </div>
      </div>

      <div
        className={cn(
          "overflow-x-auto leading-relaxed",
          variant === "compact" ? "py-2 text-[11.5px]" : "py-2.5 text-[12px]"
        )}
      >
        <table className="w-full border-collapse">
          <tbody>
            {visibleLines.map((line, idx) => {
              const tokens = tokenizeDiffLine(line)
              return (
                <tr key={idx} className="hover:bg-muted/35">
                  {effectiveLineNumbers && (
                    <td className="w-10 select-none pr-3 pl-3 text-right align-top text-[10px] text-muted-foreground/65">
                      {idx + 1}
                    </td>
                  )}
                  <td
                    className={cn(
                      "pr-4",
                      !effectiveLineNumbers && "pl-3.5",
                      wrap ? "whitespace-pre-wrap break-all" : "whitespace-pre"
                    )}
                  >
                    {tokens.length === 0 ? (
                      <span>&nbsp;</span>
                    ) : (
                      tokens.map((tok, tIdx) => (
                        <span key={tIdx} className={TOKEN_COLOR[tok.t]}>
                          {tok.v}
                        </span>
                      ))
                    )}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      {isCollapsible && (
        <div className="flex justify-center border-t border-border/80 bg-muted/45 px-3 py-1">
          <Button
            type="button"
            variant="ghost"
            size="xs"
            onClick={() => setExpanded((e) => !e)}
            className="h-6 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            {expanded ? (
              <>
                <ChevronUp className="size-3" />
                <span>Collapse code</span>
              </>
            ) : (
              <>
                <ChevronDown className="size-3" />
                <span>Show all {lines.length} lines</span>
              </>
            )}
          </Button>
        </div>
      )}
    </div>
  )
}
