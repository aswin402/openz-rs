import * as React from "react"
import {
  AlertTriangle,
  Braces,
  Check,
  CheckCircle2,
  Clipboard,
  Copy,
  FileDiff as FileDiffIcon,
  FileText,
  Image as ImageIcon,
  Loader2,
  ShieldAlert,
  Terminal,
} from "lucide-react"

import { BeuiAlert } from "./alert"
import { BeuiCollapsible } from "./collapsible"
import { BeuiFileDiff, type BeuiDiffViewMode } from "./file-diff"
import { ImageGeneration, extractToolImagePayload } from "./image-generation"
import { BeuiJsonViewer, type BeuiJsonViewerVariant } from "./json-viewer"
import { parseUnifiedDiff } from "./parsers"
import { BeuiSubagentCard, type BeuiSubagentCardVariant } from "./subagent-card"
import { BeuiTerminalOutput, type BeuiTerminalVariant } from "./terminal-output"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"
import { collapseWhitespace, formatDuration } from "@/shared/lib/format"
import type { ToolExecution } from "@/types"

export type BeuiToolResultVariant = "beui-card" | "compact-pill" | "expanded-terminal"

export interface ToolResultProps {
  tool: ToolExecution
  variant?: BeuiToolResultVariant
  diffLayout?: BeuiDiffViewMode
  imageVariant?: "beui-frame" | "minimal"
  terminalVariant?: BeuiTerminalVariant
  jsonVariant?: BeuiJsonViewerVariant
  subagentVariant?: BeuiSubagentCardVariant
  className?: string
}

function safeStringify(value: unknown): string {
  if (value === undefined || value === null) return ""
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function summarizeOneLine(value: unknown): string {
  const flat = collapseWhitespace(safeStringify(value))
  if (!flat) return "Executed native tool"
  return flat.length > 140 ? `${flat.slice(0, 137)}...` : flat
}

function isShellToolName(name: string): boolean {
  const lower = name.toLowerCase()
  return (
    lower === "exec_command" ||
    lower === "run_command" ||
    lower.startsWith("cargo_") ||
    lower === "shell"
  )
}

function isSubagentToolName(name: string): boolean {
  const lower = name.toLowerCase()
  return (
    lower === "delegate_task" ||
    lower === "parallel_research" ||
    lower === "orchestrate_workflow" ||
    lower === "evaluator_optimizer_loop"
  )
}

function extractCommandLine(args: unknown): string | undefined {
  if (!args || typeof args !== "object" || Array.isArray(args)) return undefined
  const obj = args as Record<string, unknown>
  const cand =
    obj.command ??
    obj.command_line ??
    obj.CommandLine ??
    obj.cmd ??
    obj.subcommand
  return typeof cand === "string" ? cand : undefined
}

function isStructuredJsonOutput(raw: unknown, text: string): boolean {
  if (raw !== null && typeof raw === "object") return true
  const trimmed = text.trim()
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      JSON.parse(trimmed)
      return true
    } catch {
      return false
    }
  }
  return false
}

