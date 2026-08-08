/* eslint-disable react-hooks/set-state-in-effect */
import React, { useState, useEffect, useRef, useMemo } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import type { JsonObject } from '../types';
import { X, Settings, Link, Key, Zap, Radio, Save, Cpu, ShieldAlert, ChevronUp, ChevronDown, Trash2, RotateCcw, CheckCircle2, AlertCircle } from 'lucide-react';

type SettingsForm = Record<string, string | number | boolean>;

type ProviderForm = {
  api_key?: string;
  api_base?: string;
  default_model?: string;
};

type ProvidersForm = Record<string, ProviderForm>;

type TelegramChannelForm = { enabled?: boolean; bot_token?: string };
type DiscordChannelForm = { enabled?: boolean; bot_token?: string };
type WhatsAppChannelForm = {
  enabled?: boolean;
  api_key?: string;
  phone_number_id?: string;
  webhook_port?: number;
  verify_token?: string;
};

type ChannelsForm = {
  telegram?: TelegramChannelForm;
  discord?: DiscordChannelForm;
  whatsapp?: WhatsAppChannelForm;
};

function cloneObject<T>(value: T): T {
  return JSON.parse(JSON.stringify(value || {})) as T;
}

function stableStringify(value: unknown): string {
  return JSON.stringify(value ?? {});
}

function jsonObjectToProviders(value: JsonObject): ProvidersForm {
  return cloneObject(value) as ProvidersForm;
}

function jsonObjectToChannels(value: JsonObject): ChannelsForm {
  return cloneObject(value) as ChannelsForm;
}

interface SelectOption {
  value: string;
  label: string;
}

interface SelectGroup {
  label: string;
  options: SelectOption[];
}

interface CustomSelectProps {
  label: string;
  value: string;
  onChange: (val: string) => void;
  options?: SelectOption[];
  groups?: SelectGroup[];
}

