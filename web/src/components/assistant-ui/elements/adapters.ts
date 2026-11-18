import { stripTerminalFormatting } from "@/features/activity/activityAdapters";
import { formatDuration } from "@/shared/lib/format";
import type { SecurityPromptInfo, ToolExecution } from "@/types";
import type { ApprovalState } from "./approval-card";
import type { ToolCallProps } from "./tool-call";

export type ToolResultStyleChoice =
  | "beui-activity"
  | "assistant-collapsed"
  | "assistant-error-open"
  | "assistant-expanded"
  | "beautiful-tool-chips"
  | "kokonut-tool-calls";

export type ApprovalStyleChoice =
  | "assistant-default"
  | "assistant-receipt"
  | "assistant-destructive"
  | "kokonut-approval";

export interface AssistantUiStyleOption<T extends string> {
  id: T;
  title: string;
  description: string;
  isDefault: boolean;
}

export const TOOL_CALL_STYLE_OPTIONS: ReadonlyArray<
  AssistantUiStyleOption<ToolResultStyleChoice>
> = [
  {
    id: "beui-activity",
    title: "Minimal @beui/agent-activity Row (Default)",
    description:
      "Clean single-line Read / Wrote / Ran activity row inside Completed N steps without duplicate standalone tool cards.",
    isDefault: true,
  },
  {
    id: "beautiful-tool-chips",
    title: "BeautifulUI Tool Chips (beautifului.dev/r/tool-chips)",
    description:
      "Glanceable horizontal capsule chips summarizing tool executions with click-to-inspect output.",
    isDefault: false,
  },
  {
    id: "kokonut-tool-calls",
    title: "KokonutUI Tool Call Cards (@kokonutui/ai-tool-calls)",
    description:
      "Compact monospace tool traces with inline parameter pills and expandable output inspection.",
    isDefault: false,
  },
  {
    id: "assistant-collapsed",
    title: "Collapsed Disclosure (@assistant-ui/elements-tool-call)",
    description:
      "Interactive inline tool row with arguments pill and checkmark; click to inspect Request and Result.",
    isDefault: false,
  },
  {
    id: "assistant-error-open",
    title: "Auto-Expand Failures (@assistant-ui/elements-tool-call)",
    description:
      "Keeps succeeded tool calls collapsed while automatically expanding failed calls to surface errors.",
    isDefault: false,
  },
  {
    id: "assistant-expanded",
    title: "Expanded Request & Result (@assistant-ui/elements-tool-call)",
    description:
      "Opens the Request and Result inspection card by default for every tool execution.",
    isDefault: false,
  },
];

export const APPROVAL_CARD_STYLE_OPTIONS: ReadonlyArray<
  AssistantUiStyleOption<ApprovalStyleChoice>
> = [
  {
    id: "assistant-default",
    title: "Approval Card + Tool Call Handoff (@assistant-ui/elements-approval-card)",
    description:
      "Shows assistant-ui ApprovalCard while pending or running, then hands off cleanly to ToolCall once finished.",
    isDefault: true,
  },
  {
    id: "kokonut-approval",
    title: "KokonutUI Security Approval (@kokonutui/ai-tool-approval)",
    description:
      "High-craft SecurityGuard card with command parameter block and crisp Allow / Deny actions.",
    isDefault: false,
  },
  {
    id: "assistant-receipt",
    title: "Persistent Approval Receipt (@assistant-ui/elements-approval-card)",
    description:
      "Retains the completed ApprovalCard Finished receipt above the ToolCall output for full security auditing.",
    isDefault: false,
  },
  {
    id: "assistant-destructive",
    title: "High-Caution Destructive Emphasis (@assistant-ui/elements-approval-card)",
    description:
      "Renders all SecurityGuard permission prompts with destructive red action styling.",
    isDefault: false,
  },
];

const TOOL_STYLE_NORMALIZATION: Record<string, ToolResultStyleChoice> = {
  "beui-activity": "beui-activity",
  "beautiful-tool-chips": "beautiful-tool-chips",
  "kokonut-tool-calls": "kokonut-tool-calls",
  "assistant-collapsed": "assistant-collapsed",
  "assistant-error-open": "assistant-error-open",
  "assistant-expanded": "assistant-expanded",
  // Legacy @beui values migrated seamlessly
  "beui-card": "assistant-collapsed",
  "compact-pill": "assistant-collapsed",
  "expanded-terminal": "assistant-expanded",
};