function detectDiffPayload(
  tool: ToolExecution,
  outputText: string
): { file?: string; patch: string } | null {
  const argsObj =
    tool.args && typeof tool.args === "object" && !Array.isArray(tool.args)
      ? (tool.args as Record<string, unknown>)
      : null

  const fileHint =
    (typeof argsObj?.path === "string" && argsObj.path) ||
    (typeof argsObj?.file_path === "string" && argsObj.file_path) ||
    (typeof argsObj?.file === "string" && argsObj.file) ||
    (typeof argsObj?.Path === "string" && argsObj.Path) ||
    undefined

  const patchArg =
    (typeof argsObj?.patch === "string" && argsObj.patch) ||
    (typeof argsObj?.diff === "string" && argsObj.diff) ||
    ""

  if (patchArg.includes("@@") || patchArg.startsWith("--- ")) {
    const parsed = parseUnifiedDiff(patchArg)
    return parsed.rows.length > 0 ? { file: fileHint ?? parsed.file, patch: patchArg } : null
  }

  if (outputText.includes("@@ ") && (outputText.includes("\n+") || outputText.includes("\n-"))) {
    const parsed = parseUnifiedDiff(outputText)
    return parsed.rows.length > 0 ? { file: fileHint ?? parsed.file, patch: outputText } : null
  }

  const beforeStr =
    (typeof argsObj?.old_text === "string" ? argsObj.old_text : undefined) ??
    (typeof argsObj?.old_str === "string" ? argsObj.old_str : undefined) ??
    (typeof argsObj?.search === "string" ? argsObj.search : undefined) ??
    (typeof argsObj?.target === "string" ? argsObj.target : undefined) ??
    (typeof argsObj?.Target === "string" ? argsObj.Target : undefined)

  const afterStr =
    (typeof argsObj?.new_text === "string" ? argsObj.new_text : undefined) ??
    (typeof argsObj?.new_str === "string" ? argsObj.new_str : undefined) ??
    (typeof argsObj?.replace === "string" ? argsObj.replace : undefined) ??
    (typeof argsObj?.replacement === "string" ? argsObj.replacement : undefined) ??
    (typeof argsObj?.Replacement === "string" ? argsObj.Replacement : undefined)

  if (beforeStr !== undefined && afterStr !== undefined) {
    const delLines = beforeStr.split("\n").map((l) => `-${l}`)
    const addLines = afterStr.split("\n").map((l) => `+${l}`)
    const synthetic = [
      `@@ -1,${delLines.length} +1,${addLines.length} @@`,
      ...delLines,
      ...addLines,
    ].join("\n")
    return { file: fileHint, patch: synthetic }
  }

  return null
}

/**
 * @beui/tool-result — Structured collapsible tool execution disclosure built with
 * shadcn/ui Tabs, @beui/terminal-output, @beui/file-diff, @beui/json-viewer, @beui/subagent-card, and @beui/image-generation.
 */
