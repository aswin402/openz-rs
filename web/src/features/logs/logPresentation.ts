import type {
  AgentActivityStepStatus,
  BeuiActivityContentType,
} from '../../components/beui';
import { formatDuration } from '../../shared/lib/format';
import type { LogEntry } from '../../types';
import {
  classifyBeuiToolAction,
  classifyToolContentType,
  formatBeuiToolTarget,
  stripTerminalFormatting,
} from '../activity/activityAdapters';

export type LogScopeBucket = 'session' | 'background' | 'other_session' | 'system';

const LOG_LEVEL_STATUS: Record<LogEntry['level'], AgentActivityStepStatus> = {
  ERROR: 'error',
  WARN: 'pending',
  INFO: 'info',
  DEBUG: 'info',
  TRACE: 'info',
};

export function normalizeWsSessionId(raw?: string | null): string {
  const trimmed = (raw ?? '').trim();
  if (!trimmed) return '';
  return trimmed.includes(':') ? trimmed : `ws:${trimmed}`;
}

export function isBackgroundLogEntry(entry: LogEntry): boolean {
  const cat = (entry.category ?? '').toLowerCase();
  const sess = (entry.session ?? '').trim();
  return cat === 'cron' || cat === 'curator' || sess.startsWith('cron:');
}

export function classifyLogScope(
  entry: LogEntry,
  activeChatId?: string | null,
): LogScopeBucket {
  if (isBackgroundLogEntry(entry)) {
    return 'background';
  }

  const entrySession = (entry.session ?? '').trim();
  if (!entrySession) {
    return 'system';
  }

  const normalizedActive = normalizeWsSessionId(activeChatId);
  const rawActive = (activeChatId ?? '').trim();
  if (
    normalizedActive &&
    (entrySession === normalizedActive ||
      (rawActive && entrySession === rawActive) ||
      entrySession.startsWith(`${normalizedActive}:`))
  ) {
    return 'session';
  }

  return 'other_session';
}

function isKeyChar(code: number): boolean {
  return (
    (code >= 97 && code <= 122) || // a-z
    (code >= 65 && code <= 90) || // A-Z
    (code >= 48 && code <= 57) || // 0-9
    code === 95 // _
  );
}

/**
 * Parses `key=value` / `key="escaped"` pairs appended after `"  "` by
 * `EventFieldVisitor::compose_enriched_message()` without regular expressions.
 */
function parseInlineKeyValueSuffix(suffix: string): Record<string, string> | null {
  const out: Record<string, string> = {};
  let i = 0;
  const n = suffix.length;
  let count = 0;

  while (i < n) {
    while (i < n && suffix.charCodeAt(i) === 32) i += 1;
    if (i >= n) break;

    const keyStart = i;
    while (i < n && isKeyChar(suffix.charCodeAt(i))) i += 1;
    if (i === keyStart || i >= n || suffix.charCodeAt(i) !== 61) {
      return null;
    }
    const key = suffix.slice(keyStart, i);
    i += 1; // skip '='

    let value = '';
    if (i < n && suffix.charCodeAt(i) === 34) {
      i += 1; // skip opening '"'
      while (i < n) {
        const ch = suffix[i];
        if (ch === '\\' && i + 1 < n) {
          value += suffix[i + 1];
          i += 2;
          continue;
        }
        if (ch === '"') {
          i += 1;
          break;
        }
        value += ch;
        i += 1;
      }
    } else {
      const valStart = i;
      while (i < n && suffix.charCodeAt(i) !== 32) i += 1;
      value = suffix.slice(valStart, i);
    }

    out[key] = value;
    count += 1;
  }

  return count > 0 ? out : null;
}

export function extractLogMessageAndFields(entry: LogEntry): {
  baseMessage: string;
  fields: Record<string, string>;
} {
  const cleaned = stripTerminalFormatting(entry.message).trim();
  const sepIdx = cleaned.indexOf('  ');
  let baseMessage = cleaned;
  let parsedInline: Record<string, string> | null = null;

  if (sepIdx !== -1) {
    const candidateBase = cleaned.slice(0, sepIdx).trim();
    const candidateSuffix = cleaned.slice(sepIdx + 2).trim();
    const kv = parseInlineKeyValueSuffix(candidateSuffix);
    if (kv) {
      baseMessage = candidateBase;
      parsedInline = kv;
    }
  } else {
    const kvOnly = parseInlineKeyValueSuffix(cleaned);
    if (kvOnly && Object.keys(kvOnly).length > 0 && cleaned.includes('=')) {
      baseMessage = '';
      parsedInline = kvOnly;
    }
  }

  return {
    baseMessage: baseMessage || cleaned,
    fields: {
      ...(parsedInline ?? {}),
      ...(entry.fields ?? {}),
    },
  };
}

