import type { CronRunRecord, RuntimeInventory } from './openz';

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasString(record: UnknownRecord, key: string): boolean {
  return typeof record[key] === 'string';
}

function hasNumber(record: UnknownRecord, key: string): boolean {
  const value = record[key];
  return typeof value === 'number' && Number.isFinite(value);
}

function hasBoolean(record: UnknownRecord, key: string): boolean {
  return typeof record[key] === 'boolean';
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string');
}

function isNullableString(record: UnknownRecord, key: string): boolean {
  return !(key in record) || record[key] === null || typeof record[key] === 'string';
}

function isRuntimePaths(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return [
    'configDir',
    'workspace',
    'memoryDb',
    'graphDb',
    'subagentsFile',
    'skillsDir',
    'workspaceSkillsDir',
    'sessionsDir',
  ].every((key) => hasString(value, key));
}

function isRuntimeDefaults(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return (
    hasString(value, 'model') &&
    hasString(value, 'provider') &&
    hasBoolean(value, 'streaming') &&
    hasBoolean(value, 'cavemanMode') &&
    hasNumber(value, 'maxMessages') &&
    hasNumber(value, 'maxToolIterations') &&
    hasNumber(value, 'toolTimeoutSecs')
  );
}

function isRuntimeCounts(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return [
    'subagents',
    'skills',
    'coreSubagents',
    'customSubagents',
    'channels',
    'enabledChannels',
    'tools',
    'cronJobs',
    'activeCronJobs',
    'runningCronJobs',
    'sessions',
    'activeUiSessions',
  ].every((key) => hasNumber(value, key));
}

function isChannelInventory(value: unknown): boolean {
  return isRecord(value) && hasString(value, 'name') && hasBoolean(value, 'enabled') && hasBoolean(value, 'configured');
}

function isActiveUiSession(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return [
    'sessionKey',
    'channel',
    'cwd',
    'startedAt',
    'lastSeenAt',
    'model',
    'provider',
    'preview',
  ].every((key) => hasString(value, key)) && hasNumber(value, 'pid');
}

function isRecentSession(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return (
    ['key', 'channel', 'title', 'updatedAt', 'filePath'].every((key) => hasString(value, key)) &&
    hasNumber(value, 'messageCount') &&
    hasBoolean(value, 'active')
  );
}

function isSessionChannelCount(value: unknown): boolean {
  return isRecord(value) && hasString(value, 'channel') && hasNumber(value, 'count');
}

function isSessionInventory(value: unknown): boolean {
  if (!isRecord(value) || !hasString(value, 'sessionsDir')) return false;
  const controlCenter = value.webuiControlCenter;
  if (
    !isRecord(controlCenter) ||
    !hasNumber(controlCenter, 'connectedClients') ||
    !isStringArray(controlCenter.attachedChats)
  ) {
    return false;
  }
  return (
    Array.isArray(value.activeUiSessions) && value.activeUiSessions.every(isActiveUiSession) &&
    Array.isArray(value.recentSessions) && value.recentSessions.every(isRecentSession) &&
    Array.isArray(value.channelCounts) && value.channelCounts.every(isSessionChannelCount)
  );
}

function isSubagentInventory(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return (
    ['name', 'description', 'model', 'provider', 'effectiveModel', 'effectiveProvider', 'source']
      .every((key) => hasString(value, key)) &&
    isStringArray(value.fallbacks) &&
    isStringArray(value.capabilities) &&
    hasNumber(value, 'fallbackCount') &&
    hasBoolean(value, 'supportsVision') &&
    hasBoolean(value, 'isCore') &&
    hasBoolean(value, 'isProtected') &&
    hasNumber(value, 'failureCount') &&
    isNullableString(value, 'lastSuccessfulModel') &&
    isNullableString(value, 'lastError')
  );
}

function isDatabaseInventory(value: unknown): boolean {
  return isRecord(value) && hasString(value, 'path') && hasBoolean(value, 'exists');
}

function isMemoryInventory(value: unknown): boolean {
  return isRecord(value) && isDatabaseInventory(value.memoryDb) && isDatabaseInventory(value.graphDb);
}

function isToolInventory(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return (
    ['name', 'domain', 'risk', 'description'].every((key) => hasString(value, key)) &&
    ['usesNetwork', 'writesDisk', 'spawnsProcess', 'requiresApproval'].every((key) => hasBoolean(value, key)) &&
    hasNumber(value, 'priority')
  );
}

function isCronJobInventory(value: unknown): boolean {
  if (!isRecord(value)) return false;
  return (
    ['id', 'schedule', 'prompt', 'status', 'notifyOn'].every((key) => hasString(value, key)) &&
    ['enabled', 'runOnce', 'quiet'].every((key) => hasBoolean(value, key)) &&
    ['runCount', 'failureCount'].every((key) => hasNumber(value, key)) &&
    ['nextRun', 'lastRun', 'lastStartedAt', 'lastFinishedAt', 'lastError', 'lastLogPath', 'createdAt', 'updatedAt']
      .every((key) => isNullableString(value, key))
  );
}

function isCronInventory(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasString(value, 'jobsFile') &&
    hasString(value, 'runsFile') &&
    hasNumber(value, 'recentRuns') &&
    Array.isArray(value.jobs) &&
    value.jobs.every(isCronJobInventory)
  );
}

function isSkillInfo(value: unknown): boolean {
  return isRecord(value) && hasString(value, 'name') && hasString(value, 'content');
}

function isRuntimeInventory(value: unknown): value is RuntimeInventory {
  if (!isRecord(value)) return false;
  return (
    hasString(value, 'version') &&
    isRuntimePaths(value.paths) &&
    isRuntimeDefaults(value.defaults) &&
    isRuntimeCounts(value.counts) &&
    Array.isArray(value.channels) && value.channels.every(isChannelInventory) &&
    isSessionInventory(value.sessions) &&
    Array.isArray(value.subagents) && value.subagents.every(isSubagentInventory) &&
    Array.isArray(value.skills) && value.skills.every(isSkillInfo) &&
    isMemoryInventory(value.memory) &&
    Array.isArray(value.tools) && value.tools.every(isToolInventory) &&
    isCronInventory(value.cron)
  );
}

function isCronRunRecord(value: unknown): value is CronRunRecord {
  if (!isRecord(value)) return false;
  return (
    ['run_id', 'job_id', 'schedule', 'started_at', 'status'].every((key) => hasString(value, key)) &&
    ['finished_at', 'log_path', 'summary', 'error'].every((key) => isNullableString(value, key))
  );
}

export function normalizeRuntimeInventory(value: unknown): RuntimeInventory | null {
  return isRuntimeInventory(value) ? value : null;
}

export function normalizeCronRuns(value: unknown): CronRunRecord[] {
  return Array.isArray(value) ? value.filter(isCronRunRecord) : [];
}

type EventPayload = { event?: string; [key: string]: unknown };

export type WebSocketEventMap = {
  ready: EventPayload & { chat_id?: string; client_id?: string };
  inventory: EventPayload & { inventory?: unknown };
  runtime_inventory: EventPayload & { inventory?: unknown };
  cron_runs: EventPayload & { runs?: unknown };
  cron_logs: EventPayload & { runs?: unknown };
  cron_jobs_updated: EventPayload & { id?: string; status?: string; inventory?: unknown };
  capabilities: EventPayload & { capabilities?: unknown };
  error: EventPayload & { chat_id?: string; detail?: string; message?: string };
};
