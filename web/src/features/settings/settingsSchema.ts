import type { AgentDefaultsConfig, JsonObject, WebUiCapabilities } from '../../types';
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
  { key: 'temperature', kind: 'number' },
  { key: 'max_tokens', kind: 'number' },
  { key: 'bot_name', kind: 'string' },
  { key: 'max_messages', kind: 'number' },
  { key: 'max_tool_iterations', kind: 'number' },
  { key: 'tool_timeout_secs', kind: 'number' },
  { key: 'security_mode', kind: 'string' },
  { key: 'workspace', kind: 'string' },
  { key: 'context_limit', kind: 'nullable_number', fallback: '' },
  { key: 'tool_output_limit', kind: 'nullable_number', fallback: '' },
  { key: 'show_auto_capture_notices', kind: 'boolean', fallback: true },
  { key: 'tui_thought_display', kind: 'string', fallback: 'auto' },
  { key: 'firefox_webdriver_port', kind: 'number' },
  { key: 'firefox_attach_port', kind: 'number' },
];

export function settingsToForm(settings: AgentDefaultsConfig): SettingsForm {
  const form: SettingsForm = {};
  for (const field of AGENT_SETTING_FIELDS) {
    const raw = settings[field.key];
    form[field.key] = (raw ?? field.fallback ?? '') as string | number | boolean;
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
  if (field.kind === 'number') {
    return Number(formValue) !== currentValue;
  }
  if (field.kind === 'nullable_number') {
    return String(formValue ?? '') !== String(currentValue ?? '');
  }
  if (field.kind === 'boolean') {
    return Boolean(formValue ?? field.fallback) !== Boolean(currentValue ?? field.fallback);
  }
  return formValue !== (currentValue ?? field.fallback);
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
  const patch: Record<string, unknown> = {};
  if (!settings) return patch;
  for (const field of AGENT_SETTING_FIELDS) {
    const raw = form[field.key];
    if (raw === undefined || !isFieldChanged(field, form, settings)) continue;
    if (field.kind === 'number') {
      patch[field.key] = Number(raw);
    } else if (field.kind === 'nullable_number') {
      patch[field.key] = raw === '' ? null : Number(raw);
    } else if (field.kind === 'boolean') {
      patch[field.key] = Boolean(raw);
    } else {
      patch[field.key] = raw;
    }
  }
  return patch;
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
