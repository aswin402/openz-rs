import type {
  AgentActivityStep,
  AgentActivityStepStatus,
  AgentSearchResult,
  BeuiActivityContentType,
} from '../../components/beui';
import { formatDuration } from '../../shared/lib/format';
import type {
  CronRunRecord,
  LogEntry,
  OpenZMessage,
  OrchestrationRunState,
  RuntimeInventory,
} from '../../types';

const SEARCH_TOOL_TOKENS = [
  'search',
  'grep',
  'crawl',
  'fetch',
  'find',
  'outline',
  'recall',
  'query',
  'rust_docs',
] as const;

const READ_TOOLS = new Set([
  'read_file',
  'list_dir',
  'list_directory',
  'code_outline',
  'doc_reader',
  'db_inspector',
  'openz_inventory',
  'system_info',
  'check_port',
  'web_fetch',
  'retrieve_original',
]);

const EDIT_TOOLS = new Set([
  'patch_file',
  'replace_lines',
]);

const WRITE_TOOLS = new Set([
  'write_file',
  'db_write',
  'curate_skill',
  'workflow_memory',
  'smart_memory_store',
]);

const DELEGATE_TOOLS = new Set([
  'delegate_task',
  'orchestrate_workflow',
  'parallel_research',
  'evaluator_optimizer_loop',
]);

const LOG_LEVEL_STATUS: Record<LogEntry['level'], AgentActivityStepStatus> = {
  ERROR: 'error',
  WARN: 'pending',
  INFO: 'info',
  DEBUG: 'info',
  TRACE: 'info',
};

const ORCHESTRATION_STEP_STATUS: Record<
  OrchestrationRunState['steps'][number]['status'],
  AgentActivityStepStatus
> = {
  pending: 'pending',
  running: 'running',
  success: 'success',
  failed: 'error',
  skipped: 'info',
  awaiting_review: 'pending',
};

const ORCHESTRATION_RUN_STATUS: Record<
  OrchestrationRunState['status'],
  AgentActivityStepStatus
> = {
  running: 'running',
  success: 'success',
  failed: 'error',
  cancelled: 'info',
  awaiting_review: 'pending',
};

function isDigitOrSemicolon(code: number): boolean {
  return (code >= 48 && code <= 57) || code === 59;
}

/**
 * Strips raw ANSI CSI sequences (`\x1b[1m`, `\x1b[0m`) and orphaned bracketed SGR codes
 * (`[1m`, `[0m`) without using any regular expressions.
 */
export function stripTerminalFormatting(input: string): string {
  let out = '';
  let i = 0;
  const n = input.length;

  while (i < n) {
    const code = input.charCodeAt(i);

    // ESC [ ... m
    if (code === 27 && input.charCodeAt(i + 1) === 91) {
      let j = i + 2;
      while (j < n && isDigitOrSemicolon(input.charCodeAt(j))) {
        j += 1;
      }
      if (j < n && input.charCodeAt(j) === 109) {
        i = j + 1;
        continue;
      }
    }

    // Orphaned `[1m` / `[0m` / `[32m` when ESC was stripped upstream
    if (code === 91 && i + 2 < n && isDigitOrSemicolon(input.charCodeAt(i + 1))) {
      let j = i + 1;
      while (j < n && isDigitOrSemicolon(input.charCodeAt(j))) {
        j += 1;
      }
      if (j > i + 1 && j - i <= 8 && input.charCodeAt(j) === 109) {
        i = j + 1;
        continue;
      }
    }

    out += input[i];
    i += 1;
  }

  return out;
}

function summarizeInline(value: unknown, maxLen = 128): string | undefined {
  if (value === undefined || value === null || value === '') return undefined;
  const raw =
    typeof value === 'string'
      ? stripTerminalFormatting(value).trim()
      : (() => {
          try {
            return JSON.stringify(value);
          } catch {
            return String(value);
          }
        })();
  if (!raw) return undefined;
  return raw.length > maxLen ? `${raw.slice(0, maxLen - 3)}...` : raw;
}

