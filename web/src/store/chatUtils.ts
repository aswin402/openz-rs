import type {
  OpenZMessage,
  OpenZSession,
  OrchestrationRunState,
  OrchestrationStepState,
} from '../types';
import { UI_STORAGE_KEYS } from '../config/runtime';

const DRAFT_SESSION_TITLE = 'New Session';
let msgCounter = 0;

export function newMsgId(prefix: string): string {
  return `${prefix}-${Date.now()}-${msgCounter++}`;
}

export function savedActiveChatId(): string {
  try {
    return normalizeChatId(sessionStorage.getItem(UI_STORAGE_KEYS.activeChatId) || '');
  } catch {
    return '';
  }
}

export function rememberActiveChatId(chatId: string) {
  const normalizedChatId = normalizeChatId(chatId);
  if (!normalizedChatId) return;
  try {
    sessionStorage.setItem(UI_STORAGE_KEYS.activeChatId, normalizedChatId);
  } catch {
    // Ignore storage failures; the active session still works for this page lifetime.
  }
}

export function forgetActiveChatId() {
  try {
    sessionStorage.removeItem(UI_STORAGE_KEYS.activeChatId);
  } catch {
    // Ignore storage failures.
  }
}

export function normalizeChatId(chatId: string): string {
  if (!chatId) return chatId;
  if (chatId.includes(':')) return chatId;

  const channelPrefixes = ['ws_', 'cli_', 'telegram_', 'subagent_'];
  const matchedPrefix = channelPrefixes.find((prefix) => chatId.startsWith(prefix));
  if (matchedPrefix) {
    return `${matchedPrefix.slice(0, -1)}:${chatId.slice(matchedPrefix.length)}`;
  }

  return `ws:${chatId}`;
}

function createDraftSession(chatId: string): OpenZSession {
  const now = Date.now();
  return {
    id: normalizeChatId(chatId),
    title: DRAFT_SESSION_TITLE,
    createdAt: now,
    lastMessageAt: now,
    messageCount: 0,
    isDraft: true,
  };
}

export function upsertDraftSession(sessions: OpenZSession[], chatId: string): OpenZSession[] {
  const normalizedChatId = normalizeChatId(chatId);
  if (!normalizedChatId) return sessions;
  if (sessions.some((session) => session.id === normalizedChatId)) return sessions;
  return [createDraftSession(normalizedChatId), ...sessions.filter((session) => !session.isDraft)];
}

export function titleFromFirstMessage(content: string): string {
  const compact = content.trim().replace(/\s+/g, ' ');
  if (!compact) return DRAFT_SESSION_TITLE;
  return compact.length > 42 ? compact.slice(0, 39) + '...' : compact;
}

export type OrchestrationPayload = {
  type?: unknown;
  run_id?: unknown;
  goal?: unknown;
  mode?: unknown;
  step_id?: unknown;
  agent?: unknown;
  status?: unknown;
  output?: unknown;
  summary?: unknown;
};

function asString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function normalizeWorkflowStatus(value: unknown): OrchestrationRunState['status'] {
  const raw = asString(value);
  if (raw === 'success' || raw === 'failed' || raw === 'cancelled' || raw === 'awaiting_review') return raw;
  return 'running';
}

function normalizeStepStatus(value: unknown): OrchestrationStepState['status'] {
  const raw = asString(value);
  if (raw === 'success' || raw === 'failed' || raw === 'skipped' || raw === 'awaiting_review') return raw;
  return 'running';
}

function isTerminalWorkflowStatus(status: OrchestrationRunState['status']): boolean {
  return status === 'success' || status === 'failed' || status === 'cancelled';
}

function upsertOrchestrationRun(
  runs: OrchestrationRunState[],
  run: OrchestrationRunState,
): OrchestrationRunState[] {
  const index = runs.findIndex((existing) => existing.id === run.id);
  if (index < 0) return [...runs, run];
  return runs.map((existing, i) => {
    if (i !== index) return existing;
    if (existing.status === 'cancelled' && run.status !== 'cancelled') return existing;
    if (isTerminalWorkflowStatus(existing.status) && existing.status !== run.status && !existing.provisionalFailure) return existing;
    return { ...existing, ...run, provisionalFailure: run.provisionalFailure ?? false };
  });
}

function upsertOrchestrationStep(
  steps: OrchestrationStepState[],
  step: OrchestrationStepState,
): OrchestrationStepState[] {
  const index = steps.findIndex((existing) => existing.id === step.id);
  if (index < 0) return [...steps, step];
  return steps.map((existing, i) => (i === index ? { ...existing, ...step } : existing));
}