export function extractCronJobId(
  entry: LogEntry,
  fields?: Record<string, string>,
): string | undefined {
  const fromField = fields?.job_id?.trim();
  if (fromField) return fromField;
  const sess = (entry.session ?? '').trim();
  if (sess.startsWith('cron:') && sess.length > 5) {
    return sess.slice(5);
  }
  return undefined;
}

export interface PreparedLogItem {
  entry: LogEntry;
  baseMessage: string;
  fields: Record<string, string>;
  cronJobId?: string;
  pairedWithCompletion: boolean;
}

export interface PrepareLogsOptions {
  essentialOnly?: boolean;
}

function isEssentialBoilerplate(item: PreparedLogItem): boolean {
  const { entry, baseMessage, fields, pairedWithCompletion } = item;

  // Always surface warnings, errors, and security audit entries.
  if (
    entry.level === 'ERROR' ||
    entry.level === 'WARN' ||
    entry.category === 'security'
  ) {
    return false;
  }

  if (entry.level === 'DEBUG' || entry.level === 'TRACE') {
    return true;
  }

  if (pairedWithCompletion) {
    return true;
  }

  if (
    baseMessage.startsWith('Restored session history') ||
    baseMessage === 'Session saved successfully. Turn complete.' ||
    baseMessage.startsWith('Sending completion request to LLM') ||
    baseMessage === 'Self-improvement curator: started processing.' ||
    baseMessage === 'Self-improvement curator finished successfully.' ||
    baseMessage === 'Cron scheduler background service started...' ||
    baseMessage === 'cron job log saved' ||
    baseMessage.startsWith('Loaded FastEmbed') ||
    baseMessage.startsWith('WebSocket client connected') ||
    baseMessage.startsWith('WebSocket client disconnected')
  ) {
    return true;
  }

  const isCronSession = (entry.session ?? '').startsWith('cron:');
  if (baseMessage.startsWith('Received LLM response')) {
    if (isCronSession) {
      return true;
    }
    const toolCallsCount = Number(fields.tool_calls ?? '0');
    if (baseMessage.includes('finish_reason: tool_calls') || toolCallsCount > 0) {
      return true;
    }
  }

  return false;
}

/**
 * Enriches tool completions with arguments from their preceding `Executing tool call`
 * log entry and optionally filters out low-signal internal boilerplate.
 */
export function prepareScopedLogs(
  logs: ReadonlyArray<LogEntry>,
  options: PrepareLogsOptions = {},
): { items: PreparedLogItem[]; hiddenCount: number } {
  const essentialOnly = options.essentialOnly ?? false;
  const prepared: PreparedLogItem[] = logs.map((entry) => {
    const { baseMessage, fields } = extractLogMessageAndFields(entry);
    return {
      entry,
      baseMessage,
      fields,
      cronJobId: extractCronJobId(entry, fields),
      pairedWithCompletion: false,
    };
  });

  // Logs are ordered newest-first. Pair newer `Tool call completed` / `Tool call failed`
  // entries with the matching older `Executing tool call` for the same (session, tool).
  const pendingCompletions = new Map<string, PreparedLogItem[]>();
  for (const item of prepared) {
    const toolName = item.fields.tool;
    if (!toolName) continue;
    const key = `${item.entry.session ?? ''}::${toolName}`;

    if (
      item.baseMessage === 'Tool call completed' ||
      item.baseMessage === 'Tool call failed'
    ) {
      const queue = pendingCompletions.get(key) ?? [];
      queue.push(item);
      pendingCompletions.set(key, queue);
    } else if (item.baseMessage === 'Executing tool call') {
      const queue = pendingCompletions.get(key);
      const matched = queue?.shift();
      if (matched) {
        if (item.fields.arguments && !matched.fields.arguments) {
          matched.fields = { ...matched.fields, arguments: item.fields.arguments };
        }
        item.pairedWithCompletion = true;
      }
    }
  }

  if (!essentialOnly) {
    return { items: prepared, hiddenCount: 0 };
  }

  const visible: PreparedLogItem[] = [];
  let hiddenCount = 0;
  for (const item of prepared) {
    if (isEssentialBoilerplate(item)) {
      hiddenCount += 1;
    } else {
      visible.push(item);
    }
  }

  return { items: visible, hiddenCount };
}

