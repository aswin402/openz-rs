import React, { useState } from 'react';
import {
  Cpu,
  Eye,
  EyeOff,
  FolderOpen,
  Globe,
  Key,
  Link,
  Radio,
  Shield,
  ShieldAlert,
  Sliders,
  Zap,
} from 'lucide-react';
import type { AgentDefaultsConfig, OpenZConfigPatch } from '../../types';
import {
  getCavemanLevelOption,
  normalizeCavemanLevel,
} from '../../shared/lib/cavemanLevels';
import { CustomSelect, NumberInput } from './CustomSelect';
import {
  normalizeSecurityMode,
  normalizeToolRoutingMode,
  PERMISSION_TIER_DESCRIPTORS,
  TOOL_ROUTING_MODE_DESCRIPTORS,
  type SelectGroup,
  type SelectOption,
  type SettingsForm,
} from './settingsSchema';
import { cn } from '../../lib/utils';

export interface AgentSettingsTabProps {
  settings: AgentDefaultsConfig | null;
  form: SettingsForm;
  urlInput: string;
  tokenInput: string;
  modelGroups: SelectGroup[];
  providerOptions: SelectOption[];
  securityModeOptions: SelectOption[];
  toolRoutingModeOptions: SelectOption[];
  onUrlChange: (value: string) => void;
  onTokenChange: (value: string) => void;
  onFieldChange: (key: string, value: string | number | boolean) => void;
  onUpdateConfig: (patch: OpenZConfigPatch) => void;
  onOpenCavemanSettings?: () => void;
}