const APPROVAL_STYLE_NORMALIZATION: Record<string, ApprovalStyleChoice> = {
  "assistant-default": "assistant-default",
  "kokonut-approval": "kokonut-approval",
  "assistant-receipt": "assistant-receipt",
  "assistant-destructive": "assistant-destructive",
  // Legacy @beui values migrated seamlessly
  "beui-tool-approval": "assistant-default",
  "beui-approval-card": "assistant-receipt",
};

export function normalizeToolResultStyle(raw: unknown): ToolResultStyleChoice {
  if (typeof raw !== "string") return "beui-activity";
  return TOOL_STYLE_NORMALIZATION[raw] ?? "beui-activity";
}

export function normalizeApprovalStyle(raw: unknown): ApprovalStyleChoice {
  if (typeof raw !== "string") return "assistant-default";
  return APPROVAL_STYLE_NORMALIZATION[raw] ?? "assistant-default";
}

export function resolveInitialToolCallOpen(
  style: ToolResultStyleChoice,
  failed: boolean,
): boolean {
  if (style === "assistant-expanded") return true;
  if (style === "assistant-error-open" && failed) return true;
  return false;
}

const COMMAND_KEYS = ["command", "CommandLine", "command_line", "cmd"] as const;

const PRIMARY_QUERY_KEYS = [
  "command",
  "CommandLine",
  "command_line",
  "cmd",
  "query",
  "pattern",
  "path",
  "file_path",
  "file",
  "Path",
  "url",
  "action",
  "subcommand",
  "section",
  "name",
  "topic",
  "goal",
  "task",
  "prompt",
] as const;

const RESULT_TEXT_KEYS = [
  "stdout",
  "output",
  "content",
  "result",
  "summary",
  "message",
  "stderr",
  "error",
] as const;

const SEARCH_TOKENS = [
  "search",
  "grep",
  "crawl",
  "find",
  "outline",
  "recall",
  "query",
  "rust_docs",
] as const;

const READ_TOOL_NAMES = new Set([
  "read_file",
  "list_dir",
  "list_directory",
  "doc_reader",
  "db_inspector",
  "openz_inventory",
  "system_info",
  "check_port",
  "web_fetch",
  "retrieve_original",
]);

const EDIT_TOOL_NAMES = new Set(["patch_file", "replace_lines"]);

const WRITE_TOOL_NAMES = new Set([
  "write_file",
  "db_write",
  "curate_skill",
  "workflow_memory",
  "smart_memory_store",
]);

const DELEGATE_TOOL_NAMES = new Set([
  "delegate_task",
  "orchestrate_workflow",
  "parallel_research",
  "evaluator_optimizer_loop",
]);

const DESTRUCTIVE_TOKENS = [
  "rm -",
  "rm /",
  "rmdir ",
  "sudo ",
  "mkfs",
  "dd if=",
  "chmod 777",
  "drop table",
  "truncate table",
  "delete from",
  "git reset --hard",
  "git clean -fd",
  "kill -9",
  "pkill ",
  "shutdown",
  "reboot",
] as const;

const DESTRUCTIVE_TOOL_PREFIXES = ["delete_", "remove_", "drop_", "purge_"] as const;

type ToolVerbCategory =
  | "command"
  | "search"
  | "read"
  | "edit"
  | "write"
  | "delegate"
  | "call";

interface VerbPair {
  active: string;
  done: string;
  failed: string;
}

const VERB_TABLE: Record<ToolVerbCategory, VerbPair> = {
  command: { active: "Running", done: "Ran", failed: "Failed" },
  search: { active: "Searching", done: "Searched", failed: "Search failed" },
  read: { active: "Reading", done: "Read", failed: "Read failed" },
  edit: { active: "Editing", done: "Edited", failed: "Edit failed" },
  write: { active: "Writing", done: "Wrote", failed: "Write failed" },
  delegate: { active: "Delegating", done: "Delegated", failed: "Delegation failed" },
  call: { active: "Calling", done: "Called", failed: "Failed" },
};

