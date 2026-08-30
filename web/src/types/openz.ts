export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[];
export type JsonObject = { [key: string]: JsonValue };

export interface ActivityNotice {
  id: string;
  kind: 'workflow' | 'memory' | 'research' | 'self_improvement' | 'source' | 'system';
  title: string;
  detail?: string;
  timestamp: number;
}

export interface OpenZMessage {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  timestamp: number;
  /** Real time tool executions attached to this message. */
  toolCalls?: ToolExecution[];
  /** Security-approval prompts attached to this message. */
  securityPrompts?: SecurityPromptInfo[];
  /** Structured activity notices such as workflow matches, memory saves, and research context hits. */
  activityNotices?: ActivityNotice[];
  /** Streaming chain-of-thought text (collapsible "Thinking" block). */
  reasoningContent?: string;
  isStreaming?: boolean;
  model?: string;
  /** True when this is a muted system/error notice. */
  isNotice?: boolean;
  attachments?: ChatAttachment[];
}

export type WorkspaceNoticeScope = 'skills' | 'agents' | 'settings' | 'knowledge' | 'inventory' | 'global';

export interface WorkspaceNotice {
  scope: WorkspaceNoticeScope;
  type: 'success' | 'error' | 'info';
  message: string;
  timestamp: number;
}

export interface ChatAttachment {
  id: string;
  name: string;
  mime: string;
  size: number;
  data?: string;
  previewUrl?: string;
}

export interface ToolExecution {
  /** The tool_call_id from the backend. */
  id: string;
  name: string;
  args?: Record<string, unknown> | string;
  status: 'running' | 'success' | 'error' | 'awaiting_approval';
  output?: string;
  error?: string;
  durationMs?: number;
  startedAt?: number;
  endedAt?: number;
}

export interface SecurityPromptInfo {
  /** The backend req_id used to resolve the approval round-trip. */
  id: string;
  toolName: string;
  description: string;
  arguments?: Record<string, unknown> | string;
  status: 'pending' | 'approved' | 'denied';
}

export interface OrchestrationStepState {
  id: string;
  agent: string;
  status: 'pending' | 'running' | 'success' | 'failed' | 'skipped' | 'awaiting_review';
  output?: string;
  error?: string;
  startedAt?: number;
  endedAt?: number;
}

export interface OrchestrationRunState {
  id: string;
  goal: string;
  mode: string;
  status: 'running' | 'success' | 'failed' | 'cancelled' | 'awaiting_review';
  steps: OrchestrationStepState[];
  startedAt: number;
  endedAt?: number;
  summary?: string;
  provisionalFailure?: boolean;
}

export interface OpenZSession {
  id: string;
  title: string;
  createdAt: number;
  lastMessageAt: number;
  messageCount: number;
  isDraft?: boolean;
}

export interface CognitiveNode {
  name: string;
  entity_type: string;
  observations: string;
}

export interface CognitiveEdge {
  from_name: string;
  to_name: string;
  relation_type: string;
  confidence?: number;
  valid_from?: string;
  provenance?: string;
  source?: string;
}

export interface CognitiveFact {
  text: string;
  timestamp: string;
  tags: string;
  importance: number;
}

export interface CognitiveMemoryStats {
  entitiesCount: number;
  relationsCount: number;
  factsCount: number;
  workingMemoryKeys: string[];
  nodes?: CognitiveNode[];
  edges?: CognitiveEdge[];
  facts?: CognitiveFact[];
}

export interface McpServerInfo {
  name: string;
  command: string;
  status: 'connected' | 'error' | 'disabled' | 'starting';
  enabled?: boolean;
  args?: string[];
  toolsCount: number;
}

export interface McpStats {
  loaded: number;
  failed: number;
  total: number;
}

export interface LogEntry {
  id: string;
  timestamp: string;
  level: 'TRACE' | 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
  target: string;
  message: string;
}

/** A provider + its list of models, from the backend `models_list` event. */
export interface ModelRef {
  provider: string;
  model: string;
}

export interface ProviderModelOption {
  name: string;
  display: string;
  models: string[];
  available?: boolean;
  full?: boolean;
}

export interface ProviderCapability {
  name: string;
  configKey: string;
  display: string;
  available: boolean;
  apiBaseEditable: boolean;
}