function formatFullPayload(parts: Array<{ label: string; value: unknown }>): string | undefined {
  const blocks: string[] = [];
  for (const part of parts) {
    if (part.value === undefined || part.value === null || part.value === '') continue;
    const text =
      typeof part.value === 'string'
        ? stripTerminalFormatting(part.value).trim()
        : (() => {
            try {
              return JSON.stringify(part.value, null, 2);
            } catch {
              return String(part.value);
            }
          })();
    if (text) {
      blocks.push(`${part.label}:\n${text}`);
    }
  }
  return blocks.length > 0 ? blocks.join('\n\n') : undefined;
}

export function classifyToolContentType(
  toolName: string,
): Exclude<BeuiActivityContentType, 'mixed'> {
  const lower = toolName.toLowerCase();
  const isSearch = SEARCH_TOOL_TOKENS.some((token) => lower.includes(token));
  return isSearch ? 'search' : 'tool';
}

export function classifyBeuiToolAction(toolName: string): string {
  const lower = toolName.toLowerCase();
  if (READ_TOOLS.has(lower) || lower.startsWith('read_') || lower.startsWith('list_') || lower.startsWith('get_')) {
    return 'read';
  }
  if (EDIT_TOOLS.has(lower) || lower.includes('patch') || lower.includes('edit') || lower.includes('replace')) {
    return 'edit';
  }
  if (WRITE_TOOLS.has(lower) || lower.startsWith('write_') || lower.startsWith('create_') || lower.startsWith('save_')) {
    return 'write';
  }
  if (DELEGATE_TOOLS.has(lower)) {
    return 'delegate';
  }
  return 'run';
}

function asArgsRecord(
  args: Record<string, unknown> | string | undefined,
): Record<string, unknown> | null {
  if (typeof args === 'object' && args !== null && !Array.isArray(args)) {
    return args;
  }
  if (typeof args === 'string' && args.trim().startsWith('{')) {
    try {
      const parsed = JSON.parse(args) as unknown;
      if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      return null;
    }
  }
  return null;
}

function firstStringField(
  obj: Record<string, unknown> | null,
  keys: ReadonlyArray<string>,
): string | undefined {
  if (!obj) return undefined;
  for (const key of keys) {
    const val = obj[key];
    if (typeof val === 'string' && val.trim()) {
      return stripTerminalFormatting(val).trim();
    }
  }
  return undefined;
}

export function formatBeuiToolTarget(
  toolName: string,
  args: Record<string, unknown> | string | undefined,
  delegatedActor?: string,
): string {
  const rec = asArgsRecord(args);
  const lower = toolName.toLowerCase();

  if (lower === 'exec_command') {
    const cmd = firstStringField(rec, ['command', 'CommandLine', 'command_line', 'cmd']);
    if (cmd) return summarizeInline(cmd, 96) ?? toolName;
  }

  if (lower === 'git_manager') {
    const action = firstStringField(rec, ['action', 'subcommand', 'command']);
    return action ? `git ${action}` : 'git_manager';
  }

  if (lower === 'cargo_manager' || lower.startsWith('cargo_')) {
    const sub = firstStringField(rec, ['action', 'subcommand', 'package']);
    return sub ? `${toolName} · ${sub}` : toolName;
  }

  if (delegatedActor) {
    const task = firstStringField(rec, ['task', 'goal', 'prompt', 'objective']);
    return task
      ? `${delegatedActor} · ${summarizeInline(task, 72)}`
      : delegatedActor;
  }

  const fileOrTarget = firstStringField(rec, [
    'path',
    'file_path',
    'file',
    'Path',
    'section',
    'name',
    'url',
    'query',
    'pattern',
    'action',
    'topic',
  ]);

  if (fileOrTarget) {
    if (
      lower === 'read_file' ||
      lower === 'write_file' ||
      lower === 'patch_file' ||
      lower === 'replace_lines'
    ) {
      return summarizeInline(fileOrTarget, 96) ?? fileOrTarget;
    }
    return `${toolName} · ${summarizeInline(fileOrTarget, 80) ?? fileOrTarget}`;
  }

  return toolName;
}

function countLines(text: unknown): number | undefined {
  if (typeof text !== 'string' || !text) return undefined;
  return text.split('\n').length;
}

