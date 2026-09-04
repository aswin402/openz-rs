import type { JsonObject } from '../../types';

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
