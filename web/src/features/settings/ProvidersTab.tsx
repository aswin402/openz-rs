import React, { useMemo, useState, type Dispatch, type SetStateAction } from 'react';
import {
  CheckCircle2,
  Eye,
  EyeOff,
  KeyRound,
  Plus,
  Search,
  Server,
  Trash2,
  X,
} from 'lucide-react';
import type { ProviderCapability } from '../../types';
import type { ProviderDescriptor } from '../../config/providers';
import { sanitizeProviderKey } from '../../shared/lib/format';
import type { ProviderForm, ProvidersForm } from './settingsSchema';
import { cn } from '../../lib/utils';

export interface ProvidersTabProps {
  providerKeys: string[];
  providersForm: ProvidersForm;
  providerDescriptors: ProviderDescriptor[];
  providerCapabilityByKey: ReadonlyMap<string, ProviderCapability>;
  onProvidersChange: Dispatch<SetStateAction<ProvidersForm>>;
  onDirty: () => void;
  newProviderKey: string;
  newProviderKeyError: string;
  showAddCustom: boolean;
  onNewProviderKeyChange: (value: string) => void;
  onNewProviderKeyError: (value: string) => void;
  onShowAddCustomChange: (value: boolean) => void;
}

type ProviderFilterMode = 'all' | 'configured' | 'custom';

