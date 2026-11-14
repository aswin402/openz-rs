import type { AgentDefaultsConfig, JsonObject, WebUiCapabilities } from '../../types';
import { normalizeCavemanLevel } from '../../shared/lib/cavemanLevels';
import { isWebSocketUrl } from '../../shared/lib/format';

export type SettingsForm = Record<string, string | number | boolean>;

export type ProviderForm = {
  api_key?: string;
  api_base?: string;
  default_model?: string;
};

export type ProvidersForm = Record<string, ProviderForm>;

export type ChannelFieldValue = string | number | boolean;
export type ChannelsForm = Record<string, Record<string, ChannelFieldValue>>;

export interface SelectOption {
  value: string;
  label: string;
}

export type SecurityPermissionMode = 'low' | 'medium' | 'high';
export type ToolRoutingMode = 'minimal' | 'adaptive' | 'balanced' | 'expanded';

const SECURITY_MODE_ALIASES: Readonly<Record<string, SecurityPermissionMode>> = {
  low: 'low',
  loose: 'low',
  medium: 'medium',
  normal: 'medium',
  high: 'high',
  strict: 'high',
};

export function normalizeSecurityMode(raw: unknown): SecurityPermissionMode {
  const normalized = typeof raw === 'string' ? raw.trim().toLowerCase() : '';
  return SECURITY_MODE_ALIASES[normalized] ?? 'low';
}

const TOOL_ROUTING_MODE_ALIASES: Readonly<Record<string, ToolRoutingMode>> = {
  minimal: 'minimal',
  lean: 'minimal',
  pi: 'minimal',
  compact: 'minimal',
  adaptive: 'adaptive',
  auto: 'adaptive',
  smart: 'adaptive',
  balanced: 'balanced',
  standard: 'balanced',
  default: 'balanced',
  expanded: 'expanded',
  wide: 'expanded',
  max: 'expanded',
  full: 'expanded',
};

export function normalizeToolRoutingMode(raw: unknown): ToolRoutingMode {
  const normalized = typeof raw === 'string' ? raw.trim().toLowerCase() : '';
  return TOOL_ROUTING_MODE_ALIASES[normalized] ?? 'adaptive';
}

export const DEFAULT_SECURITY_MODE_OPTIONS: SelectOption[] = [
  { value: 'low', label: 'Low — High-Risk Only (Default)' },
  { value: 'medium', label: 'Medium — High + Moderate Risk' },
  { value: 'high', label: 'High — Strict (Ask on Edits & Writes)' },
];

export const DEFAULT_TOOL_ROUTING_MODE_OPTIONS: SelectOption[] = [
  { value: 'adaptive', label: 'Adaptive — Auto Model-Aware (Default)' },
  { value: 'minimal', label: 'Minimal — Pi-Style Lean Core (~8 tools)' },
  { value: 'balanced', label: 'Balanced — Intent Packs (~20 tools)' },
  { value: 'expanded', label: 'Expanded — Wide Working Set (~36 tools)' },
];

export interface PermissionTierDescriptor {
  value: SecurityPermissionMode;
  title: string;
  badge: string;
  summary: string;
  examples: string;
}

export interface ToolRoutingModeDescriptor {
  value: ToolRoutingMode;
  title: string;
  badge: string;
  summary: string;
  budget: string;
}

export const TOOL_ROUTING_MODE_DESCRIPTORS: readonly ToolRoutingModeDescriptor[] = [
  {
    value: 'adaptive',
    title: 'Adaptive',
    badge: 'Default • Model-Aware',
    summary:
      'Automatically selects Minimal (~8 tools) on compact/lite/free models and Balanced (~20 tools) on frontier models, preserving KV prompt cache prefixes across steps.',
    budget: '8 tools (compact) • 20 tools (frontier) • <= 128 cap',
  },
  {
    value: 'minimal',
    title: 'Minimal',
    badge: 'Pi-Style Lean Core',
    summary:
      'Starts every turn with ~8 foundational coding primitives plus surgical tool_catalog JIT hot-mounting and PTC batch pipelines.',
    budget: '~8 initial tools • expands up to 20 on demand',
  },
  {
    value: 'balanced',
    title: 'Balanced',
    badge: 'Intent Packs',
    summary:
      'Exposes ~20 intent-classified domain tools per turn with BM25 lexical discovery and session working-set continuity.',
    budget: '~20 initial tools • expands up to 48 on demand',
  },
  {
    value: 'expanded',
    title: 'Expanded',
    badge: 'Wide Working Set',
    summary:
      'Mounts ~36 tools upfront across multiple domains for heavy cross-domain workflows while enforcing the 128-tool provider limit.',
    budget: '~36 initial tools • expands up to 48 (<= 128 cap)',
  },
];