function parseRecordArgs(args: unknown): Record<string, unknown> | null {
  if (typeof args === "object" && args !== null && !Array.isArray(args)) {
    return args as Record<string, unknown>;
  }
  if (typeof args === "string") {
    const trimmed = args.trim();
    if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
      try {
        const parsed = JSON.parse(trimmed) as unknown;
        if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
          return parsed as Record<string, unknown>;
        }
      } catch {
        return null;
      }
    }
  }
  return null;
}

function collapseWhitespaceSingleLine(input: string): string {
  const cleaned = stripTerminalFormatting(input).trim();
  if (!cleaned) return "";
  let out = "";
  let prevSpace = false;
  for (let i = 0; i < cleaned.length; i += 1) {
    const ch = cleaned[i];
    const isSpace = ch === " " || ch === "\n" || ch === "\r" || ch === "\t";
    if (isSpace) {
      if (!prevSpace) {
        out += " ";
        prevSpace = true;
      }
    } else {
      out += ch;
      prevSpace = false;
    }
  }
  return out.trim();
}

function classifyVerbCategory(toolName: string): ToolVerbCategory {
  const lower = toolName.toLowerCase();
  if (
    lower === "exec_command" ||
    lower === "git_manager" ||
    lower === "cargo_manager" ||
    lower.startsWith("cargo_")
  ) {
    return "command";
  }
  if (SEARCH_TOKENS.some((token) => lower.includes(token))) {
    return "search";
  }
  if (
    READ_TOOL_NAMES.has(lower) ||
    lower.startsWith("read_") ||
    lower.startsWith("list_") ||
    lower.startsWith("get_")
  ) {
    return "read";
  }
  if (
    EDIT_TOOL_NAMES.has(lower) ||
    lower.includes("patch") ||
    lower.includes("edit") ||
    lower.includes("replace")
  ) {
    return "edit";
  }
  if (
    WRITE_TOOL_NAMES.has(lower) ||
    lower.startsWith("write_") ||
    lower.startsWith("create_") ||
    lower.startsWith("save_")
  ) {
    return "write";
  }
  if (DELEGATE_TOOL_NAMES.has(lower)) {
    return "delegate";
  }
  return "call";
}

function extractCommandValue(record: Record<string, unknown> | null): {
  key: string;
  command: string;
} | null {
  if (!record) return null;
  for (const key of COMMAND_KEYS) {
    const raw = record[key];
    if (typeof raw === "string" && raw.trim()) {
      return { key, command: stripTerminalFormatting(raw).trim() };
    }
  }
  return null;
}

function extractPrimaryQuery(toolName: string, args: unknown): string {
  if (typeof args === "string") {
    const record = parseRecordArgs(args);
    if (!record) {
      return collapseWhitespaceSingleLine(args);
    }
    return extractPrimaryQuery(toolName, record);
  }

  const record = parseRecordArgs(args);
  if (!record) return "";

  const lower = toolName.toLowerCase();
  if (lower === "exec_command") {
    const cmdInfo = extractCommandValue(record);
    if (cmdInfo) {
      const single = collapseWhitespaceSingleLine(cmdInfo.command);
      return single.startsWith("$ ") ? single : `$ ${single}`;
    }
  }

  for (const key of PRIMARY_QUERY_KEYS) {
    const val = record[key];
    if (typeof val === "string" && val.trim()) {
      return collapseWhitespaceSingleLine(val);
    }
  }

  const entries = Object.entries(record);
  if (entries.length === 0) return "";
  const [firstKey, firstVal] = entries[0];
  const scalar =
    typeof firstVal === "string"
      ? collapseWhitespaceSingleLine(firstVal)
      : collapseWhitespaceSingleLine(JSON.stringify(firstVal) ?? "");
  return scalar ? `${firstKey}: ${scalar}` : firstKey;
}

function formatToolRequestText(toolName: string, args: unknown): string {
  if (args === undefined || args === null) return "No arguments";
  const record = parseRecordArgs(args);
  if (record) {
    const entries = Object.entries(record);
    if (entries.length === 0) return "No arguments";
    if (toolName.toLowerCase() === "exec_command") {
      const cmdInfo = extractCommandValue(record);
      if (cmdInfo && entries.length === 1) {
        return cmdInfo.command.startsWith("$ ")
          ? cmdInfo.command
          : `$ ${cmdInfo.command}`;
      }
    }
    try {
      return stripTerminalFormatting(JSON.stringify(record, null, 2));
    } catch {
      return String(args);
    }
  }
  if (typeof args === "string") {
    const cleaned = stripTerminalFormatting(args).trim();
    return cleaned || "No arguments";
  }
  try {
    return JSON.stringify(args, null, 2);
  } catch {
    return String(args);
  }
}