export const ProvidersTab: React.FC<ProvidersTabProps> = ({
  providerKeys,
  providersForm,
  providerDescriptors,
  providerCapabilityByKey,
  onProvidersChange,
  onDirty,
  newProviderKey,
  newProviderKeyError,
  showAddCustom,
  onNewProviderKeyChange,
  onNewProviderKeyError,
  onShowAddCustomChange,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterMode, setFilterMode] = useState<ProviderFilterMode>('all');
  const [revealedKeys, setRevealedKeys] = useState<Record<string, boolean>>({});

  const toggleReveal = (key: string) => {
    setRevealedKeys((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  const configuredCount = useMemo(
    () =>
      providerKeys.filter((k) => {
        const keyVal = providersForm[k]?.api_key?.trim();
        return Boolean(keyVal);
      }).length,
    [providerKeys, providersForm],
  );

  const customCount = useMemo(
    () => providerKeys.filter((k) => !providerCapabilityByKey.has(k)).length,
    [providerCapabilityByKey, providerKeys],
  );

  const filteredProviderKeys = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    return providerKeys.filter((providerKey) => {
      const data = providersForm[providerKey] || {};
      const isConfigured = Boolean(data.api_key?.trim());
      const isCustom = !providerCapabilityByKey.has(providerKey);

      if (filterMode === 'configured' && !isConfigured) return false;
      if (filterMode === 'custom' && !isCustom) return false;

      if (!q) return true;
      const descriptor = providerDescriptors.find((p) => p.key === providerKey);
      const haystack = `${providerKey} ${descriptor?.display || ''} ${data.default_model || ''} ${data.api_base || ''}`.toLowerCase();
      return haystack.includes(q);
    });
  }, [filterMode, providerCapabilityByKey, providerDescriptors, providerKeys, providersForm, searchQuery]);

  return (
    <div className="space-y-5">
      {/* Toolbar: Search, Filter Pills & Add Custom Button */}
      <div className="flex flex-col gap-3 rounded-2xl border border-border/70 bg-card/60 p-4 shadow-xs sm:flex-row sm:items-center sm:justify-between">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Filter providers by name, endpoint, or default model..."
            className="w-full rounded-xl border border-border/70 bg-background/70 py-2 pl-9 pr-8 text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery('')}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 rounded p-0.5 text-muted-foreground hover:text-foreground"
              aria-label="Clear search"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center rounded-xl border border-border/60 bg-background/60 p-1 text-[11px]">
            {(
              [
                { id: 'all', label: `All (${providerKeys.length})` },
                { id: 'configured', label: `Configured (${configuredCount})` },
                { id: 'custom', label: `Custom (${customCount})` },
              ] as const
            ).map((pill) => (
              <button
                key={pill.id}
                type="button"
                onClick={() => setFilterMode(pill.id)}
                className={cn(
                  'rounded-lg px-2.5 py-1 font-medium transition-colors',
                  filterMode === pill.id
                    ? 'bg-primary text-primary-foreground shadow-xs'
                    : 'text-muted-foreground hover:text-foreground',
                )}
              >
                {pill.label}
              </button>
            ))}
          </div>

          <button
            type="button"
            onClick={() => onShowAddCustomChange(!showAddCustom)}
            className="t-press inline-flex items-center gap-1.5 rounded-xl border border-primary/40 bg-primary/15 px-3 py-2 text-xs font-semibold text-primary transition hover:bg-primary/25"
          >
            <Plus className="h-3.5 w-3.5" />
            <span>Custom Endpoint</span>
          </button>
        </div>
      </div>

      {/* Inline Add Custom Provider Drawer */}
      {showAddCustom && (
        <div className="rounded-2xl border border-primary/40 bg-primary/5 p-5 shadow-md">
          <div className="mb-3 flex items-center justify-between border-b border-border/40 pb-2.5">
            <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
              <Server className="h-4 w-4 text-primary" />
              <span>Add Custom OpenAI-Compatible LLM Provider</span>
            </div>
            <button
              type="button"
              onClick={() => {
                onShowAddCustomChange(false);
                onNewProviderKeyError('');
              }}
              className="rounded-lg p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
            <div className="flex-1">
              <label className="mb-1.5 block text-xs font-medium text-muted-foreground">
                Unique Provider Identifier (e.g. <code className="text-primary">vllm-local</code>,{' '}
                <code className="text-primary">llama-cpp</code>, <code className="text-primary">corp-proxy</code>)
              </label>
              <input
                type="text"
                value={newProviderKey}
                onChange={(event) => {
                  onNewProviderKeyChange(sanitizeProviderKey(event.target.value));
                  onNewProviderKeyError('');
                }}
                placeholder="my-custom-provider"
                className="w-full rounded-xl border border-border/70 bg-background/80 px-3 py-2.5 font-mono text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
              {newProviderKeyError && (
                <span className="mt-1 block text-[11px] font-medium text-destructive">
                  {newProviderKeyError}
                </span>
              )}
            </div>

            <button
              type="button"
              onClick={() => {
                if (!newProviderKey.trim()) {
                  onNewProviderKeyError('Provider key cannot be empty.');
                  return;
                }
                if (providerKeys.includes(newProviderKey)) {
                  onNewProviderKeyError('This provider key already exists.');
                  return;
                }
                onDirty();
                onProvidersChange((current) => ({
                  ...current,
                  [newProviderKey]: { api_key: '', api_base: '', default_model: '' },
                }));
                onNewProviderKeyChange('');
                onShowAddCustomChange(false);
              }}
              className="t-press rounded-xl bg-primary px-4 py-2.5 text-xs font-semibold text-primary-foreground shadow-sm transition hover:opacity-90"
            >
              Create Provider Card
            </button>
          </div>
        </div>
      )}

      {/* Masked Info Note */}
      <p className="px-1 text-[11px] text-muted-foreground select-none">
        Masked entries (<span className="font-mono text-foreground">••••••••</span>) indicate an API key is securely saved in{' '}
        <code className="rounded bg-muted px-1 py-0.5 font-mono text-[10px]">~/.openz/config.json</code>. Overwrite a field and click{' '}
        <strong className="text-foreground">Save & Apply</strong> to update it.
      </p>

      {/* 2-Column Responsive Provider Cards Grid */}
      {filteredProviderKeys.length === 0 ? (
        <div className="rounded-2xl border border-border/50 bg-card/40 p-8 text-center text-xs text-muted-foreground">
          No providers match your current filter.
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
          {filteredProviderKeys.map((providerKey) => {
            const providerData = providersForm[providerKey] || {};
            const setProviderField = (field: keyof ProviderForm, value: string) => {
              onDirty();
              onProvidersChange((current) => ({
                ...current,
                [providerKey]: {
                  ...providerData,
                  [field]: value,
                },
              }));
            };
            const providerCapability = providerCapabilityByKey.get(providerKey);
            const providerDescriptor = providerDescriptors.find(
              (provider) => provider.key === providerKey,
            );
            const label = providerDescriptor?.display || providerKey;
            const isCustom = !providerCapability;
            const isConfigured = Boolean(providerData.api_key?.trim());
            const isRevealed = Boolean(revealedKeys[providerKey]);

            return (
              <div
                key={providerKey}
                className={cn(
                  'flex flex-col justify-between rounded-2xl border p-4 shadow-xs transition-all',
                  isConfigured
                    ? 'border-primary/30 bg-card/75'
                    : 'border-border/65 bg-card/45 hover:border-border',
                )}
              >
                <div>
                  <div className="mb-3.5 flex items-center justify-between gap-2 border-b border-border/40 pb-2.5">
                    <div className="flex min-w-0 items-center gap-2">
                      <div
                        className={cn(
                          'flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-xs font-bold uppercase',
                          isConfigured
                            ? 'bg-primary/15 text-primary'
                            : 'bg-muted text-muted-foreground',
                        )}
                      >
                        {label.slice(0, 2)}
                      </div>
                      <div className="min-w-0">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <span className="truncate text-sm font-semibold capitalize text-foreground">
                            {label}
                          </span>
                          {isConfigured && (
                            <span className="inline-flex items-center gap-1 rounded-full border border-success/30 bg-success/12 px-2 py-0.5 text-[10px] font-semibold text-success">
                              <CheckCircle2 className="h-2.5 w-2.5" /> Ready
                            </span>
                          )}
                          {isCustom && (
                            <span className="rounded-full border border-primary/30 bg-primary/10 px-2 py-0.5 text-[10px] font-semibold text-primary">
                              Custom
                            </span>
                          )}
                        </div>
                        <div className="font-mono text-[10px] text-muted-foreground">
                          key: {providerKey}
                        </div>
                      </div>
                    </div>

                    {isCustom && (
                      <button
                        type="button"
                        onClick={() => {
                          onDirty();
                          onProvidersChange((current) => {
                            const copy = { ...current };
                            delete copy[providerKey];
                            return copy;
                          });
                        }}
                        className="rounded-lg p-1.5 text-muted-foreground/70 transition hover:bg-destructive/10 hover:text-destructive"
                        title="Delete custom provider"
                        aria-label={`Delete ${label}`}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    )}
                  </div>

                  <div className="space-y-3">
                    <div>
                      <label className="mb-1 flex select-none items-center gap-1 text-[11px] font-medium text-muted-foreground">
                        <KeyRound className="h-3 w-3 text-primary" /> API Key
                      </label>
                      <div className="relative flex items-center">
                        <input
                          type={isRevealed ? 'text' : 'password'}
                          value={providerData.api_key || ''}
                          onChange={(event) => setProviderField('api_key', event.target.value)}
                          placeholder={providerData.api_key === '••••••••' ? '••••••••' : 'sk-...'}
                          className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2 pr-9 font-mono text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                        />
                        <button
                          type="button"
                          onClick={() => toggleReveal(providerKey)}
                          className="absolute right-2 rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                          aria-label={isRevealed ? 'Hide API key' : 'Show API key'}
                        >
                          {isRevealed ? (
                            <EyeOff className="h-3.5 w-3.5" />
                          ) : (
                            <Eye className="h-3.5 w-3.5" />
                          )}
                        </button>
                      </div>
                    </div>

                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                      {(providerCapability?.apiBaseEditable ?? true) && (
                        <div>
                          <label className="mb-1 block select-none text-[11px] font-medium text-muted-foreground">
                            API Base URL
                          </label>
                          <input
                            type="text"
                            value={providerData.api_base || ''}
                            onChange={(event) => setProviderField('api_base', event.target.value)}
                            placeholder={providerDescriptor?.apiBase || 'https://api.example.com/v1'}
                            className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2 font-mono text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                          />
                        </div>
                      )}
                      <div
                        className={
                          (providerCapability?.apiBaseEditable ?? true) ? '' : 'sm:col-span-2'
                        }
                      >
                        <label className="mb-1 block select-none text-[11px] font-medium text-muted-foreground">
                          Preferred Default Model
                        </label>
                        <input
                          type="text"
                          value={providerData.default_model || ''}
                          onChange={(event) =>
                            setProviderField('default_model', event.target.value)
                          }
                          placeholder="e.g. gpt-4o, claude-sonnet-4-5"
                          className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2 font-mono text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