export const PERMISSION_TIER_DESCRIPTORS: readonly PermissionTierDescriptor[] = [
  {
    value: 'low',
    title: 'Low',
    badge: 'Default • Freedom + Secure',
    summary:
      'Runs workspace file edits, builds, searches, and local commits freely; asks permission only for genuinely high-risk actions.',
    examples: 'rm / rm -rf, sudo / chmod, shutdown / reboot, | sh, db_write, secrets, external paths',
  },
  {
    value: 'medium',
    title: 'Medium',
    badge: 'Balanced',
    summary:
      'Low rules plus approval before pushing externally, downloading via shell, installing packages, or deleting state.',
    examples: 'Low + git push, curl / wget / scp, pip / cargo install, kill, cargo clean, remote PRs',
  },
  {
    value: 'high',
    title: 'High',
    badge: 'Strict',
    summary:
      'Medium rules plus approval before every workspace file write, patch, media/document write, and local Git mutation.',
    examples: 'Medium + write_file, patch_file, replace_lines, git commit, opendoc / openmedia writes',
  },
];

export interface SelectGroup {
  label: string;
  options: SelectOption[];
}

type AgentSettingFieldKind = 'string' | 'number' | 'nullable_number' | 'boolean';

interface AgentSettingFieldDescriptor {
  key: keyof AgentDefaultsConfig;
  kind: AgentSettingFieldKind;
  fallback?: string | number | boolean;
}

const AGENT_SETTING_FIELDS: readonly AgentSettingFieldDescriptor[] = [
  { key: 'model', kind: 'string' },
  { key: 'provider', kind: 'string' },
  { key: 'theme', kind: 'string', fallback: 'dark' },
  { key: 'temperature', kind: 'number' },
  { key: 'max_tokens', kind: 'number' },
  { key: 'bot_name', kind: 'string' },
  { key: 'max_messages', kind: 'number' },
  { key: 'max_tool_iterations', kind: 'number' },
  { key: 'tool_timeout_secs', kind: 'number' },
  { key: 'security_mode', kind: 'string', fallback: 'low' },
  { key: 'tool_routing_mode', kind: 'string', fallback: 'adaptive' },
  { key: 'caveman_mode', kind: 'boolean', fallback: true },
  { key: 'caveman_level', kind: 'string', fallback: 'full' },
  { key: 'workspace', kind: 'string' },
  { key: 'context_limit', kind: 'nullable_number', fallback: '' },
  { key: 'tool_output_limit', kind: 'nullable_number', fallback: '' },
  { key: 'show_auto_capture_notices', kind: 'boolean', fallback: true },
  { key: 'tui_thought_display', kind: 'string', fallback: 'auto' },
  { key: 'firefox_webdriver_port', kind: 'number' },
  { key: 'firefox_attach_port', kind: 'number' },
];

const SPECIAL_STRING_NORMALIZERS: Readonly<
  Partial<Record<keyof AgentDefaultsConfig, (raw: unknown) => string>>
> = {
  security_mode: normalizeSecurityMode,
  tool_routing_mode: normalizeToolRoutingMode,
  caveman_level: normalizeCavemanLevel,
};

const FIELD_COERCERS: Readonly<
  Record<AgentSettingFieldKind, (raw: string | number | boolean) => unknown>
> = {
  string: (raw) => String(raw),
  number: (raw) => Number(raw),
  nullable_number: (raw) => (raw === '' ? null : Number(raw)),
  boolean: (raw) => Boolean(raw),
};

export function settingsToForm(settings: AgentDefaultsConfig): SettingsForm {
  const form: SettingsForm = {};
  for (const field of AGENT_SETTING_FIELDS) {
    const raw = settings[field.key];
    const normalizer = SPECIAL_STRING_NORMALIZERS[field.key];
    form[field.key] = normalizer
      ? normalizer(raw ?? field.fallback)
      : ((raw ?? field.fallback ?? '') as string | number | boolean);
  }
  return form;
}

function isFieldChanged(
  field: AgentSettingFieldDescriptor,
  form: SettingsForm,
  settings: AgentDefaultsConfig,
): boolean {
  const formValue = form[field.key];
  const currentValue = settings[field.key];
  const normalizer = SPECIAL_STRING_NORMALIZERS[field.key];
  const comparators: Readonly<Record<AgentSettingFieldKind, () => boolean>> = {
    number: () => Number(formValue) !== currentValue,
    nullable_number: () => String(formValue ?? '') !== String(currentValue ?? ''),
    boolean: () => Boolean(formValue ?? field.fallback) !== Boolean(currentValue ?? field.fallback),
    string: () =>
      normalizer
        ? normalizer(formValue) !== normalizer(currentValue ?? field.fallback)
        : formValue !== (currentValue ?? field.fallback),
  };
  return comparators[field.kind]();
}

