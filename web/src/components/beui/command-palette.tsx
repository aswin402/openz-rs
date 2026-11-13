import * as React from "react"
import { CornerDownLeft, Search, X } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

export interface BeuiCommandItem {
  id: string
  group: string
  label: string
  subtitle?: string
  shortcut?: string
  icon?: React.ComponentType<{ className?: string }>
  onSelect: () => void
}

export interface BeuiCommandPaletteProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  items: BeuiCommandItem[]
  placeholder?: string
}

/**
 * @beui/command-palette — Global Cmd+K / Ctrl+K command launcher built with shadcn/ui.
 */
export const BeuiCommandPalette: React.FC<BeuiCommandPaletteProps> = ({
  open,
  onOpenChange,
  items,
  placeholder = "Jump to session, view, Customize UI component, or setting...",
}) => {
  const [query, setQuery] = React.useState("")
  const [activeIndex, setActiveIndex] = React.useState(0)

  React.useEffect(() => {
    if (open) {
      setQuery("")
      setActiveIndex(0)
    }
  }, [open])

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return items
    return items.filter(
      (item) =>
        item.label.toLowerCase().includes(q) ||
        item.group.toLowerCase().includes(q) ||
        (item.subtitle?.toLowerCase().includes(q) ?? false)
    )
  }, [items, query])

  const grouped = React.useMemo(() => {
    const map = new Map<string, BeuiCommandItem[]>()
    for (const item of filtered) {
      const list = map.get(item.group) ?? []
      list.push(item)
      map.set(item.group, list)
    }
    return Array.from(map.entries())
  }, [filtered])

  React.useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault()
        onOpenChange(false)
        return
      }
      if (e.key === "ArrowDown") {
        e.preventDefault()
        setActiveIndex((prev) => (filtered.length === 0 ? 0 : (prev + 1) % filtered.length))
        return
      }
      if (e.key === "ArrowUp") {
        e.preventDefault()
        setActiveIndex((prev) =>
          filtered.length === 0 ? 0 : (prev - 1 + filtered.length) % filtered.length
        )
        return
      }
      if (e.key === "Enter" && filtered[activeIndex]) {
        e.preventDefault()
        filtered[activeIndex].onSelect()
        onOpenChange(false)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [open, filtered, activeIndex, onOpenChange])

  if (!open) return null

  let flatIdx = -1

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command Palette"
      onClick={() => onOpenChange(false)}
      className="fixed inset-0 z-50 flex items-start justify-center bg-foreground/45 p-4 pt-[12vh] backdrop-blur-xs"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-xl overflow-hidden rounded-2xl border border-border bg-popover text-popover-foreground shadow-2xl"
      >
        <div className="flex items-center gap-2 border-b border-border/60 px-3.5 py-2.5">
          <Search className="size-4 shrink-0 text-primary" />
          <Input
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setActiveIndex(0)
            }}
            placeholder={placeholder}
            className="h-8 border-0 bg-transparent px-1 text-sm shadow-none focus-visible:ring-0"
            autoFocus
          />
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
            aria-label="Close command palette"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="max-h-80 overflow-y-auto p-2">
          {grouped.length === 0 ? (
            <div className="py-8 text-center text-xs text-muted-foreground">
              No matching commands or components.
            </div>
          ) : (
            grouped.map(([group, groupItems]) => (
              <div key={group} className="mb-2 last:mb-0">
                <div className="px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                  {group}
                </div>
                {groupItems.map((item) => {
                  flatIdx += 1
                  const idx = flatIdx
                  const Icon = item.icon
                  const selected = idx === activeIndex
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onMouseEnter={() => setActiveIndex(idx)}
                      onClick={() => {
                        item.onSelect()
                        onOpenChange(false)
                      }}
                      className={cn(
                        "flex w-full items-center justify-between gap-3 rounded-lg px-2.5 py-2 text-left text-xs transition-colors",
                        selected
                          ? "bg-primary/15 text-foreground"
                          : "text-muted-foreground hover:bg-muted/60 hover:text-foreground"
                      )}
                    >
                      <div className="flex min-w-0 items-center gap-2.5">
                        {Icon && (
                          <Icon
                            className={cn(
                              "size-4 shrink-0",
                              selected ? "text-primary" : "text-muted-foreground"
                            )}
                          />
                        )}
                        <div className="min-w-0">
                          <div className="truncate font-medium text-foreground">
                            {item.label}
                          </div>
                          {item.subtitle && (
                            <div className="truncate text-[10.5px] text-muted-foreground">
                              {item.subtitle}
                            </div>
                          )}
                        </div>
                      </div>
                      {item.shortcut ? (
                        <Badge variant="outline" className="font-mono text-[10px]">
                          {item.shortcut}
                        </Badge>
                      ) : selected ? (
                        <CornerDownLeft className="size-3.5 shrink-0 text-primary" />
                      ) : null}
                    </button>
                  )
                })}
              </div>
            ))
          )}
        </div>

        <div className="flex items-center justify-between border-t border-border/60 bg-muted/25 px-3.5 py-1.5 text-[10.5px] text-muted-foreground">
          <span>Navigate with ↑ ↓ · Enter to select</span>
          <span className="font-mono">ESC to close</span>
        </div>
      </div>
    </div>
  )
}
