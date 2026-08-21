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