export function applyOrchestrationEvent(
  runs: OrchestrationRunState[],
  payload: OrchestrationPayload,
  now = Date.now(),
): OrchestrationRunState[] {
  const type = asString(payload.type);
  const runId = asString(payload.run_id);
  if (!type || !runId) return runs;

  if (type === 'run_started') {
    const existingRun = runs.find((existing) => existing.id === runId);
    if (existingRun && isTerminalWorkflowStatus(existingRun.status)) return runs;
    return upsertOrchestrationRun(runs, {
      id: runId,
      goal: asString(payload.goal) || '',
      mode: asString(payload.mode) || 'sequential',
      status: 'running',
      steps: [],
      startedAt: existingRun?.startedAt || now,
    });
  }

  const run = runs.find((existing) => existing.id === runId) || {
    id: runId,
    goal: '',
    mode: 'sequential',
    status: 'running' as const,
    steps: [],
    startedAt: now,
  };

  if (run.status === 'cancelled') return runs;
  if (type !== 'run_finished' && isTerminalWorkflowStatus(run.status)) return runs;

  if (type === 'step_started') {
    const stepId = asString(payload.step_id);
    if (!stepId) return runs;
    return upsertOrchestrationRun(runs, {
      ...run,
      status: run.status === 'running' ? run.status : 'running',
      steps: upsertOrchestrationStep(run.steps, {
        id: stepId,
        agent: asString(payload.agent) || '',
        status: 'running',
        startedAt: now,
      }),
    });
  }

  if (type === 'step_finished') {
    const stepId = asString(payload.step_id);
    if (!stepId) return runs;
    const status = normalizeStepStatus(payload.status);
    return upsertOrchestrationRun(runs, {
      ...run,
      steps: upsertOrchestrationStep(run.steps, {
        id: stepId,
        agent: asString(payload.agent) || run.steps.find((step) => step.id === stepId)?.agent || '',
        status,
        output: asString(payload.output),
        error: status === 'failed' ? asString(payload.output) : undefined,
        endedAt: now,
      }),
    });
  }

  if (type === 'run_finished') {
    return upsertOrchestrationRun(runs, {
      ...run,
      status: normalizeWorkflowStatus(payload.status),
      summary: asString(payload.summary),
      endedAt: now,
      provisionalFailure: false,
    });
  }

  return runs;
}

export function settleOrchestrationRuns(
  runs: OrchestrationRunState[],
  status: OrchestrationRunState['status'],
  summary: string,
  now = Date.now(),
  overrideTerminal = false,
  runIds?: readonly string[],
): OrchestrationRunState[] {
  const targetRunIds = runIds ? new Set(runIds) : null;
  let changed = false;
  const settled = runs.map((run) => {
    if (targetRunIds && !targetRunIds.has(run.id)) return run;
    const canSettle =
      run.status === 'running' ||
      run.status === 'awaiting_review' ||
      (overrideTerminal && run.status !== status);
    if (!canSettle) return run;
    changed = true;
    const settledStepStatus: OrchestrationStepState['status'] =
      status === 'cancelled' ? 'skipped' : 'failed';
    return {
      ...run,
      status,
      summary: status === 'cancelled' ? summary : run.summary || summary,
      endedAt: now,
      provisionalFailure: status === 'failed' && !overrideTerminal,
      steps: run.steps.map((step): OrchestrationStepState =>
        step.status === 'running' || step.status === 'awaiting_review' || overrideTerminal
          ? {
              ...step,
              status: settledStepStatus,
              error: step.error || summary,
              endedAt: step.endedAt || now,
            }
          : step,
      ),
    };
  });
  return changed ? settled : runs;
}

export function mergeAssistantFinalIntoToolTurn(messages: OpenZMessage[], nextMessage: OpenZMessage): boolean {
  if (nextMessage.role !== 'assistant') return false;
  if (nextMessage.toolCalls && nextMessage.toolCalls.length > 0) return false;
  if (nextMessage.activityNotices && nextMessage.activityNotices.length > 0) return false;

  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const candidate = messages[i];
    if (candidate.role === 'user') return false;
    if (candidate.role !== 'assistant') continue;
    if (!candidate.toolCalls || candidate.toolCalls.length === 0) return false;
    if (candidate.content.trim().length > 0) return false;

    messages[i] = {
      ...candidate,
      content: nextMessage.content,
      timestamp: nextMessage.timestamp || candidate.timestamp,
      model: nextMessage.model || candidate.model,
      reasoningContent: nextMessage.reasoningContent || candidate.reasoningContent,
    };
    return true;
  }

  return false;
}

export function settleAssistantTurnMessages(
  messages: OpenZMessage[],
  reason: string,
  now = Date.now(),
): OpenZMessage[] {
  let changed = false;

  const settled = messages.map((message) => {
    if (message.role !== 'assistant') return message;

    const toolCalls = message.toolCalls?.map((tool) => {
      if (tool.status !== 'running') return tool;

      changed = true;
      return {
        ...tool,
        status: 'error' as const,
        output: tool.output || reason,
        error: tool.error || reason,
        endedAt: tool.endedAt || now,
        durationMs: tool.durationMs ?? (tool.startedAt ? now - tool.startedAt : undefined),
      };
    });

    if (message.isStreaming) changed = true;

    return {
      ...message,
      isStreaming: false,
      toolCalls,
    };
  });

  return changed ? settled : messages;
}
