import * as React from "react"
import {
  Check,
  Columns2,
  Copy,
  FileDiff as FileDiffIcon,
  Minimize2,
  Rows2,
} from "lucide-react"

import {
  parseUnifiedDiff,
  tokenizeDiffLine,
  type DiffRow,
  type DiffToken,
} from "./parsers"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export type BeuiDiffViewMode = "unified" | "split" | "compact"

export interface BeuiFileDiffProps {
  patch: string
  file?: string
  defaultView?: BeuiDiffViewMode
  className?: string
}

const TOKEN_STYLE: Record<DiffToken["t"], string> = {
  kw: "text-syntax-kw font-medium",
  fn: "text-syntax-fn",
  str: "text-syntax-str",
  num: "text-syntax-num",
  cm: "text-syntax-cm italic",
  txt: "text-foreground/95",
}

function renderTokens(text: string) {
  const tokens = tokenizeDiffLine(text)
  if (tokens.length === 0) return <span>&nbsp;</span>
  return tokens.map((tok, i) => (
    <span key={i} className={TOKEN_STYLE[tok.t]}>
      {tok.v}
    </span>
  ))
}

interface SplitPair {
  left: DiffRow | null
  right: DiffRow | null
}

function buildSplitPairs(rows: DiffRow[]): SplitPair[] {
  const pairs: SplitPair[] = []
  let i = 0
  while (i < rows.length) {
    const r = rows[i]
    if (r.type === "ctx") {
      pairs.push({ left: r, right: r })
      i += 1
      continue
    }
    const dels: DiffRow[] = []
    const adds: DiffRow[] = []
    while (i < rows.length && rows[i].type === "del") {
      dels.push(rows[i])
      i += 1
    }
    while (i < rows.length && rows[i].type === "add") {
      adds.push(rows[i])
      i += 1
    }
    const maxLen = Math.max(dels.length, adds.length)
    for (let k = 0; k < maxLen; k += 1) {
      pairs.push({
        left: dels[k] ?? null,
        right: adds[k] ?? null,
      })
    }
  }
  return pairs
}

/**
 * @beui/file-diff — Numbered unified, split, and compact patch viewer built on shadcn/ui.
 */
export const BeuiFileDiff: React.FC<BeuiFileDiffProps> = ({
  patch,
  file,
  defaultView = "unified",
  className,
}) => {
  const [view, setView] = React.useState<BeuiDiffViewMode>(defaultView)
  const [copied, setCopied] = React.useState(false)

  React.useEffect(() => {
    setView(defaultView)
  }, [defaultView])

  const parsed = React.useMemo(() => parseUnifiedDiff(patch), [patch])
  const resolvedFile = file || parsed.file || "changes.patch"

  const stats = React.useMemo(() => {
    let additions = 0
    let deletions = 0
    for (const r of parsed.rows) {
      if (r.type === "add") additions += 1
      if (r.type === "del") deletions += 1
    }
    return { additions, deletions }
  }, [parsed.rows])

  const visibleRows = React.useMemo(
    () =>
      view === "compact"
        ? parsed.rows.filter((r) => r.type !== "ctx")
        : parsed.rows,
    [parsed.rows, view]
  )

  const splitPairs = React.useMemo(
    () => (view === "split" ? buildSplitPairs(parsed.rows) : []),
    [parsed.rows, view]
  )

  const handleCopy = () => {
    navigator.clipboard?.writeText(patch).catch(() => {})
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1800)
  }

  return (
    <div
      className={cn(
        "my-3 w-full overflow-hidden rounded-xl border border-border/90 bg-card font-mono text-xs text-foreground shadow-xs",
        className
      )}
    >
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/80 bg-muted/55 px-3 py-1.5">
        <div className="flex min-w-0 items-center gap-2">
          <FileDiffIcon className="size-3.5 shrink-0 text-primary" />
          <span className="truncate text-[11px] font-semibold text-foreground">
            {resolvedFile}
          </span>
          <Badge variant="success" className="px-1.5 py-0 font-mono text-[10px]">
            +{stats.additions}
          </Badge>
          <Badge variant="destructive" className="px-1.5 py-0 font-mono text-[10px]">
            -{stats.deletions}
          </Badge>
        </div>

        <div className="flex items-center gap-1">
          <div className="inline-flex rounded-md border border-border bg-background/80 p-0.5">
            {(
              [
                { id: "unified", label: "Unified", icon: Rows2 },
                { id: "split", label: "Split", icon: Columns2 },
                { id: "compact", label: "Hunks", icon: Minimize2 },
              ] as const
            ).map((opt) => {
              const Icon = opt.icon
              return (
                <button
                  key={opt.id}
                  type="button"
                  onClick={() => setView(opt.id)}
                  className={cn(
                    "flex items-center gap-1 rounded px-2 py-0.5 text-[10px] transition-colors",
                    view === opt.id
                      ? "bg-muted font-medium text-foreground"
                      : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  <Icon className="size-3" />
                  <span>{opt.label}</span>
                </button>
              )
            })}
          </div>

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
                <span>Patch</span>
              </>
            )}
          </Button>
        </div>
      </div>

      <div className="max-h-96 overflow-auto text-[11.5px] leading-relaxed">
        {view !== "split" ? (
          <table className="w-full border-collapse">
            <tbody>
              {visibleRows.map((row, idx) => (
                <tr
                  key={idx}
                  className={cn(
                    row.type === "add" && "bg-diff-add/12",
                    row.type === "del" && "bg-diff-del/12"
                  )}
                >
                  <td className="w-9 select-none border-r border-border/60 px-2 text-right text-[10px] text-muted-foreground/65">
                    {row.old ?? ""}
                  </td>
                  <td className="w-9 select-none border-r border-border/60 px-2 text-right text-[10px] text-muted-foreground/65">
                    {row.cur ?? ""}
                  </td>
                  <td
                    className={cn(
                      "w-6 select-none px-1.5 text-center font-bold",
                      row.type === "add" && "text-diff-add",
                      row.type === "del" && "text-diff-del",
                      row.type === "ctx" && "text-muted-foreground/60"
                    )}
                  >
                    {row.type === "add" ? "+" : row.type === "del" ? "-" : " "}
                  </td>
                  <td className="whitespace-pre pr-4">{renderTokens(row.text)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <table className="w-full border-collapse">
            <tbody>
              {splitPairs.map((pair, idx) => (
                <tr key={idx} className="divide-x divide-border/70">
                  <td className="w-8 select-none px-1.5 text-right text-[10px] text-muted-foreground/65">
                    {pair.left?.old ?? ""}
                  </td>
                  <td
                    className={cn(
                      "w-1/2 whitespace-pre px-2",
                      pair.left?.type === "del" && "bg-diff-del/12"
                    )}
                  >
                    {pair.left ? renderTokens(pair.left.text) : null}
                  </td>
                  <td className="w-8 select-none px-1.5 text-right text-[10px] text-muted-foreground/65">
                    {pair.right?.cur ?? ""}
                  </td>
                  <td
                    className={cn(
                      "w-1/2 whitespace-pre px-2",
                      pair.right?.type === "add" && "bg-diff-add/12"
                    )}
                  >
                    {pair.right ? renderTokens(pair.right.text) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
