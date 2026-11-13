import React, { useState, type Dispatch, type SetStateAction } from 'react';
import { Eye, EyeOff, MessageSquareShare, Radio } from 'lucide-react';
import type { ChannelCapability } from '../../types';
import type { ChannelFieldValue, ChannelsForm } from './settingsSchema';
import { cn } from '../../lib/utils';

export interface ChannelsTabProps {
  channelCapabilities: ChannelCapability[];
  channelsForm: ChannelsForm;
  onChannelsChange: Dispatch<SetStateAction<ChannelsForm>>;
  onDirty: () => void;
}

export const ChannelsTab: React.FC<ChannelsTabProps> = ({
  channelCapabilities,
  channelsForm,
  onChannelsChange,
  onDirty,
}) => {
  const [revealedSecrets, setRevealedSecrets] = useState<Record<string, boolean>>({});

  const toggleSecret = (id: string) => {
    setRevealedSecrets((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  return (
    <div className="space-y-5">
      <div className="flex items-center gap-3 rounded-2xl border border-border/70 bg-card/60 p-4 text-xs text-muted-foreground shadow-xs">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-primary/15 text-primary">
          <MessageSquareShare className="h-4 w-4" />
        </div>
        <p className="leading-relaxed">
          Configure external messaging bridges (Telegram, Discord, WhatsApp, Email, WebSocket). Modifying bot tokens or listener ports applies on next daemon / channel reload.
        </p>
      </div>

      {channelCapabilities.length === 0 ? (
        <div className="rounded-2xl border border-border/50 bg-card/40 p-8 text-center text-xs text-muted-foreground select-none">
          Channel capabilities are loading — they appear automatically once the OpenZ Gateway responds.
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
          {channelCapabilities.map((channel) => {
            const defaults = channel.defaults || {};
            const values = channelsForm[channel.name] || {};
            const valueFor = (key: string) => values[key] ?? defaults[key] ?? '';
            const setChannelField = (key: string, value: ChannelFieldValue) => {
              onDirty();
              onChannelsChange((current) => ({
                ...current,
                [channel.name]: {
                  ...(current[channel.name] || {}),
                  [key]: value,
                },
              }));
            };
            const enabledField = channel.fields.find((field) => field.key === 'enabled');
            const enabled = Boolean(valueFor('enabled'));

            return (
              <div
                key={channel.name}
                className={cn(
                  'rounded-2xl border p-5 shadow-xs transition-all',
                  enabled
                    ? 'border-primary/35 bg-card/75'
                    : 'border-border/65 bg-card/45 hover:border-border',
                )}
              >
                <div className="mb-4 flex items-center justify-between gap-2 border-b border-border/40 pb-3">
                  <div className="flex items-center gap-2.5">
                    <div
                      className={cn(
                        'flex h-8 w-8 items-center justify-center rounded-xl',
                        enabled ? 'bg-primary/15 text-primary' : 'bg-muted text-muted-foreground',
                      )}
                    >
                      <Radio className="h-4 w-4" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-foreground select-none">
                          {channel.label}
                        </span>
                        <span
                          className={cn(
                            'rounded-full px-2 py-0.5 text-[10px] font-semibold',
                            enabled
                              ? 'border border-success/30 bg-success/12 text-success'
                              : 'bg-muted text-muted-foreground',
                          )}
                        >
                          {enabled ? 'Enabled' : 'Disabled'}
                        </span>
                      </div>
                      <div className="font-mono text-[10px] text-muted-foreground">
                        channel: {channel.name}
                      </div>
                    </div>
                  </div>

                  {enabledField && (
                    <button
                      type="button"
                      onClick={() => setChannelField('enabled', !enabled)}
                      aria-pressed={enabled}
                      className={cn(
                        'relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none',
                        enabled ? 'bg-primary' : 'bg-muted',
                      )}
                    >
                      <span
                        className={cn(
                          'pointer-events-none inline-block h-5 w-5 transform rounded-full shadow-md ring-0 transition duration-200 ease-in-out',
                          enabled ? 'translate-x-5 bg-primary-foreground' : 'translate-x-0 bg-card',
                        )}
                      />
                    </button>
                  )}
                </div>

                <div className="space-y-3">
                  {channel.fields
                    .filter((field) => field.key !== 'enabled')
                    .map((field) => {
                      const value = valueFor(field.key);
                      const secretId = `${channel.name}::${field.key}`;
                      const isRevealed = Boolean(revealedSecrets[secretId]);

                      if (field.kind === 'boolean') {
                        const boolVal = Boolean(value);
                        return (
                          <div
                            key={field.key}
                            className="flex items-center justify-between rounded-xl border border-border/50 bg-background/50 px-3.5 py-2.5"
                          >
                            <span className="text-xs font-medium text-foreground">
                              {field.label}
                            </span>
                            <button
                              type="button"
                              onClick={() => setChannelField(field.key, !boolVal)}
                              aria-pressed={boolVal}
                              className={cn(
                                'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200',
                                boolVal ? 'bg-primary' : 'bg-muted',
                              )}
                            >
                              <span
                                className={cn(
                                  'pointer-events-none inline-block h-4 w-4 transform rounded-full shadow-sm transition duration-200',
                                  boolVal ? 'translate-x-4 bg-primary-foreground' : 'translate-x-0 bg-card',
                                )}
                              />
                            </button>
                          </div>
                        );
                      }

                      return (
                        <div key={field.key}>
                          <label className="mb-1 block select-none text-[11px] font-medium text-muted-foreground">
                            {field.label}
                          </label>
                          <div className="relative flex items-center">
                            <input
                              type={
                                field.kind === 'secret'
                                  ? isRevealed
                                    ? 'text'
                                    : 'password'
                                  : field.kind === 'number'
                                    ? 'number'
                                    : 'text'
                              }
                              value={field.kind === 'number' ? Number(value || 0) : String(value)}
                              onChange={(event) =>
                                setChannelField(
                                  field.key,
                                  field.kind === 'number'
                                    ? Number(event.target.value)
                                    : event.target.value,
                                )
                              }
                              placeholder={
                                field.kind === 'secret' && String(value) === '••••••••'
                                  ? '••••••••'
                                  : field.label
                              }
                              className={cn(
                                'w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2 font-mono text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20',
                                field.kind === 'secret' && 'pr-9',
                              )}
                            />
                            {field.kind === 'secret' && (
                              <button
                                type="button"
                                onClick={() => toggleSecret(secretId)}
                                className="absolute right-2 rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                                aria-label={isRevealed ? 'Hide secret' : 'Show secret'}
                              >
                                {isRevealed ? (
                                  <EyeOff className="h-3.5 w-3.5" />
                                ) : (
                                  <Eye className="h-3.5 w-3.5" />
                                )}
                              </button>
                            )}
                          </div>
                        </div>
                      );
                    })}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