export interface PresentedLogEntry {
  title: string;
  detail: string;
  badge?: string;
  status: AgentActivityStepStatus;
  contentType: Exclude<BeuiActivityContentType, 'mixed'>;
  toolAction: string;
  toolTarget?: string;
  payload: string;
  shortTarget: string;
  cronJobId?: string;
  prettyArguments?: string;
}

function formatPrettyArguments(rawArgs?: string): string | undefined {
  if (!rawArgs) return undefined;
  const trimmed = rawArgs.trim();
  if (!trimmed) return undefined;
  if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
    try {
      return JSON.stringify(JSON.parse(trimmed), null, 2);
    } catch {
      return trimmed;
    }
  }
  return trimmed;
}

function extractModelFromRequestMessage(baseMessage: string): string | undefined {
  const marker = '(model: ';
  const idx = baseMessage.indexOf(marker);
  if (idx === -1) return undefined;
  const start = idx + marker.length;
  const end = baseMessage.indexOf(')', start);
  if (end === -1) return undefined;
  return baseMessage.slice(start, end).trim() || undefined;
}

export function presentLogItem(item: PreparedLogItem): PresentedLogEntry {
  const { entry, baseMessage, fields, cronJobId } = item;
  const targetParts = entry.target.split('::').filter(Boolean);
  const shortTarget = targetParts[targetParts.length - 1] || entry.target || 'openz';
  const prettyArguments = formatPrettyArguments(fields.arguments);

  const buildPayload = (extraLines?: string[]): string => {
    const lines = [
      `[${entry.timestamp}] ${entry.level} ${entry.target}`,
      ...(entry.session ? [`Session: ${entry.session}`] : []),
      ...(entry.category ? [`Category: ${entry.category}`] : []),
      baseMessage,
      ...(extraLines ?? []),
    ];
    return lines.filter(Boolean).join('\n');
  };

  // 1. Tool execution lifecycle
  if (
    baseMessage === 'Tool call completed' ||
    baseMessage === 'Executing tool call' ||
    baseMessage === 'Tool call failed'
  ) {
    const toolName = fields.tool || 'tool';
    const targetSummary = formatBeuiToolTarget(toolName, fields.arguments);
    const hasRichTarget = targetSummary !== toolName;
    const isFailed = baseMessage === 'Tool call failed' || entry.level === 'ERROR';
    const isRunning = baseMessage === 'Executing tool call' && !item.pairedWithCompletion;
    const contentType = classifyToolContentType(toolName);
    const toolAction = classifyBeuiToolAction(toolName);

    const status: AgentActivityStepStatus = isFailed
      ? 'error'
      : isRunning
        ? 'running'
        : 'success';
    const badge = isFailed ? 'failed' : isRunning ? 'running' : 'completed';
    const baseTitle = isFailed ? `Tool failed · ${toolName}` : toolName;
    const title = cronJobId ? `${cronJobId} · ${baseTitle}` : baseTitle;

    let detail = hasRichTarget
      ? targetSummary
      : isRunning
        ? 'Running tool…'
        : 'Completed';
    if (isFailed && fields.error) {
      detail = hasRichTarget ? `${targetSummary} — ${fields.error}` : fields.error;
    }

    return {
      title,
      detail,
      badge,
      status,
      contentType,
      toolAction,
      toolTarget: targetSummary,
      payload: buildPayload(
        prettyArguments ? [`Arguments:\n${prettyArguments}`] : undefined,
      ),
      shortTarget,
      cronJobId,
      prettyArguments,
    };
  }

  // 2. LLM completion summary
  if (baseMessage.startsWith('Received LLM response')) {
    const durationMs = Number(fields.duration_ms);
    const durationLabel =
      Number.isFinite(durationMs) && durationMs > 0
        ? formatDuration(durationMs) ?? undefined
        : undefined;
    const toolCallsCount = Number(fields.tool_calls ?? '0');
    const isLengthLimit = baseMessage.includes('finish_reason: length');

    const title = isLengthLimit
      ? 'Context limit reached (continuing)'
      : toolCallsCount > 0
        ? `Planned ${toolCallsCount} tool call${toolCallsCount === 1 ? '' : 's'}`
        : 'Assistant reply completed';
    const detail = durationLabel ? `Responded in ${durationLabel}` : baseMessage;

    return {
      title: cronJobId ? `${cronJobId} · ${title}` : title,
      detail,
      badge: durationLabel,
      status: isLengthLimit ? 'pending' : 'success',
      contentType: 'step',
      toolAction: 'run',
      payload: buildPayload(),
      shortTarget,
      cronJobId,
    };
  }

  // 3. LLM request (shown in Verbose mode)
  if (baseMessage.startsWith('Sending completion request to LLM')) {
    const model = extractModelFromRequestMessage(baseMessage);
    const iter = Number(fields.iteration);
    return {
      title: model ? `LLM request · ${model}` : 'LLM request',
      detail:
        Number.isFinite(iter) && iter >= 0
          ? `Turn step #${iter + 1}`
          : baseMessage,
      status: 'info',
      contentType: 'trace',
      toolAction: 'run',
      payload: buildPayload(),
      shortTarget,
      cronJobId,
    };
  }

  // 4. Scheduled Cron Job lifecycle
  if (
    baseMessage === 'cron job completed successfully' ||
    baseMessage === 'cron job failed' ||
    baseMessage === 'cron job interrupted by graceful shutdown'
  ) {
    const isSuccess = baseMessage === 'cron job completed successfully';
    const isFailed = baseMessage === 'cron job failed' || entry.level === 'ERROR';
    return {
      title: cronJobId ? `Cron · ${cronJobId}` : 'Scheduled cron job',
      detail: isSuccess
        ? 'Scheduled run completed cleanly'
        : fields.error
          ? `Failed: ${fields.error}`
          : baseMessage,
      badge: isSuccess ? 'ok' : isFailed ? 'failed' : 'stopped',
      status: isFailed ? 'error' : isSuccess ? 'success' : 'info',
      contentType: 'step',
      toolAction: 'run',
      payload: buildPayload(),
      shortTarget,
      cronJobId,
    };
  }

  // 5. Self-Improvement Curator learnings
  if (baseMessage.startsWith('Self-improvement curator')) {
    if (baseMessage.includes('updated USER.md')) {
      return {
        title: 'Curator · Updated USER.md',
        detail: 'Saved user profile & workflow preferences',
        badge: 'memory',
        status: 'success',
        contentType: 'step',
        toolAction: 'write',
        payload: buildPayload(),
        shortTarget,
        cronJobId,
      };
    }
    if (baseMessage.includes('updated global MEMORY.md')) {
      return {
        title: 'Curator · Updated MEMORY.md',
        detail: 'Consolidated durable session notes',
        badge: 'memory',
        status: 'success',
        contentType: 'step',
        toolAction: 'write',
        payload: buildPayload(),
        shortTarget,
        cronJobId,
      };
    }
    if (baseMessage.includes('saved skill')) {
      const skillName = fields.skill ? ` (${fields.skill})` : '';
      return {
        title: `Curator · Saved skill${skillName}`,
        detail: 'Updated reusable procedural skill bundle',
        badge: 'skill',
        status: 'success',
        contentType: 'step',
        toolAction: 'write',
        payload: buildPayload(),
        shortTarget,
        cronJobId,
      };
    }
  }

  // 6. Generic / fallback log entry (preserves shortTarget for standard gateway/module logs)
  const fallbackDetail =
    fields.error && !baseMessage.includes(fields.error)
      ? `${baseMessage} — ${fields.error}`
      : baseMessage;

  return {
    title: cronJobId && shortTarget === 'scheduler' ? `Cron · ${cronJobId}` : shortTarget,
    detail: fallbackDetail,
    status: LOG_LEVEL_STATUS[entry.level] ?? 'info',
    contentType: 'trace',
    toolAction: 'run',
    payload: buildPayload(),
    shortTarget,
    cronJobId,
    prettyArguments,
  };
}
