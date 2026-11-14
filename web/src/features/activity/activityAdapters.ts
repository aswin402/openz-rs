import type {
  AgentActivityStep,
  AgentActivityStepStatus,
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

function summarizeInline(value: unknown, maxLen = 128): string | undefined {
  if (value === undefined || value === null || value === '') return undefined;
  const raw =
    typeof value === 'string'
      ? value.trim()
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
        ? part.value.trim()
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

function extractDelegatedSubagent(
  toolName: string,
  args: Record<string, unknown> | string | undefined,
): string | undefined {
  if (typeof args === 'object' && args !== null) {
    const candidate =
      args.profile ?? args.subagent ?? args.agent ?? args.agent_name ?? args.target;
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
      const firstLine = message.reasoningContent
        .split('\n')
        .map((line) => line.trim())
        .find(Boolean);
      items.push({
        id: `${message.id}-reasoning`,
        kind: 'reasoning',
        contentType: 'reasoning',
        title: message.isStreaming ? 'Reasoning stream active' : 'Reasoning captured',
        detail: firstLine,
        payload: message.reasoningContent,
        actor: message.model || undefined,
        status: message.isStreaming ? 'running' : 'info',
        timestamp: message.timestamp,
      });
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
        title: prompt.toolName,
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
      const isSearchNotice =
        notice.kind === 'research' || notice.kind === 'source';
      items.push({
        id: notice.id,
        kind: isSearchNotice ? 'search' : 'notice',
        contentType: isSearchNotice ? 'search' : 'step',
        noticeKind: notice.kind,
        title: notice.title,
        detail: notice.detail,
        payload: notice.detail,
        meta: notice.kind,
        status: 'info',
        timestamp: notice.timestamp,
      });
    }

    for (const tool of message.toolCalls ?? []) {
      const contentType = classifyToolContentType(tool.name);
      const delegatedActor = extractDelegatedSubagent(tool.name, tool.args);
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
      items.push({
        id: `orch-step-${run.id}-${step.id}`,
        kind: 'tool',
        contentType: 'step',
        title: step.id || step.agent,
        detail: summarizeInline(step.error) || summarizeInline(step.output) || run.goal,
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

      items.push({
        id: `delegated-${message.id}-${tool.id}`,
        kind: 'tool',
        contentType: 'tool',
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
        actor: delegated ?? tool.name,
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

    return {
      id: `log-${entry.id}-${idx}`,
      kind: 'trace',
      contentType: 'trace',
      title: shortTarget,
      detail: entry.message,
      payload: `[${entry.timestamp}] ${entry.level} ${entry.target}\n${entry.message}`,
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
