import type { JsonObject, ProviderCapability, WebUiCapabilities } from '../types';

export type ProviderDescriptor = {
  key: string;
  display: string;
  available: boolean;
  apiBase?: string;
};

/** Keep provider display formatting out of feature components. */
export function formatProviderDisplayName(key: string): string {
  const normalized = key.trim().toLowerCase();
  if (normalized === 'z_ai' || normalized === 'z.ai') return 'z.ai';
  if (normalized === 'opencode_zen' || normalized === 'opencode-zen') return 'OpenCode Zen';
  if (normalized === 'google_ai_studio' || normalized === 'google-ai-studio') return 'Google AI Studio';

  return normalized
    .replace(/[-_]+/g, ' ')
    .split(' ')
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

function descriptorFromCapability(capability: ProviderCapability): ProviderDescriptor {
  return {
    key: capability.configKey,
    display: capability.display || formatProviderDisplayName(capability.configKey),
    available: capability.available,
    ...(capability.apiBase ? { apiBase: capability.apiBase } : {}),
  };
}

/**
 * Merge authoritative gateway capabilities with locally configured custom
 * providers. Capability order is preserved; custom keys are deterministic.
 */
export function resolveProviderDescriptors(
  capabilities: WebUiCapabilities,
  configured: Record<string, unknown> | JsonObject,
): ProviderDescriptor[] {
  const byKey = new Map<string, ProviderDescriptor>();

  for (const capability of capabilities.providers) {
    const descriptor = descriptorFromCapability(capability);
    byKey.set(descriptor.key, descriptor);
  }

  for (const key of Object.keys(configured).sort()) {
    if (byKey.has(key)) continue;
    byKey.set(key, {
      key,
      display: formatProviderDisplayName(key),
      available: false,
    });
  }

  return Array.from(byKey.values());
}

/** Resolve an explicit provider/model prefix using gateway capability keys. */
export function providerKeyFromModel(
  model: string,
  capabilities: WebUiCapabilities,
): string | undefined {
  const prefix = model.trim().split('/')[0]?.toLowerCase();
  if (!prefix || !model.includes('/')) return undefined;

  return capabilities.providers.find((provider) =>
    provider.configKey.toLowerCase() === prefix || provider.name.toLowerCase() === prefix,
  )?.configKey;
}