function formatToolResultText(tool: ToolExecution): string {
  if (tool.status === "running" || tool.status === "awaiting_approval") {
    return "Running…";
  }

  const raw = tool.status === "error" ? tool.error ?? tool.output ?? "" : tool.output ?? tool.error ?? "";
  const trimmed = stripTerminalFormatting(raw).trim();
  const durationSuffix = formatDuration(tool.durationMs);

  if (!trimmed) {
    const base = tool.status === "error" ? "Tool execution failed." : "Completed with no output.";
    return durationSuffix ? `${base} (${durationSuffix})` : base;
  }

  if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
        const obj = parsed as Record<string, unknown>;
        const parts: string[] = [];
        for (const key of RESULT_TEXT_KEYS) {
          const val = obj[key];
          if (typeof val === "string" && val.trim()) {
            const cleanedVal = stripTerminalFormatting(val).trim();
            if (!parts.includes(cleanedVal)) {
              parts.push(cleanedVal);
            }
          }
        }
        const exitCode =
          typeof obj.status_code === "number"
            ? obj.status_code
            : typeof obj.exit_code === "number"
              ? obj.exit_code
              : undefined;
        if (parts.length > 0) {
          const joined = parts.join("\n");
          const meta: string[] = [];
          if (exitCode !== undefined && exitCode !== 0) meta.push(`exit ${exitCode}`);
          if (durationSuffix) meta.push(durationSuffix);
          return meta.length > 0 ? `${joined}\n\n[${meta.join(" · ")}]` : joined;
        }
        const pretty = stripTerminalFormatting(JSON.stringify(obj, null, 2));
        return durationSuffix ? `${pretty}\n\n[${durationSuffix}]` : pretty;
      }
    } catch {
      // Fall through to raw cleaned text
    }
  }

  return durationSuffix ? `${trimmed}\n\n[${durationSuffix}]` : trimmed;
}

export type MappedToolCallData = Omit<
  ToolCallProps,
  "open" | "onOpenChange" | "className"
>;

export function mapToolExecutionToToolCallData(
  tool: ToolExecution,
): MappedToolCallData {
  const category = classifyVerbCategory(tool.name);
  const verbs = VERB_TABLE[category];
  const running =
    tool.status === "running" || tool.status === "awaiting_approval";
  const failed = tool.status === "error";

  const activeLabel = `${verbs.active} ${tool.name}`;
  const label = `${failed ? verbs.failed : verbs.done} ${tool.name}`;
  const query = extractPrimaryQuery(tool.name, tool.args);
  const request = formatToolRequestText(tool.name, tool.args);
  const result = formatToolResultText(tool);

  return {
    label,
    activeLabel,
    query,
    request,
    result,
    running,
    failed,
  };
}

export function formatApprovalArgs(args: unknown): string {
  if (!args) return "";
  if (typeof args === "string") return stripTerminalFormatting(args);
  try {
    return stripTerminalFormatting(JSON.stringify(args, null, 2));
  } catch {
    return String(args);
  }
}

export function summarizeApprovalInline(prompt: {
  description?: string;
  arguments?: unknown;
}): string {
  const desc = prompt.description ? stripTerminalFormatting(prompt.description).trim() : "";
  if (desc) return desc;
  if (!prompt.arguments) return "";
  if (typeof prompt.arguments === "string") {
    return stripTerminalFormatting(prompt.arguments).trim();
  }
  const record = parseRecordArgs(prompt.arguments);
  if (!record) return "";
  const entries = Object.entries(record);
  if (entries.length === 0) return "";
  const [firstKey, firstVal] = entries[0];
  const rendered =
    typeof firstVal === "string" ? firstVal : JSON.stringify(firstVal);
  return `${firstKey}: ${rendered}`;
}

export function isDestructiveToolPrompt(
  toolName: string,
  args: unknown,
  description?: string,
): boolean {
  const lowerTool = toolName.toLowerCase();
  if (DESTRUCTIVE_TOOL_PREFIXES.some((prefix) => lowerTool.startsWith(prefix))) {
    return true;
  }
  const combined = `${description ?? ""} ${formatApprovalArgs(args)}`.toLowerCase();
  return DESTRUCTIVE_TOKENS.some((token) => combined.includes(token));
}

