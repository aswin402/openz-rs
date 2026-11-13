import * as React from "react"
import { ArrowDownAZ, ArrowUpAZ, Check, Copy, Search, Table2 } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

export type BeuiDataTableVariant = "interactive" | "striped" | "minimal"

export interface BeuiDataTableProps {
  headers: string[]
  rows: string[][]
  variant?: BeuiDataTableVariant
  className?: string
}

/**
 * @beui/data-table — Interactive sortable, filterable, copyable Markdown table built with shadcn/ui.
 */
export const BeuiDataTable: React.FC<BeuiDataTableProps> = ({
  headers,
  rows,
  variant = "interactive",
  className,
}) => {
  const [query, setQuery] = React.useState("")
  const [sortCol, setSortCol] = React.useState<number | null>(null)
  const [sortAsc, setSortAsc] = React.useState(true)
  const [copied, setCopied] = React.useState(false)

  const filteredAndSorted = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    const base = q
      ? rows.filter((row) => row.some((cell) => cell.toLowerCase().includes(q)))
      : rows

    if (sortCol === null) return base
    return [...base].sort((a, b) => {
      const av = a[sortCol] ?? ""
      const bv = b[sortCol] ?? ""
      const an = Number(av)
      const bn = Number(bv)
      const cmp =
        !Number.isNaN(an) && !Number.isNaN(bn) && av.trim() !== "" && bv.trim() !== ""
          ? an - bn
          : av.localeCompare(bv)
      return sortAsc ? cmp : -cmp
    })
  }, [rows, query, sortCol, sortAsc])

  const handleSort = (idx: number) => {
    if (variant !== "interactive") return
    if (sortCol === idx) {
      setSortAsc((prev) => !prev)
    } else {
      setSortCol(idx)
      setSortAsc(true)
    }
  }

  const handleCopyCsv = () => {
    const csvLines = [
      headers.join("\t"),
      ...filteredAndSorted.map((r) => r.join("\t")),
    ].join("\n")
    navigator.clipboard?.writeText(csvLines).catch(() => {})
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1600)
  }

  if (variant === "minimal") {
    return (
      <div className={cn("my-3 w-full overflow-x-auto", className)}>
        <table className="w-full border-collapse text-left text-xs">
          <thead>
            <tr className="border-b border-border text-[11px] font-semibold text-muted-foreground">
              {headers.map((h, i) => (
                <th key={i} className="px-3 py-2">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-border/50">
            {rows.map((row, rIdx) => (
              <tr key={rIdx} className="hover:bg-muted/25">
                {headers.map((_, cIdx) => (
                  <td key={cIdx} className="px-3 py-2 text-foreground/90">
                    {row[cIdx] ?? ""}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    )
  }

  return (
    <div
      className={cn(
        "my-3 w-full overflow-hidden rounded-xl border border-border/80 bg-card/65 shadow-2xs",
        className
      )}
    >
      {variant === "interactive" && (
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/60 bg-muted/30 px-3 py-2">
          <div className="flex items-center gap-2">
            <Table2 className="size-3.5 text-primary" />
            <Badge variant="secondary" className="px-1.5 py-0 font-mono text-[10px]">
              {filteredAndSorted.length} {filteredAndSorted.length === 1 ? "row" : "rows"}
            </Badge>
          </div>
          <div className="flex items-center gap-1.5">
            {rows.length > 3 && (
              <div className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="Filter rows..."
                  className="h-6 w-36 pl-6 text-[11px]"
                />
              </div>
            )}
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={handleCopyCsv}
              className="h-6 text-[10.5px] text-muted-foreground"
            >
              {copied ? (
                <>
                  <Check className="size-3 text-success" />
                  <span>Copied</span>
                </>
              ) : (
                <>
                  <Copy className="size-3" />
                  <span>Copy TSV</span>
                </>
              )}
            </Button>
          </div>
        </div>
      )}

      <div className="overflow-x-auto">
        <table className="w-full min-w-[380px] border-collapse text-left text-xs">
          <thead>
            <tr className="border-b border-border/70 bg-muted/40 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
              {headers.map((header, idx) => (
                <th
                  key={idx}
                  onClick={() => handleSort(idx)}
                  className={cn(
                    "select-none px-3.5 py-2.5",
                    variant === "interactive" && "cursor-pointer hover:text-foreground"
                  )}
                >
                  <span className="inline-flex items-center gap-1">
                    <span>{header}</span>
                    {variant === "interactive" && sortCol === idx && (
                      sortAsc ? (
                        <ArrowDownAZ className="size-3 text-primary" />
                      ) : (
                        <ArrowUpAZ className="size-3 text-primary" />
                      )
                    )}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-border/50">
            {filteredAndSorted.map((row, rIdx) => (
              <tr
                key={rIdx}
                className={cn(
                  "transition-colors hover:bg-muted/30",
                  variant === "striped" && rIdx % 2 === 1 && "bg-muted/20"
                )}
              >
                {headers.map((_, cIdx) => (
                  <td key={cIdx} className="px-3.5 py-2.5 text-foreground/90">
                    {row[cIdx] ?? ""}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
