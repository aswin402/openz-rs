import React, { useState, useEffect } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, Settings, Link, Key, Zap, Radio, Save, Cpu, ShieldAlert } from 'lucide-react';

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
                  <div>
                    <label className="mb-1 block font-medium text-foreground">Default Model</label>
                    <select
                      value={String(form.model ?? settings.model)}
                      onChange={(e) => setField('model', e.target.value)}
                      className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                    >
                      {groups.length === 0 && <option value={settings.model}>{settings.model}</option>}
                      {groups.map((g) => (
                        <optgroup key={g.name} label={g.display || g.name}>
                          {g.models.map((m) => (
                            <option key={m} value={m}>
                              {m}
                            </option>
                          ))}
                        </optgroup>
                      ))}
                    </select>
                  </div>

                  <div>
                    <label className="mb-1 block font-medium text-foreground">Default Provider</label>
                    <select
                      value={String(form.provider ?? settings.provider)}
                      onChange={(e) => setField('provider', e.target.value)}
                      className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                    >
                      <option value={settings.provider}>{settings.provider}</option>
                      {providers.map((p) => (
                        <option key={p.name} value={p.name}>
                          {p.display || p.name}
                        </option>
                      ))}
                    </select>
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="mb-1 block font-medium text-foreground">Temperature</label>
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="2"
                        value={form.temperature !== undefined ? Math.round(Number(form.temperature) * 100) / 100 : Math.round(Number(settings.temperature) * 100) / 100}
                        onChange={(e) => setField('temperature', Number(e.target.value))}
                        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-foreground">Max Tokens</label>
                      <input
                        type="number"
                        min="1"
                        value={Number(form.max_tokens ?? settings.max_tokens)}
                        onChange={(e) => setField('max_tokens', Number(e.target.value))}
                        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-foreground">Max Messages</label>
                      <input
                        type="number"
                        min="1"
                        value={Number(form.max_messages ?? settings.max_messages)}
                        onChange={(e) => setField('max_messages', Number(e.target.value))}
                        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-foreground">Max Tool Iterations</label>
                      <input
                        type="number"
                        min="1"
                        value={Number(form.max_tool_iterations ?? settings.max_tool_iterations)}
                        onChange={(e) => setField('max_tool_iterations', Number(e.target.value))}
                        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-foreground">Tool Timeout (sec)</label>
                      <input
                        type="number"
                        min="1"
                        value={Number(form.tool_timeout_secs ?? settings.tool_timeout_secs)}
                        onChange={(e) => setField('tool_timeout_secs', Number(e.target.value))}
                        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block font-medium text-foreground">Security Mode</label>
                      <select
                        value={String(form.security_mode ?? settings.security_mode)}
                        onChange={(e) => setField('security_mode', e.target.value)}
                        className="w-full rounded-lg border border-border bg-muted/40 p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500"
                      >
                        <option value="strict">strict</option>
                        <option value="moderate">moderate</option>
                        <option value="permissive">permissive</option>
                      </select>
                    </div>
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