export type CapabilityFieldKind = "boolean" | "secret" | "number" | "text";

export interface ChannelFieldCapability {
  key: string;
  label: string;
  kind: CapabilityFieldKind;
}

export interface ChannelCapability {
  name: string;
  label: string;
  fields: ChannelFieldCapability[];
  defaults: Record<string, JsonValue>;
}

export interface AttachmentCapabilities {
  maxCount: number;
  maxFileBytes: number;
  maxTotalBytes: number;
  maxMessageBytes: number;
  ttlSeconds: number;
  allowedMimeTypes: string[];
}

export interface WebUiCapabilities {
  version: number;
  providers: ProviderCapability[];
  securityModes: Array<{ value: string; label: string }>;
  channels: ChannelCapability[];
  attachments: AttachmentCapabilities;
}

function capabilityRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function capabilityNumber(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0;
}

function capabilityString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}

const CAPABILITY_FIELD_KINDS: CapabilityFieldKind[] = ['boolean', 'secret', 'number', 'text'];

export function normalizeWebUiCapabilities(value: unknown): WebUiCapabilities {
  const source = capabilityRecord(value);
  const providers = Array.isArray(source?.providers)
    ? source.providers.flatMap((value) => {
        const row = capabilityRecord(value);
        const name = capabilityString(row?.name);
        const configKey = capabilityString(row?.configKey);
        const display = capabilityString(row?.display);
        if (!name || !configKey || !display) return [];
        return [{
          name,
          configKey,
          display,
          available: row?.available === true,
          apiBaseEditable: row?.apiBaseEditable === true,
        }];
      })
    : [];
  const securityModes = Array.isArray(source?.securityModes)
    ? source.securityModes.flatMap((value) => {
        const row = capabilityRecord(value);
        const mode = capabilityString(row?.value);
        const label = capabilityString(row?.label);
        return mode && label ? [{ value: mode, label }] : [];
      })
    : [];
  const channels = Array.isArray(source?.channels)
    ? source.channels.flatMap((value) => {
        const row = capabilityRecord(value);
        const name = capabilityString(row?.name);
        const label = capabilityString(row?.label);
        if (!name || !label) return [];
        const fields = Array.isArray(row?.fields)
          ? row.fields.flatMap((fieldValue) => {
              const field = capabilityRecord(fieldValue);
              const key = capabilityString(field?.key);
              const fieldLabel = capabilityString(field?.label);
              const kind = capabilityString(field?.kind);
              return key && fieldLabel && kind && CAPABILITY_FIELD_KINDS.includes(kind as CapabilityFieldKind)
                ? [{ key, label: fieldLabel, kind: kind as CapabilityFieldKind }]
                : [];
            })
          : [];
        const defaults: Record<string, JsonValue> = {};
        const rawDefaults = capabilityRecord(row?.defaults);
        if (rawDefaults) {
          Object.entries(rawDefaults).forEach(([key, defaultValue]) => {
            if (
              defaultValue === null ||
              typeof defaultValue === 'string' ||
              typeof defaultValue === 'number' ||
              typeof defaultValue === 'boolean'
            ) {
              defaults[key] = defaultValue;
            }
          });
        }
        return [{ name, label, fields, defaults }];
      })
    : [];
  const attachmentSource = capabilityRecord(source?.attachments);
  const allowedMimeTypes = Array.isArray(attachmentSource?.allowedMimeTypes)
    ? attachmentSource.allowedMimeTypes.flatMap((mime) => {
        const normalized = capabilityString(mime);
        return normalized ? [normalized] : [];
      })
    : [];

  return {
    version: capabilityNumber(source?.version),
    providers,
    securityModes,
    channels,
    attachments: {
      maxCount: capabilityNumber(attachmentSource?.maxCount),
      maxFileBytes: capabilityNumber(attachmentSource?.maxFileBytes),
      maxTotalBytes: capabilityNumber(attachmentSource?.maxTotalBytes),
      maxMessageBytes: capabilityNumber(attachmentSource?.maxMessageBytes),
      ttlSeconds: capabilityNumber(attachmentSource?.ttlSeconds),
      allowedMimeTypes,
    },
  };
}

