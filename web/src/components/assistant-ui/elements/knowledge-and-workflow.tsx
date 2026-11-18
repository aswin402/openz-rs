"use client";

import * as React from "react";
import {
  ArrowRight,
  BookOpen,
  Bot,
  Brain,
  Check,
  ChevronRight,
  Database,
  Inbox,
  Lightbulb,
  Loader2,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { ToolExecution } from "@/types";
import {
  CollapsibleRoot as Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "./collapsible-root";
import { collapsePanel, field, mono } from "./surfaces";

/* -------------------------------------------------------------------------- */
/* 1. Retrieval Chunks (`assistant-ui/elements/retrieval-chunks`)             */
/* -------------------------------------------------------------------------- */

export type RetrievalChunksStyleChoice =
  | "assistant-chunks"
  | "compact-passages"
  | "pills-only";

export interface RetrievalChunk {
  id: string;
  source: string;
  snippet: string;
  score?: number;
}

export const RETRIEVAL_CHUNKS_STYLE_OPTIONS: ReadonlyArray<{
  id: RetrievalChunksStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-chunks",
    title: "Assistant-UI Retrieval Chunks (Default)",
    description:
      "assistant-ui/elements/retrieval-chunks — Collapsible RAG/vector passages with relevance scores and source paths.",
    isDefault: true,
  },
  {
    id: "compact-passages",
    title: "Minimal Numbered Passages",
    description:
      "Clean single-line excerpt rows without outer card borders.",
  },
  {
    id: "pills-only",
    title: "Source Chunk Chips",
    description:
      "Ultra-minimal retrieved file/memory chips.",
  },
];

export function extractTurnRetrievalChunks(
  toolCalls: ReadonlyArray<ToolExecution> | undefined,
): RetrievalChunk[] {
  if (!toolCalls || toolCalls.length === 0) return [];
  const chunks: RetrievalChunk[] = [];

  for (const tc of toolCalls) {
    if (tc.status !== "success" || !tc.output) continue;
    const lower = tc.name.toLowerCase();
    const isRetrieval =
      lower === "semantic_search" ||
      lower === "retrieve_original" ||
      lower === "docs_search_docs" ||
      lower === "recall_memory";
    if (!isRetrieval) continue;

    if (tc.output.trim().startsWith("{") || tc.output.trim().startsWith("[")) {
      try {
        const parsed = JSON.parse(tc.output) as unknown;
        const arr = Array.isArray(parsed)
          ? parsed
          : parsed && typeof parsed === "object" && Array.isArray((parsed as Record<string, unknown>).results)
            ? ((parsed as Record<string, unknown>).results as unknown[])
            : parsed && typeof parsed === "object" && Array.isArray((parsed as Record<string, unknown>).matches)
              ? ((parsed as Record<string, unknown>).matches as unknown[])
              : [];

        for (let i = 0; i < Math.min(arr.length, 5); i += 1) {
          const raw = arr[i];
          if (raw && typeof raw === "object") {
            const obj = raw as Record<string, unknown>;
            const source =
              (typeof obj.path === "string" && obj.path) ||
              (typeof obj.file === "string" && obj.file) ||
              (typeof obj.title === "string" && obj.title) ||
              `${tc.name} #${i + 1}`;
            const snippet =
              (typeof obj.snippet === "string" && obj.snippet) ||
              (typeof obj.content === "string" && obj.content) ||
              (typeof obj.text === "string" && obj.text) ||
              "";
            const score = typeof obj.score === "number" ? obj.score : undefined;
            if (snippet) {
              chunks.push({
                id: `${tc.id}-${i}`,
                source,
                snippet: snippet.slice(0, 220),
                score,
              });
            }
          }
        }
      } catch {
        // ignore non-JSON
      }
    }
  }

  return chunks;
}

export interface RetrievalChunksElementProps {
  chunks: ReadonlyArray<RetrievalChunk>;
  variant?: RetrievalChunksStyleChoice;
  defaultOpen?: boolean;
  className?: string;
}

export const RetrievalChunksElement: React.FC<RetrievalChunksElementProps> = ({
  chunks,
  variant = "assistant-chunks",
  defaultOpen = false,
  className,
}) => {
  const [open, setOpen] = React.useState(defaultOpen);
  if (chunks.length === 0) return null;

  if (variant === "pills-only") {
    return (
      <div className={cn("my-1.5 flex flex-wrap items-center gap-1.5 not-prose", className)}>
        {chunks.map((chunk) => (
          <span
            key={chunk.id}
            className="inline-flex items-center gap-1.5 rounded-md border border-border/55 bg-foreground/[0.035] px-2 py-0.5 font-mono text-[10.5px] text-muted-foreground"
          >
            <Database className="size-2.5 text-primary/80" />
            <span className="max-w-[180px] truncate text-foreground/85">{chunk.source}</span>
            {typeof chunk.score === "number" && (
              <span>{Math.round(chunk.score * 100)}%</span>
            )}
          </span>
        ))}
      </div>
    );
  }

  return (
    <Collapsible
      data-slot="retrieval-chunks"
      open={open}
      onOpenChange={setOpen}
      className={cn("my-1.5 w-full not-prose", className)}
    >
      <CollapsibleTrigger className="group flex w-full items-center gap-2 rounded-md py-1 text-left text-xs text-muted-foreground transition-colors hover:text-foreground">
        <ChevronRight
          className={cn("size-3.5 shrink-0 transition-transform", open && "rotate-90")}
        />
        <Database className="size-3.5 shrink-0 text-primary/85" />
        <span className="font-medium text-foreground/90">
          Retrieved {chunks.length} context {chunks.length === 1 ? "chunk" : "chunks"}
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent className={cn(collapsePanel, "outline-none")}>
        <div
          className={cn(
            "mt-1.5 space-y-2",
            variant === "assistant-chunks" && cn(field, "rounded-xl p-2.5"),
          )}
        >
          {chunks.map((chunk, idx) => (
            <div
              key={chunk.id}
              className={cn(
                "rounded-lg p-2 text-xs",
                variant === "assistant-chunks"
                  ? "border border-border/50 bg-card/80"
                  : "border-l border-border/70 pl-2.5",
              )}
            >
              <div className="flex items-center justify-between gap-2 text-[10.5px]">
                <span className="truncate font-mono font-medium text-foreground/90">
                  [{idx + 1}] {chunk.source}
                </span>
                {typeof chunk.score === "number" && (
                  <span className={cn(mono, "text-primary")}>
                    {Math.round(chunk.score * 100)}% match
                  </span>
                )}
              </div>
              <p className="mt-1 line-clamp-2 text-[11px] text-muted-foreground">
                {chunk.snippet}
              </p>
            </div>
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
};

/* -------------------------------------------------------------------------- */
/* 2. Memory Chips (`assistant-ui/elements/memory-chips`)                     */
/* -------------------------------------------------------------------------- */

export interface MemoryChipItem {
  id: string;
  label: string;
  detail?: string;
  onInspect?: () => void;
  onDismiss?: () => void;
}

export const MemoryChipsElement: React.FC<{
  items: ReadonlyArray<MemoryChipItem>;
  className?: string;
}> = ({ items, className }) => {
  if (items.length === 0) return null;
  return (
    <div className={cn("flex flex-wrap items-center gap-1.5 not-prose", className)}>
      {items.map((item) => (
        <span
          key={item.id}
          onClick={item.onInspect}
          role={item.onInspect ? "button" : undefined}
          tabIndex={item.onInspect ? 0 : undefined}
          className={cn(
            "inline-flex max-w-full items-center gap-1.5 rounded-lg border border-border/55 bg-foreground/[0.04] px-2 py-0.5 text-[11px] text-foreground/85 transition-colors",
            item.onInspect && "cursor-pointer hover:border-border hover:bg-foreground/[0.07]",
          )}
        >
          <Brain className="size-3 shrink-0 text-primary/80" />
          <span className="font-medium">{item.label}</span>
          {item.detail && (
            <span className="max-w-[200px] truncate text-muted-foreground">
              · {item.detail}
            </span>
          )}
          {item.onDismiss && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                item.onDismiss?.();
              }}
              aria-label="Remove memory chip"
              className="ml-0.5 rounded p-0.5 text-muted-foreground hover:text-foreground"
            >
              <X className="size-2.5" />
            </button>
          )}
        </span>
      ))}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 3. Research Report (`assistant-ui/elements/research-report`)               */
/* -------------------------------------------------------------------------- */

export type ResearchReportStyleChoice =
  | "assistant-outline"
  | "editorial-brief"
  | "compact-summary";

export interface ResearchSectionItem {
  id: string;
  heading: string;
  summary: string;
  sourcesCount?: number;
}

export const RESEARCH_REPORT_STYLE_OPTIONS: ReadonlyArray<{
  id: ResearchReportStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-outline",
    title: "Assistant-UI Sectioned Research Report (Default)",
    description:
      "assistant-ui/elements/research-report — Structured multi-section research brief with per-section source counts.",
    isDefault: true,
  },
  {
    id: "editorial-brief",
    title: "Editorial Magazine Brief",
    description:
      "Clean typographic research card with subtle horizontal section rules.",
  },
  {
    id: "compact-summary",
    title: "Collapsible Executive Summary",
    description:
      "Single-line expandable research digest for compact chat threads.",
  },
];

export const ResearchReportElement: React.FC<{
  title: string;
  sections: ReadonlyArray<ResearchSectionItem>;
  variant?: ResearchReportStyleChoice;
  className?: string;
}> = ({ title, sections, variant = "assistant-outline", className }) => {
  const [open, setOpen] = React.useState(variant !== "compact-summary");
  if (sections.length === 0) return null;

  return (
    <div
      data-slot="research-report"
      className={cn(
        "my-3 w-full overflow-hidden rounded-xl border border-border/70 bg-card/85 text-xs not-prose",
        variant === "editorial-brief" && "border-x-0 border-b border-t bg-transparent rounded-none px-0",
        className,
      )}
    >
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between gap-2 px-3.5 py-2.5 text-left hover:bg-foreground/[0.025]"
      >
        <div className="flex min-w-0 items-center gap-2">
          <BookOpen className="size-3.5 shrink-0 text-primary" />
          <span className="truncate font-semibold text-foreground">{title}</span>
        </div>
        <span className={cn(mono, "shrink-0 text-muted-foreground")}>
          {sections.length} sections
        </span>
      </button>

      {open && (
        <div className="divide-y divide-border/45 border-t border-border/50">
          {sections.map((sec, idx) => (
            <div key={sec.id} className="px-3.5 py-2.5">
              <div className="flex items-center justify-between gap-2">
                <span className="font-medium text-foreground">
                  {idx + 1}. {sec.heading}
                </span>
                {typeof sec.sourcesCount === "number" && sec.sourcesCount > 0 && (
                  <span className="rounded bg-foreground/[0.045] px-1.5 py-0 font-mono text-[10px] text-muted-foreground">
                    {sec.sourcesCount} {sec.sourcesCount === 1 ? "source" : "sources"}
                  </span>
                )}
              </div>
              <p className="mt-1 text-[11.5px] leading-relaxed text-muted-foreground">
                {sec.summary}
              </p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 4. Timeline (`assistant-ui/elements/timeline`)                             */
/* -------------------------------------------------------------------------- */

export type TimelineStyleChoice =
  | "assistant-timeline"
  | "horizontal-stepper"
  | "minimal-log";

export interface TimelineNodeItem {
  id: string;
  title: string;
  detail?: string;
  timeLabel?: string;
  status: "completed" | "active" | "upcoming";
}

export const TIMELINE_STYLE_OPTIONS: ReadonlyArray<{
  id: TimelineStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-timeline",
    title: "Assistant-UI Vertical Axis Timeline (Default)",
    description:
      "assistant-ui/elements/timeline — Chronological axis tracking completed, active, and upcoming milestones.",
    isDefault: true,
  },
  {
    id: "horizontal-stepper",
    title: "Horizontal Milestone Rail",
    description:
      "Compact horizontal step rail ideal for multi-stage workflows.",
  },
  {
    id: "minimal-log",
    title: "Minimal Timestamped List",
    description:
      "Ultra-clean single-line chronological entries.",
  },
];

export const TimelineElement: React.FC<{
  items: ReadonlyArray<TimelineNodeItem>;
  variant?: TimelineStyleChoice;
  className?: string;
}> = ({ items, variant = "assistant-timeline", className }) => {
  if (items.length === 0) return null;

  if (variant === "horizontal-stepper") {
    return (
      <div
        className={cn(
          "my-2 flex flex-wrap items-center gap-2 rounded-xl border border-border/65 bg-card/75 p-3 text-xs not-prose",
          className,
        )}
      >
        {items.map((item, idx) => (
          <React.Fragment key={item.id}>
            <div className="flex items-center gap-1.5">
              <span
                className={cn(
                  "inline-flex size-4 shrink-0 items-center justify-center rounded-full text-[9.5px] font-mono",
                  item.status === "completed"
                    ? "bg-success/15 text-success"
                    : item.status === "active"
                      ? "bg-primary/20 text-primary font-semibold"
                      : "bg-muted text-muted-foreground",
                )}
              >
                {item.status === "completed" ? <Check className="size-2.5" /> : idx + 1}
              </span>
              <span
                className={cn(
                  "text-[11.5px]",
                  item.status === "upcoming" ? "text-muted-foreground" : "text-foreground font-medium",
                )}
              >
                {item.title}
              </span>
            </div>
            {idx < items.length - 1 && (
              <ArrowRight className="size-3 shrink-0 text-muted-foreground/50" />
            )}
          </React.Fragment>
        ))}
      </div>
    );
  }

  return (
    <ol
      data-slot="timeline"
      className={cn(
        "my-2 space-y-2 text-xs not-prose",
        variant === "assistant-timeline" && "border-l border-border/75 pl-4 ml-1.5",
        className,
      )}
    >
      {items.map((item) => (
        <li key={item.id} className="relative">
          {variant === "assistant-timeline" && (
            <span
              className={cn(
                "absolute -left-[20.5px] top-1.5 size-2 rounded-full ring-3 ring-background",
                item.status === "completed"
                  ? "bg-success"
                  : item.status === "active"
                    ? "animate-pulse bg-primary"
                    : "bg-muted-foreground/40",
              )}
            />
          )}
          <div className="flex items-baseline justify-between gap-2">
            <span
              className={cn(
                "font-medium",
                item.status === "upcoming" ? "text-muted-foreground" : "text-foreground/95",
              )}
            >
              {item.title}
            </span>
            {item.timeLabel && (
              <span className={cn(mono, "shrink-0 text-muted-foreground")}>
                {item.timeLabel}
              </span>
            )}
          </div>
          {item.detail && (
            <p className="mt-0.5 text-[11px] text-muted-foreground">{item.detail}</p>
          )}
        </li>
      ))}
    </ol>
  );
};

/* -------------------------------------------------------------------------- */
/* 5. Recommendation Card (`assistant-ui/elements/recommendation-card`)       */
/* -------------------------------------------------------------------------- */

export type RecommendationStyleChoice =
  | "assistant-rec"
  | "compact-pills"
  | "editorial-callout";

export interface RecommendationItem {
  id: string;
  title: string;
  rationale?: string;
  actionLabel?: string;
  onSelect?: () => void;
}

export const RECOMMENDATION_STYLE_OPTIONS: ReadonlyArray<{
  id: RecommendationStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-rec",
    title: "Assistant-UI Recommendation Cards (Default)",
    description:
      "assistant-ui/elements/recommendation-card — Actionable next-step suggestion cards with rationale.",
    isDefault: true,
  },
  {
    id: "compact-pills",
    title: "Minimal Follow-Up Chips",
    description:
      "Single-row clickable follow-up chips.",
  },
  {
    id: "editorial-callout",
    title: "Editorial Left-Accent Suggestion",
    description:
      "Quiet borderless suggestion strip with action link.",
  },
];

export const RecommendationCardElement: React.FC<{
  items: ReadonlyArray<RecommendationItem>;
  variant?: RecommendationStyleChoice;
  className?: string;
}> = ({ items, variant = "assistant-rec", className }) => {
  if (items.length === 0) return null;

  if (variant === "compact-pills") {
    return (
      <div className={cn("my-2 flex flex-wrap gap-1.5 not-prose", className)}>
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={item.onSelect}
            className="inline-flex items-center gap-1.5 rounded-full border border-border/60 bg-foreground/[0.035] px-2.5 py-1 text-xs font-medium text-foreground/85 transition-colors hover:border-primary/40 hover:text-primary"
          >
            <Lightbulb className="size-3 text-primary/80" />
            <span>{item.title}</span>
          </button>
        ))}
      </div>
    );
  }

  return (
    <div className={cn("my-2 space-y-2 not-prose", className)}>
      {items.map((item) => (
        <div
          key={item.id}
          className={cn(
            "flex items-center justify-between gap-3 text-xs",
            variant === "editorial-callout"
              ? "border-l-2 border-primary/50 py-1 pl-3"
              : "rounded-xl border border-border/65 bg-card/80 px-3.5 py-2.5",
          )}
        >
          <div className="min-w-0">
            <div className="flex items-center gap-1.5 font-medium text-foreground">
              <Lightbulb className="size-3.5 shrink-0 text-primary" />
              <span className="truncate">{item.title}</span>
            </div>
            {item.rationale && (
              <p className="mt-0.5 text-[11px] text-muted-foreground">{item.rationale}</p>
            )}
          </div>
          {item.onSelect && (
            <button
              type="button"
              onClick={item.onSelect}
              className="shrink-0 rounded-lg border border-border/70 bg-background/80 px-2.5 py-1 text-[11px] font-medium text-foreground transition-colors hover:border-primary/45 hover:text-primary"
            >
              {item.actionLabel ?? "Run"}
            </button>
          )}
        </div>
      ))}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 7. Agent Handoff (`assistant-ui/elements/agent-handoff`)                   */
/* -------------------------------------------------------------------------- */

export type AgentHandoffStyleChoice =
  | "assistant-handoff"
  | "compact-transfer"
  | "timeline-pass";

export interface AgentHandoffData {
  id: string;
  fromAgent: string;
  toAgent: string;
  taskSummary?: string;
  status: "running" | "completed" | "failed";
}

export const AGENT_HANDOFF_STYLE_OPTIONS: ReadonlyArray<{
  id: AgentHandoffStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-handoff",
    title: "Assistant-UI Agent Handoff Card (Default)",
    description:
      "assistant-ui/elements/agent-handoff — Visual specialist transfer card showing active subagent and context.",
    isDefault: true,
  },
  {
    id: "compact-transfer",
    title: "Minimal Handoff Pill",
    description:
      "Single-line `openz → specialist` transfer badge.",
  },
  {
    id: "timeline-pass",
    title: "Connected Handoff Chain",
    description:
      "Chronological multi-hop handoff trace.",
  },
];

export function extractTurnAgentHandoffs(
  toolCalls: ReadonlyArray<ToolExecution> | undefined,
): AgentHandoffData[] {
  if (!toolCalls || toolCalls.length === 0) return [];
  const list: AgentHandoffData[] = [];

  for (const tc of toolCalls) {
    const lower = tc.name.toLowerCase();
    const argsObj =
      tc.args && typeof tc.args === "object" && !Array.isArray(tc.args)
        ? (tc.args as Record<string, unknown>)
        : null;

    const isFork =
      lower === "curate_skill" &&
      typeof argsObj?.action === "string" &&
      argsObj.action === "fork";
    const isDelegation =
      lower === "delegate_task" ||
      lower === "orchestrate_workflow" ||
      lower === "parallel_research" ||
      isFork;

    if (!isDelegation) continue;

    const toAgent =
      (typeof argsObj?.profile === "string" && argsObj.profile) ||
      (typeof argsObj?.agent === "string" && argsObj.agent) ||
      (typeof argsObj?.name === "string" && argsObj.name) ||
      tc.name;
    const taskSummary =
      (typeof argsObj?.task === "string" && argsObj.task) ||
      (typeof argsObj?.goal === "string" && argsObj.goal) ||
      undefined;

    list.push({
      id: tc.id,
      fromAgent: "openz",
      toAgent,
      taskSummary,
      status:
        tc.status === "running"
          ? "running"
          : tc.status === "error"
            ? "failed"
            : "completed",
    });
  }

  return list;
}

export const AgentHandoffElement: React.FC<{
  handoffs: ReadonlyArray<AgentHandoffData>;
  variant?: AgentHandoffStyleChoice;
  className?: string;
}> = ({ handoffs, variant = "assistant-handoff", className }) => {
  if (handoffs.length === 0) return null;

  if (variant === "compact-transfer") {
    return (
      <div className={cn("my-1.5 flex flex-wrap gap-1.5 not-prose", className)}>
        {handoffs.map((h) => (
          <span
            key={h.id}
            className="inline-flex items-center gap-1.5 rounded-full border border-border/60 bg-foreground/[0.035] px-2.5 py-0.5 text-[11px] text-foreground/85"
          >
            <Bot className="size-3 text-primary" />
            <span className="font-mono">{h.fromAgent}</span>
            <ArrowRight className="size-2.5 text-muted-foreground" />
            <span className="font-mono font-medium text-primary">{h.toAgent}</span>
          </span>
        ))}
      </div>
    );
  }

  return (
    <div className={cn("my-2 space-y-1.5 not-prose", className)}>
      {handoffs.map((h) => (
        <div
          key={h.id}
          className={cn(
            "flex items-center justify-between gap-3 text-xs",
            variant === "timeline-pass"
              ? "border-l-2 border-primary/45 py-1 pl-3"
              : "rounded-xl border border-border/65 bg-card/80 px-3.5 py-2.5",
          )}
        >
          <div className="min-w-0">
            <div className="flex items-center gap-1.5 font-mono text-[11px]">
              <Bot className="size-3.5 text-primary" />
              <span className="text-muted-foreground">{h.fromAgent}</span>
              <ArrowRight className="size-3 text-muted-foreground" />
              <span className="font-semibold text-foreground">{h.toAgent}</span>
            </div>
            {h.taskSummary && (
              <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                {h.taskSummary}
              </p>
            )}
          </div>
          <span
            className={cn(
              mono,
              "shrink-0",
              h.status === "running"
                ? "animate-pulse text-primary"
                : h.status === "failed"
                  ? "text-destructive"
                  : "text-success",
            )}
          >
            {h.status}
          </span>
        </div>
      ))}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 8. Background Inbox (`assistant-ui/elements/background-inbox`)             */
/* -------------------------------------------------------------------------- */

export type BackgroundInboxStyleChoice =
  | "assistant-inbox"
  | "compact-pill-tray"
  | "grouped-feed";

export interface BackgroundInboxTask {
  id: string;
  title: string;
  detail?: string;
  status: "running" | "completed" | "failed";
  timeLabel?: string;
  onInspect?: () => void;
}

export const BACKGROUND_INBOX_STYLE_OPTIONS: ReadonlyArray<{
  id: BackgroundInboxStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}> = [
  {
    id: "assistant-inbox",
    title: "Assistant-UI Background Task Tray (Default)",
    description:
      "assistant-ui/elements/background-inbox — Clean async tray for Curator self-improvement, Cron jobs, and background sessions.",
    isDefault: true,
  },
  {
    id: "compact-pill-tray",
    title: "Minimal Status Strip",
    description:
      "Compact inline background pills with status dots.",
  },
  {
    id: "grouped-feed",
    title: "Grouped Running / Completed Feed",
    description:
      "Separates in-flight background agents from completed runs.",
  },
];

export const BackgroundInboxElement: React.FC<{
  tasks: ReadonlyArray<BackgroundInboxTask>;
  variant?: BackgroundInboxStyleChoice;
  className?: string;
}> = ({ tasks, variant = "assistant-inbox", className }) => {
  if (tasks.length === 0) return null;

  if (variant === "compact-pill-tray") {
    return (
      <div className={cn("flex flex-wrap items-center gap-1.5", className)}>
        {tasks.map((task) => (
          <button
            key={task.id}
            type="button"
            onClick={task.onInspect}
            className="inline-flex items-center gap-1.5 rounded-full border border-border/60 bg-foreground/[0.035] px-2.5 py-0.5 text-[11px] text-foreground/85 hover:border-border"
          >
            <span
              className={cn(
                "size-1.5 rounded-full",
                task.status === "running"
                  ? "animate-pulse bg-primary"
                  : task.status === "failed"
                    ? "bg-destructive"
                    : "bg-success",
              )}
            />
            <span className="max-w-[160px] truncate">{task.title}</span>
          </button>
        ))}
      </div>
    );
  }

  return (
    <div
      data-slot="background-inbox"
      className={cn(
        "w-full overflow-hidden rounded-xl border border-border/65 bg-card/80 text-xs",
        className,
      )}
    >
      <div className="flex items-center justify-between border-b border-border/50 px-3 py-2">
        <span className="inline-flex items-center gap-1.5 font-semibold text-foreground">
          <Inbox className="size-3.5 text-primary" />
          <span>Background Inbox</span>
        </span>
        <span className={cn(mono, "text-muted-foreground")}>{tasks.length}</span>
      </div>
      <div className="divide-y divide-border/45">
        {tasks.map((task) => (
          <div
            key={task.id}
            onClick={task.onInspect}
            role={task.onInspect ? "button" : undefined}
            tabIndex={task.onInspect ? 0 : undefined}
            className={cn(
              "flex items-center justify-between gap-2.5 px-3 py-2",
              task.onInspect && "cursor-pointer hover:bg-foreground/[0.03]",
            )}
          >
            <div className="flex min-w-0 items-center gap-2">
              {task.status === "running" ? (
                <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
              ) : (
                <span
                  className={cn(
                    "size-1.5 shrink-0 rounded-full",
                    task.status === "failed" ? "bg-destructive" : "bg-success",
                  )}
                />
              )}
              <div className="min-w-0">
                <div className="truncate font-medium text-foreground/90">{task.title}</div>
                {task.detail && (
                  <div className="truncate text-[10.5px] text-muted-foreground">
                    {task.detail}
                  </div>
                )}
              </div>
            </div>
            {task.timeLabel && (
              <span className={cn(mono, "shrink-0 text-muted-foreground")}>
                {task.timeLabel}
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};