export function hasDefaultsChanged(
  form: SettingsForm,
  settings: AgentDefaultsConfig | null,
): boolean {
  return settings
    ? AGENT_SETTING_FIELDS.some((field) => isFieldChanged(field, form, settings))
    : false;
}

export function buildDefaultsPatch(
  form: SettingsForm,
  settings: AgentDefaultsConfig | null,
): Record<string, unknown> {
  return settings
    ? AGENT_SETTING_FIELDS.reduce<Record<string, unknown>>((patch, field) => {
        const raw = form[field.key];
        const changed = raw !== undefined && isFieldChanged(field, form, settings);
        const normalizer = SPECIAL_STRING_NORMALIZERS[field.key];
        const nextValue = normalizer ? normalizer(raw) : FIELD_COERCERS[field.kind](raw);
        return changed ? { ...patch, [field.key]: nextValue } : patch;
      }, {})
    : {};
}

export function validateSettingsForm(
  urlInput: string,
  form: SettingsForm,
  settings: AgentDefaultsConfig | null,
  capabilities: WebUiCapabilities,
  channelsForm: ChannelsForm,
): string | null {
  const firefoxWebdriverPort = Number(
    form.firefox_webdriver_port ??
      settings?.firefox_webdriver_port ??
      capabilities.browser.firefoxWebdriverPort,
  );
  const firefoxAttachPort = Number(
    form.firefox_attach_port ??
      settings?.firefox_attach_port ??
      capabilities.browser.firefoxAttachPort,
  );

  const checks: ReadonlyArray<{ invalid: boolean; message: string }> = [
    { invalid: !urlInput.trim(), message: 'Gateway websocket URL is required.' },
    { invalid: !isWebSocketUrl(urlInput), message: 'Gateway URL must start with ws:// or wss://.' },
    {
      invalid: Number(form.temperature ?? 0) < 0 || Number(form.temperature ?? 0) > 2,
      message: 'Temperature must be between 0 and 2.',
    },
    { invalid: Number(form.max_tokens ?? 1) < 1, message: 'Max tokens must be at least 1.' },
    { invalid: Number(form.max_messages ?? 1) < 1, message: 'Max messages must be at least 1.' },
    {
      invalid: firefoxWebdriverPort < 1024 || firefoxWebdriverPort > 65535,
      message: 'Firefox WebDriver port must be 1024-65535.',
    },
    {
      invalid: firefoxAttachPort < 1024 || firefoxAttachPort > 65535,
      message: 'Firefox attach port must be 1024-65535.',
    },
    {
      invalid: Number(form.max_tool_iterations ?? 1) < 1,
      message: 'Max tool iterations must be at least 1.',
    },
    {
      invalid: Number(form.tool_timeout_secs ?? 1) < 1,
      message: 'Tool timeout must be at least 1 second.',
    },
    {
      invalid:
        form.context_limit !== '' &&
        form.context_limit !== undefined &&
        Number(form.context_limit) < 1,
      message: 'Context limit must be blank or at least 1.',
    },
    {
      invalid:
        form.tool_output_limit !== '' &&
        form.tool_output_limit !== undefined &&
        Number(form.tool_output_limit) < 1,
      message: 'Tool output limit must be blank or at least 1.',
    },
  ];

  const failedCheck = checks.find((check) => check.invalid);
  if (failedCheck) return failedCheck.message;

  for (const channel of capabilities.channels) {
    const values = channelsForm[channel.name] || {};
    for (const field of channel.fields) {
      if (field.kind !== 'number') continue;
      const raw = values[field.key] ?? channel.defaults[field.key];
      if (raw === undefined || raw === '') continue;
      const number = Number(raw);
      if (!Number.isFinite(number) || number < 1 || number > 65535) {
        return `${channel.label} ${field.label} must be 1-65535.`;
      }
    }
  }
  return null;
}

function cloneObject<T>(value: T): T {
  return JSON.parse(JSON.stringify(value || {})) as T;
}

export function stableStringify(value: unknown): string {
  return JSON.stringify(value ?? {});
}

export function jsonObjectToProviders(value: JsonObject): ProvidersForm {
  return cloneObject(value) as ProvidersForm;
}

export function jsonObjectToChannels(value: JsonObject): ChannelsForm {
  return cloneObject(value) as ChannelsForm;
}
