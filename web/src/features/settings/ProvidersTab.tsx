import React, { type Dispatch, type SetStateAction } from 'react';
import { Trash2 } from 'lucide-react';
import type { ProviderCapability } from '../../types';
import type { ProviderDescriptor } from '../../config/providers';
import type { ProviderForm, ProvidersForm } from './settingsSchema';

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
}) => (
  <div className="space-y-6 animate-in fade-in duration-150">
    <div className="text-muted-foreground mb-2 text-[11px] leading-relaxed select-none">
      Configure API keys and model endpoints. Masked entries (••••••••) mean a key is stored. Overwrite them to edit.
    </div>
    {providerKeys.map((providerKey) => {
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
      const providerDescriptor = providerDescriptors.find((provider) => provider.key === providerKey);
      const label = providerDescriptor?.display || providerKey;
      const isCustom = !providerCapability;

      return (
        <div key={providerKey} className="rounded-xl border border-border/50 bg-muted/15 p-4 space-y-3 relative">
          <div className="flex items-center justify-between border-b border-border/30 pb-1.5">
            <div className="font-semibold text-foreground capitalize select-none flex items-center gap-1.5">
              {label} Setup {isCustom && <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[9px] font-bold text-amber-500 select-none">Custom</span>}
            </div>
            {isCustom && (
              <button
                onClick={() => {
                  onDirty();
                  onProvidersChange((current) => {
                    const copy = { ...current };
                    delete copy[providerKey];
                    return copy;
                  });
                }}
                className="rounded-lg p-1 text-muted-foreground/60 hover:text-red-500 hover:bg-red-500/10 transition"
                title="Delete custom provider"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
          <div className="grid grid-cols-1 gap-2.5">
            <div>
              <label className="mb-1 block font-medium text-muted-foreground select-none">API Key</label>
              <input
                type="password"
                value={providerData.api_key || ''}
                onChange={(event) => setProviderField('api_key', event.target.value)}
                placeholder={providerData.api_key === '••••••••' ? '••••••••' : 'api_key_here'}
                className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
              />
            </div>
            {(providerCapability?.apiBaseEditable ?? true) && (
              <div>
                <label className="mb-1 block font-medium text-muted-foreground select-none">API Base Endpoint</label>
                <input
                  type="text"
                  value={providerData.api_base || ''}
                  onChange={(event) => setProviderField('api_base', event.target.value)}
                  placeholder={providerDescriptor?.apiBase || 'https://provider.example/v1'}
                  className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                />
              </div>
            )}
            <div>
              <label className="mb-1 block font-medium text-muted-foreground select-none">Preferred Default Model</label>
              <input
                type="text"
                value={providerData.default_model || ''}
                onChange={(event) => setProviderField('default_model', event.target.value)}
                placeholder="e.g. gpt-4o, claude-3-5-sonnet"
                className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
              />
            </div>
          </div>
        </div>
      );
    })}

    {showAddCustom ? (
      <div className="rounded-xl border border-dashed border-amber-500/50 bg-amber-500/5 p-4 space-y-3 animate-in slide-in-from-bottom-2 duration-150">
        <div className="font-semibold text-foreground border-b border-border/30 pb-1.5 select-none">
          Add New Custom LLM Provider Setup
        </div>
        <div className="space-y-2.5">
          <div>
            <label className="mb-1 block font-medium text-muted-foreground select-none">Unique Provider Key (e.g. `llama-local` / `corp-gateway` / `vllm-host` ...)</label>
            <input
              type="text"
              value={newProviderKey}
              onChange={(event) => {
                onNewProviderKeyChange(event.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, ''));
                onNewProviderKeyError('');
              }}
              placeholder="my-custom-provider"
              className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
            />
            {newProviderKeyError && <span className="text-red-500 text-[10px] mt-0.5 block">{newProviderKeyError}</span>}
          </div>
          <button
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
            className="w-full rounded-lg bg-amber-500/10 hover:bg-amber-500/20 text-amber-500 font-semibold py-2.5 text-xs transition duration-150"
          >
            Confirm Add Provider
          </button>
          <button
            onClick={() => {
              onShowAddCustomChange(false);
              onNewProviderKeyError('');
            }}
            className="w-full text-center text-muted-foreground hover:text-foreground text-[10px] pt-1 transition"
          >
            Cancel
          </button>
        </div>
      </div>
    ) : (
      <button
        onClick={() => onShowAddCustomChange(true)}
        className="w-full flex items-center justify-center gap-1.5 rounded-xl border border-dashed border-border hover:border-amber-500/40 bg-muted/10 hover:bg-muted/30 py-3.5 text-xs font-semibold text-muted-foreground hover:text-foreground transition duration-150"
      >
        + Add Custom LLM Provider Endpoint
      </button>
    )}
  </div>
);
