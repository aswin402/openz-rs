import React, { type Dispatch, type SetStateAction } from 'react';
import type { ChannelCapability } from '../../types';
import type { ChannelFieldValue, ChannelsForm } from './settingsSchema';

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
}) => (
  <div className="space-y-6">
    <div className="text-muted-foreground mb-2 text-[11px] leading-relaxed select-none">
      Enable external channels and background listeners. Modifying keys/tokens requires a daemon restart to re-init connections.
    </div>

    {channelCapabilities.length === 0 ? (
      <div className="rounded-lg border border-border/40 bg-muted/20 p-3 text-[11px] text-muted-foreground select-none">
        Channel capabilities are not loaded yet — they appear once the gateway responds.
      </div>
    ) : channelCapabilities.map((channel) => {
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
        <div key={channel.name} className="rounded-xl border border-border/50 bg-muted/15 p-4 space-y-3">
          <div className="flex items-center justify-between border-b border-border/30 pb-1.5">
            <div className="font-semibold text-foreground select-none">{channel.label}</div>
            {enabledField && (
              <button
                type="button"
                onClick={() => setChannelField('enabled', !enabled)}
                aria-pressed={enabled}
                className={enabled ? 'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none bg-amber-500' : 'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none bg-muted'}
              >
                <span
                  className={enabled ? 'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out translate-x-4' : 'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out translate-x-0'}
                />
              </button>
            )}
          </div>
          <div className="space-y-2.5">
            {channel.fields.filter((field) => field.key !== 'enabled').map((field) => {
              const value = valueFor(field.key);
              if (field.kind === 'boolean') {
                return (
                  <label key={field.key} className="flex items-center justify-between rounded-lg border border-border/40 bg-muted/20 px-3 py-2">
                    <span className="font-medium text-muted-foreground">{field.label}</span>
                    <input
                      type="checkbox"
                      checked={Boolean(value)}
                      onChange={(event) => setChannelField(field.key, event.target.checked)}
                      className="h-3.5 w-3.5 accent-amber-500"
                    />
                  </label>
                );
              }
              return (
                <div key={field.key}>
                  <label className="mb-1 block font-medium text-muted-foreground select-none">{field.label}</label>
                  <input
                    type={field.kind === 'secret' ? 'password' : field.kind === 'number' ? 'number' : 'text'}
                    value={field.kind === 'number' ? Number(value || 0) : String(value)}
                    onChange={(event) => setChannelField(
                      field.key,
                      field.kind === 'number' ? Number(event.target.value) : event.target.value,
                    )}
                    placeholder={field.kind === 'secret' && String(value) === '••••••••' ? '••••••••' : field.label}
                    className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                  />
                </div>
              );
            })}
          </div>
        </div>
      );
    })}
  </div>
);
