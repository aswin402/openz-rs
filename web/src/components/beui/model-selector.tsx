import * as React from "react"
import {
  Check,
  ChevronsUpDown,
  Cpu,
  Loader2,
  Search,
  Star,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

export type BeuiModelSelectorVariant = "command" | "compact-pill" | "provider-badge"

export interface BeuiModelGroup {
  name: string
  display?: string
  models: string[]
  available?: boolean
  full?: boolean
}

export interface BeuiModelEntry {
  provider: string
  model: string
}

export interface BeuiModelSelectorProps {
  activeModel: string
  activeProvider: string
  groups: BeuiModelGroup[]
  favorites?: BeuiModelEntry[]
  recents?: BeuiModelEntry[]
  loadingProvider?: string | null
  variant?: BeuiModelSelectorVariant
  onSelectModel: (provider: string, model: string) => void
  onToggleFavorite?: (provider: string, model: string) => void
  onLoadAllForProvider?: (provider: string) => void
  className?: string
}

/**
 * @beui/model-selector — Searchable shadcn/ui + BEUI provider/model command switcher.
 */
export const BeuiModelSelector: React.FC<BeuiModelSelectorProps> = ({
  activeModel,
  activeProvider,
  groups,
  favorites = [],
  recents = [],
  loadingProvider = null,
  variant = "command",
  onSelectModel,
  onToggleFavorite,
  onLoadAllForProvider,
  className,
}) => {
  const [open, setOpen] = React.useState(false)
  const [search, setSearch] = React.useState("")
  const triggerRef = React.useRef<HTMLButtonElement>(null)
  const panelRef = React.useRef<HTMLDivElement>(null)

  React.useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent) => {
      if (triggerRef.current?.contains(e.target as Node)) return
      if (panelRef.current?.contains(e.target as Node)) return
      setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false)
    }
    document.addEventListener("mousedown", onDown)
    window.addEventListener("keydown", onKey)
    return () => {
      document.removeEventListener("mousedown", onDown)
      window.removeEventListener("keydown", onKey)
    }
  }, [open])

  const shortModel = React.useMemo(() => {
    if (!activeModel) return "Select model"
    const parts = activeModel.split("/")
    return parts[parts.length - 1] || activeModel
  }, [activeModel])

  const activeProviderDisplay = React.useMemo(() => {
    const found = groups.find((g) => g.name === activeProvider)
    return found?.display || activeProvider || "auto"
  }, [groups, activeProvider])

  const query = search.trim().toLowerCase()

  const filteredGroups = React.useMemo(
    () =>
      groups
        .map((g) => ({
          ...g,
          models: query
            ? g.models.filter(
                (m) =>
                  m.toLowerCase().includes(query) ||
                  (g.display || g.name).toLowerCase().includes(query)
              )
            : g.models,
        }))
        .filter((g) => g.models.length > 0),
    [groups, query]
  )

  const isFav = (provider: string, model: string) =>
    favorites.some((f) => f.provider === provider && f.model === model)

  const pick = (provider: string, model: string) => {
    onSelectModel(provider, model)
    setOpen(false)
    setSearch("")
  }

  return (
    <div className={cn("relative inline-flex", className)}>
      <Button
        ref={triggerRef}
        type="button"
        variant={variant === "compact-pill" ? "ghost" : "secondary"}
        size="xs"
        onClick={() => setOpen((o) => !o)}
        className={cn(
          "max-w-[230px] gap-1.5 font-mono text-[10.5px]",
          variant === "compact-pill" &&
            "rounded-full border border-border/65 bg-muted/40 px-2.5",
          variant === "provider-badge" && "pl-1.5"
        )}
        title="Switch active LLM model"
      >
        {!activeModel ? (
          <>
            <Loader2 className="size-3 animate-spin text-primary" />
            <span className="text-muted-foreground">model</span>
          </>
        ) : (
          <>
            {variant === "provider-badge" ? (
              <Badge
                variant="outline"
                className="border-primary/35 bg-primary/10 px-1.5 py-0 text-[9px] text-primary"
              >
                {activeProviderDisplay}
              </Badge>
            ) : (
              <Cpu className="size-3 shrink-0 text-primary" />
            )}
            <span className="truncate">{shortModel}</span>
            <ChevronsUpDown className="size-3 shrink-0 text-muted-foreground" />
          </>
        )}
      </Button>

      {open && (
        <div
          ref={panelRef}
          className="t-popover absolute bottom-full left-0 z-40 mb-2 w-80 overflow-hidden rounded-xl border border-border bg-popover text-popover-foreground shadow-xl"
        >
          <div className="border-b border-border/60 p-2">
            <div className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search models or providers..."
                className="h-7 pl-8 text-xs"
                autoFocus
              />
            </div>
          </div>

          <div className="max-h-72 overflow-y-auto p-1.5 text-xs">
            {!query && favorites.length > 0 && (
              <div className="mb-1.5">
                <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                  Favorites
                </div>
                {favorites.slice(0, 6).map((entry) => {
                  const selected =
                    entry.model === activeModel && entry.provider === activeProvider
                  return (
                    <button
                      key={`fav::${entry.provider}::${entry.model}`}
                      type="button"
                      onClick={() => pick(entry.provider, entry.model)}
                      className={cn(
                        "flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left transition-colors hover:bg-muted/70",
                        selected && "bg-primary/15 font-medium text-primary"
                      )}
                    >
                      <span className="truncate font-mono text-[11px]">{entry.model}</span>
                      <span className="ml-2 shrink-0 text-[10px] text-muted-foreground">
                        {entry.provider}
                      </span>
                    </button>
                  )
                })}
              </div>
            )}

            {!query && recents.length > 0 && (
              <div className="mb-1.5">
                <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                  Recent
                </div>
                {recents.slice(0, 4).map((entry) => {
                  const selected =
                    entry.model === activeModel && entry.provider === activeProvider
                  return (
                    <button
                      key={`rec::${entry.provider}::${entry.model}`}
                      type="button"
                      onClick={() => pick(entry.provider, entry.model)}
                      className={cn(
                        "flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left transition-colors hover:bg-muted/70",
                        selected && "bg-primary/15 font-medium text-primary"
                      )}
                    >
                      <span className="truncate font-mono text-[11px]">{entry.model}</span>
                      <span className="ml-2 shrink-0 text-[10px] text-muted-foreground">
                        {entry.provider}
                      </span>
                    </button>
                  )
                })}
              </div>
            )}

            {filteredGroups.length === 0 ? (
              <div className="px-3 py-4 text-center text-[11px] text-muted-foreground">
                No matching models found.
              </div>
            ) : (
              filteredGroups.map((g) => (
                <div key={g.name} className="mb-1.5">
                  <div className="flex items-center justify-between px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    <span>{g.display || g.name}</span>
                    {onLoadAllForProvider && (
                      <button
                        type="button"
                        onClick={() => onLoadAllForProvider(g.name)}
                        disabled={loadingProvider === g.name || g.full === true}
                        className="rounded px-1.5 py-0.5 font-sans text-[9.5px] font-medium normal-case text-primary hover:bg-primary/10 disabled:text-muted-foreground"
                      >
                        {loadingProvider === g.name
                          ? "loading…"
                          : g.full
                            ? "all loaded"
                            : "load all"}
                      </button>
                    )}
                  </div>

                  {g.models.map((m) => {
                    const selected = m === activeModel && g.name === activeProvider
                    const fav = isFav(g.name, m)
                    return (
                      <div
                        key={`${g.name}::${m}`}
                        className={cn(
                          "flex items-center rounded-lg transition-colors hover:bg-muted/70",
                          selected && "bg-primary/15 font-medium text-primary"
                        )}
                      >
                        <button
                          type="button"
                          onClick={() => pick(g.name, m)}
                          className="flex min-w-0 flex-1 items-center gap-1.5 px-2 py-1.5 text-left"
                        >
                          <Check
                            className={cn(
                              "size-3 shrink-0",
                              selected ? "text-primary opacity-100" : "opacity-0"
                            )}
                          />
                          <span className="truncate font-mono text-[11px]">{m}</span>
                        </button>
                        {onToggleFavorite && (
                          <button
                            type="button"
                            onClick={() => onToggleFavorite(g.name, m)}
                            className={cn(
                              "mr-1 flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-muted",
                              fav && "text-primary"
                            )}
                            aria-label={fav ? "Remove favorite" : "Add favorite"}
                          >
                            <Star
                              className="size-3"
                              fill={fav ? "currentColor" : "none"}
                            />
                          </button>
                        )}
                      </div>
                    )
                  })}
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  )
}