/** Runtime agent defaults editable over the `set_config` WS command. */
export interface AgentDefaultsConfig {
  model: string;
  provider: string;
  temperature: number;
  max_tokens: number;
  streaming: boolean;
  caveman_mode: boolean;
  security_mode: string;
  workspace: string;
  bot_name: string;
  max_messages: number;
  max_tool_iterations: number;
  tool_timeout_secs: number;
  enable_sandbox: boolean;
  context_limit?: number | null;
  tool_output_limit?: number | null;
  show_auto_capture_notices?: boolean;
  tui_thought_display?: string;
}

/** Full config response from the `get_config` WS command. */
export interface ConfigData {
  defaults: AgentDefaultsConfig;
  skills: SkillInfo[];
  mcp_servers: McpServerInfo[];
  capabilities: WebUiCapabilities;
  version: string;
}

export interface OpenZConfigPatch {
  defaults?: Partial<AgentDefaultsConfig>;
  providers?: JsonObject;
  channels?: JsonObject;
}

/** A single slash command from the backend `SLASH_COMMANDS`. */
export interface SlashCommand {
  cmd: string;
  desc: string;
}

/** Agent/gateway status from the backend `status` event. */
export interface AgentStatus {
  version: string;
  mcp: McpStats;
}

export interface CronRunRecord {
  run_id: string;
  job_id: string;
  schedule: string;
  started_at: string;
  finished_at?: string | null;
  status: string;
  log_path?: string | null;
  summary?: string | null;
  error?: string | null;
}

export interface RuntimeInventory {
  version: string;
  paths: {
    configDir: string;
    workspace: string;
    memoryDb: string;
    graphDb: string;
    subagentsFile: string;
    skillsDir: string;
    workspaceSkillsDir: string;
  };
  defaults: {
    model: string;
    provider: string;
    streaming: boolean;
    cavemanMode: boolean;
    maxMessages: number;
    maxToolIterations: number;
    toolTimeoutSecs: number;
  };
  counts: {
    subagents: number;
    skills: number;
    coreSubagents: number;
    customSubagents: number;
    channels: number;
    enabledChannels: number;
    tools: number;
    cronJobs: number;
    activeCronJobs: number;
  };
  channels: Array<{ name: string; enabled: boolean; configured: boolean }>;
  subagents: Array<{
    name: string;
    description: string;
    model: string;
    provider: string;
    fallbackCount: number;
    isCore: boolean;
    isProtected: boolean;
    source: string;
  }>;
  skills: SkillInfo[];
  memory: {
    memoryDb: { path: string; exists: boolean };
    graphDb: { path: string; exists: boolean };
  };
  tools: Array<{
    name: string;
    domain: string;
    risk: string;
    usesNetwork: boolean;
    writesDisk: boolean;
    spawnsProcess: boolean;
    requiresApproval: boolean;
    priority: number;
    description: string;
  }>;
  cron: {
    jobsFile: string;
    runsFile: string;
    recentRuns: number;
    jobs: Array<{
      id: string;
      schedule: string;
      enabled: boolean;
      runOnce: boolean;
      status: string;
      quiet: boolean;
      notifyOn: string;
      nextRun?: string | null;
      lastRun?: string | null;
      lastStartedAt?: string | null;
      lastFinishedAt?: string | null;
      lastError?: string | null;
      lastLogPath?: string | null;
      runCount: number;
      failureCount: number;
    }>;
  };
}

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'unauthorized'
  | 'error';

export interface BackgroundServerInfo {
  id: string;
  pid: number;
  kind: string;
  command: string;
}

export interface SkillInfo {
  name: string;
  content: string;
  scope?: string;
  profile?: string | null;
  source?: string;
  path?: string | null;
  enabled?: boolean;
  isProtected?: boolean;
  useCount?: number;
  createdAt?: string | null;
  lastUsed?: string | null;
  validationErrors?: string[];
}

export interface SubagentInfo {
  name: string;
  description: string;
  systemPrompt: string;
  model: string;
  provider: string;
  fallbacks?: string[];
  isCore?: boolean;
  isProtected?: boolean;
  source?: 'core' | 'user' | string;
  fallbackLimit?: number;
}

export interface ChannelConfigInfo {
  name: string;
  enabled: boolean;
  status: string;
  token_configured: boolean;
}