function computeToolDiffCounts(
  action: string,
  args: Record<string, unknown> | string | undefined,
): { additions?: number; deletions?: number } {
  const rec = asArgsRecord(args);
  if (!rec) return {};

  if (action === 'edit') {
    const added = countLines(rec.new_str ?? rec.replacement ?? rec.Replacement ?? rec.new_content);
    const removed = countLines(rec.old_str ?? rec.target ?? rec.Target ?? rec.old_content);
    return { additions: added, deletions: removed };
  }

  if (action === 'write') {
    const added = countLines(rec.content ?? rec.Content);
    return added ? { additions: added } : {};
  }

  return {};
}

function extractUrlDomain(url: string): string | undefined {
  const trimmed = url.trim();
  const schemeIdx = trimmed.indexOf('://');
  if (schemeIdx === -1) return undefined;
  const rest = trimmed.slice(schemeIdx + 3);
  const slashIdx = rest.indexOf('/');
  const host = (slashIdx === -1 ? rest : rest.slice(0, slashIdx)).trim();
  return host.startsWith('www.') ? host.slice(4) : host || undefined;
}

function extractSearchMetadata(
  toolId: string,
  toolName: string,
  args: Record<string, unknown> | string | undefined,
  output: string | undefined,
): {
  searchQuery: string;
  searchResults?: AgentSearchResult[];
  moreCount?: number;
} {
  const rec = asArgsRecord(args);
  const rawQuery =
    firstStringField(rec, ['query', 'pattern', 'url', 'symbol', 'topic', 'crate_name', 'path']) ??
    toolName;
  const searchQuery = summarizeInline(rawQuery, 96) ?? toolName;

  if (!output || !output.trim().startsWith('{') && !output.trim().startsWith('[')) {
    return { searchQuery };
  }

  try {
    const parsed = JSON.parse(output) as unknown;
    const candidates: unknown[] = Array.isArray(parsed)
      ? parsed
      : typeof parsed === 'object' && parsed !== null
        ? ((parsed as Record<string, unknown>).results as unknown[]) ??
          ((parsed as Record<string, unknown>).matches as unknown[]) ??
          ((parsed as Record<string, unknown>).items as unknown[]) ??
          ((parsed as Record<string, unknown>).files as unknown[]) ??
          []
        : [];

    if (!Array.isArray(candidates) || candidates.length === 0) {
      return { searchQuery };
    }

    const mapped: AgentSearchResult[] = [];
    for (let idx = 0; idx < candidates.length && mapped.length < 4; idx += 1) {
      const entry = candidates[idx];
      if (typeof entry === 'string' && entry.trim()) {
        const clean = entry.trim();
        mapped.push({
          id: `${toolId}-sr-${idx}`,
          title: clean,
          domain: extractUrlDomain(clean),
          url: clean.startsWith('http://') || clean.startsWith('https://') ? clean : undefined,
        });
      } else if (typeof entry === 'object' && entry !== null) {
        const obj = entry as Record<string, unknown>;
        const url = typeof obj.url === 'string' ? obj.url : undefined;
        const path = typeof obj.path === 'string' ? obj.path : typeof obj.file === 'string' ? obj.file : undefined;
        const title =
          (typeof obj.title === 'string' && obj.title.trim()) ||
          path ||
          url ||
          undefined;
        if (title) {
          mapped.push({
            id: `${toolId}-sr-${idx}`,
            title,
            domain: (url && extractUrlDomain(url)) || (typeof obj.domain === 'string' ? obj.domain : undefined),
            url,
          });
        }
      }
    }

    if (mapped.length === 0) return { searchQuery };
    const moreCount = candidates.length > mapped.length ? candidates.length - mapped.length : undefined;
    return { searchQuery, searchResults: mapped, moreCount };
  } catch {
    return { searchQuery };
  }
}

function extractDelegatedSubagent(
  toolName: string,
  args: Record<string, unknown> | string | undefined,
): string | undefined {
  const rec = asArgsRecord(args);
  if (rec) {
    const candidate =
      rec.profile ?? rec.subagent ?? rec.agent ?? rec.agent_name ?? rec.target;
    if (typeof candidate === 'string' && candidate.trim()) {
      return candidate.trim();
    }
  }
  return toolName === 'delegate_task' ? 'subagent' : undefined;
}

