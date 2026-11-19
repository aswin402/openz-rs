import * as React from "react"
import { Braces, Check, ChevronDown, ChevronRight, Copy, Search } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export type BeuiJsonViewerVariant = "tree" | "uiarc-inspector" | "lined" | "compact"

export interface BeuiJsonViewerProps {
  value: unknown
  label?: string
  variant?: BeuiJsonViewerVariant
  defaultDepth?: number
  className?: string
}

function JsonNode({
  name,
  val,
  depth,
  defaultDepth,
  showTypePills = false,
  filterQuery = "",
}: {
  name?: string
  val: unknown
  depth: number
  defaultDepth: number
  showTypePills?: boolean
  filterQuery?: string
}) {
  const [open, setOpen] = React.useState(depth < defaultDepth)
  const lowerQuery = filterQuery.trim().toLowerCase()

  if (val === null || typeof val !== "object") {
    const valStr = String(val)
    if (
      lowerQuery &&
      !(name ?? "").toLowerCase().includes(lowerQuery) &&
      !valStr.toLowerCase().includes(lowerQuery)
    ) {
      return null
    }
    const typeLabel =
      val === null
        ? "null"
        : typeof val === "string"
          ? "str"
          : typeof val === "number"
            ? "num"
            : typeof val === "boolean"
              ? "bool"
              : "val"
    const rendered =
      typeof val === "string" ? (
        <span className="text-syntax-str">&quot;{val}&quot;</span>
      ) : typeof val === "number" ? (
        <span className="text-syntax-num">{String(val)}</span>
      ) : typeof val === "boolean" ? (
        <span className="text-syntax-fn">{String(val)}</span>
      ) : (
        <span className="text-muted-foreground">null</span>
      )

    return (
      <div className="flex items-baseline gap-1.5 rounded px-1 py-0.5 font-mono text-[11px] hover:bg-foreground/[0.035]">
        {name !== undefined && <span className="text-foreground/85">{name}:</span>}
        {showTypePills && (
          <span className="rounded bg-foreground/[0.05] px-1 py-0 text-[9px] text-muted-foreground">
            {typeLabel}
          </span>
        )}
        <span className="break-all">{rendered}</span>
      </div>
    )
  }

  const isArr = Array.isArray(val)
  const entries = isArr
    ? (val as unknown[]).map((item, i) => [String(i), item] as const)
    : Object.entries(val as Record<string, unknown>)

  return (
    <div className="font-mono text-[11px]">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1 rounded px-1 py-0.5 text-left hover:bg-foreground/[0.035] hover:text-primary"
      >
        {open ? (
          <ChevronDown className="size-3 shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight className="size-3 shrink-0 text-muted-foreground" />
        )}
        {name !== undefined && <span className="text-foreground/90">{name}:</span>}
        <span className="rounded bg-foreground/[0.05] px-1.5 py-0 text-[9.5px] text-muted-foreground">
          {isArr ? `arr[${entries.length}]` : `obj{${entries.length}}`}
        </span>
      </button>

      {open && entries.length > 0 && (
        <div className="ml-2.5 border-l border-border/55 pl-2.5">
          {entries.slice(0, 60).map(([k, child]) => (
            <JsonNode
              key={k}
              name={isArr ? `[${k}]` : k}
              val={child}
              depth={depth + 1}
              defaultDepth={defaultDepth}
              showTypePills={showTypePills}
              filterQuery={filterQuery}
            />
          ))}
        </div>
      )}
    </div>
  )
}

/**
 * @beui/json-viewer + `@uiarc/json-viewer` — Interactive collapsible JSON tree & payload inspector.
 */
export const BeuiJsonViewer: React.FC<BeuiJsonViewerProps> = ({
  value,
  label = "JSON Payload",
  variant = "tree",
  defaultDepth = 2,
  className,
}) => {
  const [copied, setCopied] = React.useState(false)
  const [filterQuery, setFilterQuery] = React.useState("")

  const parsed = React.useMemo(() => {
    if (typeof value === "string") {
      const trimmed = value.trim()
      if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
        try {
          return JSON.parse(trimmed) as unknown
        } catch {
          return value
        }
      }
    }
    return value
  }, [value])

  const formatted = React.useMemo(() => {
    try {
      return typeof parsed === "string" ? parsed : JSON.stringify(parsed, null, 2)
    } catch {
      return String(parsed)
    }
  }, [parsed])

  const handleCopy = () => {
    navigator.clipboard?.writeText(formatted).catch(() => {})
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1600)
  }

  return (
    <div
      className={cn(
        "overflow-hidden rounded-xl border border-border/75 bg-card/75",
        className
      )}
    >
      <div className="flex items-center justify-between border-b border-border/55 bg-muted/35 px-3 py-1.5 text-[11px]">
        <div className="flex items-center gap-1.5 font-medium text-foreground">
          <Braces className="size-3.5 text-primary" />
          <span>{label}</span>
          <Badge variant="outline" className="px-1.5 py-0 font-mono text-[9.5px]">
            {variant}
          </Badge>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={handleCopy}
          aria-label="Copy JSON"
        >
          {copied ? (
            <Check className="size-3 text-success" />
          ) : (
            <Copy className="size-3 text-muted-foreground" />
          )}
        </Button>
      </div>

      {variant === "uiarc-inspector" && (
        <div className="flex items-center gap-1.5 border-b border-border/45 bg-background/50 px-3 py-1">
          <Search className="size-3 text-muted-foreground" />
          <input
            type="text"
            value={filterQuery}
            onChange={(e) => setFilterQuery(e.target.value)}
            placeholder="Filter keys or values…"
            className="w-full bg-transparent font-mono text-[10.5px] text-foreground placeholder:text-muted-foreground/70 focus:outline-none"
          />
        </div>
      )}

      {(variant === "tree" || variant === "uiarc-inspector") &&
      parsed !== null &&
      typeof parsed === "object" ? (
        <div className="max-h-60 overflow-auto p-3">
          <JsonNode
            val={parsed}
            depth={0}
            defaultDepth={defaultDepth}
            showTypePills={variant === "uiarc-inspector"}
            filterQuery={filterQuery}
          />
        </div>
      ) : (
        <pre
          className={cn(
            "max-h-60 overflow-auto p-3 font-mono text-[11px] leading-relaxed text-foreground/90",
            variant === "compact" && "whitespace-pre-wrap text-[10.5px]"
          )}
        >
          {variant === "compact" ? JSON.stringify(parsed) : formatted}
        </pre>
      )}
    </div>
  )
}
