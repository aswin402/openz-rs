/* eslint-disable react-hooks/set-state-in-effect */
import React, { useState, useEffect, useMemo } from 'react';
import { useOpenZStore } from '../../store/useOpenZStore';
import type { JsonObject } from '../../types';
import { resolveProviderDescriptors } from '../../config/providers';
import {
  jsonObjectToChannels,
  jsonObjectToProviders,
  stableStringify,
  type ChannelsForm,
  type ProvidersForm,
  type SettingsForm,
} from './settingsSchema';
import { AgentSettingsTab } from './AgentSettingsTab';
import { ChannelsTab } from './ChannelsTab';
import { ProvidersTab } from './ProvidersTab';
import { X, Settings, RotateCcw, CheckCircle2, AlertCircle, Save } from 'lucide-react';

export const SettingsModal: React.FC = () => {
  const isSettingsOpen = useOpenZStore((s) => s.isSettingsOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);

  const wsUrl = useOpenZStore((s) => s.wsUrl);
  const wsToken = useOpenZStore((s) => s.wsToken);
  const setWsConfig = useOpenZStore((s) => s.setWsConfig);

  const settings = useOpenZStore((s) => s.settings);
  const capabilities = useOpenZStore((s) => s.capabilities);
  const channelCapabilities = capabilities.channels;
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
          workspace: settings.workspace,
          context_limit: settings.context_limit ?? '',
          tool_output_limit: settings.tool_output_limit ?? '',
          show_auto_capture_notices: settings.show_auto_capture_notices ?? true,
          tui_thought_display: settings.tui_thought_display ?? 'auto',
          firefox_webdriver_port: settings.firefox_webdriver_port,
          firefox_attach_port: settings.firefox_attach_port,
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
        workspace: settings.workspace,
        context_limit: settings.context_limit ?? '',
        tool_output_limit: settings.tool_output_limit ?? '',
        show_auto_capture_notices: settings.show_auto_capture_notices ?? true,
        tui_thought_display: settings.tui_thought_display ?? 'auto',
        firefox_webdriver_port: settings.firefox_webdriver_port,
        firefox_attach_port: settings.firefox_attach_port,
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
    const firefoxWebdriverPort = Number(form.firefox_webdriver_port ?? settings?.firefox_webdriver_port ?? capabilities.browser.firefoxWebdriverPort);
    const firefoxAttachPort = Number(form.firefox_attach_port ?? settings?.firefox_attach_port ?? capabilities.browser.firefoxAttachPort);
    if (firefoxWebdriverPort < 1024 || firefoxWebdriverPort > 65535) return "Firefox WebDriver port must be 1024-65535.";
    if (firefoxAttachPort < 1024 || firefoxAttachPort > 65535) return "Firefox attach port must be 1024-65535.";
    if (Number(form.max_tool_iterations ?? 1) < 1) return 'Max tool iterations must be at least 1.';
    if (Number(form.tool_timeout_secs ?? 1) < 1) return 'Tool timeout must be at least 1 second.';
    if (form.context_limit !== '' && form.context_limit !== undefined && Number(form.context_limit) < 1) return 'Context limit must be blank or at least 1.';
    if (form.tool_output_limit !== '' && form.tool_output_limit !== undefined && Number(form.tool_output_limit) < 1) return 'Tool output limit must be blank or at least 1.';
    for (const channel of channelCapabilities) {
      const values = channelsForm[channel.name] || {};
      for (const field of channel.fields) {
        if (field.kind !== 'number') continue;
        const raw = values[field.key] ?? channel.defaults[field.key];
        if (raw === undefined || raw === '') continue;
        const number = Number(raw);
        if (!Number.isFinite(number) || number < 1 || number > 65535) {
          return channel.label + ' ' + field.label + ' must be 1-65535.';
        }
      }
    }
    return null;
  }, [channelCapabilities, channelsForm, form.context_limit, form.max_messages, form.max_tokens, form.max_tool_iterations, form.temperature, form.tool_output_limit, form.tool_timeout_secs, form.firefox_attach_port, form.firefox_webdriver_port, urlInput]);

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
      form.security_mode !== settings.security_mode ||
      form.workspace !== settings.workspace ||
      String(form.context_limit ?? '') !== String(settings.context_limit ?? '') ||
      String(form.tool_output_limit ?? '') !== String(settings.tool_output_limit ?? '') ||
      Boolean(form.show_auto_capture_notices ?? true) !== Boolean(settings.show_auto_capture_notices ?? true) ||
      form.tui_thought_display !== (settings.tui_thought_display ?? 'auto') ||
      Number(form.firefox_webdriver_port) !== settings.firefox_webdriver_port ||
      Number(form.firefox_attach_port) !== settings.firefox_attach_port
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
      if (form.workspace !== undefined && form.workspace !== settings.workspace) defaultsPatch.workspace = form.workspace;
      if (form.context_limit !== undefined && String(form.context_limit) !== String(settings.context_limit ?? '')) defaultsPatch.context_limit = form.context_limit === '' ? null : Number(form.context_limit);
      if (form.tool_output_limit !== undefined && String(form.tool_output_limit) !== String(settings.tool_output_limit ?? '')) defaultsPatch.tool_output_limit = form.tool_output_limit === '' ? null : Number(form.tool_output_limit);
      if (form.show_auto_capture_notices !== undefined && Boolean(form.show_auto_capture_notices) !== Boolean(settings.show_auto_capture_notices ?? true)) defaultsPatch.show_auto_capture_notices = Boolean(form.show_auto_capture_notices);
      if (form.tui_thought_display !== undefined && form.tui_thought_display !== (settings.tui_thought_display ?? 'auto')) defaultsPatch.tui_thought_display = form.tui_thought_display;
      if (form.firefox_webdriver_port !== undefined && Number(form.firefox_webdriver_port) !== settings.firefox_webdriver_port) defaultsPatch.firefox_webdriver_port = Number(form.firefox_webdriver_port);
      if (form.firefox_attach_port !== undefined && Number(form.firefox_attach_port) !== settings.firefox_attach_port) defaultsPatch.firefox_attach_port = Number(form.firefox_attach_port);
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
    label: g.available === false ? `${g.display || g.name} (not configured)` : g.display || g.name,
    options: g.models.map((m) => ({ value: `${g.name}::${m}`, label: m })),
  }));
  const finalModelGroups = customModelGroups.length > 0
    ? customModelGroups
    : settings ? [{ label: 'Default', options: [{ value: settings.model, label: settings.model }] }] : [];

  const providerDescriptors = resolveProviderDescriptors(capabilities, providersForm);
  const providerOptions = settings ? [
    {
      value: settings.provider,
      label: providerDescriptors.find((provider) => provider.key === settings.provider)?.display || settings.provider,
    },
    ...providerDescriptors.map((provider) => ({ value: provider.key, label: provider.display })),
  ] : [];
  const uniqueProviders = Array.from(new Map(providerOptions.map(item => [item.value, item])).values());

  const providerCapabilities = capabilities.providers;
  const providerCapabilityByKey = new Map(providerCapabilities.map((provider) => [provider.configKey, provider]));
  const allProviderKeys = providerDescriptors.map((provider) => provider.key);
  const securityModeOptions = capabilities.securityModes.length > 0
    ? capabilities.securityModes
    : settings?.security_mode
      ? [{ value: settings.security_mode, label: settings.security_mode }]
      : [];

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
            <AgentSettingsTab
              settings={settings}
              form={form}
              urlInput={urlInput}
              tokenInput={tokenInput}
              modelGroups={finalModelGroups}
              providerOptions={uniqueProviders}
              securityModeOptions={securityModeOptions}
              onUrlChange={setUrlInput}
              onTokenChange={setTokenInput}
              onFieldChange={setField}
              onUpdateConfig={updateConfig}
            />
          )}

          {activeTab === 'providers' && (
            <ProvidersTab
              providerKeys={allProviderKeys}
              providersForm={providersForm}
              providerDescriptors={providerDescriptors}
              providerCapabilityByKey={providerCapabilityByKey}
              onProvidersChange={setProvidersForm}
              onDirty={() => {
                setSaveNotice(null);
                clearWorkspaceNotice('settings');
              }}
              newProviderKey={newProvKey}
              newProviderKeyError={newProvKeyErr}
              showAddCustom={showAddCustom}
              onNewProviderKeyChange={setNewProvKey}
              onNewProviderKeyError={setNewProvKeyErr}
              onShowAddCustomChange={setShowAddCustom}
            />
          )}

          {activeTab === 'channels' && (
            <ChannelsTab
              channelCapabilities={channelCapabilities}
              channelsForm={channelsForm}
              onChannelsChange={setChannelsForm}
              onDirty={() => {
                setSaveNotice(null);
                clearWorkspaceNotice('settings');
              }}
            />
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
