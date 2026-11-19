/* eslint-disable react-hooks/set-state-in-effect */
import React, { useEffect, useMemo, useState } from 'react';
import {
  ArrowLeft,
  Cpu,
  Download,
  KeyRound,
  Palette,
  Radio,
  RotateCcw,
  Save,
  Upload,
  X,
  Zap,
} from 'lucide-react';
import { useOpenZStore } from '../../store/useOpenZStore';
import { useThemeStore } from '../../store/useThemeStore';
import {
  useUiCustomizationStore,
  type GatewaySettingsSectionId,
} from '../../store/useUiCustomizationStore';
import type { JsonObject } from '../../types';
import { resolveProviderDescriptors } from '../../config/providers';
import {
  buildDefaultsPatch,
  DEFAULT_SECURITY_MODE_OPTIONS,
  DEFAULT_TOOL_ROUTING_MODE_OPTIONS,
  hasDefaultsChanged,
  jsonObjectToChannels,
  jsonObjectToProviders,
  settingsToForm,
  stableStringify,
  validateSettingsForm,
  type ChannelsForm,
  type ProvidersForm,
  type SettingsForm,
} from './settingsSchema';
import { AgentSettingsTab } from './AgentSettingsTab';
import { CavemanSettingsTab } from './CavemanSettingsTab';
import { ChannelsTab } from './ChannelsTab';
import { ProvidersTab } from './ProvidersTab';
import { BeuiAlert } from '../../components/beui';
import { OpenZFoxIcon } from '../../components/OpenZFoxIcon';
import { SpectrumNavListCard } from '../../components/registries/modernRegistries';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { cn } from '../../lib/utils';

interface NavSectionMeta {
  id: GatewaySettingsSectionId;
  label: string;
  subtitle: string;
  icon: React.ComponentType<{ className?: string }>;
}

const NAV_SECTIONS: NavSectionMeta[] = [
  {
    id: 'agent',
    label: 'Agent & Gateway',
    subtitle: 'Gateway WebSocket URL, auth token, default model, inference limits & runtime switches',
    icon: Cpu,
  },
  {
    id: 'caveman',
    label: 'Caveman Mode',
    subtitle:
      'Enable or disable caveman mode, configure intensity modes (Lite, Full, Ultra, Symbolic), and inspect OpenCaveman token savings',
    icon: Zap,
  },
  {
    id: 'providers',
    label: 'LLM Providers',
    subtitle: 'Configure API keys, custom OpenAI-compatible endpoints, and preferred default models',
    icon: KeyRound,
  },
  {
    id: 'channels',
    label: 'Bot Channels',
    subtitle: 'Manage Telegram, Discord, WhatsApp, Email, and external listener configurations',
    icon: Radio,
  },
];

