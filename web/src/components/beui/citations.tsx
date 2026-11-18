import * as React from "react"
import { BookOpen, ExternalLink, Globe } from "lucide-react"

import { BeuiCollapsible } from "./collapsible"
import { extractMarkdownCitations, type CiteRef } from "./parsers"
import { Badge } from "@/components/ui/badge"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

export type BeuiCitationsVariant =
  | "pills"
  | "cards"
  | "footnotes"
  | "kokonut-citations"
  | "minimal-inline"

export interface BeuiCitationsProps {
  refs: CiteRef[]
  defaultOpen?: boolean
  variant?: BeuiCitationsVariant
  className?: string
}

function parseHostnameZeroRegex(url: string): string {
  const protoIdx = url.indexOf("://")
  const afterProto = protoIdx !== -1 ? url.slice(protoIdx + 3) : url
  const slashIdx = afterProto.indexOf("/")
  const rawHost = slashIdx !== -1 ? afterProto.slice(0, slashIdx) : afterProto
  return rawHost.startsWith("www.") ? rawHost.slice(4) : rawHost
}

export function extractAllMarkdownSources(content: string): {
  bodyText: string
  refs: CiteRef[]
} {
  const base = extractMarkdownCitations(content)
  if (base.refs.length > 0) {
    return { bodyText: base.bodyText, refs: base.refs }
  }

  const refs: CiteRef[] = []
  const seenUrls = new Set<string>()
  let cursor = 0

  while (cursor < content.length) {
    const openBracket = content.indexOf("[", cursor)
    if (openBracket === -1) break
    if (openBracket > 0 && content[openBracket - 1] === "!") {
      cursor = openBracket + 1
      continue
    }
    const closeBracket = content.indexOf("](", openBracket + 1)
    if (closeBracket === -1) {
      cursor = openBracket + 1
      continue
    }
    const closeParen = content.indexOf(")", closeBracket + 2)
    if (closeParen === -1) {
      cursor = closeBracket + 2
      continue
    }

    const label = content.slice(openBracket + 1, closeBracket).trim()
    const rawUrl = content.slice(closeBracket + 2, closeParen).trim()
    if (
      (rawUrl.startsWith("https://") || rawUrl.startsWith("http://")) &&
      !seenUrls.has(rawUrl)
    ) {
      seenUrls.add(rawUrl)
      const host = parseHostnameZeroRegex(rawUrl)
      refs.push({
        n: refs.length + 1,
        label: label || host,
        host,
        url: rawUrl,
      })
    }
    cursor = closeParen + 1
  }

  return { bodyText: content, refs }
}

/**
 * @beui/citations — Interactive numbered source badges, source card deck, or footnotes.
 */
export const BeuiCitations: React.FC<BeuiCitationsProps> = ({
  refs,
  defaultOpen = false,
  variant = "pills",
  className,
}) => {
  if (refs.length === 0) return null

  if (variant === "footnotes") {
    return (
      <div className={cn("space-y-1 text-xs", className)}>
        {refs.map((ref) => (
          <a
            key={`${ref.n}-${ref.url}`}
            href={ref.url}
            target="_blank"
            rel="noreferrer"
            className="group flex items-center gap-2 rounded-md px-1.5 py-1 text-muted-foreground transition-colors hover:bg-muted/45 hover:text-foreground"
          >
            <span className="font-mono text-[10px] font-semibold text-primary">
              [{ref.n}]
            </span>
            <span className="truncate font-medium text-foreground/90 group-hover:text-primary">
              {ref.label || ref.host}
            </span>
            <span className="font-mono text-[10px] text-muted-foreground">
              · {ref.host}
            </span>
            <ExternalLink className="ml-auto size-3 shrink-0 opacity-60 group-hover:opacity-100" />
          </a>
        ))}
      </div>
    )
  }

  if (variant === "minimal-inline" || variant === "kokonut-citations") {
    return (
      <div className={cn("flex flex-wrap items-center gap-1.5 text-xs", className)}>
        {refs.map((ref) => (
          <a
            key={`${ref.n}-${ref.url}`}
            href={ref.url}
            target="_blank"
            rel="noreferrer"
            className={cn(
              "inline-flex items-center gap-1 transition-colors",
              variant === "kokonut-citations"
                ? "rounded-md border border-border/55 bg-foreground/[0.035] px-2 py-0.5 text-[11px] text-foreground/85 hover:border-primary/40 hover:text-primary"
                : "text-[11px] text-muted-foreground hover:text-primary"
            )}
          >
            <Globe className="size-2.5 shrink-0 text-primary/75" />
            <span className="max-w-[170px] truncate">{ref.host}</span>
            <span className="font-mono text-[9.5px] text-muted-foreground">[{ref.n}]</span>
          </a>
        ))}
      </div>
    )
  }

  if (variant === "pills" && !defaultOpen) {
    return (
      <div className={cn("flex flex-wrap items-center gap-1.5", className)}>
        <span className="mr-1 inline-flex items-center gap-1 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
          <BookOpen className="size-3 text-primary" />
          <span>Sources</span>
        </span>
        {refs.map((ref) => (
          <Tooltip key={`${ref.n}-${ref.url}`}>
            <TooltipTrigger asChild>
              <a
                href={ref.url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex max-w-[210px] items-center gap-1.5 rounded-full border border-border/60 bg-foreground/[0.035] px-2.5 py-0.5 text-[11px] text-foreground/90 transition-colors hover:border-primary/45 hover:bg-foreground/[0.06]"
              >
                <span className="flex size-4 shrink-0 items-center justify-center rounded-full bg-primary/18 font-mono text-[9px] font-bold text-primary">
                  {ref.n}
                </span>
                <span className="truncate font-medium">{ref.label || ref.host}</span>
                <ExternalLink className="size-2.5 shrink-0 text-muted-foreground" />
              </a>
            </TooltipTrigger>
            <TooltipContent className="max-w-xs space-y-0.5 p-2.5">
              <div className="font-semibold text-popover-foreground">{ref.label}</div>
              <div className="flex items-center gap-1 font-mono text-[10px] text-muted-foreground">
                <Globe className="size-2.5 text-primary" />
                <span className="truncate">{ref.host}</span>
              </div>
            </TooltipContent>
          </Tooltip>
        ))}
      </div>
    )
  }

  return (
    <BeuiCollapsible
      title={`Cited Sources (${refs.length})`}
      icon={<BookOpen className="size-3.5" />}
      badge={<Badge variant="outline">{refs.length}</Badge>}
      defaultOpen={defaultOpen || variant === "cards"}
      variant="subtle"
      className={className}
    >
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {refs.map((ref) => (
          <a
            key={`${ref.n}-${ref.url}`}
            href={ref.url}
            target="_blank"
            rel="noreferrer"
            className="group flex items-start gap-2.5 rounded-lg border border-border/65 bg-card/80 p-2.5 text-xs transition-colors hover:border-primary/45 hover:bg-card"
          >
            <span className="mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-md bg-primary/15 font-mono text-[10px] font-bold text-primary">
              {ref.n}
            </span>
            <span className="min-w-0 flex-1">
              <span className="line-clamp-1 font-medium text-foreground group-hover:text-primary">
                {ref.label}
              </span>
              <span className="mt-0.5 flex items-center gap-1 font-mono text-[10px] text-muted-foreground">
                <Globe className="size-2.5" />
                <span className="truncate">{ref.host}</span>
              </span>
            </span>
            <ExternalLink className="size-3 shrink-0 text-muted-foreground opacity-60 group-hover:opacity-100" />
          </a>
        ))}
      </div>
    </BeuiCollapsible>
  )
}
