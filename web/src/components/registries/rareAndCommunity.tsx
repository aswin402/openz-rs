"use client";

import * as React from "react";
import {
  AlertTriangle,
  Check,
  ChevronDown,
  ExternalLink,
  Globe,
  ShieldCheck,
  Sparkles,
  Terminal,
  Trash2,
  Wrench,
  X,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { cn } from "@/lib/utils";
import type { SecurityPromptInfo, ToolExecution } from "@/types";

/* -------------------------------------------------------------------------- */
/* 1. Rare-UI Delete Button (`swamimalode07/rare-ui/delete-button`)           */
/* -------------------------------------------------------------------------- */

export type DeleteButtonStyleChoice = "rare-confirm" | "hold-to-delete" | "minimal-icon";

export interface DeleteButtonStyleDescriptor {
  id: DeleteButtonStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const DELETE_BUTTON_STYLE_OPTIONS: ReadonlyArray<DeleteButtonStyleDescriptor> = [
  {
    id: "rare-confirm",
    title: "Rare-UI Morphing Confirm (Default)",
    description:
      "swamimalode07/rare-ui/delete-button — Smoothly morphs into a two-step Confirm / Cancel pill to prevent accidental deletion.",
    isDefault: true,
  },
  {
    id: "hold-to-delete",
    title: "Press & Hold Guard",
    description:
      "Hold for 650ms to fill the destructive progress bar before confirming deletion.",
  },
  {
    id: "minimal-icon",
    title: "Minimal Instant Icon",
    description:
      "Single-click quiet trash icon with zero confirmation step.",
  },
];

export interface RareDeleteButtonProps {
  onDelete: () => void;
  variant?: DeleteButtonStyleChoice;
  label?: string;
  confirmLabel?: string;
  iconOnly?: boolean;
  disabled?: boolean;
  ariaLabel?: string;
  className?: string;
}

export const RareDeleteButton: React.FC<RareDeleteButtonProps> = ({
  onDelete,
  variant = "rare-confirm",
  label,
  confirmLabel = "Confirm",
  iconOnly = true,
  disabled = false,
  ariaLabel = "Delete item",
  className,
}) => {
  const [armed, setArmed] = React.useState(false);
  const [holding, setHolding] = React.useState(false);
  const holdTimerRef = React.useRef<number | null>(null);

  React.useEffect(() => {
    if (!armed) return undefined;
    const t = window.setTimeout(() => setArmed(false), 2800);
    return () => window.clearTimeout(t);
  }, [armed]);

  React.useEffect(() => {
    return () => {
      if (holdTimerRef.current !== null) {
        window.clearTimeout(holdTimerRef.current);
      }
    };
  }, []);

  if (variant === "minimal-icon") {
    return (
      <button
        type="button"
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          onDelete();
        }}
        aria-label={ariaLabel}
        className={cn(
          "inline-flex items-center gap-1.5 rounded-md px-1.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:pointer-events-none disabled:opacity-40",
          iconOnly && "px-1 py-1",
          className,
        )}
      >
        <Trash2 className="size-3.5 shrink-0" />
        {!iconOnly && label && <span>{label}</span>}
      </button>
    );
  }

  if (variant === "hold-to-delete") {
    const startHold = (e: React.SyntheticEvent) => {
      e.stopPropagation();
      if (disabled) return;
      setHolding(true);
      holdTimerRef.current = window.setTimeout(() => {
        setHolding(false);
        onDelete();
      }, 650);
    };
    const cancelHold = (e: React.SyntheticEvent) => {
      e.stopPropagation();
      setHolding(false);
      if (holdTimerRef.current !== null) {
        window.clearTimeout(holdTimerRef.current);
        holdTimerRef.current = null;
      }
    };

    return (
      <button
        type="button"
        disabled={disabled}
        onMouseDown={startHold}
        onMouseUp={cancelHold}
        onMouseLeave={cancelHold}
        onTouchStart={startHold}
        onTouchEnd={cancelHold}
        onClick={(e) => e.stopPropagation()}
        aria-label={`${ariaLabel} (hold to confirm)`}
        className={cn(
          "relative inline-flex items-center gap-1.5 overflow-hidden rounded-md border border-transparent px-2 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:border-destructive/30 hover:text-destructive disabled:pointer-events-none disabled:opacity-40",
          iconOnly && !holding && "px-1 py-1",
          className,
        )}
      >
        {holding && (
          <motion.span
            initial={{ scaleX: 0 }}
            animate={{ scaleX: 1 }}
            transition={{ duration: 0.65, ease: "linear" }}
            style={{ transformOrigin: "left center" }}
            className="pointer-events-none absolute inset-0 bg-destructive/20"
          />
        )}
        <Trash2 className="relative z-10 size-3.5 shrink-0" />
        {(holding || (!iconOnly && label)) && (
          <span className="relative z-10 whitespace-nowrap">
            {holding ? "Hold…" : label}
          </span>
        )}
      </button>
    );
  }

  // Default: `rare-confirm` (`swamimalode07/rare-ui/delete-button` morphing pill)
  return (
    <span
      onClick={(e) => e.stopPropagation()}
      className={cn("inline-flex items-center", className)}
    >
      <AnimatePresence mode="wait" initial={false}>
        {armed ? (
          <motion.span
            key="confirm"
            initial={{ opacity: 0, scale: 0.92 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.92 }}
            transition={{ duration: 0.14 }}
            className="inline-flex items-center gap-1 rounded-full border border-destructive/35 bg-destructive/12 px-2 py-0.5 text-[10.5px] font-medium text-destructive"
          >
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                setArmed(false);
                onDelete();
              }}
              className="inline-flex items-center gap-1 font-semibold hover:underline"
            >
              <Check className="size-3" />
              <span>{confirmLabel}</span>
            </button>
            <button
              type="button"
              aria-label="Cancel delete"
              onClick={(e) => {
                e.stopPropagation();
                setArmed(false);
              }}
              className="ml-0.5 rounded-full p-0.5 text-muted-foreground hover:bg-foreground/10 hover:text-foreground"
            >
              <X className="size-2.5" />
            </button>
          </motion.span>
        ) : (
          <motion.button
            key="idle"
            type="button"
            disabled={disabled}
            initial={{ opacity: 0, scale: 0.94 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.94 }}
            transition={{ duration: 0.14 }}
            onClick={(e) => {
              e.stopPropagation();
              setArmed(true);
            }}
            aria-label={ariaLabel}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-md px-1.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:pointer-events-none disabled:opacity-40",
              iconOnly && "px-1 py-1",
            )}
          >
            <Trash2 className="size-3.5 shrink-0" />
            {!iconOnly && label && <span>{label}</span>}
          </motion.button>
        )}
      </AnimatePresence>
    </span>
  );
};