export const SettingsModal: React.FC = () => {
  const isSettingsOpen = useOpenZStore((s) => s.isSettingsOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);

  const activeSection = useUiCustomizationStore((s) => s.activeGatewaySection);
  const setActiveSection = useUiCustomizationStore((s) => s.setActiveGatewaySection);
  const navMenuStyle = useUiCustomizationStore((s) => s.navMenuStyle);
  const setIsCustomizeOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);
  const exportBackupJson = useUiCustomizationStore((s) => s.exportBackupJson);
  const importBackupJson = useUiCustomizationStore((s) => s.importBackupJson);
  const currentThemeMode = useThemeStore((s) => s.theme);
  const setThemeMode = useThemeStore((s) => s.setTheme);
  const backupInputRef = React.useRef<HTMLInputElement>(null);

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

  const [urlInput, setUrlInput] = useState(wsUrl);
  const [tokenInput, setTokenInput] = useState(wsToken);

  const [form, setForm] = useState<SettingsForm>({});
  const [providersForm, setProvidersForm] = useState<ProvidersForm>({});
  const [channelsForm, setChannelsForm] = useState<ChannelsForm>({});
  const [saveNotice, setSaveNotice] = useState<{ type: 'success' | 'error'; message: string } | null>(
    null,
  );

  const [newProvKey, setNewProvKey] = useState('');
  const [newProvKeyErr, setNewProvKeyErr] = useState('');
  const [showAddCustom, setShowAddCustom] = useState(false);

  useEffect(() => {
    if (isSettingsOpen) {
      setUrlInput(wsUrl);
      setTokenInput(wsToken);
      if (settings) setForm(settingsToForm(settings));
      if (providersConfig) setProvidersForm(jsonObjectToProviders(providersConfig));
      if (channelsConfig) setChannelsForm(jsonObjectToChannels(channelsConfig));
      setShowAddCustom(false);
      setNewProvKey('');
      setNewProvKeyErr('');
      setSaveNotice(null);
      clearWorkspaceNotice('settings');
    }
  }, [
    isSettingsOpen,
    wsUrl,
    wsToken,
    settings,
    providersConfig,
    channelsConfig,
    clearWorkspaceNotice,
  ]);

  useEffect(() => {
    if (!isSettingsOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsSettingsOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [isSettingsOpen, setIsSettingsOpen]);

  const setField = (key: string, value: string | number | boolean) => {
    setForm((f) => ({ ...f, [key]: value }));
    setSaveNotice(null);
    clearWorkspaceNotice('settings');
  };

  const resetForms = () => {
    setUrlInput(wsUrl);
    setTokenInput(wsToken);
    if (settings) setForm(settingsToForm(settings));
    setProvidersForm(jsonObjectToProviders(providersConfig));
    setChannelsForm(jsonObjectToChannels(channelsConfig));
    setSaveNotice(null);
  };

  const validationError = useMemo(
    () => validateSettingsForm(urlInput, form, settings, capabilities, channelsForm),
    [capabilities, channelsForm, form, settings, urlInput],
  );

  const hasChanges = useMemo(
    () =>
      urlInput !== wsUrl ||
      tokenInput !== wsToken ||
      hasDefaultsChanged(form, settings) ||
      stableStringify(providersForm) !== stableStringify(providersConfig) ||
      stableStringify(channelsForm) !== stableStringify(channelsConfig),
    [
      channelsConfig,
      channelsForm,
      form,
      providersConfig,
      providersForm,
      settings,
      tokenInput,
      urlInput,
      wsToken,
      wsUrl,
    ],
  );

  const pageNotice = workspaceNotice?.scope === 'settings' ? workspaceNotice : saveNotice;
  const visibleNotice = validationError
    ? { type: 'error' as const, message: validationError }
    : pageNotice;

  const handleSave = () => {
    if (validationError) {
      setSaveNotice({ type: 'error', message: validationError });
      return;
    }

    setWsConfig(urlInput.trim(), tokenInput);

    updateConfig({
      defaults: buildDefaultsPatch(form, settings),
      providers: providersForm as unknown as JsonObject,
      channels: channelsForm as unknown as JsonObject,
    });

    setSaveNotice({
      type: 'success',
      message: 'Gateway settings saved and backed up to ~/.openz/backups.',
    });
  };

  const handleExportFullBackup = () => {
    const json = exportBackupJson({
      themeMode: currentThemeMode,
      gatewayDefaults: buildDefaultsPatch(form, settings) as Record<string, unknown>,
    });
    const blob = new Blob([json], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `openz-settings-backup-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
    setSaveNotice({
      type: 'success',
      message: 'Exported WebUI & non-secret gateway preferences backup JSON.',
    });
  };

  const handleImportFullBackup = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const content = typeof reader.result === 'string' ? reader.result : '';
      const res = importBackupJson(content);
      if (!res.ok) {
        setSaveNotice({ type: 'error', message: res.error || 'Invalid backup JSON file.' });
        return;
      }
      if (res.themeMode) {
        setThemeMode(res.themeMode);
      }
      try {
        const parsed = JSON.parse(content) as { gatewayDefaults?: Record<string, unknown> };
        if (parsed.gatewayDefaults && typeof parsed.gatewayDefaults === 'object') {
          updateConfig({ defaults: parsed.gatewayDefaults });
        }
      } catch {
        // UI customization already restored cleanly
      }
      setSaveNotice({
        type: 'success',
        message: 'Backup restored and synced to ~/.openz/backups.',
      });
    };
    reader.readAsText(file);
  };

  const groups = providers.filter((p) => p.models.length > 0);
  const customModelGroups = groups.map((g) => ({
    label: g.available === false ? `${g.display || g.name} (not configured)` : g.display || g.name,
    options: g.models.map((m) => ({ value: `${g.name}::${m}`, label: m })),
  }));
  const finalModelGroups =
    customModelGroups.length > 0
      ? customModelGroups
      : settings
        ? [{ label: 'Default', options: [{ value: settings.model, label: settings.model }] }]
        : [];

  const providerDescriptors = resolveProviderDescriptors(capabilities, providersForm);
  const providerOptions = settings
    ? [
        {
          value: settings.provider,
          label:
            providerDescriptors.find((provider) => provider.key === settings.provider)?.display ||
            settings.provider,
        },
        ...providerDescriptors.map((provider) => ({
          value: provider.key,
          label: provider.display,
        })),
      ]
    : [];
  const uniqueProviders = Array.from(
    new Map(providerOptions.map((item) => [item.value, item])).values(),
  );

  const providerCapabilities = capabilities.providers;
  const providerCapabilityByKey = new Map(
    providerCapabilities.map((provider) => [provider.configKey, provider]),
  );
  const allProviderKeys = providerDescriptors.map((provider) => provider.key);
  const securityModeOptions =
    capabilities.securityModes.length > 0
      ? capabilities.securityModes
      : DEFAULT_SECURITY_MODE_OPTIONS;
  const toolRoutingModeOptions =
    capabilities.toolRoutingModes.length > 0
      ? capabilities.toolRoutingModes
      : DEFAULT_TOOL_ROUTING_MODE_OPTIONS;

  const currentMeta =
    NAV_SECTIONS.find((s) => s.id === activeSection) ?? NAV_SECTIONS[0];
  const CurrentIcon = currentMeta.icon;

  if (!isSettingsOpen) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="OpenZ Gateway Settings"
      className="fixed inset-0 z-50 flex h-screen w-screen flex-col overflow-hidden bg-background text-foreground"
    >
      {/* Top Full-Screen Header Bar (matching Customize UI) */}
      <header className="flex h-14 shrink-0 items-center justify-between gap-3 border-b border-border/70 bg-card/80 px-4 backdrop-blur-md md:px-6">
        <div className="flex min-w-0 items-center gap-3">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setIsSettingsOpen(false)}
            className="gap-1.5"
          >
            <ArrowLeft className="size-3.5" />
            <span>Back to Chat</span>
          </Button>
          <div className="hidden h-4 w-px bg-border sm:block" />
          <div className="flex min-w-0 items-center gap-2">
            <OpenZFoxIcon size={18} variant="classic" />
            <span className="truncate text-sm font-bold tracking-tight">Gateway Settings</span>
            <Badge variant="secondary" className="hidden font-mono text-[10px] sm:inline-flex">
              shadcn/ui
            </Badge>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <input
            ref={backupInputRef}
            type="file"
            accept=".json,application/json"
            onChange={handleImportFullBackup}
            className="hidden"
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleExportFullBackup}
            title="Download backup JSON of settings & UI themes"
            className="hidden md:inline-flex"
          >
            <Download className="size-3.5" />
            <span>Backup</span>
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => backupInputRef.current?.click()}
            title="Restore settings & UI themes from backup JSON"
            className="hidden md:inline-flex"
          >
            <Upload className="size-3.5" />
            <span>Restore</span>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              setIsSettingsOpen(false);
              setIsCustomizeOpen(true);
            }}
            className="hidden sm:inline-flex"
          >
            <Palette className="size-3.5 text-primary" />
            <span>Customize UI</span>
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={resetForms}
            disabled={!hasChanges}
          >
            <RotateCcw className="size-3.5" />
            <span className="hidden sm:inline">Reset</span>
          </Button>
          <Button
            type="button"
            size="sm"
            onClick={handleSave}
            disabled={!hasChanges || Boolean(validationError)}
          >
            <Save className="size-3.5" />
            <span>Save &amp; Apply</span>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => setIsSettingsOpen(false)}
            aria-label="Close settings"
            title="Close (Esc)"
          >
            <X className="size-4" />
          </Button>
        </div>
      </header>

      {/* Body: Left shadcn/ui Docs Sidebar + Right Settings Content */}
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden md:flex-row">
        <aside className="flex w-full shrink-0 flex-col border-b border-border/65 bg-sidebar md:h-full md:w-64 md:border-b-0 md:border-r">
          <nav className="flex gap-1 overflow-x-auto p-3 md:flex-1 md:flex-col md:overflow-x-hidden md:overflow-y-auto">
            <div className="hidden px-2.5 pb-1.5 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground/75 md:block">
              Configuration
            </div>
            {/* Mobile horizontal pills */}
            <div className="flex gap-1 md:hidden">
              {NAV_SECTIONS.map((sec) => {
                const Icon = sec.icon;
                const active = activeSection === sec.id;
                return (
                  <button
                    key={sec.id}
                    type="button"
                    onClick={() => setActiveSection(sec.id)}
                    className={cn(
                      'flex shrink-0 items-center gap-2 rounded-md px-2.5 py-2 text-left text-xs transition-colors',
                      active
                        ? 'bg-primary/15 font-semibold text-primary'
                        : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground',
                    )}
                  >
                    <Icon
                      className={cn(
                        'size-3.5 shrink-0',
                        active ? 'text-primary' : 'text-muted-foreground/80',
                      )}
                    />
                    <span className="truncate">{sec.label}</span>
                  </button>
                );
              })}
            </div>
            {/* Desktop @spectrumui/nav-list-card */}
            <div className="hidden md:block">
              <SpectrumNavListCard
                items={NAV_SECTIONS.map((sec) => ({
                  id: sec.id,
                  label: sec.label,
                  icon: sec.icon,
                }))}
                activeId={activeSection}
                onSelect={(id) => setActiveSection(id as GatewaySettingsSectionId)}
                variant={navMenuStyle}
                layoutGroupId="gateway-settings-nav"
              />
            </div>
          </nav>

          <div className="hidden border-t border-border/50 p-3 md:block">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="w-full justify-center gap-2"
              onClick={() => {
                setIsSettingsOpen(false);
                setIsCustomizeOpen(true);
              }}
            >
              <Palette className="size-3.5 text-primary" />
              <span>Customize UI</span>
            </Button>
          </div>
        </aside>

        {/* Main Full-Screen Settings Content */}
        <main className="min-w-0 flex-1 overflow-y-auto bg-background px-5 py-6 md:px-10 md:py-8">
          <div className="mx-auto w-full max-w-5xl space-y-6 pb-14">
            <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border/60 pb-5">
              <div className="space-y-1">
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <span>Gateway Configuration</span>
                  <span>/</span>
                  <Badge variant="outline" className="font-mono text-[10.5px] text-primary">
                    {currentMeta.id}
                  </Badge>
                </div>
                <div className="flex items-center gap-2.5">
                  <CurrentIcon className="size-5 text-primary" />
                  <h1 className="text-xl font-bold tracking-tight text-foreground">
                    {currentMeta.label}
                  </h1>
                </div>
                <p className="text-xs text-muted-foreground md:text-sm">{currentMeta.subtitle}</p>
              </div>
            </div>

            {visibleNotice && (
              <BeuiAlert
                tone={visibleNotice.type === 'success' ? 'success' : 'destructive'}
                title={
                  visibleNotice.type === 'success' ? 'Configuration Saved' : 'Validation Notice'
                }
              >
                {visibleNotice.message}
              </BeuiAlert>
            )}

            {activeSection === 'agent' && (
              <AgentSettingsTab
                settings={settings}
                form={form}
                urlInput={urlInput}
                tokenInput={tokenInput}
                modelGroups={finalModelGroups}
                providerOptions={uniqueProviders}
                securityModeOptions={securityModeOptions}
                toolRoutingModeOptions={toolRoutingModeOptions}
                onUrlChange={setUrlInput}
                onTokenChange={setTokenInput}
                onFieldChange={setField}
                onUpdateConfig={updateConfig}
                onOpenCavemanSettings={() => setActiveSection('caveman')}
              />
            )}

            {activeSection === 'caveman' && (
              <CavemanSettingsTab
                settings={settings}
                form={form}
                onFieldChange={setField}
              />
            )}

            {activeSection === 'providers' && (
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

            {activeSection === 'channels' && (
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
        </main>
      </div>
    </div>
  );
};