export interface MappedApprovalCardData {
  state: ApprovalState;
  title: string;
  subtitle: string;
  description?: string;
  command?: string;
  details?: ReadonlyArray<{ label: string; value: string }>;
  variant: "default" | "destructive";
}

const PROMPT_STATUS_TO_APPROVAL_STATE: Record<
  SecurityPromptInfo["status"],
  ApprovalState
> = {
  pending: "request",
  approved: "done",
  denied: "denied",
};

function formatDetailScalar(val: unknown): string {
  if (typeof val === "string") return stripTerminalFormatting(val);
  if (typeof val === "number" || typeof val === "boolean") return String(val);
  if (val === null || val === undefined) return "";
  try {
    return stripTerminalFormatting(JSON.stringify(val));
  } catch {
    return String(val);
  }
}

export function mapSecurityPromptToApprovalData(
  prompt: SecurityPromptInfo,
  options?: {
    stateOverride?: ApprovalState;
    forceDestructive?: boolean;
  },
): MappedApprovalCardData {
  const state =
    options?.stateOverride ?? PROMPT_STATUS_TO_APPROVAL_STATE[prompt.status];
  const record = parseRecordArgs(prompt.arguments);
  const cmdInfo = extractCommandValue(record);

  let command: string | undefined;
  const detailsList: Array<{ label: string; value: string }> = [];

  if (cmdInfo) {
    command = cmdInfo.command.startsWith("$ ")
      ? cmdInfo.command
      : `$ ${cmdInfo.command}`;
    if (record) {
      for (const [key, val] of Object.entries(record)) {
        if (key === cmdInfo.key) continue;
        const formatted = formatDetailScalar(val);
        if (formatted) {
          detailsList.push({ label: key, value: formatted });
        }
      }
    }
  } else if (typeof prompt.arguments === "string" && prompt.arguments.trim()) {
    const rawStr = stripTerminalFormatting(prompt.arguments).trim();
    if (prompt.toolName.toLowerCase() === "exec_command") {
      command = rawStr.startsWith("$ ") ? rawStr : `$ ${rawStr}`;
    } else {
      detailsList.push({ label: "arguments", value: rawStr });
    }
  } else if (record) {
    for (const [key, val] of Object.entries(record)) {
      const formatted = formatDetailScalar(val);
      if (formatted) {
        detailsList.push({ label: key, value: formatted });
      }
    }
  }

  const rawDesc = prompt.description
    ? stripTerminalFormatting(prompt.description).trim()
    : "";
  const normalizedCommandRaw = cmdInfo?.command ?? "";
  const isRedundantDesc =
    !rawDesc ||
    rawDesc === normalizedCommandRaw ||
    rawDesc === command ||
    (detailsList.length === 1 &&
      rawDesc === `${detailsList[0].label}: ${detailsList[0].value}`) ||
    (detailsList.length === 1 &&
      rawDesc === `${detailsList[0].label}: "${detailsList[0].value}"`);

  const destructive =
    Boolean(options?.forceDestructive) ||
    isDestructiveToolPrompt(prompt.toolName, prompt.arguments, rawDesc);

  return {
    state,
    title: destructive ? "Destructive Action Approval" : "Tool Permission Required",
    subtitle: prompt.toolName,
    description: isRedundantDesc ? undefined : rawDesc,
    command,
    details: detailsList.length > 0 ? detailsList : undefined,
    variant: destructive ? "destructive" : "default",
  };
}

function canonicalizeArgsKey(args: unknown): string {
  const record = parseRecordArgs(args);
  if (record) {
    const sortedKeys = Object.keys(record).sort();
    const ordered: Record<string, unknown> = {};
    for (const k of sortedKeys) {
      ordered[k] = record[k];
    }
    try {
      return JSON.stringify(ordered);
    } catch {
      return "";
    }
  }
  if (typeof args === "string") return args.trim();
  return "";
}

export interface CoordinatedApprovalItem {
  prompt: SecurityPromptInfo;
  state: ApprovalState;
}

export interface CoordinatedToolAndApprovalView {
  visibleToolCalls: ToolExecution[];
  visibleApprovals: CoordinatedApprovalItem[];
}