/* -------------------------------------------------------------------------- */
/* 2. Inline Citations (`aicss.dev/r/inline-citations.json` + `@kokonutui`)   */
/* -------------------------------------------------------------------------- */

export type InlineCitationStyleChoice =
  | "aicss-pill"
  | "kokonut-chip"
  | "minimal-sup"
  | "plain-link";

export interface InlineCitationStyleDescriptor {
  id: InlineCitationStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const INLINE_CITATION_STYLE_OPTIONS: ReadonlyArray<InlineCitationStyleDescriptor> = [
  {
    id: "aicss-pill",
    title: "AICSS Hover Source Pill (Default)",
    description:
      "aicss.dev/r/inline-citations — Compact inline domain badge with hover preview card and external jump.",
    isDefault: true,
  },
  {
    id: "kokonut-chip",
    title: "KokonutUI Citation Chip",
    description:
      "@kokonutui/ai-citations — Monochrome hairline source capsule woven smoothly into prose.",
  },
  {
    id: "minimal-sup",
    title: "Minimal Academic Superscript",
    description:
      "Ultra-minimal numbered/domain superscript tag that keeps reading flow uninterrupted.",
  },
  {
    id: "plain-link",
    title: "Standard Editorial Link",
    description:
      "Clean underlined prose anchor without inline badge framing.",
  },
];

function extractHostZeroRegex(href: string): string {
  const idx = href.indexOf("://");
  const after = idx !== -1 ? href.slice(idx + 3) : href;
  const slash = after.indexOf("/");
  const raw = slash !== -1 ? after.slice(0, slash) : after;
  return raw.startsWith("www.") ? raw.slice(4) : raw;
}

export interface InlineCitationLinkProps {
  href: string;
  children: React.ReactNode;
  index?: number;
  variant?: InlineCitationStyleChoice;
  className?: string;
}

export const InlineCitationLink: React.FC<InlineCitationLinkProps> = ({
  href,
  children,
  index,
  variant = "aicss-pill",
  className,
}) => {
  const [hovered, setHovered] = React.useState(false);
  const host = React.useMemo(() => extractHostZeroRegex(href), [href]);
  const textLabel = typeof children === "string" ? children.trim() : "";

  if (variant === "plain-link") {
    return (
      <a
        href={href}
        target="_blank"
        rel="noreferrer"
        className={cn(
          "font-medium text-primary underline decoration-primary/40 underline-offset-3 transition-colors hover:decoration-primary",
          className,
        )}
      >
        {children}
      </a>
    );
  }

  if (variant === "minimal-sup") {
    return (
      <a
        href={href}
        target="_blank"
        rel="noreferrer"
        className={cn(
          "mx-0.5 inline-flex items-baseline gap-0.5 font-mono text-[10px] font-medium text-primary hover:underline",
          className,
        )}
      >
        <span>{children}</span>
        <sup className="rounded bg-primary/12 px-1 py-0 text-[9px]">
          {index ?? host.slice(0, 12)}
        </sup>
      </a>
    );
  }

  if (variant === "kokonut-chip") {
    return (
      <a
        href={href}
        target="_blank"
        rel="noreferrer"
        className={cn(
          "mx-0.5 inline-flex items-center gap-1 rounded-md border border-border/60 bg-foreground/[0.04] px-1.5 py-0 text-[11px] font-medium text-foreground/85 no-underline transition-colors hover:border-primary/40 hover:bg-primary/10 hover:text-primary align-baseline",
          className,
        )}
      >
        <Globe className="size-2.5 shrink-0 opacity-70" />
        <span className="max-w-[160px] truncate">{textLabel || host}</span>
      </a>
    );
  }

  // Default: `aicss-pill` with floating hover source preview card
  return (
    <span
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="relative mx-0.5 inline-flex align-baseline"
    >
      <a
        href={href}
        target="_blank"
        rel="noreferrer"
        className={cn(
          "inline-flex items-center gap-1 rounded-full border border-border/60 bg-card/85 px-2 py-0 text-[11px] font-medium text-foreground/90 no-underline transition-colors hover:border-primary/45 hover:text-primary",
          className,
        )}
      >
        <span className="size-1.5 shrink-0 rounded-full bg-primary/80" />
        <span className="max-w-[150px] truncate">{textLabel || host}</span>
        {index !== undefined && (
          <span className="font-mono text-[9.5px] text-muted-foreground">{index}</span>
        )}
      </a>

      <AnimatePresence>
        {hovered && (
          <motion.div
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 4 }}
            transition={{ duration: 0.14 }}
            className="pointer-events-none absolute bottom-full left-0 z-40 mb-1.5 w-64 rounded-xl border border-border/75 bg-popover/95 p-2.5 text-left text-xs shadow-xl backdrop-blur-xl"
          >
            <div className="flex items-center justify-between gap-1.5 text-[10px] text-muted-foreground">
              <span className="inline-flex items-center gap-1 font-mono truncate">
                <Globe className="size-2.5 text-primary" />
                {host}
              </span>
              <ExternalLink className="size-2.5 shrink-0" />
            </div>
            <div className="mt-1 line-clamp-2 font-medium text-foreground">
              {textLabel || href}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </span>
  );
};

/* -------------------------------------------------------------------------- */
/* 3. Comparison Table & Card (`aicss.dev/r/comparison-table` + assistant-ui) */
/* -------------------------------------------------------------------------- */

export type ComparisonStyleChoice =
  | "aicss-table"
  | "assistant-cards"
  | "minimal-versus";

export interface ComparisonStyleDescriptor {
  id: ComparisonStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const COMPARISON_STYLE_OPTIONS: ReadonlyArray<ComparisonStyleDescriptor> = [
  {
    id: "aicss-table",
    title: "AICSS Feature Comparison Matrix (Default)",
    description:
      "aicss.dev/r/comparison-table — Side-by-side matrix highlighting recommended options and boolean checkmarks.",
    isDefault: true,
  },
  {
    id: "assistant-cards",
    title: "Assistant-UI Comparison Cards",
    description:
      "assistant-ui/elements/comparison-card — Side-by-side option cards surfacing key strengths and winner rationale.",
  },
  {
    id: "minimal-versus",
    title: "Minimal Hairline Versus Grid",
    description:
      "Borderless low-noise comparison grid optimized for dense technical trade-offs.",
  },
];

export function isComparisonTableHeaders(headers: ReadonlyArray<string>): boolean {
  if (headers.length < 2 || headers.length > 5) return false;
  const first = (headers[0] ?? "").toLowerCase();
  const joined = headers.map((h) => h.toLowerCase()).join(" ");
  return (
    first === "feature" ||
    first === "criterion" ||
    first === "dimension" ||
    first === "capability" ||
    first === "aspect" ||
    joined.includes(" vs ") ||
    joined.includes("option a") ||
    joined.includes("recommended")
  );
}

function renderComparisonCell(val: string) {
  const norm = val.trim().toLowerCase();
  if (norm === "yes" || norm === "true" || norm === "✓" || norm === "✅") {
    return (
      <span className="inline-flex items-center gap-1 text-success font-medium">
        <Check className="size-3.5" />
        <span>Yes</span>
      </span>
    );
  }
  if (norm === "no" || norm === "false" || norm === "✗" || norm === "❌") {
    return (
      <span className="inline-flex items-center gap-1 text-muted-foreground/65">
        <X className="size-3.5" />
        <span>No</span>
      </span>
    );
  }
  return <span>{val}</span>;
}

export interface ComparisonTableElementProps {
  headers: string[];
  rows: string[][];
  highlightedColumnIndex?: number;
  recommendationNote?: string;
  variant?: ComparisonStyleChoice;
  className?: string;
}

export const ComparisonTableElement: React.FC<ComparisonTableElementProps> = ({
  headers,
  rows,
  highlightedColumnIndex = 1,
  recommendationNote,
  variant = "aicss-table",
  className,
}) => {
  if (variant === "assistant-cards" && headers.length >= 2) {
    const optionHeaders = headers.slice(1);
    return (
      <div className={cn("my-3 space-y-2.5 not-prose", className)}>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          {optionHeaders.map((optionTitle, optIdx) => {
            const colIdx = optIdx + 1;
            const isRecommended = colIdx === highlightedColumnIndex;
            return (
              <div
                key={`${optionTitle}-${colIdx}`}
                className={cn(
                  "flex flex-col justify-between rounded-xl border p-3.5 text-xs transition-colors",
                  isRecommended
                    ? "border-primary/45 bg-primary/[0.04]"
                    : "border-border/65 bg-card/75",
                )}
              >
                <div>
                  <div className="mb-2.5 flex items-center justify-between gap-2">
                    <span className="font-semibold text-foreground">{optionTitle}</span>
                    {isRecommended && (
                      <span className="inline-flex items-center gap-1 rounded-full border border-primary/30 bg-primary/12 px-2 py-0.5 text-[10px] font-medium text-primary">
                        <Sparkles className="size-2.5" />
                        <span>Recommended</span>
                      </span>
                    )}
                  </div>
                  <dl className="space-y-1.5">
                    {rows.map((row, rIdx) => (
                      <div
                        key={rIdx}
                        className="flex items-start justify-between gap-3 border-t border-border/40 pt-1.5 text-[11px]"
                      >
                        <dt className="text-muted-foreground">{row[0]}</dt>
                        <dd className="text-right font-medium text-foreground/90">
                          {renderComparisonCell(row[colIdx] ?? "—")}
                        </dd>
                      </div>
                    ))}
                  </dl>
                </div>
              </div>
            );
          })}
        </div>
        {recommendationNote && (
          <p className="px-1 text-[11px] text-muted-foreground">{recommendationNote}</p>
        )}
      </div>
    );
  }

  return (
    <div
      className={cn(
        "my-3 w-full overflow-x-auto not-prose",
        variant === "aicss-table"
          ? "rounded-xl border border-border/75 bg-card/85 shadow-2xs"
          : "border-y border-border/60",
        className,
      )}
    >
      <table className="w-full min-w-[380px] border-collapse text-left text-xs">
        <thead>
          <tr className="border-b border-border/65 bg-foreground/[0.025]">
            {headers.map((header, idx) => {
              const highlighted = variant === "aicss-table" && idx === highlightedColumnIndex;
              return (
                <th
                  key={idx}
                  className={cn(
                    "px-3.5 py-2.5 font-semibold",
                    idx === 0 ? "text-muted-foreground" : "text-foreground",
                    highlighted && "bg-primary/[0.06] text-primary",
                  )}
                >
                  <div className="flex items-center gap-1.5">
                    <span>{header}</span>
                    {highlighted && (
                      <span className="rounded-full border border-primary/30 bg-primary/12 px-1.5 py-0 text-[9.5px] font-medium text-primary">
                        Best fit
                      </span>
                    )}
                  </div>
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, rIdx) => (
            <tr
              key={rIdx}
              className="border-b border-border/45 last:border-b-0 hover:bg-foreground/[0.02]"
            >
              {headers.map((_, cIdx) => {
                const highlighted = variant === "aicss-table" && cIdx === highlightedColumnIndex;
                return (
                  <td
                    key={cIdx}
                    className={cn(
                      "px-3.5 py-2 text-[11.5px]",
                      cIdx === 0
                        ? "font-medium text-muted-foreground"
                        : "text-foreground/90",
                      highlighted && "bg-primary/[0.035]",
                    )}
                  >
                    {cIdx === 0 ? row[cIdx] : renderComparisonCell(row[cIdx] ?? "—")}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 4. BeautifulUI Tool Chips (`beautifului.dev/r/tool-chips.json`)            */
/* -------------------------------------------------------------------------- */

export interface BeautifulToolChipsProps {
  tools: ReadonlyArray<ToolExecution>;
  className?: string;
}

export const BeautifulToolChips: React.FC<BeautifulToolChipsProps> = ({
  tools,
  className,
}) => {
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  if (tools.length === 0) return null;

  const activeTool = tools.find((t) => t.id === selectedId) ?? null;

  return (
    <div className={cn("my-1.5 space-y-1.5 not-prose", className)}>
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="mr-0.5 font-mono text-[10px] text-muted-foreground">
          {tools.length} {tools.length === 1 ? "tool" : "tools"}
        </span>
        {tools.map((tool) => {
          const isSelected = tool.id === selectedId;
          const isRunning = tool.status === "running";
          const isFailed = tool.status === "error";
          return (
            <button
              key={tool.id}
              type="button"
              onClick={() => setSelectedId(isSelected ? null : tool.id)}
              className={cn(
                "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[11px] font-medium transition-colors",
                isSelected
                  ? "border-primary/45 bg-primary/12 text-primary"
                  : isFailed
                    ? "border-destructive/35 bg-destructive/10 text-destructive"
                    : "border-border/60 bg-foreground/[0.035] text-foreground/85 hover:border-border hover:bg-foreground/[0.06]",
              )}
            >
              <span
                className={cn(
                  "size-1.5 rounded-full",
                  isRunning
                    ? "animate-pulse bg-primary"
                    : isFailed
                      ? "bg-destructive"
                      : "bg-success",
                )}
              />
              <span className="font-mono text-[10.5px]">{tool.name}</span>
              {typeof tool.durationMs === "number" && (
                <span className="font-mono text-[9.5px] text-muted-foreground">
                  {tool.durationMs}ms
                </span>
              )}
            </button>
          );
        })}
      </div>

      {activeTool && (
        <div className="rounded-xl border border-border/65 bg-card/85 p-3 font-mono text-[11px]">
          <div className="mb-1 flex items-center justify-between text-[10px] text-muted-foreground">
            <span>{activeTool.name}</span>
            <button
              type="button"
              onClick={() => setSelectedId(null)}
              className="hover:text-foreground"
            >
              Close
            </button>
          </div>
          <pre className="max-h-40 overflow-auto whitespace-pre-wrap text-foreground/85">
            {activeTool.output || JSON.stringify(activeTool.args ?? {}, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 5. KokonutUI Tool Calls & Approval (`@kokonutui/ai-tool-calls` & approval) */
/* -------------------------------------------------------------------------- */

export const KokonutToolCallCard: React.FC<{
  label: string;
  query?: string;
  request: string;
  result: string;
  running: boolean;
  failed?: boolean;
  defaultOpen?: boolean;
  className?: string;
}> = ({
  label,
  query,
  request,
  result,
  running,
  failed = false,
  defaultOpen = false,
  className,
}) => {
  const [open, setOpen] = React.useState(defaultOpen);
  return (
    <div
      className={cn(
        "w-full max-w-xl overflow-hidden rounded-xl border border-border/65 bg-card/75 text-xs transition-colors",
        className,
      )}
    >
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left hover:bg-foreground/[0.03]"
      >
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex size-5 shrink-0 items-center justify-center rounded-md border border-border/55 bg-foreground/[0.04] text-primary">
            <Wrench className="size-3" />
          </span>
          <span className="font-medium text-foreground/90">{label}</span>
          {query && (
            <span className="truncate rounded bg-foreground/[0.05] px-1.5 py-0.5 font-mono text-[10.5px] text-muted-foreground">
              {query}
            </span>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <span
            className={cn(
              "font-mono text-[10px]",
              running
                ? "animate-pulse text-primary"
                : failed
                  ? "text-destructive"
                  : "text-success",
            )}
          >
            {running ? "running" : failed ? "failed" : "ok"}
          </span>
          <ChevronDown
            className={cn(
              "size-3.5 text-muted-foreground transition-transform",
              open && "rotate-180",
            )}
          />
        </div>
      </button>

      {open && (
        <div className="space-y-2 border-t border-border/55 bg-background/60 p-3 font-mono text-[11px]">
          <div>
            <div className="mb-0.5 text-[9.5px] uppercase tracking-wider text-muted-foreground">
              Parameters
            </div>
            <pre className="max-h-32 overflow-auto whitespace-pre-wrap text-muted-foreground">
              {request}
            </pre>
          </div>
          <div>
            <div className="mb-0.5 text-[9.5px] uppercase tracking-wider text-muted-foreground">
              Output
            </div>
            <pre className="max-h-44 overflow-auto whitespace-pre-wrap text-foreground/90">
              {result}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
};

export const KokonutApprovalCard: React.FC<{
  prompt: SecurityPromptInfo;
  state?: "pending" | "approved" | "denied";
  onAllow: () => void;
  onDeny: () => void;
  className?: string;
}> = ({ prompt, state = "pending", onAllow, onDeny, className }) => {
  const [inspectOpen, setInspectOpen] = React.useState(true);
  const argsPreview =
    typeof prompt.arguments === "string"
      ? prompt.arguments
      : JSON.stringify(prompt.arguments ?? {}, null, 2);

  return (
    <motion.div
      initial={{ opacity: 0, y: 6, scale: 0.99 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ type: "spring", stiffness: 360, damping: 28 }}
      className={cn(
        "w-full max-w-xl overflow-hidden rounded-2xl border border-warning/35 bg-card/85 p-3.5 text-xs shadow-md backdrop-blur-xl",
        className,
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-xl border border-warning/35 bg-warning/12 text-warning">
            <AlertTriangle className="size-4" />
          </span>
          <div>
            <div className="flex items-center gap-1.5 font-semibold text-foreground">
              <span>Approve {prompt.toolName}</span>
            </div>
            <div className="text-[11px] text-muted-foreground">
              {prompt.description || "SecurityGuard human-in-the-loop verification"}
            </div>
          </div>
        </div>
        <span
          className={cn(
            "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[10px]",
            state === "approved"
              ? "border-emerald-500/35 bg-emerald-500/10 text-emerald-500"
              : state === "denied"
                ? "border-destructive/35 bg-destructive/10 text-destructive"
                : "border-warning/35 bg-warning/10 text-warning",
          )}
        >
          <ShieldCheck className="size-3" />
          <span>{state}</span>
        </span>
      </div>

      <div className="mt-2.5 overflow-hidden rounded-xl border border-border/60 bg-background/70 font-mono text-[11px] text-foreground/85">
        <button
          type="button"
          onClick={() => setInspectOpen((o) => !o)}
          className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left text-[10px] text-muted-foreground hover:bg-foreground/[0.03]"
        >
          <Terminal className="size-3 text-primary" />
          <span className="font-semibold text-foreground/90">{prompt.toolName}</span>
          <span className="ml-auto text-[9.5px] opacity-75">
            {inspectOpen ? "Hide payload" : "Inspect payload"}
          </span>
          <ChevronDown
            className={cn(
              "size-3 transition-transform",
              inspectOpen && "rotate-180",
            )}
          />
        </button>
        <AnimatePresence initial={false}>
          {inspectOpen && (
            <motion.pre
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.16 }}
              className="max-h-36 overflow-auto border-t border-border/50 px-2.5 py-2 whitespace-pre-wrap"
            >
              {argsPreview}
            </motion.pre>
          )}
        </AnimatePresence>
      </div>

      {state === "pending" && (
        <div className="mt-3 flex items-center justify-end gap-2">
          <motion.button
            type="button"
            whileTap={{ scale: 0.96 }}
            onClick={onDeny}
            className="rounded-xl border border-border/70 bg-transparent px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-muted/60 hover:text-foreground"
          >
            Deny
          </motion.button>
          <motion.button
            type="button"
            whileTap={{ scale: 0.96 }}
            onClick={onAllow}
            className="inline-flex items-center gap-1.5 rounded-xl bg-primary px-3.5 py-1.5 text-xs font-semibold text-primary-foreground shadow-xs transition-opacity hover:opacity-90"
          >
            <Check className="size-3.5" />
            <span>Allow Execution</span>
          </motion.button>
        </div>
      )}
    </motion.div>
  );
};