function parseTimestampSafely(raw?: string | number | null): number | undefined {
  if (typeof raw === 'number' && Number.isFinite(raw)) return raw;
  if (typeof raw === 'string' && raw.trim()) {
    const parsed = Date.parse(raw);
    if (!Number.isNaN(parsed)) return parsed;
  }
  return undefined;
}

export interface BuildMessageStepsOptions {
  limit?: number;
  chronological?: boolean;
}

export function buildMessageActivitySteps(
  messages: ReadonlyArray<OpenZMessage>,
  options: BuildMessageStepsOptions = {},
): AgentActivityStep[] {
  const limit = options.limit ?? 32;
  const items: AgentActivityStep[] = [];

  for (const message of messages) {
    if (message.reasoningContent) {
      const cleanedReasoning = stripTerminalFormatting(message.reasoningContent).trim();
      if (cleanedReasoning) {
        const firstLine = cleanedReasoning
          .split('\n')
          .map((line) => line.trim())
          .find(Boolean);
        const hasToolsStarted = Boolean(message.toolCalls && message.toolCalls.length > 0);
        const hasAnswerStarted = message.content.trim().length > 0;
        const isReasoningActive = Boolean(
          message.isStreaming && !hasToolsStarted && !hasAnswerStarted,
        );

        items.push({
          id: `${message.id}-reasoning`,
          kind: 'reasoning',
          contentType: 'reasoning',
          title: isReasoningActive ? 'Thinking…' : 'Thought process',
          detail: firstLine,
          payload: cleanedReasoning,
          actor: message.model || undefined,
          status: isReasoningActive ? 'running' : 'info',
          timestamp: message.timestamp,
        });
      }
    }

    for (const prompt of message.securityPrompts ?? []) {
      const statusMap: Record<typeof prompt.status, AgentActivityStepStatus> = {
        pending: 'pending',
        denied: 'error',
        approved: 'success',
      };
      items.push({
        id: `${message.id}-security-${prompt.id}`,
        kind: 'security',
        contentType: 'step',
        title: `Approval · ${prompt.toolName}`,
        detail: prompt.description || summarizeInline(prompt.arguments),
        payload: formatFullPayload([
          { label: 'Approval', value: prompt.status },
          { label: 'Description', value: prompt.description },
          { label: 'Arguments', value: prompt.arguments },
        ]),
        meta: prompt.status,
        status: statusMap[prompt.status],
        timestamp: message.timestamp,
      });
    }

    for (const notice of message.activityNotices ?? []) {
      // Skip legacy/raw CLI Progress notices that duplicate structured tool/reasoning events
      if (notice.kind === 'system' && notice.title === 'Progress') {
        continue;
      }
      const cleanTitle = stripTerminalFormatting(notice.title).trim();
      const cleanDetail = notice.detail
        ? stripTerminalFormatting(notice.detail).trim()
        : undefined;
      const isSearchNotice =
        notice.kind === 'research' || notice.kind === 'source';

      items.push({
        id: notice.id,
        kind: isSearchNotice ? 'search' : 'notice',
        contentType: isSearchNotice ? 'search' : 'step',
        noticeKind: notice.kind,
        title: cleanTitle,
        detail: cleanDetail,
        payload: cleanDetail,
        searchQuery: isSearchNotice ? (cleanDetail ?? cleanTitle) : undefined,
        meta: notice.kind,
        status: 'info',
        timestamp: notice.timestamp,
      });
    }

    for (const tool of message.toolCalls ?? []) {
      const contentType = classifyToolContentType(tool.name);
      const delegatedActor = extractDelegatedSubagent(tool.name, tool.args);
      const toolAction = classifyBeuiToolAction(tool.name);
      const toolTarget = formatBeuiToolTarget(tool.name, tool.args, delegatedActor);
      const diffCounts = computeToolDiffCounts(toolAction, tool.args);
      const searchMeta =
        contentType === 'search'
          ? extractSearchMetadata(tool.id, tool.name, tool.args, tool.output)
          : undefined;

      const durationMs =
        tool.durationMs ??
        (tool.startedAt && tool.endedAt && tool.endedAt >= tool.startedAt
          ? tool.endedAt - tool.startedAt
          : undefined);

      items.push({
        id: `${message.id}-tool-${tool.id}`,
        kind: contentType === 'search' ? 'search' : 'tool',
        contentType,
        title: tool.name,
        toolAction,
        toolTarget,
        additions: diffCounts.additions,
        deletions: diffCounts.deletions,
        searchQuery: searchMeta?.searchQuery,
        searchResults: searchMeta?.searchResults,
        moreCount: searchMeta?.moreCount,
        detail:
          summarizeInline(tool.error) ||
          summarizeInline(tool.args) ||
          summarizeInline(tool.output),
        payload: formatFullPayload([
          { label: 'Arguments', value: tool.args },
          { label: 'Output', value: tool.output },
          { label: 'Error', value: tool.error },
        ]),
        actor: delegatedActor,
        status: tool.status === 'awaiting_approval' ? 'pending' : tool.status,
        durationMs,
        durationLabel: formatDuration(durationMs) || undefined,
        timestamp: tool.startedAt || message.timestamp,
      });
    }
  }

  const sorted = items.sort((a, b) =>
    options.chronological
      ? (a.timestamp ?? 0) - (b.timestamp ?? 0)
      : (b.timestamp ?? 0) - (a.timestamp ?? 0),
  );
  return limit > 0 ? sorted.slice(0, limit) : sorted;
}