/**
 * Deterministically pairs `message.securityPrompts` with `message.toolCalls` so:
 * - Pending permissions render `ApprovalCard(state="request")` without a premature `Running` ToolCall row.
 * - Approved permissions currently executing render `ApprovalCard(state="running")` (`Approved, running`).
 * - Once execution completes (`tool_end`), `ToolCall` renders the collapsible Request & Result row
 *   (and `ApprovalCard(state="done")` is optionally retained if `keepApprovedReceipt` is enabled).
 * - Denied permissions render `ApprovalCard(state="denied")` without a duplicate error ToolCall row.
 */
export function pairSecurityPromptsWithToolCalls(
  toolCalls: ReadonlyArray<ToolExecution> | undefined,
  securityPrompts: ReadonlyArray<SecurityPromptInfo> | undefined,
  options?: { keepApprovedReceipt?: boolean },
): CoordinatedToolAndApprovalView {
  const tools = toolCalls ?? [];
  const prompts = securityPrompts ?? [];
  const keepApprovedReceipt = Boolean(options?.keepApprovedReceipt);

  if (prompts.length === 0) {
    return {
      visibleToolCalls: [...tools],
      visibleApprovals: [],
    };
  }

  const pairedToolIndexByPrompt = new Map<number, number>();
  const pairedPromptByToolIndex = new Map<number, SecurityPromptInfo>();

  for (let pIdx = 0; pIdx < prompts.length; pIdx += 1) {
    const prompt = prompts[pIdx];
    const promptArgsKey = canonicalizeArgsKey(prompt.arguments);

    let matchedToolIdx = -1;
    for (let tIdx = 0; tIdx < tools.length; tIdx += 1) {
      if (pairedPromptByToolIndex.has(tIdx)) continue;
      const tool = tools[tIdx];
      if (tool.name !== prompt.toolName) continue;
      if (canonicalizeArgsKey(tool.args) === promptArgsKey) {
        matchedToolIdx = tIdx;
        break;
      }
    }

    if (matchedToolIdx === -1) {
      for (let tIdx = 0; tIdx < tools.length; tIdx += 1) {
        if (pairedPromptByToolIndex.has(tIdx)) continue;
        if (tools[tIdx].name === prompt.toolName) {
          matchedToolIdx = tIdx;
          break;
        }
      }
    }

    if (matchedToolIdx >= 0) {
      pairedToolIndexByPrompt.set(pIdx, matchedToolIdx);
      pairedPromptByToolIndex.set(matchedToolIdx, prompt);
    }
  }

  const visibleToolCalls: ToolExecution[] = [];
  for (let tIdx = 0; tIdx < tools.length; tIdx += 1) {
    const tool = tools[tIdx];
    const pairedPrompt = pairedPromptByToolIndex.get(tIdx);
    if (!pairedPrompt) {
      visibleToolCalls.push(tool);
      continue;
    }
    if (pairedPrompt.status === "pending" || pairedPrompt.status === "denied") {
      continue;
    }
    // pairedPrompt.status === "approved"
    if (tool.status === "running" || tool.status === "awaiting_approval") {
      continue;
    }
    visibleToolCalls.push(tool);
  }

  const visibleApprovals: CoordinatedApprovalItem[] = [];
  for (let pIdx = 0; pIdx < prompts.length; pIdx += 1) {
    const prompt = prompts[pIdx];
    const pairedIdx = pairedToolIndexByPrompt.get(pIdx);
    const pairedTool = pairedIdx !== undefined ? tools[pairedIdx] : undefined;

    if (prompt.status === "pending") {
      visibleApprovals.push({ prompt, state: "request" });
      continue;
    }
    if (prompt.status === "denied") {
      visibleApprovals.push({ prompt, state: "denied" });
      continue;
    }
    // prompt.status === "approved"
    if (!pairedTool) {
      visibleApprovals.push({ prompt, state: "done" });
      continue;
    }
    if (
      pairedTool.status === "running" ||
      pairedTool.status === "awaiting_approval"
    ) {
      visibleApprovals.push({ prompt, state: "running" });
      continue;
    }
    if (keepApprovedReceipt) {
      visibleApprovals.push({ prompt, state: "done" });
    }
  }

  return { visibleToolCalls, visibleApprovals };
}
