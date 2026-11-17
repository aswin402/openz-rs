import { asString, isRecord } from '../shared/lib/format';
import type {
  CavemanLevelSlug,
  CavemanStatsSnapshot,
} from '../shared/lib/cavemanLevels';

export type { CavemanLevelSlug, CavemanStatsSnapshot };

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[];
export type JsonObject = { [key: string]: JsonValue };

export interface ActivityNotice {
  id: string;
  kind:
    | "workflow"
    | "skill"
    | "memory"
    | "research"
    | "self_improvement"
    | "source"
    | "system";
  title: string;
  detail?: string;
  timestamp: number;
}

export interface OpenZMessage {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
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

export type WorkspaceNoticeScope =
  "skills" | "agents" | "settings" | "knowledge" | "inventory" | "global";

export interface WorkspaceNotice {
  scope: WorkspaceNoticeScope;
  type: "success" | "error" | "info";
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
  status: "running" | "success" | "error" | "awaiting_approval";
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
  status: "pending" | "approved" | "denied";
}

export interface OrchestrationStepState {
  id: string;
  agent: string;
  status:
    | "pending"
    | "running"
    | "success"
    | "failed"
    | "skipped"
    | "awaiting_review";
  output?: string;
  error?: string;
  startedAt?: number;
  endedAt?: number;
}

export interface OrchestrationRunState {
  id: string;
  goal: string;
  mode: string;
  status: "running" | "success" | "failed" | "cancelled" | "awaiting_review";
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
  paths?: {
    memoryDb: string;
    graphDb: string;
  };
}

export interface McpServerInfo {
  name: string;
  command: string;
  status: "connected" | "error" | "disabled" | "starting";
  enabled?: boolean;
  args?: string[];
  toolsCount: number;
}

export interface McpStats {
  loaded: number;
  failed: number;
  total: number;
}

export type LogCategoryName =
  | "session"
  | "tool"
  | "cron"
  | "subagent"
  | "curator"
  | "channel"
  | "security"
  | "system";

export interface LogEntry {
  id: string;
  timestamp: string;
  level: "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR";
  category?: LogCategoryName | string;
  target: string;
  message: string;
  session?: string;
  fields?: Record<string, string>;
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
  apiBase?: string;
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

export interface BrowserCapabilities {
  firefoxWebdriverPort: number;
  firefoxAttachPort: number;
}

export interface WebUiCapabilities {
  version: number;
  providers: ProviderCapability[];
  securityModes: Array<{ value: string; label: string }>;
  toolRoutingModes: Array<{ value: string; label: string }>;
  channels: ChannelCapability[];
  browser: BrowserCapabilities;
  attachments: AttachmentCapabilities;
}

function capabilityRecord(value: unknown): Record<string, unknown> | null {
  return isRecord(value) ? value : null;
}

function capabilityNumber(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0;
}

function capabilityString(value: unknown): string | null {
  const str = asString(value)?.trim();
  return str || null;
}

const CAPABILITY_FIELD_KINDS: CapabilityFieldKind[] = ['boolean', 'secret', 'number', 'text'];

export function normalizeWebUiCapabilities(value: unknown): WebUiCapabilities {
  const source = capabilityRecord(value);
  const browserSource = capabilityRecord(source?.browser);
  const providers = Array.isArray(source?.providers)
    ? source.providers.flatMap((value) => {
        const row = capabilityRecord(value);
        const name = capabilityString(row?.name);
        const configKey = capabilityString(row?.configKey);
        const display = capabilityString(row?.display);
        const apiBase = capabilityString(row?.apiBase);
        if (!name || !configKey || !display) return [];
        return [{
          name,
          configKey,
          display,
          available: row?.available === true,
          apiBaseEditable: row?.apiBaseEditable === true,
          ...(apiBase ? { apiBase } : {}),
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
  const toolRoutingModes = Array.isArray(source?.toolRoutingModes)
    ? source.toolRoutingModes.flatMap((value) => {
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
    toolRoutingModes,
    channels,
    browser: {
      firefoxWebdriverPort: capabilityNumber(browserSource?.firefoxWebdriverPort),
      firefoxAttachPort: capabilityNumber(browserSource?.firefoxAttachPort),
    },
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
  theme?: 'light' | 'dark' | 'system' | string;
  theme_explicit?: 'light' | 'dark' | 'system' | string | null;
  active_theme_id?: string | null;
  ui_customization?: JsonObject | null;
  ui_surface?: string;
  temperature: number;
  max_tokens: number;
  streaming: boolean;
  caveman_mode: boolean;
  caveman_level?: CavemanLevelSlug | string;
  caveman_stats?: CavemanStatsSnapshot | null;
  security_mode: string;
  tool_routing_mode?: string;
  workspace: string;
  bot_name: string;
  max_messages: number;
  max_tool_iterations: number;
  tool_timeout_secs: number;
  enable_sandbox: boolean;
  context_limit?: number | null;
  resolved_context_window?: number | null;
  resolved_max_output_tokens?: number | null;
  tool_output_limit?: number | null;
  show_auto_capture_notices?: boolean;
  tui_thought_display?: string;
  firefox_webdriver_port: number;
  firefox_attach_port: number;
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
    sessionsDir: string;
  };
  defaults: {
    model: string;
    provider: string;
    streaming: boolean;
    cavemanMode: boolean;
    cavemanLevel?: string;
    cavemanStats?: CavemanStatsSnapshot | null;
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
    runningCronJobs: number;
    sessions: number;
    activeUiSessions: number;
  };
  channels: Array<{ name: string; enabled: boolean; configured: boolean }>;
  sessions: {
    sessionsDir: string;
    webuiControlCenter: { connectedClients: number; attachedChats: string[] };
    activeUiSessions: Array<{
      sessionKey: string;
      channel: string;
      pid: number;
      cwd: string;
      startedAt: string;
      lastSeenAt: string;
      model: string;
      provider: string;
      preview: string;
    }>;
    recentSessions: Array<{
      key: string;
      channel: string;
      title: string;
      updatedAt: string;
      messageCount: number;
      filePath: string;
      active: boolean;
    }>;
    channelCounts: Array<{ channel: string; count: number }>;
  };
  subagents: Array<{
    name: string;
    description: string;
    model: string;
    provider: string;
    fallbacks: string[];
    fallbackCount: number;
    effectiveModel: string;
    effectiveProvider: string;
    capabilities: string[];
    supportsVision: boolean;
    lastSuccessfulModel?: string | null;
    lastError?: string | null;
    failureCount: number;
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
      prompt: string;
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
      createdAt?: string | null;
      updatedAt?: string | null;
    }>;
  };
}

export type ConnectionStatus =
  "disconnected" | "connecting" | "connected" | "unauthorized" | "error";

export interface BackgroundServerInfo {
  id: string;
  pid: number;
  kind: string;
  command: string;
}

export interface SkillInfo {
  name: string;
  content: string;
  description?: string;
  triggers?: string[];
  scope?: string;
  profile?: string | null;
  source?: string;
  path?: string | null;
  enabled?: boolean;
  isProtected?: boolean;
  useCount?: number;
  successCount?: number;
  revisionCount?: number;
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
  source?: "core" | "user" | string;
  fallbackLimit?: number;
}

export interface ChannelConfigInfo {
  name: string;
  enabled: boolean;
  status: string;
  token_configured: boolean;
}