export interface BuildSubagentActivityOptions {
  agentName?: string | null;
  messages?: ReadonlyArray<OpenZMessage>;
  limit?: number;
}

export function buildOrchestrationActivitySteps(
  runs: ReadonlyArray<OrchestrationRunState>,
  options: BuildSubagentActivityOptions = {},
): AgentActivityStep[] {
  const targetAgent = options.agentName?.trim().toLowerCase() || null;
  const limit = options.limit ?? 28;
  const items: AgentActivityStep[] = [];

  for (const run of runs) {
    const matchingRunSteps = targetAgent
      ? run.steps.filter((s) => s.agent.toLowerCase() === targetAgent)
      : run.steps;

    if (targetAgent && matchingRunSteps.length === 0) {
      continue;
    }

    const runDurationMs =
      run.endedAt && run.endedAt >= run.startedAt
        ? run.endedAt - run.startedAt
        : undefined;

    if (!targetAgent) {
      items.push({
        id: `orch-run-${run.id}`,
        kind: 'notice',
        noticeKind: 'workflow',
        contentType: 'step',
        title: `${run.mode} workflow`,
        detail: run.goal,
        payload: formatFullPayload([
          { label: 'Goal', value: run.goal },
          { label: 'Summary', value: run.summary },
        ]),
        meta: `${run.steps.length} steps`,
        status: ORCHESTRATION_RUN_STATUS[run.status],
        durationMs: runDurationMs,
        durationLabel: formatDuration(runDurationMs) || undefined,
        timestamp: run.startedAt,
      });
    }

    for (const step of matchingRunSteps) {
      const stepDurationMs =
        step.startedAt && step.endedAt && step.endedAt >= step.startedAt
          ? step.endedAt - step.startedAt
          : undefined;
      const summaryText =
        summarizeInline(step.error) || summarizeInline(step.output) || run.goal;
      items.push({
        id: `orch-step-${run.id}-${step.id}`,
        kind: 'tool',
        contentType: 'step',
        toolAction: 'delegate',
        toolTarget: `${step.agent} · ${summaryText}`,
        title: step.id || step.agent,
        detail: summaryText,
        payload: formatFullPayload([
          { label: 'Workflow Goal', value: run.goal },
          { label: 'Agent', value: step.agent },
          { label: 'Output', value: step.output },
          { label: 'Error', value: step.error },
        ]),
        actor: step.agent,
        meta: run.mode,
        status: ORCHESTRATION_STEP_STATUS[step.status],
        durationMs: stepDurationMs,
        durationLabel: formatDuration(stepDurationMs) || undefined,
        timestamp: step.startedAt || run.startedAt,
      });
    }
  }

  for (const message of options.messages ?? []) {
    for (const tool of message.toolCalls ?? []) {
      const delegated = extractDelegatedSubagent(tool.name, tool.args);
      const matchesAgent = targetAgent
        ? tool.name.toLowerCase() === targetAgent ||
          delegated?.toLowerCase() === targetAgent
        : Boolean(delegated || tool.name === 'orchestrate_workflow');

      if (!matchesAgent) continue;

      const actorName = delegated ?? tool.name;
      items.push({
        id: `delegated-${message.id}-${tool.id}`,
        kind: 'tool',
        contentType: 'tool',
        toolAction: 'delegate',
        toolTarget: formatBeuiToolTarget(tool.name, tool.args, actorName),
        title: tool.name,
        detail:
          summarizeInline(tool.error) ||
          summarizeInline(tool.args) ||
          summarizeInline(tool.output),
        payload: formatFullPayload([
          { label: 'Arguments', value: tool.args },
          { label: 'Output', value: tool.output },
          { label: 'Error', value: tool.error },
        ]),
        actor: actorName,
        status: tool.status === 'awaiting_approval' ? 'pending' : tool.status,
        durationMs: tool.durationMs,
        durationLabel: formatDuration(tool.durationMs) || undefined,
        timestamp: tool.startedAt || message.timestamp,
      });
    }
  }

  return items
    .sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0))
    .slice(0, limit);
}