export const AgentSettingsTab: React.FC<AgentSettingsTabProps> = ({
  settings,
  form,
  urlInput,
  tokenInput,
  modelGroups,
  providerOptions,
  securityModeOptions,
  toolRoutingModeOptions,
  onUrlChange,
  onTokenChange,
  onFieldChange,
  onUpdateConfig,
  onOpenCavemanSettings,
}) => {
  const [showToken, setShowToken] = useState(false);

  return (
    <div className="space-y-6">
      {/* Card 1: Gateway Connection & Authentication */}
      <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
        <div className="mb-4 flex items-center gap-2.5 border-b border-border/40 pb-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
            <Globe className="h-4 w-4" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              Gateway Connection & Authentication
            </h3>
            <p className="text-[11px] text-muted-foreground">
              WebSocket endpoint and optional bearer token for the OpenZ Gateway server.
            </p>
          </div>
        </div>

        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <div>
            <label className="mb-1.5 flex select-none items-center gap-1.5 text-xs font-medium text-muted-foreground">
              <Link className="h-3.5 w-3.5 text-primary" /> OpenZ Gateway WebSocket URL
            </label>
            <input
              type="text"
              value={urlInput}
              onChange={(event) => onUrlChange(event.target.value)}
              placeholder="Auto-detected from current gateway"
              className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2.5 font-mono text-xs text-foreground transition-all hover:border-border focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
          </div>

          <div>
            <label className="mb-1.5 flex select-none items-center gap-1.5 text-xs font-medium text-muted-foreground">
              <Key className="h-3.5 w-3.5 text-primary" /> Gateway Authorization Token (OPENZ_GATEWAY_TOKEN)
            </label>
            <div className="relative flex items-center">
              <input
                type={showToken ? 'text' : 'password'}
                value={tokenInput}
                onChange={(event) => onTokenChange(event.target.value)}
                placeholder="Leave empty if token auth is disabled"
                className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2.5 pr-9 font-mono text-xs text-foreground transition-all hover:border-border focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
              <button
                type="button"
                onClick={() => setShowToken(!showToken)}
                className="absolute right-2.5 rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                aria-label={showToken ? 'Hide token' : 'Show token'}
              >
                {showToken ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
              </button>
            </div>
          </div>
        </div>
      </section>

      {settings ? (
        <>
          {/* Card 2: WebUI Agent Profile & Identity */}
          <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
            <div className="mb-4 flex items-center gap-2.5 border-b border-border/40 pb-3">
              <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
                <Cpu className="h-4 w-4" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-foreground">
                  WebUI Agent Profile (Isolated from TUI)
                </h3>
                <p className="text-[11px] text-muted-foreground">
                  Default LLM model, provider routing, bot persona name, and active workspace root.
                </p>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
              <div className="lg:col-span-2">
                <CustomSelect
                  label="Active WebUI Model"
                  value={`${String(form.provider ?? settings.provider)}::${String(form.model ?? settings.model)}`}
                  onChange={(value) => {
                    const [provider, ...modelParts] = value.split('::');
                    const model = modelParts.join('::');
                    onFieldChange('model', model || value);
                    if (provider && model) onFieldChange('provider', provider);
                  }}
                  groups={modelGroups}
                />
              </div>

              <CustomSelect
                label="Default Provider Routing"
                value={String(form.provider ?? settings.provider)}
                onChange={(value) => onFieldChange('provider', value)}
                options={providerOptions}
              />

              <div>
                <label className="mb-1.5 block select-none text-xs font-medium text-muted-foreground">
                  Bot Display Name
                </label>
                <input
                  type="text"
                  value={String(form.bot_name ?? settings.bot_name)}
                  onChange={(event) => onFieldChange('bot_name', event.target.value)}
                  className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2.5 text-xs text-foreground transition-all hover:border-border focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                />
              </div>

              <div className="lg:col-span-2">
                <label className="mb-1.5 flex select-none items-center gap-1.5 text-xs font-medium text-muted-foreground">
                  <FolderOpen className="h-3.5 w-3.5 text-primary" /> Default Workspace Directory
                </label>
                <input
                  type="text"
                  value={String(form.workspace ?? settings.workspace ?? '')}
                  onChange={(event) => onFieldChange('workspace', event.target.value)}
                  placeholder="~/projects/current"
                  className="w-full rounded-xl border border-border/70 bg-background/70 px-3 py-2.5 font-mono text-xs text-foreground transition-all hover:border-border focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                />
              </div>
            </div>
          </section>

          {/* Card 2.5: Ask Permission / Approval Policy (Low, Medium, High) */}
          <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
            <div className="mb-4 flex items-center gap-2.5 border-b border-border/40 pb-3">
              <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
                <Shield className="h-4 w-4" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-foreground">
                  Ask Permission Policy (Low / Medium / High)
                </h3>
                <p className="text-[11px] text-muted-foreground">
                  Choose when OpenZ asks interactive permission before executing shell commands or mutating tools. Catastrophic wipes (rm -rf /, dd, mkfs) remain hard-blocked in every mode.
                </p>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
              {PERMISSION_TIER_DESCRIPTORS.map((tier) => {
                const activeMode = normalizeSecurityMode(
                  form.security_mode ?? settings.security_mode,
                );
                const selected = activeMode === tier.value;
                return (
                  <button
                    key={tier.value}
                    type="button"
                    onClick={() => onFieldChange('security_mode', tier.value)}
                    aria-pressed={selected}
                    className={cn(
                      'flex flex-col justify-between rounded-xl border p-3.5 text-left transition-all',
                      selected
                        ? 'border-primary bg-primary/10 shadow-xs'
                        : 'border-border/70 bg-background/50 hover:border-border hover:bg-muted/30',
                    )}
                  >
                    <div>
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-xs font-bold text-foreground">{tier.title}</span>
                        <span
                          className={cn(
                            'rounded-full px-2 py-0.5 text-[10px] font-medium',
                            selected
                              ? 'bg-primary text-primary-foreground'
                              : 'bg-muted text-muted-foreground',
                          )}
                        >
                          {tier.badge}
                        </span>
                      </div>
                      <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
                        {tier.summary}
                      </p>
                    </div>
                    <p className="mt-2.5 border-t border-border/50 pt-2 font-mono text-[10px] leading-relaxed text-foreground/80">
                      Gates: {tier.examples}
                    </p>
                  </button>
                );
              })}
            </div>
          </section>

          {/* Card 2.75: Dynamic JIT Tool Router Mode */}
          <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
            <div className="mb-4 flex items-center gap-2.5 border-b border-border/40 pb-3">
              <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
                <Zap className="h-4 w-4" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-foreground">
                  Dynamic JIT Tool Router (Adaptive / Minimal / Balanced / Expanded)
                </h3>
                <p className="text-[11px] text-muted-foreground">
                  Controls how OpenZ scopes its 240 native tools per turn, preserving LLM KV prompt caches via monotonic append-only expansion while discovering long-tail tools through tool_catalog.
                </p>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-4">
              {TOOL_ROUTING_MODE_DESCRIPTORS.map((descriptor) => {
                const activeRoutingMode = normalizeToolRoutingMode(
                  form.tool_routing_mode ?? settings.tool_routing_mode,
                );
                const selected = activeRoutingMode === descriptor.value;
                return (
                  <button
                    key={descriptor.value}
                    type="button"
                    onClick={() => onFieldChange('tool_routing_mode', descriptor.value)}
                    aria-pressed={selected}
                    className={cn(
                      'flex flex-col justify-between rounded-xl border p-3.5 text-left transition-all',
                      selected
                        ? 'border-primary bg-primary/10 shadow-xs'
                        : 'border-border/70 bg-background/50 hover:border-border hover:bg-muted/30',
                    )}
                  >
                    <div>
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-xs font-bold text-foreground">{descriptor.title}</span>
                        <span
                          className={cn(
                            'rounded-full px-2 py-0.5 text-[10px] font-medium',
                            selected
                              ? 'bg-primary text-primary-foreground'
                              : 'bg-muted text-muted-foreground',
                          )}
                        >
                          {descriptor.badge}
                        </span>
                      </div>
                      <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
                        {descriptor.summary}
                      </p>
                    </div>
                    <p className="mt-2.5 border-t border-border/50 pt-2 font-mono text-[10px] leading-relaxed text-foreground/80">
                      {descriptor.budget}
                    </p>
                  </button>
                );
              })}
            </div>
          </section>

          {/* Card 2.85: Caveman Mode Quick Summary (Full controls in Caveman Mode tab) */}
          <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-2.5">
                <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
                  <Zap className="h-4 w-4" />
                </div>
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <h3 className="text-sm font-semibold text-foreground">Caveman Mode</h3>
                    <span className="rounded-md bg-muted px-2 py-0.5 font-mono text-[10px] font-semibold text-foreground">
                      {
                        getCavemanLevelOption(
                          normalizeCavemanLevel(form.caveman_level ?? settings.caveman_level),
                        ).label
                      }
                    </span>
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    Enable or disable terse mode here, or open Caveman Mode settings to configure intensity tiers.
                  </p>
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                {settings.caveman_stats && settings.caveman_stats.lifetimeTotalTokensSaved > 0 && (
                  <span className="rounded-full border border-primary/30 bg-primary/10 px-2.5 py-1 font-mono text-[10px] font-semibold text-primary">
                    ⛏ {settings.caveman_stats.compactLifetimeBadge} saved
                  </span>
                )}
                <button
                  type="button"
                  onClick={() => {
                    const next = !settings.caveman_mode;
                    onFieldChange('caveman_mode', next);
                    onUpdateConfig({ defaults: { caveman_mode: next } });
                  }}
                  aria-pressed={Boolean(settings.caveman_mode)}
                  className={cn(
                    'inline-flex items-center gap-1.5 rounded-xl border px-3 py-1.5 text-xs font-semibold transition-all',
                    settings.caveman_mode
                      ? 'border-primary bg-primary text-primary-foreground shadow-xs'
                      : 'border-border/70 bg-background/60 text-muted-foreground hover:text-foreground',
                  )}
                >
                  <Zap className="h-3.5 w-3.5" />
                  <span>Caveman {settings.caveman_mode ? 'ON' : 'OFF'}</span>
                </button>
                {onOpenCavemanSettings && (
                  <button
                    type="button"
                    onClick={onOpenCavemanSettings}
                    className="inline-flex items-center gap-1 rounded-xl border border-border/70 bg-background/60 px-3 py-1.5 text-xs font-semibold text-foreground transition hover:border-primary/45 hover:bg-primary/10 hover:text-primary"
                  >
                    <span>Configure Mode →</span>
                  </button>
                )}
              </div>
            </div>
          </section>

          {/* Card 3: Generation Limits, Security & Browser Automation Ports */}
          <section className="rounded-2xl border border-border/70 bg-card/60 p-5 shadow-xs">
            <div className="mb-4 flex items-center gap-2.5 border-b border-border/40 pb-3">
              <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-primary/15 text-primary">
                <Sliders className="h-4 w-4" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-foreground">
                  Inference Limits, Security Guard & Browser Ports
                </h3>
                <p className="text-[11px] text-muted-foreground">
                  Fine-tune token budgets, tool execution timeouts, context compaction thresholds, and WebDriver ports.
                </p>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
              <NumberInput
                label="Temperature"
                step={0.1}
                min={0}
                max={2}
                value={
                  form.temperature !== undefined
                    ? Math.round(Number(form.temperature) * 100) / 100
                    : Math.round(Number(settings.temperature) * 100) / 100
                }
                onChange={(value) => onFieldChange('temperature', value)}
              />
              <NumberInput
                label="Max Output Tokens"
                min={1}
                step={256}
                value={Number(form.max_tokens ?? settings.max_tokens)}
                onChange={(value) => onFieldChange('max_tokens', value)}
              />
              <NumberInput
                label="Max Session Messages"
                min={1}
                step={10}
                value={Number(form.max_messages ?? settings.max_messages)}
                onChange={(value) => onFieldChange('max_messages', value)}
              />
              <NumberInput
                label="Max Tool Iterations"
                min={1}
                step={10}
                value={Number(form.max_tool_iterations ?? settings.max_tool_iterations)}
                onChange={(value) => onFieldChange('max_tool_iterations', value)}
              />
              <NumberInput
                label="Tool Timeout (sec)"
                min={1}
                step={15}
                value={Number(form.tool_timeout_secs ?? settings.tool_timeout_secs)}
                onChange={(value) => onFieldChange('tool_timeout_secs', value)}
              />
              <NumberInput
                label="Context Limit (chars)"
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
                label="Ask Permission Level"
                value={normalizeSecurityMode(form.security_mode ?? settings.security_mode)}
                onChange={(value) => onFieldChange('security_mode', value)}
                options={securityModeOptions}
              />
              <CustomSelect
                label="Tool Router Mode"
                value={normalizeToolRoutingMode(
                  form.tool_routing_mode ?? settings.tool_routing_mode,
                )}
                onChange={(value) => onFieldChange('tool_routing_mode', value)}
                options={toolRoutingModeOptions}
              />
              <CustomSelect
                label="Gateway Theme Mode"
                value={String(form.theme ?? settings.theme ?? 'dark')}
                onChange={(value) => onFieldChange('theme', value)}
                options={[
                  { value: 'dark', label: 'Dark' },
                  { value: 'light', label: 'Light' },
                  { value: 'system', label: 'System' },
                ]}
              />
              <CustomSelect
                label="TUI Thought Display"
                value={String(form.tui_thought_display ?? settings.tui_thought_display ?? 'auto')}
                onChange={(value) => onFieldChange('tui_thought_display', value)}
                options={[
                  { value: 'auto', label: 'Auto' },
                  { value: 'hidden', label: 'Hidden' },
                  { value: 'summary', label: 'Summary' },
                  { value: 'full', label: 'Full' },
                ]}
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
            </div>
          </section>

          {/* Card 4: Live Runtime Switches */}
          <section className="grid grid-cols-1 gap-4 md:grid-cols-2">
            {[
              {
                key: 'caveman',
                icon: Zap,
                title: 'Caveman Terseness Mode',
                desc: 'Strips filler words and articles for concise, high-speed responses.',
                checked: Boolean(settings.caveman_mode),
                onToggle: () => onUpdateConfig({ defaults: { caveman_mode: !settings.caveman_mode } }),
              },
              {
                key: 'streaming',
                icon: Radio,
                title: 'Real-Time Token Streaming',
                desc: 'Stream LLM token deltas and live reasoning traces as they arrive.',
                checked: Boolean(settings.streaming),
                onToggle: () => onUpdateConfig({ defaults: { streaming: !settings.streaming } }),
              },
              {
                key: 'notices',
                icon: Link,
                title: 'Research Auto-Capture Notices',
                desc: 'Display subtle notices when links and briefs are stored to memory.',
                checked: Boolean(settings.show_auto_capture_notices ?? true),
                onToggle: () =>
                  onUpdateConfig({
                    defaults: { show_auto_capture_notices: !(settings.show_auto_capture_notices ?? true) },
                  }),
              },
              {
                key: 'sandbox',
                icon: ShieldAlert,
                title: 'Linux seccomp BPF Sandbox',
                desc: 'Restrict subprocess syscalls to block raw network sockets & kernel modules.',
                checked: Boolean(settings.enable_sandbox),
                onToggle: () => onUpdateConfig({ defaults: { enable_sandbox: !settings.enable_sandbox } }),
              },
            ].map((item) => {
              const Icon = item.icon;
              return (
                <div
                  key={item.key}
                  className="flex items-center justify-between gap-4 rounded-2xl border border-border/70 bg-card/60 p-4 shadow-xs transition-colors hover:border-border"
                >
                  <div className="min-w-0">
                    <div className="flex select-none items-center gap-2 text-xs font-semibold text-foreground">
                      <Icon className="h-4 w-4 shrink-0 text-primary" />
                      <span>{item.title}</span>
                    </div>
                    <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">
                      {item.desc}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={item.onToggle}
                    aria-pressed={item.checked}
                    className={cn(
                      'relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none',
                      item.checked ? 'bg-primary' : 'bg-muted',
                    )}
                  >
                    <span
                      className={cn(
                        'pointer-events-none inline-block h-5 w-5 transform rounded-full shadow-md ring-0 transition duration-200 ease-in-out',
                        item.checked ? 'translate-x-5 bg-primary-foreground' : 'translate-x-0 bg-card',
                      )}
                    />
                  </button>
                </div>
              );
            })}
          </section>
        </>
      ) : (
        <div className="rounded-2xl border border-border/50 bg-card/40 p-6 text-center text-xs text-muted-foreground select-none">
          Agent defaults are loading — they appear automatically once the OpenZ Gateway responds.
        </div>
      )}
    </div>
  );
};
