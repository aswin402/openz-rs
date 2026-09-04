import React from 'react';
import { Cpu, Key, Link, Radio, ShieldAlert, Zap } from 'lucide-react';
import type { AgentDefaultsConfig, OpenZConfigPatch } from '../../types';
import { CustomSelect, NumberInput } from './CustomSelect';
import type { SelectGroup, SelectOption, SettingsForm } from './settingsSchema';

export interface AgentSettingsTabProps {
  settings: AgentDefaultsConfig | null;
  form: SettingsForm;
  urlInput: string;
  tokenInput: string;
  modelGroups: SelectGroup[];
  providerOptions: SelectOption[];
  securityModeOptions: SelectOption[];
  onUrlChange: (value: string) => void;
  onTokenChange: (value: string) => void;
  onFieldChange: (key: string, value: string | number | boolean) => void;
  onUpdateConfig: (patch: OpenZConfigPatch) => void;
}

export const AgentSettingsTab: React.FC<AgentSettingsTabProps> = ({
  settings,
  form,
  urlInput,
  tokenInput,
  modelGroups,
  providerOptions,
  securityModeOptions,
  onUrlChange,
  onTokenChange,
  onFieldChange,
  onUpdateConfig,
}) => (
  <>
    <div>
      <label className="mb-1 block font-medium text-foreground flex items-center gap-1.5 select-none">
        <Link className="h-3.5 w-3.5 text-amber-500" /> OpenZ Gateway WebSocket URL
      </label>
      <input
        type="text"
        value={urlInput}
        onChange={(event) => onUrlChange(event.target.value)}
        placeholder="Auto-detected from current gateway"
        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
      />
    </div>

    <div>
      <label className="mb-1 block font-medium text-foreground flex items-center gap-1.5 select-none">
        <Key className="h-3.5 w-3.5 text-amber-500" /> Gateway Authorization Token (OPENZ_GATEWAY_TOKEN)
      </label>
      <input
        type="password"
        value={tokenInput}
        onChange={(event) => onTokenChange(event.target.value)}
        placeholder="Leave empty if token auth is disabled"
        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
      />
    </div>

    {settings ? (
      <>
        <div className="pt-1">
          <div className="mb-3 flex items-center gap-1.5 font-semibold text-foreground border-b border-border/40 pb-1.5 select-none">
            <Cpu className="h-3.5 w-3.5 text-amber-500" /> Agent Defaults
          </div>
          <div className="space-y-3">
            <CustomSelect
              label="Default Model"
              value={`${String(form.provider ?? settings.provider)}::${String(form.model ?? settings.model)}`}
              onChange={(value) => {
                const [provider, ...modelParts] = value.split('::');
                const model = modelParts.join('::');
                onFieldChange('model', model || value);
                if (provider && model) onFieldChange('provider', provider);
              }}
              groups={modelGroups}
            />

            <CustomSelect
              label="Default Provider"
              value={String(form.provider ?? settings.provider)}
              onChange={(value) => onFieldChange('provider', value)}
              options={providerOptions}
            />

            <div className="grid grid-cols-2 gap-3">
              <NumberInput
                label="Temperature"
                step={0.1}
                min={0}
                max={2}
                value={form.temperature !== undefined ? Math.round(Number(form.temperature) * 100) / 100 : Math.round(Number(settings.temperature) * 100) / 100}
                onChange={(value) => onFieldChange('temperature', value)}
              />
              <NumberInput
                label="Max Tokens"
                min={1}
                step={1}
                value={Number(form.max_tokens ?? settings.max_tokens)}
                onChange={(value) => onFieldChange('max_tokens', value)}
              />
              <NumberInput
                label="Max Messages"
                min={1}
                step={1}
                value={Number(form.max_messages ?? settings.max_messages)}
                onChange={(value) => onFieldChange('max_messages', value)}
              />
              <NumberInput
                label="Max Tool Iterations"
                min={1}
                step={1}
                value={Number(form.max_tool_iterations ?? settings.max_tool_iterations)}
                onChange={(value) => onFieldChange('max_tool_iterations', value)}
              />
              <NumberInput
                label="Tool Timeout (sec)"
                min={1}
                step={1}
                value={Number(form.tool_timeout_secs ?? settings.tool_timeout_secs)}
                onChange={(value) => onFieldChange('tool_timeout_secs', value)}
              />
              <NumberInput
                label="Firefox WebDriver Port"
                min={1024}
                max={65535}
                step={1}
                value={Number(form.firefox_webdriver_port ?? settings.firefox_webdriver_port)}
                onChange={(value) => onFieldChange('firefox_webdriver_port', value)}
              />
              <NumberInput
                label="Firefox Attach Port"
                min={1024}
                max={65535}
                step={1}
                value={Number(form.firefox_attach_port ?? settings.firefox_attach_port)}
                onChange={(value) => onFieldChange('firefox_attach_port', value)}
              />
              <CustomSelect
                label="Security Mode"
                value={String(form.security_mode ?? settings.security_mode)}
                onChange={(value) => onFieldChange('security_mode', value)}
                options={securityModeOptions}
              />
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div>
                <label className="mb-1 block font-medium text-foreground">Bot Name</label>
                <input
                  type="text"
                  value={String(form.bot_name ?? settings.bot_name)}
                  onChange={(event) => onFieldChange('bot_name', event.target.value)}
                  className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                />
              </div>
              <div>
                <label className="mb-1 block font-medium text-foreground">Workspace Path</label>
                <input
                  type="text"
                  value={String(form.workspace ?? settings.workspace ?? '')}
                  onChange={(event) => onFieldChange('workspace', event.target.value)}
                  placeholder="~/projects/current"
                  className="w-full rounded-lg border border-border bg-muted/40 p-2.5 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                />
              </div>
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <NumberInput
                label="Context Limit"
                min={1}
                step={1000}
                value={Number(form.context_limit || settings.context_limit || 1)}
                onChange={(value) => onFieldChange('context_limit', value)}
              />
              <NumberInput
                label="Tool Output Limit"
                min={1}
                step={1000}
                value={Number(form.tool_output_limit || settings.tool_output_limit || 1)}
                onChange={(value) => onFieldChange('tool_output_limit', value)}
              />
              <CustomSelect
                label="Thought Display"
                value={String(form.tui_thought_display ?? settings.tui_thought_display ?? 'auto')}
                onChange={(value) => onFieldChange('tui_thought_display', value)}
                options={[
                  { value: 'auto', label: 'auto' },
                  { value: 'hidden', label: 'hidden' },
                  { value: 'summary', label: 'summary' },
                  { value: 'full', label: 'full' },
                ]}
              />
            </div>
          </div>
        </div>

        <div className="pt-2 space-y-3">
          <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 p-3">
            <div>
              <div className="font-semibold text-foreground flex items-center gap-1.5 select-none">
                <Zap className="h-3.5 w-3.5 text-amber-500" /> Caveman Terseness Mode
              </div>
              <div className="text-[11px] text-muted-foreground mt-0.5">Strips filler words and articles for maximum speed</div>
            </div>
            <button
              onClick={() => onUpdateConfig({ defaults: { caveman_mode: !settings.caveman_mode } })}
              aria-pressed={settings.caveman_mode}
              className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${settings.caveman_mode ? 'bg-amber-500' : 'bg-muted'}`}
            >
              <span className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${settings.caveman_mode ? 'translate-x-5' : 'translate-x-0'}`} />
            </button>
          </div>

          <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 p-3">
            <div>
              <div className="font-semibold text-foreground flex items-center gap-1.5 select-none">
                <Radio className="h-3.5 w-3.5 text-amber-500" /> Real-Time Response Streaming
              </div>
              <div className="text-[11px] text-muted-foreground mt-0.5">Stream token deltas as they are generated by LLM</div>
            </div>
            <button
              onClick={() => onUpdateConfig({ defaults: { streaming: !settings.streaming } })}
              aria-pressed={settings.streaming}
              className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${settings.streaming ? 'bg-amber-500' : 'bg-muted'}`}
            >
              <span className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${settings.streaming ? 'translate-x-5' : 'translate-x-0'}`} />
            </button>
          </div>

          <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 p-3">
            <div>
              <div className="font-semibold text-foreground flex items-center gap-1.5 select-none">
                <Link className="h-3.5 w-3.5 text-amber-500" /> Research Source Notices
              </div>
              <div className="text-[11px] text-muted-foreground mt-0.5">Show when research links and briefs are saved to knowledge memory</div>
            </div>
            <button
              onClick={() => onUpdateConfig({ defaults: { show_auto_capture_notices: !(settings.show_auto_capture_notices ?? true) } })}
              aria-pressed={settings.show_auto_capture_notices ?? true}
              className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${(settings.show_auto_capture_notices ?? true) ? 'bg-amber-500' : 'bg-muted'}`}
            >
              <span className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${(settings.show_auto_capture_notices ?? true) ? 'translate-x-5' : 'translate-x-0'}`} />
            </button>
          </div>

          <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 p-3">
            <div>
              <div className="font-semibold text-foreground flex items-center gap-1.5 select-none">
                <ShieldAlert className="h-3.5 w-3.5 text-amber-500" /> Linux seccomp BPF Sandbox
              </div>
              <div className="text-[11px] text-muted-foreground mt-0.5">Restricts subprocess syscalls to block raw network access and loading modules</div>
            </div>
            <button
              onClick={() => onUpdateConfig({ defaults: { enable_sandbox: !settings.enable_sandbox } })}
              aria-pressed={settings.enable_sandbox}
              className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${settings.enable_sandbox ? 'bg-amber-500' : 'bg-muted'}`}
            >
              <span className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${settings.enable_sandbox ? 'translate-x-5' : 'translate-x-0'}`} />
            </button>
          </div>
        </div>
      </>
    ) : (
      <div className="rounded-lg border border-border/40 bg-muted/20 p-3 text-[11px] text-muted-foreground select-none">
        Agent defaults are not loaded yet — they appear once the gateway responds.
      </div>
    )}
  </>
);