export interface BuildLogStepsOptions {
  levelFilter?: 'ALL' | LogEntry['level'];
  query?: string;
  limit?: number;
}

export function buildLogActivitySteps(
  logs: ReadonlyArray<LogEntry>,
  options: BuildLogStepsOptions = {},
): AgentActivityStep[] {
  const levelFilter = options.levelFilter ?? 'ALL';
  const query = (options.query ?? '').trim().toLowerCase();
  const limit = options.limit ?? 80;

  const filtered = logs.filter((entry) => {
    if (levelFilter !== 'ALL' && entry.level !== levelFilter) return false;
    if (!query) return true;
    return (
      entry.message.toLowerCase().includes(query) ||
      entry.target.toLowerCase().includes(query) ||
      entry.level.toLowerCase().includes(query)
    );
  });

  return filtered.slice(0, limit).map((entry, idx) => {
    const targetParts = entry.target.split('::').filter(Boolean);
    const shortTarget =
      targetParts[targetParts.length - 1] || entry.target || 'openz';
    const parsedTs = parseTimestampSafely(entry.timestamp);
    const cleanMsg = stripTerminalFormatting(entry.message);

    return {
      id: `log-${entry.id}-${idx}`,
      kind: 'trace',
      contentType: 'trace',
      toolAction: 'run',
      title: shortTarget,
      detail: cleanMsg,
      payload: `[${entry.timestamp}] ${entry.level} ${entry.target}\n${cleanMsg}`,
      actor: entry.level,
      meta: entry.timestamp,
      status: LOG_LEVEL_STATUS[entry.level] ?? 'info',
      timestamp: parsedTs,
    };
  });
}

