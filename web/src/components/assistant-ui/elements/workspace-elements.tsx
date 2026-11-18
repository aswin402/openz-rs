"use client";

import * as React from "react";
import {
  AlertCircle,
  Check,
  ChevronRight,
  Copy,
  Download,
  ExternalLink,
  FileCode2,
  FilePlus2,
  FileText,
  Folder,
  FolderOpen,
  Globe,
  RefreshCw,
  Search,
  Sigma,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { ToolExecution } from "@/types";
import {
  CollapsibleRoot as Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "./collapsible-root";
import { collapsePanel, field, mono, ShimmerLabel } from "./surfaces";

/* -------------------------------------------------------------------------- */
/* 1. File Tree (`assistant-ui/elements/file-tree`)                           */
/* -------------------------------------------------------------------------- */

export type FileTreeStyleChoice = "assistant-tree" | "compact-paths" | "card-explorer";

export interface FileTreeEntry {
  path: string;
  action: "read" | "created" | "modified" | "listed";
  additions?: number;
  deletions?: number;
}

export const FILE_TREE_STYLE_OPTIONS: ReadonlyArray<{
  id: FileTreeStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-tree",
    title: "Assistant-UI Hierarchical File Tree (Default)",
    description:
      "assistant-ui/elements/file-tree — Collapsible folder tree with file icons, action status, and +/- churn metrics.",
    isDefault: true,
  },
  {
    id: "compact-paths",
    title: "Minimal Path Chips",
    description:
      "Ultra-compact inline file path pills for quiet turn summaries.",
  },
  {
    id: "card-explorer",
    title: "Framed Workspace Explorer",
    description:
      "Expanded bordered file explorer card grouping touched directories.",
  },
];

interface TreeFolderNode {
  name: string;
  folders: Map<string, TreeFolderNode>;
  files: Array<{ name: string; entry: FileTreeEntry }>;
}

function buildDirectoryTree(entries: ReadonlyArray<FileTreeEntry>): TreeFolderNode {
  const root: TreeFolderNode = { name: "", folders: new Map(), files: [] };
  for (const entry of entries) {
    const cleaned = entry.path.startsWith("./") ? entry.path.slice(2) : entry.path;
    const parts = cleaned.split("/").filter(Boolean);
    if (parts.length === 0) continue;
    let cursor = root;
    for (let i = 0; i < parts.length - 1; i += 1) {
      const segment = parts[i] ?? "";
      let next = cursor.folders.get(segment);
      if (!next) {
        next = { name: segment, folders: new Map(), files: [] };
        cursor.folders.set(segment, next);
      }
      cursor = next;
    }
    cursor.files.push({ name: parts[parts.length - 1] ?? entry.path, entry });
  }
  return root;
}

export function extractTurnFileTree(
  toolCalls: ReadonlyArray<ToolExecution> | undefined,
): FileTreeEntry[] {
  if (!toolCalls || toolCalls.length === 0) return [];
  const map = new Map<string, FileTreeEntry>();

  for (const tc of toolCalls) {
    const lower = tc.name.toLowerCase();
    const argsObj =
      tc.args && typeof tc.args === "object" && !Array.isArray(tc.args)
        ? (tc.args as Record<string, unknown>)
        : null;
    const rawPath =
      (typeof argsObj?.path === "string" && argsObj.path) ||
      (typeof argsObj?.file_path === "string" && argsObj.file_path) ||
      (typeof argsObj?.target_file === "string" && argsObj.target_file) ||
      "";
    if (!rawPath) continue;

    let action: FileTreeEntry["action"] | null = null;
    if (lower === "write_file" || lower === "create_file") action = "created";
    else if (lower === "patch_file" || lower === "replace_lines" || lower === "edit_file")
      action = "modified";
    else if (lower === "read_file") action = "read";
    else if (lower === "list_dir" || lower === "find_files") action = "listed";

    if (!action) continue;

    const contentStr = typeof argsObj?.content === "string" ? argsObj.content : "";
    const additions =
      action === "created" && contentStr ? contentStr.split("\n").length : undefined;

    const prev = map.get(rawPath);
    if (!prev || action === "created" || action === "modified") {
      map.set(rawPath, {
        path: rawPath,
        action,
        additions,
      });
    }
  }

  return Array.from(map.values());
}

const FolderBranchView: React.FC<{ node: TreeFolderNode; depth: number }> = ({
  node,
  depth,
}) => {
  const [open, setOpen] = React.useState(true);
  const childFolders = Array.from(node.folders.values());

  return (
    <div className="space-y-0.5">
      {node.name && (
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          style={{ paddingLeft: `${depth * 12}px` }}
          className="flex w-full items-center gap-1.5 rounded-md py-0.5 pr-2 text-left text-xs text-muted-foreground transition-colors hover:bg-foreground/[0.04] hover:text-foreground"
        >
          <ChevronRight
            className={cn("size-3 shrink-0 transition-transform", open && "rotate-90")}
          />
          {open ? (
            <FolderOpen className="size-3.5 shrink-0 text-primary/80" />
          ) : (
            <Folder className="size-3.5 shrink-0 text-muted-foreground" />
          )}
          <span className="truncate font-mono text-[11.5px]">{node.name}</span>
        </button>
      )}
      {open && (
        <div>
          {childFolders.map((sub) => (
            <FolderBranchView
              key={sub.name}
              node={sub}
              depth={node.name ? depth + 1 : depth}
            />
          ))}
          {node.files.map(({ name, entry }) => {
            const padLeft = (node.name ? depth + 1 : depth) * 12 + 14;
            return (
              <div
                key={entry.path}
                style={{ paddingLeft: `${padLeft}px` }}
                className="flex items-center justify-between gap-2 rounded-md py-0.5 pr-2 text-xs hover:bg-foreground/[0.035]"
              >
                <div className="flex min-w-0 items-center gap-1.5">
                  <FileCode2 className="size-3.5 shrink-0 text-muted-foreground" />
                  <span className="truncate font-mono text-[11.5px] text-foreground/90">
                    {name}
                  </span>
                </div>
                <div className="flex shrink-0 items-center gap-1.5 font-mono text-[10px]">
                  {typeof entry.additions === "number" && entry.additions > 0 && (
                    <span className="text-success">+{entry.additions}</span>
                  )}
                  {typeof entry.deletions === "number" && entry.deletions > 0 && (
                    <span className="text-destructive">-{entry.deletions}</span>
                  )}
                  <span
                    className={cn(
                      "rounded px-1 py-0 text-[9.5px]",
                      entry.action === "created"
                        ? "bg-success/12 text-success"
                        : entry.action === "modified"
                          ? "bg-primary/12 text-primary"
                          : "text-muted-foreground",
                    )}
                  >
                    {entry.action}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export interface FileTreeElementProps {
  entries: ReadonlyArray<FileTreeEntry>;
  title?: string;
  variant?: FileTreeStyleChoice;
  defaultOpen?: boolean;
  className?: string;
}

export const FileTreeElement: React.FC<FileTreeElementProps> = ({
  entries,
  title,
  variant = "assistant-tree",
  defaultOpen = false,
  className,
}) => {
  const [open, setOpen] = React.useState(defaultOpen || variant === "card-explorer");
  const tree = React.useMemo(() => buildDirectoryTree(entries), [entries]);

  if (entries.length === 0) return null;

  if (variant === "compact-paths") {
    return (
      <div className={cn("my-1.5 flex flex-wrap items-center gap-1.5 not-prose", className)}>
        {entries.map((entry) => (
          <span
            key={entry.path}
            className="inline-flex items-center gap-1.5 rounded-md border border-border/55 bg-foreground/[0.035] px-2 py-0.5 font-mono text-[10.5px] text-foreground/85"
          >
            <span
              className={cn(
                "size-1.5 rounded-full",
                entry.action === "created"
                  ? "bg-success"
                  : entry.action === "modified"
                    ? "bg-primary"
                    : "bg-muted-foreground/60",
              )}
            />
            <span className="max-w-[220px] truncate">{entry.path}</span>
          </span>
        ))}
      </div>
    );
  }

  const label =
    title ??
    `${entries.length} workspace ${entries.length === 1 ? "file" : "files"} touched`;

  return (
    <Collapsible
      data-slot="file-tree"
      open={open}
      onOpenChange={setOpen}
      className={cn(
        "my-2 w-full not-prose",
        variant === "card-explorer" &&
          "rounded-xl border border-border/70 bg-card/80 p-3",
        className,
      )}
    >
      <CollapsibleTrigger className="group flex w-full items-center gap-2 rounded-md py-1 text-left text-xs text-muted-foreground transition-colors hover:text-foreground">
        <ChevronRight
          className={cn("size-3.5 shrink-0 transition-transform", open && "rotate-90")}
        />
        <Folder className="size-3.5 shrink-0 text-primary/85" />
        <span className="font-medium text-foreground/90">{label}</span>
      </CollapsibleTrigger>
      <CollapsibleContent className={cn(collapsePanel, "outline-none")}>
        <div className={cn(field, "mt-1.5 rounded-xl p-2.5")}>
          <FolderBranchView node={tree} depth={0} />
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
};

/* -------------------------------------------------------------------------- */
/* 2. Error State (`assistant-ui/elements/error-state`)                       */
/* -------------------------------------------------------------------------- */

export type ErrorStateStyleChoice =
  | "assistant-quiet"
  | "diagnostic-card"
  | "minimal-inline";

export const ERROR_STATE_STYLE_OPTIONS: ReadonlyArray<{
  id: ErrorStateStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-quiet",
    title: "Assistant-UI Quiet Failure Banner (Default)",
    description:
      "assistant-ui/elements/error-state — Calm low-noise error banner with integrated one-click Retry.",
    isDefault: true,
  },
  {
    id: "diagnostic-card",
    title: "Expandable Diagnostic Error Card",
    description:
      "Structured error frame with collapsible stack/output trace for fast debugging.",
  },
  {
    id: "minimal-inline",
    title: "Minimal Single-Line Error",
    description:
      "Hairline inline indicator with subtle red dot and Retry action.",
  },
];

export interface ErrorStateElementProps {
  title?: string;
  message: string;
  detail?: string;
  onRetry?: () => void;
  variant?: ErrorStateStyleChoice;
  className?: string;
}

export const ErrorStateElement: React.FC<ErrorStateElementProps> = ({
  title = "Step encountered an error",
  message,
  detail,
  onRetry,
  variant = "assistant-quiet",
  className,
}) => {
  const [showDetail, setShowDetail] = React.useState(false);

  if (variant === "minimal-inline") {
    return (
      <div
        className={cn(
          "my-1.5 flex flex-wrap items-center gap-2 text-xs text-destructive not-prose",
          className,
        )}
      >
        <span className="size-1.5 shrink-0 rounded-full bg-destructive" />
        <span className="min-w-0 flex-1 truncate">{message}</span>
        {onRetry && (
          <button
            type="button"
            onClick={onRetry}
            className="inline-flex items-center gap-1 font-medium text-foreground/85 hover:text-primary"
          >
            <RefreshCw className="size-3" />
            <span>Retry</span>
          </button>
        )}
      </div>
    );
  }

  return (
    <div
      data-slot="error-state"
      className={cn(
        "my-2 w-full rounded-xl border border-destructive/25 bg-destructive/[0.06] px-3.5 py-2.5 text-xs not-prose",
        className,
      )}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <AlertCircle className="size-4 shrink-0 text-destructive" />
          <div className="min-w-0">
            <div className="font-medium text-foreground">{title}</div>
            <div className="truncate text-[11.5px] text-muted-foreground">{message}</div>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {variant === "diagnostic-card" && detail && (
            <button
              type="button"
              onClick={() => setShowDetail((s) => !s)}
              className="rounded-md px-2 py-1 font-mono text-[10.5px] text-muted-foreground hover:bg-foreground/[0.06] hover:text-foreground"
            >
              {showDetail ? "Hide trace" : "Trace"}
            </button>
          )}
          {onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="inline-flex items-center gap-1.5 rounded-lg border border-border/70 bg-card/90 px-2.5 py-1 text-[11px] font-medium text-foreground transition-colors hover:border-primary/40 hover:text-primary"
            >
              <RefreshCw className="size-3" />
              <span>Retry</span>
            </button>
          )}
        </div>
      </div>

      {variant === "diagnostic-card" && showDetail && detail && (
        <pre className="mt-2 max-h-40 overflow-auto rounded-lg border border-border/50 bg-background/75 p-2.5 font-mono text-[10.5px] whitespace-pre-wrap text-muted-foreground">
          {detail}
        </pre>
      )}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 3. Web Search (`assistant-ui/elements/web-search` + `rare-ui/web-search`)  */
/* -------------------------------------------------------------------------- */

export type WebSearchStyleChoice =
  | "assistant-web-search"
  | "rare-web-search"
  | "minimal-links";

export interface WebSearchHit {
  title: string;
  url: string;
  snippet?: string;
}

export interface WebSearchData {
  id: string;
  query: string;
  running: boolean;
  results: WebSearchHit[];
}

export const WEB_SEARCH_STYLE_OPTIONS: ReadonlyArray<{
  id: WebSearchStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-web-search",
    title: "Assistant-UI Web Search Deck (Default)",
    description:
      "assistant-ui/elements/web-search — Collapsible search query row with domain cards and snippets.",
    isDefault: true,
  },
  {
    id: "rare-web-search",
    title: "Rare-UI Search Capsule",
    description:
      "swamimalode07/rare-ui/web-search — Sleek floating search capsule with source domain pills.",
  },
  {
    id: "minimal-links",
    title: "Minimal Domain Strip",
    description:
      "Single-line quiet domain chips without snippet expansion.",
  },
];

function extractDomainZeroRegex(url: string): string {
  const idx = url.indexOf("://");
  const rest = idx !== -1 ? url.slice(idx + 3) : url;
  const slash = rest.indexOf("/");
  const host = slash !== -1 ? rest.slice(0, slash) : rest;
  return host.startsWith("www.") ? host.slice(4) : host;
}

export function extractTurnWebSearches(
  toolCalls: ReadonlyArray<ToolExecution> | undefined,
): WebSearchData[] {
  if (!toolCalls || toolCalls.length === 0) return [];
  const items: WebSearchData[] = [];

  for (const tc of toolCalls) {
    const lower = tc.name.toLowerCase();
    if (lower !== "web_search" && lower !== "web_fetch" && lower !== "crawl_site") {
      continue;
    }
    const argsObj =
      tc.args && typeof tc.args === "object" && !Array.isArray(tc.args)
        ? (tc.args as Record<string, unknown>)
        : null;
    const query =
      (typeof argsObj?.query === "string" && argsObj.query) ||
      (typeof argsObj?.url === "string" && argsObj.url) ||
      tc.name;

    const results: WebSearchHit[] = [];
    if (tc.output && tc.output.trim().startsWith("{")) {
      try {
        const parsed = JSON.parse(tc.output) as Record<string, unknown>;
        const arr = Array.isArray(parsed.results) ? parsed.results : [];
        for (const rawHit of arr) {
          if (rawHit && typeof rawHit === "object") {
            const h = rawHit as Record<string, unknown>;
            if (typeof h.url === "string") {
              results.push({
                title: typeof h.title === "string" ? h.title : extractDomainZeroRegex(h.url),
                url: h.url,
                snippet: typeof h.content === "string" ? h.content : typeof h.snippet === "string" ? h.snippet : undefined,
              });
            }
          }
        }
      } catch {
        // ignore non-JSON output
      }
    } else if (typeof argsObj?.url === "string") {
      results.push({
        title: extractDomainZeroRegex(argsObj.url),
        url: argsObj.url,
      });
    }

    items.push({
      id: tc.id,
      query,
      running: tc.status === "running",
      results,
    });
  }

  return items;
}

export interface WebSearchElementProps {
  data: WebSearchData;
  variant?: WebSearchStyleChoice;
  defaultOpen?: boolean;
  className?: string;
}

export const WebSearchElement: React.FC<WebSearchElementProps> = ({
  data,
  variant = "assistant-web-search",
  defaultOpen = false,
  className,
}) => {
  const [open, setOpen] = React.useState(defaultOpen);

  if (variant === "minimal-links") {
    return (
      <div className={cn("my-1.5 flex flex-wrap items-center gap-1.5 text-xs not-prose", className)}>
        <Search className="size-3 text-muted-foreground" />
        <span className="text-muted-foreground">{data.query}</span>
        {data.results.slice(0, 4).map((hit, idx) => (
          <a
            key={`${hit.url}-${idx}`}
            href={hit.url}
            target="_blank"
            rel="noreferrer"
            className="rounded-md border border-border/55 bg-foreground/[0.035] px-1.5 py-0.5 font-mono text-[10px] text-foreground/80 hover:border-primary/40 hover:text-primary"
          >
            {extractDomainZeroRegex(hit.url)}
          </a>
        ))}
      </div>
    );
  }

  if (variant === "rare-web-search") {
    return (
      <div
        className={cn(
          "my-2 rounded-2xl border border-border/65 bg-card/80 px-3.5 py-2.5 text-xs not-prose",
          className,
        )}
      >
        <div className="flex items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <span className="inline-flex size-6 shrink-0 items-center justify-center rounded-full border border-primary/25 bg-primary/10 text-primary">
              <Search className="size-3" />
            </span>
            <ShimmerLabel active={data.running} className="truncate font-medium text-foreground">
              {data.running ? `Searching "${data.query}"…` : data.query}
            </ShimmerLabel>
          </div>
          <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
            {data.results.length} {data.results.length === 1 ? "source" : "sources"}
          </span>
        </div>
        {data.results.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {data.results.slice(0, 5).map((hit, idx) => (
              <a
                key={`${hit.url}-${idx}`}
                href={hit.url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1.5 rounded-full border border-border/55 bg-background/70 px-2.5 py-0.5 text-[11px] text-foreground/85 transition-colors hover:border-primary/40 hover:text-primary"
              >
                <Globe className="size-2.5 text-primary/80" />
                <span className="max-w-[160px] truncate">{hit.title}</span>
              </a>
            ))}
          </div>
        )}
      </div>
    );
  }

  // Default: `assistant-web-search`
  return (
    <Collapsible
      data-slot="web-search"
      open={open}
      onOpenChange={setOpen}
      className={cn("my-1.5 w-full not-prose", className)}
    >
      <CollapsibleTrigger className="group flex w-full items-center gap-2 rounded-md py-1 text-left text-xs text-muted-foreground transition-colors hover:text-foreground">
        <ChevronRight
          className={cn("size-3.5 shrink-0 transition-transform", open && "rotate-90")}
        />
        <Search className="size-3.5 shrink-0 text-primary/85" />
        <ShimmerLabel active={data.running} className="truncate font-medium text-foreground/90">
          {data.running ? `Searching ${data.query}…` : `Searched "${data.query}"`}
        </ShimmerLabel>
        {data.results.length > 0 && (
          <span className={cn(mono, "ms-auto text-muted-foreground")}>
            {data.results.length} results
          </span>
        )}
      </CollapsibleTrigger>
      {data.results.length > 0 && (
        <CollapsibleContent className={cn(collapsePanel, "outline-none")}>
          <div className={cn(field, "mt-1.5 space-y-1.5 rounded-xl p-2.5")}>
            {data.results.slice(0, 6).map((hit, idx) => (
              <a
                key={`${hit.url}-${idx}`}
                href={hit.url}
                target="_blank"
                rel="noreferrer"
                className="flex items-start justify-between gap-2 rounded-lg p-2 transition-colors hover:bg-foreground/[0.045]"
              >
                <div className="min-w-0">
                  <div className="truncate text-xs font-medium text-foreground">
                    {hit.title}
                  </div>
                  <div className="truncate font-mono text-[10px] text-muted-foreground">
                    {extractDomainZeroRegex(hit.url)}
                  </div>
                  {hit.snippet && (
                    <p className="mt-0.5 line-clamp-2 text-[11px] text-muted-foreground">
                      {hit.snippet}
                    </p>
                  )}
                </div>
                <ExternalLink className="mt-0.5 size-3 shrink-0 text-muted-foreground" />
              </a>
            ))}
          </div>
        </CollapsibleContent>
      )}
    </Collapsible>
  );
};

/* -------------------------------------------------------------------------- */
/* 4. Artifact Card (`assistant-ui/elements/artifact-card`)                   */
/* -------------------------------------------------------------------------- */

export type ArtifactCardStyleChoice =
  | "assistant-artifact"
  | "compact-chip"
  | "split-preview";

export interface ArtifactItem {
  id: string;
  title: string;
  path: string;
  kind: "code" | "markdown" | "svg" | "document";
  preview?: string;
  lineCount?: number;
}

export const ARTIFACT_CARD_STYLE_OPTIONS: ReadonlyArray<{
  id: ArtifactCardStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-artifact",
    title: "Assistant-UI Artifact Card (Default)",
    description:
      "assistant-ui/elements/artifact-card — Generated file/document card with preview disclosure, copy, and download.",
    isDefault: true,
  },
  {
    id: "split-preview",
    title: "Expanded Code/Doc Artifact",
    description:
      "Shows an inline snippet preview directly beneath the artifact header.",
  },
  {
    id: "compact-chip",
    title: "Minimal Artifact Pill",
    description:
      "Single-line generated file badge for clean minimal chats.",
  },
];

export function extractTurnArtifacts(
  toolCalls: ReadonlyArray<ToolExecution> | undefined,
): ArtifactItem[] {
  if (!toolCalls || toolCalls.length === 0) return [];
  const items: ArtifactItem[] = [];

  for (const tc of toolCalls) {
    if (tc.status !== "success") continue;
    const lower = tc.name.toLowerCase();
    if (lower !== "write_file" && lower !== "create_file") continue;

    const argsObj =
      tc.args && typeof tc.args === "object" && !Array.isArray(tc.args)
        ? (tc.args as Record<string, unknown>)
        : null;
    const path =
      (typeof argsObj?.path === "string" && argsObj.path) ||
      (typeof argsObj?.file_path === "string" && argsObj.file_path) ||
      "";
    if (!path) continue;

    const content = typeof argsObj?.content === "string" ? argsObj.content : "";
    const segments = path.split("/");
    const title = segments[segments.length - 1] || path;
    const kind: ArtifactItem["kind"] = path.endsWith(".md")
      ? "markdown"
      : path.endsWith(".svg")
        ? "svg"
        : path.endsWith(".rs") ||
            path.endsWith(".ts") ||
            path.endsWith(".tsx") ||
            path.endsWith(".py") ||
            path.endsWith(".json")
          ? "code"
          : "document";

    items.push({
      id: tc.id,
      title,
      path,
      kind,
      preview: content ? content.split("\n").slice(0, 8).join("\n") : undefined,
      lineCount: content ? content.split("\n").length : undefined,
    });
  }

  return items;
}

export interface ArtifactCardElementProps {
  artifact: ArtifactItem;
  variant?: ArtifactCardStyleChoice;
  className?: string;
}

export const ArtifactCardElement: React.FC<ArtifactCardElementProps> = ({
  artifact,
  variant = "assistant-artifact",
  className,
}) => {
  const [open, setOpen] = React.useState(variant === "split-preview");
  const [copied, setCopied] = React.useState(false);

  const handleCopy = () => {
    navigator.clipboard?.writeText(artifact.preview || artifact.path).catch(() => {});
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };

  const handleDownload = () => {
    if (!artifact.preview) return;
    const blob = new Blob([artifact.preview], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = artifact.title;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (variant === "compact-chip") {
    return (
      <span
        className={cn(
          "my-1 inline-flex items-center gap-1.5 rounded-full border border-border/60 bg-foreground/[0.04] px-2.5 py-0.5 text-xs text-foreground/85 not-prose",
          className,
        )}
      >
        <FilePlus2 className="size-3 text-primary" />
        <span className="font-mono text-[11px]">{artifact.title}</span>
        {artifact.lineCount && (
          <span className="font-mono text-[9.5px] text-muted-foreground">
            {artifact.lineCount}L
          </span>
        )}
      </span>
    );
  }

  return (
    <div
      data-slot="artifact-card"
      className={cn(
        "my-2 w-full max-w-xl overflow-hidden rounded-xl border border-border/70 bg-card/85 text-xs not-prose",
        className,
      )}
    >
      <div className="flex items-center justify-between gap-2 px-3.5 py-2.5">
        <button
          type="button"
          onClick={() => artifact.preview && setOpen((o) => !o)}
          className="flex min-w-0 flex-1 items-center gap-2.5 text-left"
        >
          <span className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-primary/25 bg-primary/10 text-primary">
            {artifact.kind === "code" ? (
              <FileCode2 className="size-3.5" />
            ) : (
              <FileText className="size-3.5" />
            )}
          </span>
          <div className="min-w-0">
            <div className="truncate font-semibold text-foreground">{artifact.title}</div>
            <div className="truncate font-mono text-[10px] text-muted-foreground">
              {artifact.path}
              {artifact.lineCount ? ` · ${artifact.lineCount} lines` : ""}
            </div>
          </div>
        </button>

        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            onClick={handleCopy}
            aria-label="Copy artifact"
            className="rounded-md p-1.5 text-muted-foreground hover:bg-foreground/[0.06] hover:text-foreground"
          >
            {copied ? <Check className="size-3.5 text-success" /> : <Copy className="size-3.5" />}
          </button>
          {artifact.preview && (
            <button
              type="button"
              onClick={handleDownload}
              aria-label="Download artifact"
              className="rounded-md p-1.5 text-muted-foreground hover:bg-foreground/[0.06] hover:text-foreground"
            >
              <Download className="size-3.5" />
            </button>
          )}
        </div>
      </div>

      {open && artifact.preview && (
        <pre className="max-h-44 overflow-auto border-t border-border/55 bg-background/70 p-3 font-mono text-[11px] whitespace-pre-wrap text-foreground/85">
          {artifact.preview}
        </pre>
      )}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 5. Math Block (`assistant-ui/elements/math-block`)                         */
/* -------------------------------------------------------------------------- */

export type MathBlockStyleChoice =
  | "assistant-math"
  | "minimal-centered"
  | "left-rail";

export const MATH_BLOCK_STYLE_OPTIONS: ReadonlyArray<{
  id: MathBlockStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-math",
    title: "Assistant-UI Framed Math Block (Default)",
    description:
      "assistant-ui/elements/math-block — Clean equation surface with LaTeX source copy button.",
    isDefault: true,
  },
  {
    id: "minimal-centered",
    title: "Minimal Centered Equation",
    description:
      "Unboxed display equation centered cleanly inside prose.",
  },
  {
    id: "left-rail",
    title: "Editorial Left-Accent Math",
    description:
      "Academic left-accent equation block with subtle background tint.",
  },
];

export interface MathBlockElementProps {
  latex?: string;
  variant?: MathBlockStyleChoice;
  children?: React.ReactNode;
  className?: string;
}

export const MathBlockElement: React.FC<MathBlockElementProps> = ({
  latex,
  variant = "assistant-math",
  children,
  className,
}) => {
  const [copied, setCopied] = React.useState(false);

  const handleCopy = () => {
    if (!latex) return;
    navigator.clipboard?.writeText(latex).catch(() => {});
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };

  if (variant === "minimal-centered") {
    return (
      <div className={cn("my-3 overflow-x-auto py-2 text-center not-prose", className)}>
        {children ?? <code className="font-mono text-sm text-foreground">{latex}</code>}
      </div>
    );
  }

  if (variant === "left-rail") {
    return (
      <div
        className={cn(
          "my-3 overflow-x-auto border-l-2 border-primary/50 bg-foreground/[0.025] py-2.5 pl-4 pr-3 not-prose",
          className,
        )}
      >
        {children ?? <code className="font-mono text-sm text-foreground">{latex}</code>}
      </div>
    );
  }

  return (
    <div
      data-slot="math-block"
      className={cn(
        "my-3 overflow-hidden rounded-xl border border-border/65 bg-card/75 not-prose",
        className,
      )}
    >
      <div className="flex items-center justify-between border-b border-border/50 px-3 py-1 text-[10px] text-muted-foreground">
        <span className="inline-flex items-center gap-1 font-mono">
          <Sigma className="size-3 text-primary" />
          <span>Equation</span>
        </span>
        {latex && (
          <button
            type="button"
            onClick={handleCopy}
            className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-foreground/[0.06] hover:text-foreground"
          >
            {copied ? <Check className="size-2.5 text-success" /> : <Copy className="size-2.5" />}
            <span>LaTeX</span>
          </button>
        )}
      </div>
      <div className="overflow-x-auto px-4 py-3 text-center">
        {children ?? <code className="font-mono text-sm text-foreground">{latex}</code>}
      </div>
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 6. Spec Sheet (`assistant-ui/elements/spec-sheet`)                         */
/* -------------------------------------------------------------------------- */

export type SpecSheetStyleChoice =
  | "assistant-spec"
  | "striped-spec"
  | "minimal-inline";

export const SPEC_SHEET_STYLE_OPTIONS: ReadonlyArray<{
  id: SpecSheetStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-spec",
    title: "Assistant-UI Spec Sheet (Default)",
    description:
      "assistant-ui/elements/spec-sheet — Labeled 2-column specification sheet with hairline dividers.",
    isDefault: true,
  },
  {
    id: "striped-spec",
    title: "Striped Property Card",
    description:
      "Alternating subtle rows inside a bordered card container.",
  },
  {
    id: "minimal-inline",
    title: "Minimal Compact Specs",
    description:
      "Compact wrap-around key/value pills for runtime metadata.",
  },
];

export function isSpecSheetTable(
  headers: ReadonlyArray<string>,
  rows: ReadonlyArray<ReadonlyArray<string>>,
): boolean {
  if (headers.length !== 2 || rows.length < 2) return false;
  const h0 = (headers[0] ?? "").toLowerCase();
  return (
    h0 === "property" ||
    h0 === "specification" ||
    h0 === "spec" ||
    h0 === "parameter" ||
    h0 === "setting" ||
    h0 === "metric" ||
    h0 === "attribute"
  );
}

export interface SpecSheetElementProps {
  title?: string;
  specs: ReadonlyArray<{ label: string; value: string }>;
  variant?: SpecSheetStyleChoice;
  className?: string;
}

export const SpecSheetElement: React.FC<SpecSheetElementProps> = ({
  title,
  specs,
  variant = "assistant-spec",
  className,
}) => {
  if (specs.length === 0) return null;

  if (variant === "minimal-inline") {
    return (
      <div className={cn("my-2 flex flex-wrap gap-1.5 not-prose", className)}>
        {specs.map((item, idx) => (
          <span
            key={`${item.label}-${idx}`}
            className="inline-flex items-center gap-1.5 rounded-md border border-border/55 bg-foreground/[0.03] px-2 py-0.5 text-[11px]"
          >
            <span className="text-muted-foreground">{item.label}:</span>
            <span className="font-mono font-medium text-foreground/90">{item.value}</span>
          </span>
        ))}
      </div>
    );
  }

  return (
    <div
      data-slot="spec-sheet"
      className={cn(
        "my-3 w-full overflow-hidden rounded-xl border border-border/70 bg-card/80 text-xs not-prose",
        className,
      )}
    >
      {title && (
        <div className="border-b border-border/55 bg-foreground/[0.025] px-3.5 py-2 font-semibold text-foreground">
          {title}
        </div>
      )}
      <dl className="divide-y divide-border/45">
        {specs.map((item, idx) => (
          <div
            key={`${item.label}-${idx}`}
            className={cn(
              "grid grid-cols-[minmax(120px,38%)_1fr] gap-3 px-3.5 py-2",
              variant === "striped-spec" && idx % 2 === 1 && "bg-foreground/[0.02]",
            )}
          >
            <dt className="truncate text-muted-foreground">{item.label}</dt>
            <dd className="font-mono text-[11.5px] font-medium text-foreground/90">
              {item.value}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
};