const CustomSelect: React.FC<CustomSelectProps> = ({ label, value, onChange, options, groups }) => {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  let displayValue = value;
  if (options) {
    const matched = options.find((o) => o.value === value);
    if (matched) displayValue = matched.label;
  } else if (groups) {
    for (const g of groups) {
      const matched = g.options.find((o) => o.value === value);
      if (matched) {
        displayValue = matched.label;
        break;
      }
    }
  }

  return (
    <div ref={containerRef} className="relative w-full">
      <label className="mb-1 block font-medium text-foreground">{label}</label>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="flex w-full items-center justify-between rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground hover:bg-muted/60 transition-colors focus:outline-none focus:ring-1 focus:ring-amber-500"
      >
        <span className="truncate">{displayValue}</span>
        <ChevronDown
          className="h-4 w-4 shrink-0 text-amber-500 transition-transform duration-200"
          style={{ transform: isOpen ? 'rotate(180deg)' : 'rotate(0)' }}
        />
      </button>

      {isOpen && (
        <div className="absolute left-0 right-0 z-50 mt-1 max-h-60 overflow-y-auto rounded-lg border border-border bg-card p-1.5 shadow-xl animate-in fade-in slide-in-from-top-1 duration-150 scrollbar-thin">
          {options && (
            <div className="space-y-0.5">
              {options.map((opt) => (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => {
                    onChange(opt.value);
                    setIsOpen(false);
                  }}
                  className={`flex w-full items-center rounded-md px-2.5 py-2 text-left text-xs transition-colors hover:bg-amber-500 hover:text-white ${
                    value === opt.value ? 'bg-amber-500/10 text-amber-500 font-semibold' : 'text-foreground'
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          )}

          {groups && (
            <div className="space-y-3">
              {groups.map((group) => (
                <div key={group.label}>
                  <div className="px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-muted-foreground/60 select-none">
                    {group.label}
                  </div>
                  <div className="mt-1 space-y-0.5 pl-1 border-l border-border/40 ml-1">
                    {group.options.map((opt) => (
                      <button
                        key={opt.value}
                        type="button"
                        onClick={() => {
                          onChange(opt.value);
                          setIsOpen(false);
                        }}
                        className={`flex w-full items-center rounded-md px-2.5 py-1.5 text-left text-xs transition-colors hover:bg-amber-500 hover:text-white ${
                          value === opt.value ? 'bg-amber-500/10 text-amber-500 font-semibold' : 'text-foreground'
                        }`}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

interface NumberInputProps {
  label: string;
  value: number;
  onChange: (val: number) => void;
  min?: number;
  max?: number;
  step?: number;
}

const NumberInput: React.FC<NumberInputProps> = ({ label, value, onChange, min = 1, max, step = 1 }) => {
  const handleIncrement = () => {
    const newVal = value + step;
    if (max !== undefined && newVal > max) return;
    onChange(Number(newVal.toFixed(2)));
  };

  const handleDecrement = () => {
    const newVal = value - step;
    if (min !== undefined && newVal < min) return;
    onChange(Number(newVal.toFixed(2)));
  };

  return (
    <div>
      <label className="mb-1 block font-medium text-foreground">{label}</label>
      <div className="relative flex items-center">
        <input
          type="number"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          className="w-full rounded-lg border border-border bg-muted/40 p-2.5 pr-8 text-xs text-foreground font-mono focus:outline-none focus:ring-1 focus:ring-amber-500 [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
        />
        <div className="absolute right-1.5 flex flex-col gap-0.5 select-none">
          <button
            type="button"
            onClick={handleIncrement}
            className="flex h-3.5 w-4 items-center justify-center rounded-sm bg-muted/50 hover:bg-amber-500 hover:text-white text-muted-foreground/80 transition-colors"
          >
            <ChevronUp className="h-2 w-2" strokeWidth={3} />
          </button>
          <button
            type="button"
            onClick={handleDecrement}
            className="flex h-3.5 w-4 items-center justify-center rounded-sm bg-muted/50 hover:bg-amber-500 hover:text-white text-muted-foreground/80 transition-colors"
          >
            <ChevronDown className="h-2 w-2" strokeWidth={3} />
          </button>
        </div>
      </div>
    </div>
  );
};

export const SettingsModal: React.FC = () => {
  const isSettingsOpen = useOpenZStore((s) => s.isSettingsOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);

  const wsUrl = useOpenZStore((s) => s.wsUrl);
  const wsToken = useOpenZStore((s) => s.wsToken);
  const setWsConfig = useOpenZStore((s) => s.setWsConfig);

  const settings = useOpenZStore((s) => s.settings);
  const providers = useOpenZStore((s) => s.providers);
  const providersConfig = useOpenZStore((s) => s.providersConfig);
  const channelsConfig = useOpenZStore((s) => s.channelsConfig);
  const updateConfig = useOpenZStore((s) => s.updateConfig);
  const workspaceNotice = useOpenZStore((s) => s.workspaceNotice);
  const clearWorkspaceNotice = useOpenZStore((s) => s.clearWorkspaceNotice);

  const [activeTab, setActiveTab] = useState<'agent' | 'providers' | 'channels'>('agent');
  const [urlInput, setUrlInput] = useState(wsUrl);
  const [tokenInput, setTokenInput] = useState(wsToken);

  const [form, setForm] = useState<SettingsForm>({});
  const [providersForm, setProvidersForm] = useState<ProvidersForm>({});
  const [channelsForm, setChannelsForm] = useState<ChannelsForm>({});
  const [saveNotice, setSaveNotice] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  const [newProvKey, setNewProvKey] = useState('');
  const [newProvKeyErr, setNewProvKeyErr] = useState('');
  const [showAddCustom, setShowAddCustom] = useState(false);

  useEffect(() => {
    if (isSettingsOpen) {
      setUrlInput(wsUrl);
      setTokenInput(wsToken);
      if (settings) {
        setForm({
          model: settings.model,
          provider: settings.provider,
          temperature: settings.temperature,
          max_tokens: settings.max_tokens,
          bot_name: settings.bot_name,
          max_messages: settings.max_messages,
          max_tool_iterations: settings.max_tool_iterations,
          tool_timeout_secs: settings.tool_timeout_secs,
          security_mode: settings.security_mode,
        });
      }
      if (providersConfig) {
        setProvidersForm(jsonObjectToProviders(providersConfig));
      }
      if (channelsConfig) {
        setChannelsForm(jsonObjectToChannels(channelsConfig));
      }
      setShowAddCustom(false);
      setNewProvKey('');
      setNewProvKeyErr('');
      setSaveNotice(null);
      clearWorkspaceNotice('settings');
    }
  }, [isSettingsOpen, wsUrl, wsToken, settings, providersConfig, channelsConfig, clearWorkspaceNotice]);


  const setField = (key: string, value: string | number | boolean) => {
    setForm((f) => ({ ...f, [key]: value }));
    setSaveNotice(null);
    clearWorkspaceNotice('settings');
  };

  const resetForms = () => {
    setUrlInput(wsUrl);
    setTokenInput(wsToken);
    if (settings) {
      setForm({
        model: settings.model,
        provider: settings.provider,
        temperature: settings.temperature,
        max_tokens: settings.max_tokens,
        bot_name: settings.bot_name,
        max_messages: settings.max_messages,
        max_tool_iterations: settings.max_tool_iterations,
        tool_timeout_secs: settings.tool_timeout_secs,
        security_mode: settings.security_mode,
      });
    }
    setProvidersForm(jsonObjectToProviders(providersConfig));
    setChannelsForm(jsonObjectToChannels(channelsConfig));
    setSaveNotice(null);
  };

  const validationError = useMemo(() => {
    if (!urlInput.trim()) return 'Gateway websocket URL is required.';
    if (!/^wss?:\/\//.test(urlInput.trim())) return 'Gateway URL must start with ws:// or wss://.';
    if (Number(form.temperature ?? 0) < 0 || Number(form.temperature ?? 0) > 2) return 'Temperature must be between 0 and 2.';
    if (Number(form.max_tokens ?? 1) < 1) return 'Max tokens must be at least 1.';
    if (Number(form.max_messages ?? 1) < 1) return 'Max messages must be at least 1.';
    if (Number(form.max_tool_iterations ?? 1) < 1) return 'Max tool iterations must be at least 1.';
    if (Number(form.tool_timeout_secs ?? 1) < 1) return 'Tool timeout must be at least 1 second.';
    const whatsappPort = channelsForm.whatsapp?.webhook_port;
    if (whatsappPort !== undefined && (Number(whatsappPort) < 1 || Number(whatsappPort) > 65535)) return 'WhatsApp webhook port must be 1-65535.';
    return null;
  }, [channelsForm.whatsapp?.webhook_port, form.max_messages, form.max_tokens, form.max_tool_iterations, form.temperature, form.tool_timeout_secs, urlInput]);

  const hasChanges = useMemo(() => {
    const defaultsChanged = settings ? (
      form.model !== settings.model ||
      form.provider !== settings.provider ||
      Number(form.temperature) !== settings.temperature ||
      Number(form.max_tokens) !== settings.max_tokens ||
      form.bot_name !== settings.bot_name ||
      Number(form.max_messages) !== settings.max_messages ||
      Number(form.max_tool_iterations) !== settings.max_tool_iterations ||
      Number(form.tool_timeout_secs) !== settings.tool_timeout_secs ||
      form.security_mode !== settings.security_mode
    ) : false;
    return urlInput !== wsUrl ||
      tokenInput !== wsToken ||
      defaultsChanged ||
      stableStringify(providersForm) !== stableStringify(providersConfig) ||
      stableStringify(channelsForm) !== stableStringify(channelsConfig);
  }, [channelsConfig, channelsForm, form, providersConfig, providersForm, settings, tokenInput, urlInput, wsToken, wsUrl]);

  const pageNotice = workspaceNotice?.scope === 'settings' ? workspaceNotice : saveNotice;
  const visibleNotice = validationError ? { type: 'error' as const, message: validationError } : pageNotice;

  const handleSave = () => {
    if (validationError) {
      setSaveNotice({ type: 'error', message: validationError });
      return;
    }

    setWsConfig(urlInput.trim(), tokenInput);

    const defaultsPatch: Record<string, unknown> = {};
    if (settings) {
      if (form.model !== undefined && form.model !== settings.model) defaultsPatch.model = form.model;
      if (form.provider !== undefined && form.provider !== settings.provider) defaultsPatch.provider = form.provider;
      if (form.temperature !== undefined && Number(form.temperature) !== settings.temperature) defaultsPatch.temperature = Number(form.temperature);
      if (form.max_tokens !== undefined && Number(form.max_tokens) !== settings.max_tokens) defaultsPatch.max_tokens = Number(form.max_tokens);
      if (form.bot_name !== undefined && form.bot_name !== settings.bot_name) defaultsPatch.bot_name = form.bot_name;
      if (form.max_messages !== undefined && Number(form.max_messages) !== settings.max_messages) defaultsPatch.max_messages = Number(form.max_messages);
      if (form.max_tool_iterations !== undefined && Number(form.max_tool_iterations) !== settings.max_tool_iterations) defaultsPatch.max_tool_iterations = Number(form.max_tool_iterations);
      if (form.tool_timeout_secs !== undefined && Number(form.tool_timeout_secs) !== settings.tool_timeout_secs) defaultsPatch.tool_timeout_secs = Number(form.tool_timeout_secs);
      if (form.security_mode !== undefined && form.security_mode !== settings.security_mode) defaultsPatch.security_mode = form.security_mode;
    }

    updateConfig({
      defaults: defaultsPatch,
      providers: providersForm as unknown as JsonObject,
      channels: channelsForm as unknown as JsonObject,
    });

    setSaveNotice({ type: 'success', message: 'Settings save requested. Waiting for gateway refresh.' });
  };

  const groups = providers.filter((p) => p.models.length > 0);

  const customModelGroups = groups.map((g) => ({
    label: g.display || g.name,
    options: g.models.map((m) => ({ value: m, label: m })),
  }));
  const finalModelGroups = customModelGroups.length > 0
    ? customModelGroups
    : settings ? [{ label: 'Default', options: [{ value: settings.model, label: settings.model }] }] : [];

  const providerOptions = settings ? [
    { value: settings.provider, label: settings.provider },
    ...providers.map((p) => ({ value: p.name, label: p.display || p.name })),
  ] : [];
  const uniqueProviders = Array.from(new Map(providerOptions.map(item => [item.value, item])).values());

  const builtins = ['openai', 'anthropic', 'deepseek', 'groq', 'openrouter', 'google_ai_studio', 'ollama'];
  const customKeys = Object.keys(providersForm).filter(k => !builtins.includes(k));
  const allProviderKeys = [...builtins, ...customKeys];

  if (!isSettingsOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-150">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card shadow-2xl animate-in zoom-in-95 duration-150 max-h-[85vh] flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-6 pb-4 border-b border-border/50 shrink-0">
          <div className="flex items-center gap-2 text-foreground font-semibold text-base select-none">
            <Settings className="h-5 w-5 text-amber-500" /> Gateway Configuration UI
          </div>
          <button
            onClick={() => setIsSettingsOpen(false)}
            className="rounded-lg p-1 text-muted-foreground hover:text-foreground hover:bg-muted"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Tab Selection */}
        <div className="flex border-b border-border/40 px-6 shrink-0 bg-muted/10 text-xs">
          <button
            onClick={() => setActiveTab('agent')}
            className={`py-3 px-4 font-semibold border-b-2 transition-all ${
              activeTab === 'agent'
                ? 'border-amber-500 text-amber-500 bg-muted/20'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            Agent & Gateway
          </button>
          <button
            onClick={() => setActiveTab('providers')}
            className={`py-3 px-4 font-semibold border-b-2 transition-all ${
              activeTab === 'providers'
                ? 'border-amber-500 text-amber-500 bg-muted/20'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            LLM Providers
          </button>
          <button
            onClick={() => setActiveTab('channels')}
            className={`py-3 px-4 font-semibold border-b-2 transition-all ${
              activeTab === 'channels'
                ? 'border-amber-500 text-amber-500 bg-muted/20'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            Bot Channels
          </button>
        </div>

        {visibleNotice && (
          <div className={`mx-6 mt-4 flex items-start gap-2 rounded-lg border px-3 py-2 text-[11px] ${visibleNotice.type === 'success' ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300' : visibleNotice.type === 'error' ? 'border-red-500/30 bg-red-500/10 text-red-300' : 'border-amber-500/30 bg-amber-500/10 text-amber-300'}`}>
            {visibleNotice.type === 'success' ? <CheckCircle2 className="mt-0.5 h-3.5 w-3.5 shrink-0" /> : <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />}
            <span>{visibleNotice.message}</span>
          </div>
        )}

        {/* Scrollable Body */}
        <div className="flex-1 overflow-y-auto p-6 space-y-5 text-xs">
          {activeTab === 'agent' && (
            <>
              {/* WebSocket Server Endpoint */}
              <div>
                <label className="mb-1 block font-medium text-foreground flex items-center gap-1.5 select-none">
                  <Link className="h-3.5 w-3.5 text-amber-500" /> OpenZ Gateway WebSocket URL
                </label>
                <input
                  type="text"
                  value={urlInput}
                  onChange={(e) => setUrlInput(e.target.value)}
                  placeholder="ws://127.0.0.1:8765/ws"
                  className="w-full rounded-lg border border-border bg-muted/40 p-2.5 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                />
              </div>

              {/* Gateway Token */}
              <div>
                <label className="mb-1 block font-medium text-foreground flex items-center gap-1.5 select-none">
                  <Key className="h-3.5 w-3.5 text-amber-500" /> Gateway Authorization Token (OPENZ_GATEWAY_TOKEN)
                </label>
                <input
                  type="password"
                  value={tokenInput}
                  onChange={(e) => setTokenInput(e.target.value)}
                  placeholder="Leave empty if token auth is disabled"
                  className="w-full rounded-lg border border-border bg-muted/40 p-2.5 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                />
              </div>

              {/* Real editable agent defaults */}
              {settings ? (
                <>
                  <div className="pt-1">
                    <div className="mb-3 flex items-center gap-1.5 font-semibold text-foreground border-b border-border/40 pb-1.5 select-none">
                      <Cpu className="h-3.5 w-3.5 text-amber-500" /> Agent Defaults
                    </div>
                    <div className="space-y-3">
                      <CustomSelect
                        label="Default Model"
                        value={String(form.model ?? settings.model)}
                        onChange={(val) => {
                          setField('model', val);
                          const group = groups.find((g) => g.models.includes(val));
                          if (group) {
                            setField('provider', group.name);
                          }
                        }}
                        groups={finalModelGroups}
                      />

                      <CustomSelect
                        label="Default Provider"
                        value={String(form.provider ?? settings.provider)}
                        onChange={(val) => setField('provider', val)}
                        options={uniqueProviders}
                      />

                      <div className="grid grid-cols-2 gap-3">
                        <NumberInput
                          label="Temperature"
                          step={0.1}
                          min={0}
                          max={2}
                          value={form.temperature !== undefined ? Math.round(Number(form.temperature) * 100) / 100 : Math.round(Number(settings.temperature) * 100) / 100}
                          onChange={(val) => setField('temperature', val)}
                        />
                        <NumberInput
                          label="Max Tokens"
                          min={1}
                          step={1}
                          value={Number(form.max_tokens ?? settings.max_tokens)}
                          onChange={(val) => setField('max_tokens', val)}
                        />
                        <NumberInput
                          label="Max Messages"
                          min={1}
                          step={1}
                          value={Number(form.max_messages ?? settings.max_messages)}
                          onChange={(val) => setField('max_messages', val)}
                        />
                        <NumberInput
                          label="Max Tool Iterations"
                          min={1}
                          step={1}
                          value={Number(form.max_tool_iterations ?? settings.max_tool_iterations)}
                          onChange={(val) => setField('max_tool_iterations', val)}
                        />
                        <NumberInput
                          label="Tool Timeout (sec)"
                          min={1}
                          step={1}
                          value={Number(form.tool_timeout_secs ?? settings.tool_timeout_secs)}
                          onChange={(val) => setField('tool_timeout_secs', val)}
                        />
                        <CustomSelect
                          label="Security Mode"
                          value={String(form.security_mode ?? settings.security_mode)}
                          onChange={(val) => setField('security_mode', val)}
                          options={[
                            { value: 'strict', label: 'strict' },
                            { value: 'moderate', label: 'moderate' },
                            { value: 'permissive', label: 'permissive' },
                          ]}
                        />
                      </div>

                      <div>
                        <label className="mb-1 block font-medium text-foreground">Bot Name</label>
                        <input
                          type="text"
                          value={String(form.bot_name ?? settings.bot_name)}
                          onChange={(e) => setField('bot_name', e.target.value)}
                          className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                        />
                      </div>
                    </div>
                  </div>

                  {/* Behavior toggles */}
                  <div className="pt-2 space-y-3">
                    <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 p-3">
                      <div>
                        <div className="font-semibold text-foreground flex items-center gap-1.5 select-none">
                          <Zap className="h-3.5 w-3.5 text-amber-500" /> Caveman Terseness Mode
                        </div>
                        <div className="text-[11px] text-muted-foreground mt-0.5">
                          Strips filler words and articles for maximum speed
                        </div>
                      </div>
                      <button
                        onClick={() =>
                          updateConfig({ defaults: { caveman_mode: !settings.caveman_mode } })
                        }
                        aria-pressed={settings.caveman_mode}
                        className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                          settings.caveman_mode ? 'bg-amber-500' : 'bg-muted'
                        }`}
                      >
                        <span
                          className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
                            settings.caveman_mode ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
                      </button>
                    </div>

                    <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 p-3">
                      <div>
                        <div className="font-semibold text-foreground flex items-center gap-1.5 select-none">
                          <Radio className="h-3.5 w-3.5 text-amber-500" /> Real-Time Response Streaming
                        </div>
                        <div className="text-[11px] text-muted-foreground mt-0.5">
                          Stream token deltas as they are generated by LLM
                        </div>
                      </div>
                      <button
                        onClick={() => updateConfig({ defaults: { streaming: !settings.streaming } })}
                        aria-pressed={settings.streaming}
                        className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                          settings.streaming ? 'bg-amber-500' : 'bg-muted'
                        }`}
                      >
                        <span
                          className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
                            settings.streaming ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
                      </button>
                    </div>

                    <div className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/20 p-3">
                      <div>
                        <div className="font-semibold text-foreground flex items-center gap-1.5 select-none">
                          <ShieldAlert className="h-3.5 w-3.5 text-amber-500" /> Linux seccomp BPF Sandbox
                        </div>
                        <div className="text-[11px] text-muted-foreground mt-0.5">
                          Restricts subprocess syscalls to block raw network access and loading modules
                        </div>
                      </div>
                      <button
                        onClick={() => updateConfig({ defaults: { enable_sandbox: !settings.enable_sandbox } })}
                        aria-pressed={settings.enable_sandbox}
                        className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                          settings.enable_sandbox ? 'bg-amber-500' : 'bg-muted'
                        }`}
                      >
                        <span
                          className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
                            settings.enable_sandbox ? 'translate-x-5' : 'translate-x-0'
                          }`}
                        />
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
          )}

          {activeTab === 'providers' && (
            <div className="space-y-6 animate-in fade-in duration-150">
              <div className="text-muted-foreground mb-2 text-[11px] leading-relaxed select-none">
                Configure API keys and model endpoints. Masked entries (••••••••) mean a key is stored. Overwrite them to edit.
              </div>
              {allProviderKeys.map((provKey) => {
                const provData = providersForm[provKey] || {};
                const setProvField = (fld: keyof ProviderForm, val: string) => {
                  setSaveNotice(null);
                  setProvidersForm((pf: ProvidersForm) => ({
                    ...pf,
                    [provKey]: {
                      ...provData,
                      [fld]: val,
                    },
                  }));
                };
                const label = provKey === 'google_ai_studio' ? 'Google AI Studio' : provKey.toUpperCase();
                const isCustom = !builtins.includes(provKey);

                return (
                  <div key={provKey} className="rounded-xl border border-border/50 bg-muted/15 p-4 space-y-3 relative">
                    <div className="flex items-center justify-between border-b border-border/30 pb-1.5">
                      <div className="font-semibold text-foreground capitalize select-none flex items-center gap-1.5">
                        {label} Setup {isCustom && <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[9px] font-bold text-amber-500 select-none">Custom</span>}
                      </div>
                      {isCustom && (
                        <button
                          onClick={() => {
                            setProvidersForm((pf: ProvidersForm) => {
                              const copy = { ...pf };
                              delete copy[provKey];
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
                          value={provData.api_key || ''}
                          onChange={(e) => setProvField('api_key', e.target.value)}
                          placeholder={provData.api_key === '••••••••' ? '••••••••' : 'api_key_here'}
                          className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                        />
                      </div>
                      {(provKey !== 'anthropic' && provKey !== 'google_ai_studio') && (
                        <div>
                          <label className="mb-1 block font-medium text-muted-foreground select-none">API Base Endpoint</label>
                          <input
                            type="text"
                            value={provData.api_base || ''}
                            onChange={(e) => setProvField('api_base', e.target.value)}
                            placeholder={isCustom ? "http://127.0.0.1:8080/v1" : `https://api.${provKey}.com/v1`}
                            className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                          />
                        </div>
                      )}
                      <div>
                        <label className="mb-1 block font-medium text-muted-foreground select-none">Preferred Default Model</label>
                        <input
                          type="text"
                          value={provData.default_model || ''}
                          onChange={(e) => setProvField('default_model', e.target.value)}
                          placeholder="e.g. gpt-4o, claude-3-5-sonnet"
                          className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                        />
                      </div>
                    </div>
                  </div>
                );
              })}

              {/* Add Custom Provider Form */}
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
                        value={newProvKey}
                        onChange={(e) => {
                          setNewProvKey(e.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, ''));
                          setNewProvKeyErr('');
                        }}
                        placeholder="my-custom-provider"
                        className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                      {newProvKeyErr && (
                        <span className="text-red-500 text-[10px] mt-0.5 block">{newProvKeyErr}</span>
                      )}
                    </div>
                    <button
                      onClick={() => {
                        if (!newProvKey.trim()) {
                          setNewProvKeyErr('Provider key cannot be empty.');
                          return;
                        }
                        if (allProviderKeys.includes(newProvKey)) {
                          setNewProvKeyErr('This provider key already exists.');
                          return;
                        }
                        setProvidersForm((pf: ProvidersForm) => ({
                          ...pf,
                          [newProvKey]: { api_key: '', api_base: '', default_model: '' }
                        }));
                        setNewProvKey('');
                        setShowAddCustom(false);
                      }}
                      className="w-full rounded-lg bg-amber-500/10 hover:bg-amber-500/20 text-amber-500 font-semibold py-2.5 text-xs transition duration-150"
                    >
                      Confirm Add Provider
                    </button>
                    <button
                      onClick={() => {
                        setShowAddCustom(false);
                        setNewProvKeyErr('');
                      }}
                      className="w-full text-center text-muted-foreground hover:text-foreground text-[10px] pt-1 transition"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  onClick={() => setShowAddCustom(true)}
                  className="w-full flex items-center justify-center gap-1.5 rounded-xl border border-dashed border-border hover:border-amber-500/40 bg-muted/10 hover:bg-muted/30 py-3.5 text-xs font-semibold text-muted-foreground hover:text-foreground transition duration-150"
                >
                  + Add Custom LLM Provider Endpoint
                </button>
              )}
            </div>
          )}

          {activeTab === 'channels' && (
            <div className="space-y-6">
              <div className="text-muted-foreground mb-2 text-[11px] leading-relaxed select-none">
                Enable external channels and background listeners. Modifying keys/tokens requires a daemon restart to re-init connections.
              </div>

              {/* Telegram Bot */}
              <div className="rounded-xl border border-border/50 bg-muted/15 p-4 space-y-3">
                <div className="flex items-center justify-between border-b border-border/30 pb-1.5">
                  <div className="font-semibold text-foreground select-none">Telegram Bot Listener</div>
                  <button
                    onClick={() => {
                      const tg = channelsForm.telegram || { enabled: false, bot_token: '' };
                      setChannelsForm((cf: ChannelsForm) => ({
                        ...cf,
                        telegram: { ...tg, enabled: !tg.enabled },
                      }));
                    }}
                    className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                      channelsForm.telegram?.enabled ? 'bg-amber-500' : 'bg-muted'
                    }`}
                  >
                    <span
                      className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
                        channelsForm.telegram?.enabled ? 'translate-x-4' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
                <div className="space-y-2.5">
                  <div>
                    <label className="mb-1 block font-medium text-muted-foreground select-none">Bot API Token</label>
                    <input
                      type="password"
                      value={channelsForm.telegram?.bot_token || ''}
                      onChange={(e) => {
                        const tg = channelsForm.telegram || { enabled: false, bot_token: '' };
                        setChannelsForm((cf: ChannelsForm) => ({
                          ...cf,
                          telegram: { ...tg, bot_token: e.target.value },
                        }));
                      }}
                      placeholder={channelsForm.telegram?.bot_token === '••••••••' ? '••••••••' : 'bot_token'}
                      className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                    />
                  </div>
                </div>
              </div>

              {/* Discord Bot */}
              <div className="rounded-xl border border-border/50 bg-muted/15 p-4 space-y-3">
                <div className="flex items-center justify-between border-b border-border/30 pb-1.5">
                  <div className="font-semibold text-foreground select-none">Discord Bot Gateway</div>
                  <button
                    onClick={() => {
                      const dc = channelsForm.discord || { enabled: false, bot_token: '' };
                      setChannelsForm((cf: ChannelsForm) => ({
                        ...cf,
                        discord: { ...dc, enabled: !dc.enabled },
                      }));
                    }}
                    className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                      channelsForm.discord?.enabled ? 'bg-amber-500' : 'bg-muted'
                    }`}
                  >
                    <span
                      className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
                        channelsForm.discord?.enabled ? 'translate-x-4' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
                <div className="space-y-2.5">
                  <div>
                    <label className="mb-1 block font-medium text-muted-foreground select-none">Bot Token</label>
                    <input
                      type="password"
                      value={channelsForm.discord?.bot_token || ''}
                      onChange={(e) => {
                        const dc = channelsForm.discord || { enabled: false, bot_token: '' };
                        setChannelsForm((cf: ChannelsForm) => ({
                          ...cf,
                          discord: { ...dc, bot_token: e.target.value },
                        }));
                      }}
                      placeholder={channelsForm.discord?.bot_token === '••••••••' ? '••••••••' : 'bot_token'}
                      className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                    />
                  </div>
                </div>
              </div>

              {/* WhatsApp Business API */}
              <div className="rounded-xl border border-border/50 bg-muted/15 p-4 space-y-3">
                <div className="flex items-center justify-between border-b border-border/30 pb-1.5">
                  <div className="font-semibold text-foreground select-none">WhatsApp Webhook Receiver</div>
                  <button
                    onClick={() => {
                      const wa = channelsForm.whatsapp || { enabled: false, api_key: '', phone_number_id: '', webhook_port: 8090, verify_token: 'openz' };
                      setChannelsForm((cf: ChannelsForm) => ({
                        ...cf,
                        whatsapp: { ...wa, enabled: !wa.enabled },
                      }));
                    }}
                    className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                      channelsForm.whatsapp?.enabled ? 'bg-amber-500' : 'bg-muted'
                    }`}
                  >
                    <span
                      className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
                        channelsForm.whatsapp?.enabled ? 'translate-x-4' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
                <div className="space-y-2.5">
                  <div>
                    <label className="mb-1 block font-medium text-muted-foreground select-none">API Key</label>
                    <input
                      type="password"
                      value={channelsForm.whatsapp?.api_key || ''}
                      onChange={(e) => {
                        const wa = channelsForm.whatsapp || { enabled: false, api_key: '', phone_number_id: '', webhook_port: 8090, verify_token: 'openz' };
                        setChannelsForm((cf: ChannelsForm) => ({
                          ...cf,
                          whatsapp: { ...wa, api_key: e.target.value },
                        }));
                      }}
                      placeholder={channelsForm.whatsapp?.api_key === '••••••••' ? '••••••••' : 'api_key'}
                      className="w-full rounded-lg border border-border bg-muted/40 p-2 font-mono text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                    />
                  </div>
                  <div>
                    <label className="mb-1 block font-medium text-muted-foreground select-none">Phone Number ID</label>
                    <input
                      type="text"
                      value={channelsForm.whatsapp?.phone_number_id || ''}
                      onChange={(e) => {
                        const wa = channelsForm.whatsapp || { enabled: false, api_key: '', phone_number_id: '', webhook_port: 8090, verify_token: 'openz' };
                        setChannelsForm((cf: ChannelsForm) => ({
                          ...cf,
                          whatsapp: { ...wa, phone_number_id: e.target.value },
                        }));
                      }}
                      placeholder="Phone number ID"
                      className="w-full rounded-lg border border-border bg-muted/40 p-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="mb-1 block font-medium text-muted-foreground select-none">Webhook Port</label>
                      <input
                        type="number"
                        value={channelsForm.whatsapp?.webhook_port ?? 8090}
                        onChange={(e) => {
                          const wa = channelsForm.whatsapp || { enabled: false, api_key: '', phone_number_id: '', webhook_port: 8090, verify_token: 'openz' };
                          setChannelsForm((cf: ChannelsForm) => ({
                            ...cf,
                            whatsapp: { ...wa, webhook_port: Number(e.target.value) },
                          }));
                        }}
                        className="w-full rounded-lg border border-border bg-muted/40 p-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-muted-foreground select-none">Verify Token</label>
                      <input
                        type="text"
                        value={channelsForm.whatsapp?.verify_token || ''}
                        onChange={(e) => {
                          const wa = channelsForm.whatsapp || { enabled: false, api_key: '', phone_number_id: '', webhook_port: 8090, verify_token: 'openz' };
                          setChannelsForm((cf: ChannelsForm) => ({
                            ...cf,
                            whatsapp: { ...wa, verify_token: e.target.value },
                          }));
                        }}
                        placeholder="openz"
                        className="w-full rounded-lg border border-border bg-muted/40 p-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer Buttons */}
        <div className="flex flex-wrap justify-end gap-2.5 p-6 pt-4 border-t border-border/50 bg-muted/20 shrink-0">
          <button
            onClick={resetForms}
            disabled={!hasChanges}
            className="flex items-center gap-1.5 rounded-lg border border-border px-4 py-2 text-xs font-semibold text-muted-foreground hover:bg-muted disabled:opacity-40"
          >
            <RotateCcw className="h-3.5 w-3.5" /> Reset
          </button>
          <button
            onClick={() => setIsSettingsOpen(false)}
            className="rounded-lg border border-border px-4 py-2 text-xs font-semibold text-muted-foreground hover:bg-muted"
          >
            Close
          </button>
          <button
            onClick={handleSave}
            disabled={!hasChanges || Boolean(validationError)}
            className="flex items-center gap-1.5 rounded-lg bg-gradient-to-r from-amber-500 to-orange-500 px-4 py-2 text-xs font-semibold text-white shadow-md hover:opacity-90 transition duration-150 active:scale-95 disabled:opacity-40"
          >
            <Save className="h-3.5 w-3.5" /> Save & Apply
          </button>
        </div>
      </div>
    </div>
  );
};