export function buildInventoryActivitySteps(
  inventory: RuntimeInventory | null | undefined,
  cronLogs: ReadonlyArray<CronRunRecord> = [],
  limit = 20,
): AgentActivityStep[] {
  if (!inventory) return [];
  const items: AgentActivityStep[] = [];

  for (const session of inventory.sessions.activeUiSessions) {
    items.push({
      id: `inv-session-${session.sessionKey}-${session.pid}`,
      kind: 'message',
      contentType: 'step',
      title: `Active session · ${session.sessionKey}`,
      detail: session.preview || `${session.provider}/${session.model} (${session.cwd})`,
      payload: formatFullPayload([
        { label: 'Channel', value: session.channel },
        { label: 'Model', value: `${session.provider}/${session.model}` },
        { label: 'Workspace', value: session.cwd },
        { label: 'Preview', value: session.preview },
      ]),
      actor: session.channel,
      meta: `PID ${session.pid}`,
      status: 'running',
      timestamp: parseTimestampSafely(session.lastSeenAt),
    });
  }

  for (const job of inventory.cron.jobs) {
    const jobStatus: AgentActivityStepStatus =
      job.status === 'running'
        ? 'running'
        : job.status === 'failed' || Boolean(job.lastError)
          ? 'error'
          : job.enabled
            ? 'success'
            : 'info';
    items.push({
      id: `inv-cron-job-${job.id}`,
      kind: 'tool',
      contentType: 'step',
      toolAction: 'run',
      toolTarget: `${job.id} · ${job.schedule}`,
      title: `Cron · ${job.id}`,
      detail: job.lastError || job.prompt || `Schedule ${job.schedule}`,
      payload: formatFullPayload([
        { label: 'Schedule', value: job.schedule },
        { label: 'Prompt', value: job.prompt },
        { label: 'Runs', value: `${job.runCount} total / ${job.failureCount} failed` },
        { label: 'Last Error', value: job.lastError },
      ]),
      actor: job.enabled ? 'scheduled' : 'paused',
      meta: job.schedule,
      status: jobStatus,
      timestamp: parseTimestampSafely(job.lastRun || job.updatedAt),
    });
  }

  for (const run of cronLogs) {
    const runStatus: AgentActivityStepStatus =
      run.status === 'failed' || Boolean(run.error)
        ? 'error'
        : run.status === 'running'
          ? 'running'
          : 'success';
    items.push({
      id: `inv-cron-run-${run.run_id}`,
      kind: 'trace',
      contentType: 'trace',
      toolAction: 'run',
      title: `Cron run · ${run.job_id}`,
      detail: run.error || run.summary || `Started ${run.started_at}`,
      payload: formatFullPayload([
        { label: 'Run ID', value: run.run_id },
        { label: 'Started', value: run.started_at },
        { label: 'Finished', value: run.finished_at },
        { label: 'Summary', value: run.summary },
        { label: 'Error', value: run.error },
      ]),
      actor: run.status,
      meta: run.schedule,
      status: runStatus,
      timestamp: parseTimestampSafely(run.started_at),
    });
  }

  for (const sub of inventory.subagents) {
    if (!sub.lastError && sub.failureCount === 0) continue;
    items.push({
      id: `inv-subagent-err-${sub.name}`,
      kind: 'notice',
      noticeKind: 'system',
      contentType: 'step',
      title: `Subagent issue · ${sub.name}`,
      detail: sub.lastError || `${sub.failureCount} recorded failure(s)`,
      actor: sub.effectiveProvider,
      meta: sub.effectiveModel,
      status: 'error',
    });
  }

  return items
    .sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0))
    .slice(0, limit);
}

export interface UnifiedDashboardActivityInput {
  messagesByChat: Record<string, OpenZMessage[]>;
  activeChatId: string;
  orchestrationRunsByChat: Record<string, OrchestrationRunState[]>;
  logs: ReadonlyArray<LogEntry>;
  inventory?: RuntimeInventory | null;
  cronLogs?: ReadonlyArray<CronRunRecord>;
  limit?: number;
}

export function buildUnifiedDashboardActivitySteps(
  input: UnifiedDashboardActivityInput,
): AgentActivityStep[] {
  const limit = input.limit ?? 18;
  const activeMessages = input.messagesByChat[input.activeChatId] ?? [];
  const otherMessages = Object.entries(input.messagesByChat)
    .filter(([chatId]) => chatId !== input.activeChatId)
    .flatMap(([, msgs]) => msgs.slice(-6));

  const chatSteps = buildMessageActivitySteps(
    [...activeMessages, ...otherMessages],
    { limit: 14 },
  );

  const allRuns = Object.values(input.orchestrationRunsByChat).flat();
  const orchSteps = buildOrchestrationActivitySteps(allRuns, { limit: 8 });
  const invSteps = buildInventoryActivitySteps(
    input.inventory,
    input.cronLogs ?? [],
    6,
  );
  const logSteps = buildLogActivitySteps(input.logs, { limit: 8 });

  return [...chatSteps, ...orchSteps, ...invSteps, ...logSteps]
    .sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0))
    .slice(0, limit);
}
