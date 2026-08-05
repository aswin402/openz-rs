import React, { useState, useEffect, useRef } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, Settings, Link, Key, Zap, Radio, Save, Cpu, ShieldAlert, ChevronUp, ChevronDown } from 'lucide-react';

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
  const updateSettings = useOpenZStore((s) => s.updateSettings);

  const [urlInput, setUrlInput] = useState(wsUrl);
  const [tokenInput, setTokenInput] = useState(wsToken);
  const [form, setForm] = useState<Record<string, string | number | boolean>>({});

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
    }
  }, [isSettingsOpen, wsUrl, wsToken, settings]);

  if (!isSettingsOpen) return null;

  const setField = (key: string, value: string | number | boolean) => {
    setForm((f) => ({ ...f, [key]: value }));
  };

  const handleSave = () => {
    setWsConfig(urlInput, tokenInput);
    // Push editable fields back to the backend (persisted to config.json).
    const patch: Record<string, unknown> = {};
    if (form.model !== undefined && form.model !== settings?.model) patch.model = form.model;
    if (form.provider !== undefined && form.provider !== settings?.provider) patch.provider = form.provider;
    if (form.temperature !== undefined && Number(form.temperature) !== settings?.temperature)
      patch.temperature = Number(form.temperature);
    if (form.max_tokens !== undefined && Number(form.max_tokens) !== settings?.max_tokens)
      patch.max_tokens = Number(form.max_tokens);
    if (form.bot_name !== undefined && form.bot_name !== settings?.bot_name) patch.bot_name = form.bot_name;
    if (form.max_messages !== undefined && Number(form.max_messages) !== settings?.max_messages)
      patch.max_messages = Number(form.max_messages);
    if (form.max_tool_iterations !== undefined && Number(form.max_tool_iterations) !== settings?.max_tool_iterations)
      patch.max_tool_iterations = Number(form.max_tool_iterations);
    if (form.tool_timeout_secs !== undefined && Number(form.tool_timeout_secs) !== settings?.tool_timeout_secs)
      patch.tool_timeout_secs = Number(form.tool_timeout_secs);
    if (form.security_mode !== undefined && form.security_mode !== settings?.security_mode)
      patch.security_mode = form.security_mode;
    if (Object.keys(patch).length > 0) updateSettings(patch);
    setIsSettingsOpen(false);
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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card shadow-2xl animate-in fade-in zoom-in-95 duration-150 max-h-[85vh] flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-6 pb-4 border-b border-border/50 shrink-0">
          <div className="flex items-center gap-2 text-foreground font-semibold text-base">
            <Settings className="h-5 w-5 text-amber-500" /> Gateway & Agent Settings
          </div>
          <button
            onClick={() => setIsSettingsOpen(false)}
            className="rounded-lg p-1 text-muted-foreground hover:text-foreground hover:bg-muted"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Scrollable Body */}
        <div className="flex-1 overflow-y-auto p-6 space-y-5 text-xs">
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

          {/* Real editable agent defaults (from get_config) */}
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
                    onChange={(val) => setField('model', val)}
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

              {/* Behavior toggles bound to real config */}
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
                      updateSettings({ caveman_mode: !settings.caveman_mode })
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
                    onClick={() => updateSettings({ streaming: !settings.streaming })}
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
                    onClick={() => updateSettings({ enable_sandbox: !settings.enable_sandbox })}
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
        </div>

        {/* Footer Buttons */}
        <div className="flex justify-end gap-2.5 p-6 pt-4 border-t border-border/50 bg-muted/20 shrink-0">
          <button
            onClick={() => setIsSettingsOpen(false)}
            className="rounded-lg border border-border px-4 py-2 text-xs font-semibold text-muted-foreground hover:bg-muted"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="flex items-center gap-1.5 rounded-lg bg-gradient-to-r from-amber-500 to-orange-500 px-4 py-2 text-xs font-semibold text-white shadow-md hover:opacity-90 transition duration-150 active:scale-95"
          >
            <Save className="h-3.5 w-3.5" /> Save & Apply
          </button>
        </div>
      </div>
    </div>
  );
};