export const ToolResult: React.FC<ToolResultProps> = ({
  tool,
  variant = "beui-card",
  diffLayout = "unified",
  imageVariant = "beui-frame",
  terminalVariant = "console",
  jsonVariant = "tree",
  subagentVariant = "card",
  className,
}) => {
  const argsText = React.useMemo(() => safeStringify(tool.args), [tool.args])
  const outputText = React.useMemo(() => safeStringify(tool.output), [tool.output])
  const errorText = React.useMemo(() => safeStringify(tool.error), [tool.error])
  const duration = formatDuration(tool.durationMs)

  const diffPayload = React.useMemo(
    () => detectDiffPayload(tool, outputText),
    [tool, outputText]
  )
  const imagePayload = React.useMemo(
    () => extractToolImagePayload(tool.name, tool.args, outputText),
    [tool.name, tool.args, outputText]
  )
  const hasImage = Boolean(
    imagePayload.src || (imagePayload.isImageTool && tool.status === "running")
  )
  const isShell = isShellToolName(tool.name)
  const isSubagent = isSubagentToolName(tool.name)
  const isJsonOutput = React.useMemo(
    () => !isShell && isStructuredJsonOutput(tool.output, outputText),
    [isShell, tool.output, outputText]
  )

  const defaultOpen =
    variant === "expanded-terminal" ||
    tool.status === "error" ||
    Boolean(diffPayload) ||
    hasImage

  const [open, setOpen] = React.useState(defaultOpen)
  const [copiedKey, setCopiedKey] = React.useState<string | null>(null)

  if (isSubagent) {
    const argsObj =
      tool.args && typeof tool.args === "object" && !Array.isArray(tool.args)
        ? (tool.args as Record<string, unknown>)
        : {}
    const profileName =
      (typeof argsObj.profile === "string" && argsObj.profile) ||
      (typeof argsObj.agent === "string" && argsObj.agent) ||
      tool.name
    const goalText =
      (typeof argsObj.task === "string" && argsObj.task) ||
      (typeof argsObj.prompt === "string" && argsObj.prompt) ||
      (typeof argsObj.goal === "string" && argsObj.goal) ||
      summarizeOneLine(tool.args)

    return (
      <BeuiSubagentCard
        agentName={profileName}
        mode={tool.name}
        goal={goalText}
        status={tool.status}
        durationLabel={duration || undefined}
        summary={errorText || (outputText ? summarizeOneLine(outputText) : undefined)}
        variant={subagentVariant}
        className={className}
      />
    )
  }

  const initialTab = errorText
    ? "error"
    : hasImage
      ? "image"
      : diffPayload
        ? "diff"
        : outputText
          ? "output"
          : "args"

  const handleCopy = (key: string, value: string) => {
    navigator.clipboard?.writeText(value).catch(() => {})
    setCopiedKey(key)
    window.setTimeout(() => setCopiedKey(null), 1600)
  }

  const statusBadge = (() => {
    switch (tool.status) {
      case "running":
        return (
          <Badge variant="warning" className="gap-1">
            <Loader2 className="size-2.5 animate-spin" />
            <span>Running</span>
          </Badge>
        )
      case "success":
        return (
          <Badge variant="success" className="gap-1">
            <CheckCircle2 className="size-2.5" />
            <span>Done</span>
          </Badge>
        )
      case "error":
        return (
          <Badge variant="destructive" className="gap-1">
            <AlertTriangle className="size-2.5" />
            <span>Failed</span>
          </Badge>
        )
      case "awaiting_approval":
        return (
          <Badge variant="warning" className="gap-1">
            <ShieldAlert className="size-2.5" />
            <span>Approval</span>
          </Badge>
        )
    }
  })()

  if (variant === "compact-pill" && !open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className={cn(
          "my-1 mr-1.5 inline-flex max-w-full items-center gap-1.5 rounded-full border border-border/75 bg-card/75 px-2.5 py-1 text-[11px] font-medium text-foreground transition-colors hover:border-primary/45",
          className
        )}
      >
        {tool.status === "running" ? (
          <Loader2 className="size-3 animate-spin text-primary" />
        ) : (
          <Terminal className="size-3 text-primary" />
        )}
        <span className="truncate font-mono">{tool.name}</span>
        {duration ? (
          <span className="text-[10px] text-muted-foreground">{duration}</span>
        ) : null}
      </button>
    )
  }

  const summaryPreview = summarizeOneLine(tool.error || tool.output || tool.args)

  return (
    <BeuiCollapsible
      open={open}
      onOpenChange={setOpen}
      variant="bordered"
      className={cn("my-2", className)}
      icon={
        tool.status === "running" ? (
          <Loader2 className="size-3.5 animate-spin text-primary" />
        ) : (
          <Terminal className="size-3.5 text-primary" />
        )
      }
      title={
        <span className="flex min-w-0 items-center gap-2">
          <span className="truncate font-mono text-xs font-semibold text-foreground">
            {tool.name}
          </span>
          {!open && (
            <span className="hidden truncate text-[11px] font-normal text-muted-foreground md:inline">
              {summaryPreview}
            </span>
          )}
        </span>
      }
      badge={
        <span className="flex items-center gap-1.5">
          {duration && (
            <Badge variant="outline" className="px-1.5 py-0 font-mono text-[10px]">
              {duration}
            </Badge>
          )}
          {statusBadge}
        </span>
      }
    >
      <Tabs defaultValue={initialTab} className="w-full gap-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <TabsList className="h-7 bg-muted/70 p-0.5">
            {hasImage && (
              <TabsTrigger value="image" className="px-2 text-[11px]">
                <ImageIcon className="size-3" />
                <span>Visual</span>
              </TabsTrigger>
            )}
            {diffPayload && (
              <TabsTrigger value="diff" className="px-2 text-[11px]">
                <FileDiffIcon className="size-3" />
                <span>Diff</span>
              </TabsTrigger>
            )}
            {outputText && (
              <TabsTrigger value="output" className="px-2 text-[11px]">
                {isJsonOutput ? (
                  <Braces className="size-3" />
                ) : isShell ? (
                  <Terminal className="size-3" />
                ) : (
                  <FileText className="size-3" />
                )}
                <span>{isShell ? "Console" : isJsonOutput ? "JSON" : "Result"}</span>
              </TabsTrigger>
            )}
            {argsText && (
              <TabsTrigger value="args" className="px-2 text-[11px]">
                <Clipboard className="size-3" />
                <span>Args</span>
              </TabsTrigger>
            )}
            {errorText && (
              <TabsTrigger value="error" className="px-2 text-[11px] text-destructive">
                <AlertTriangle className="size-3" />
                <span>Error</span>
              </TabsTrigger>
            )}
          </TabsList>

          {(outputText || argsText) && (
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={() => handleCopy("payload", outputText || argsText)}
              className="h-6 text-[10px] text-muted-foreground"
            >
              {copiedKey === "payload" ? (
                <>
                  <Check className="size-3 text-success" />
                  <span>Copied</span>
                </>
              ) : (
                <>
                  <Copy className="size-3" />
                  <span>Copy</span>
                </>
              )}
            </Button>
          )}
        </div>

        {hasImage && (
          <TabsContent value="image">
            <ImageGeneration
              src={imagePayload.src}
              prompt={imagePayload.prompt}
              status={
                tool.status === "running"
                  ? "generating"
                  : tool.status === "error"
                    ? "failed"
                    : "completed"
              }
              variant={imageVariant}
              className="my-0"
            />
          </TabsContent>
        )}

        {diffPayload && (
          <TabsContent value="diff">
            <BeuiFileDiff
              file={diffPayload.file}
              patch={diffPayload.patch}
              defaultView={diffLayout}
              className="my-0"
            />
          </TabsContent>
        )}

        {outputText && (
          <TabsContent value="output">
            {isShell ? (
              <BeuiTerminalOutput
                command={extractCommandLine(tool.args)}
                output={outputText}
                exitCode={tool.status === "error" ? 1 : 0}
                isRunning={tool.status === "running"}
                variant={terminalVariant}
              />
            ) : isJsonOutput ? (
              <BeuiJsonViewer
                value={tool.output ?? outputText}
                label={tool.name}
                variant={jsonVariant}
              />
            ) : (
              <pre className="max-h-64 overflow-auto rounded-lg border border-border/55 bg-muted/40 p-2.5 font-mono text-[11px] leading-relaxed text-foreground/90">
                {outputText}
              </pre>
            )}
          </TabsContent>
        )}

        {argsText && (
          <TabsContent value="args">
            {isStructuredJsonOutput(tool.args, argsText) ? (
              <BeuiJsonViewer value={tool.args} label="Arguments" variant={jsonVariant} />
            ) : (
              <pre className="max-h-56 overflow-auto rounded-lg border border-border/55 bg-muted/40 p-2.5 font-mono text-[11px] leading-relaxed text-foreground/90">
                {argsText}
              </pre>
            )}
          </TabsContent>
        )}

        {errorText && (
          <TabsContent value="error">
            <BeuiAlert tone="destructive" title="Tool execution failed">
              <pre className="mt-1 max-h-48 overflow-auto font-mono text-[11px] whitespace-pre-wrap">
                {errorText}
              </pre>
            </BeuiAlert>
          </TabsContent>
        )}

        {!hasImage && !diffPayload && !outputText && !argsText && !errorText && (
          <div className="rounded-lg border border-dashed border-border/60 px-3 py-2 text-[11px] text-muted-foreground">
            Waiting for tool telemetry…
          </div>
        )}
      </Tabs>
    </BeuiCollapsible>
  )
}
