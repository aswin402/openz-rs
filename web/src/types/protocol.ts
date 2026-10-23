import type { CronRunRecord, RuntimeInventory } from './openz';
import { isRecord } from '../shared/lib/format';

type UnknownRecord = Record<string, unknown>;

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

interface RecordShapeSpec {
  strings?: readonly string[];
  numbers?: readonly string[];
  booleans?: readonly string[];
  nullableStrings?: readonly string[];
  stringArrays?: readonly string[];
}

function hasRecordShape(value: unknown, spec: RecordShapeSpec): value is UnknownRecord {
  if (!isRecord(value)) return false;
  return (
    (!spec.strings || spec.strings.every((key) => hasString(value, key))) &&
    (!spec.numbers || spec.numbers.every((key) => hasNumber(value, key))) &&
    (!spec.booleans || spec.booleans.every((key) => hasBoolean(value, key))) &&
    (!spec.nullableStrings || spec.nullableStrings.every((key) => isNullableString(value, key))) &&
    (!spec.stringArrays || spec.stringArrays.every((key) => isStringArray(value[key])))
  );
}

function isRuntimePaths(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: [
      'configDir',
      'workspace',
      'memoryDb',
      'graphDb',
      'subagentsFile',
      'skillsDir',
      'workspaceSkillsDir',
      'sessionsDir',
    ],
  });
}

function isRuntimeDefaults(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: ['model', 'provider'],
    booleans: ['streaming', 'cavemanMode'],
    numbers: ['maxMessages', 'maxToolIterations', 'toolTimeoutSecs'],
  });
}

function isRuntimeCounts(value: unknown): boolean {
  return hasRecordShape(value, {
    numbers: [
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
    ],
  });
}

function isChannelInventory(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: ['name'],
    booleans: ['enabled', 'configured'],
  });
}

function isActiveUiSession(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: [
      'sessionKey',
      'channel',
      'cwd',
      'startedAt',
      'lastSeenAt',
      'model',
      'provider',
      'preview',
    ],
    numbers: ['pid'],
  });
}

function isRecentSession(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: ['key', 'channel', 'title', 'updatedAt', 'filePath'],
    numbers: ['messageCount'],
    booleans: ['active'],
  });
}

function isSessionChannelCount(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: ['channel'],
    numbers: ['count'],
  });
}

function isSessionInventory(value: unknown): boolean {
  if (!hasRecordShape(value, { strings: ['sessionsDir'] })) return false;
  if (
    !hasRecordShape(value.webuiControlCenter, {
      numbers: ['connectedClients'],
      stringArrays: ['attachedChats'],
    })
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
  return hasRecordShape(value, {
    strings: ['name', 'description', 'model', 'provider', 'effectiveModel', 'effectiveProvider', 'source'],
    stringArrays: ['fallbacks', 'capabilities'],
    numbers: ['fallbackCount', 'failureCount'],
    booleans: ['supportsVision', 'isCore', 'isProtected'],
    nullableStrings: ['lastSuccessfulModel', 'lastError'],
  });
}

function isDatabaseInventory(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: ['path'],
    booleans: ['exists'],
  });
}

function isMemoryInventory(value: unknown): boolean {
  return isRecord(value) && isDatabaseInventory(value.memoryDb) && isDatabaseInventory(value.graphDb);
}

function isToolInventory(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: ['name', 'domain', 'risk', 'description'],
    booleans: ['usesNetwork', 'writesDisk', 'spawnsProcess', 'requiresApproval'],
    numbers: ['priority'],
  });
}

function isCronJobInventory(value: unknown): boolean {
  return hasRecordShape(value, {
    strings: ['id', 'schedule', 'prompt', 'status', 'notifyOn'],
    booleans: ['enabled', 'runOnce', 'quiet'],
    numbers: ['runCount', 'failureCount'],
    nullableStrings: ['nextRun', 'lastRun', 'lastStartedAt', 'lastFinishedAt', 'lastError', 'lastLogPath', 'createdAt', 'updatedAt'],
  });
}

function isCronInventory(value: unknown): boolean {
  return (
    hasRecordShape(value, {
      strings: ['jobsFile', 'runsFile'],
      numbers: ['recentRuns'],
    }) &&
    Array.isArray(value.jobs) &&
    value.jobs.every(isCronJobInventory)
  );
}

function isSkillInfo(value: unknown): boolean {
  return hasRecordShape(value, { strings: ['name', 'content'] });
}

function isRuntimeInventory(value: unknown): value is RuntimeInventory {
  if (!hasRecordShape(value, { strings: ['version'] })) return false;
  return (
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
  return hasRecordShape(value, {
    strings: ['run_id', 'job_id', 'schedule', 'started_at', 'status'],
    nullableStrings: ['finished_at', 'log_path', 'summary', 'error'],
  });
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
