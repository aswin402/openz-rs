import React, { useMemo, useState } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import { wsService } from '../services/websocket';
import { resolveProviderDescriptors } from '../config/providers';
import { MetricCard } from '../shared/ui/MetricCard';
import {
  ACTIVITY_VARIANT_OPTIONS,
  AgentActivity,
  type BeuiActivityContentType,
} from './beui';
import { buildUnifiedDashboardActivitySteps } from '../features/activity/activityAdapters';
import { cn } from '../lib/utils';
import type { JsonObject, JsonValue } from '../types';
import {
  Activity,
  Cpu,
  Database,
  MessageSquare,
  Network,
  Plus,
  ScrollText,
  SlidersHorizontal,
  Sparkles,
  Zap,
  Bot,
  BookOpen,
  BrainCircuit,
  Server,
} from 'lucide-react';

function isJsonObject(value: JsonValue | undefined): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function jsonString(value: JsonValue | undefined): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function jsonBool(value: JsonValue | undefined): boolean {
  return typeof value === 'boolean' ? value : false;
}

function jsonNumber(value: JsonValue | undefined): number | undefined {
  return typeof value === 'number' ? value : undefined;
}

interface LauncherCardProps {
  label: string;
  desc: string;
  badge?: string;
  icon: React.ComponentType<{ className?: string }>;
  accent?: string;
  onClick: () => void;
}

const LauncherCard: React.FC<LauncherCardProps> = ({ label, desc, badge, icon: Icon, accent = 'text-primary bg-primary/10 border-primary/20', onClick }) => (
  <button
    onClick={onClick}
    className="group flex flex-col justify-between rounded-2xl border border-border/60 bg-card/40 p-5 text-left shadow-sm backdrop-blur-md transition-all duration-300 hover:-translate-y-1 hover:border-primary/40 hover:bg-card hover:shadow-md focus:outline-none focus:ring-2 focus:ring-primary/50"
  >
    <div className="w-full">
      <div className="flex items-start justify-between">
        <div className={`rounded-xl p-2.5 transition-colors duration-300 group-hover:scale-110 ${accent}`}>
          <Icon className="h-5 w-5" />
        </div>
        {badge && (
          <span className="rounded-full bg-primary/12 px-2.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-primary">
            {badge}
          </span>
        )}
      </div>
      <h3 className="mt-4 text-sm font-bold text-foreground transition-colors group-hover:text-primary">{label}</h3>
      <p className="mt-1.5 text-xs text-muted-foreground leading-relaxed">{desc}</p>
    </div>
    <div className="mt-4 flex items-center gap-1 text-[10px] font-bold text-primary opacity-0 transition-opacity duration-300 group-hover:opacity-100">
      Launch <span>→</span>
    </div>
  </button>
);

export const DashboardView: React.FC = () => {
  const connectionStatus = useOpenZStore((s) => s.connectionStatus);
  const wsUrl = useOpenZStore((s) => s.wsUrl);
  const activeModel = useOpenZStore((s) => s.activeModel);
  const settings = useOpenZStore((s) => s.settings);
  const status = useOpenZStore((s) => s.status);
  const runtimeInventory = useOpenZStore((s) => s.runtimeInventory);
  const sessions = useOpenZStore((s) => s.sessions);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const messagesByChat = useOpenZStore((s) => s.messages);
  const orchestrationRunsByChat = useOpenZStore((s) => s.orchestrationRuns);
  const cronLogs = useOpenZStore((s) => s.cronLogs);
  const isStreaming = useOpenZStore((s) => s.isStreaming);
  const activeTurnIds = useOpenZStore((s) => s.activeTurnIds);
  const mcpStats = useOpenZStore((s) => s.mcpStats);
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const logs = useOpenZStore((s) => s.logs);
  const servers = useOpenZStore((s) => s.servers);
  const streamingMode = useOpenZStore((s) => s.streamingMode);
  const cavemanMode = useOpenZStore((s) => s.cavemanMode);
  const toggleStreamingMode = useOpenZStore((s) => s.toggleStreamingMode);
  const toggleCavemanMode = useOpenZStore((s) => s.toggleCavemanMode);
  const providersConfig = useOpenZStore((s) => s.providersConfig) || {};
  const channelsConfig = useOpenZStore((s) => s.channelsConfig) || {};
  const capabilities = useOpenZStore((s) => s.capabilities);
  const providerDescriptors = resolveProviderDescriptors(capabilities, providersConfig);

  const activityVariant = useUiCustomizationStore((s) => s.activityVariant);
  const activityOrbStyle = useUiCustomizationStore((s) => s.activityOrbStyle);
  const activityDensity = useUiCustomizationStore((s) => s.activityDensity);
  const activityMaxHeight = useUiCustomizationStore((s) => s.activityMaxHeight);
  const activityShowDetails = useUiCustomizationStore((s) => s.activityShowDetails);
  const showDashboardActivity = useUiCustomizationStore((s) => s.showDashboardActivity);
  const applyCustomization = useUiCustomizationStore((s) => s.applyCustomization);
  const setActiveCustomizeSection = useUiCustomizationStore((s) => s.setActiveCustomizeSection);
  const setIsCustomizeOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);

  const [dashboardContentType, setDashboardContentType] =
    useState<BeuiActivityContentType>('mixed');

  const unifiedActivitySteps = useMemo(
    () =>
      buildUnifiedDashboardActivitySteps({
        messagesByChat,
        activeChatId,
        orchestrationRunsByChat,
        logs,
        inventory: runtimeInventory,
        cronLogs,
        limit: 20,
      }),
    [messagesByChat, activeChatId, orchestrationRunsByChat, logs, runtimeInventory, cronLogs],
  );

  const isAnySessionActive =
    isStreaming || Object.keys(activeTurnIds).length > 0;

  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setIsMcpsOpen = useOpenZStore((s) => s.setIsMcpsOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);
  const setIsServersOpen = useOpenZStore((s) => s.setIsServersOpen);
  const newSession = useOpenZStore((s) => s.newSession);

  const activeSession = sessions.find((s) => s.id === activeChatId);

  return (
    <div className="mx-auto max-w-5xl px-4 py-8">
      {/* Hero strip */}
      <div className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <Sparkles className="h-5 w-5 text-primary animate-pulse" />
            Dashboard
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Live system overview — every value streams from the gateway in realtime
            {status?.version ? ` · v${status.version}` : ''}.
          </p>
        </div>
        <button
          onClick={newSession}
          className="flex items-center justify-center gap-2 rounded-xl border border-primary/30 bg-primary/10 px-4 py-2 text-xs font-semibold text-primary transition hover:bg-primary/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
        >
          <Plus className="h-4 w-4" /> New Session
        </button>
      </div>

      {/* Stat grid */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        <MetricCard
          label="Connection"
          value={connectionStatus}
          detail={wsUrl}
          icon={Network}
          accent={connectionStatus === 'connected' ? 'text-success' : connectionStatus === 'connecting' ? 'text-warning' : 'text-destructive'}
          onClick={() => setIsSettingsOpen(true)}
        />
        <MetricCard
          label="Active Model"
          value={activeModel || '—'}
          detail={settings?.provider || 'provider unknown'}
          icon={Zap}
          onClick={() => setIsSettingsOpen(true)}
        />
        <MetricCard
          label="Sessions"
          value={String(sessions.length)}
          detail={activeSession?.title ? `Active: ${activeSession.title}` : 'No active session'}
          icon={MessageSquare}
          onClick={() => setActiveView('chats')}
        />
        <MetricCard
          label="Core Inventory"
          value={runtimeInventory ? `${runtimeInventory.counts.subagents} agents / ${runtimeInventory.counts.tools} tools` : 'Loading'}
          detail={runtimeInventory ? `${runtimeInventory.counts.skills} skills · ${runtimeInventory.counts.activeCronJobs}/${runtimeInventory.counts.cronJobs} cron active` : 'Waiting for gateway inventory'}
          icon={Database}
          onClick={() => setActiveView('inventory')}
        />
      </div>

      {/* Live Unified @beui/agent-activity Feed */}
      {showDashboardActivity && (
        <div className="mt-7 rounded-2xl border border-border/65 bg-card/45 p-5 backdrop-blur-md">
          <div className="mb-3 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <h2 className="flex items-center gap-2 text-sm font-bold text-foreground">
                <Activity className="h-4 w-4 text-primary" />
                <span>Live Agent &amp; Runtime Activity</span>
              </h2>
              <p className="mt-0.5 text-xs text-muted-foreground">
                Unified @beui/agent-activity stream across chat turns, orchestrations, cron runs, and gateway traces.
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-1.5">
              {(
                [
                  { id: 'mixed', label: 'All' },
                  { id: 'tool', label: 'Tools' },
                  { id: 'search', label: 'Search' },
                  { id: 'reasoning', label: 'Think' },
                  { id: 'trace', label: 'Trace' },
                ] as const
              ).map((tab) => (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setDashboardContentType(tab.id)}
                  className={cn(
                    'rounded-lg border px-2 py-1 text-[10.5px] font-semibold transition-colors',
                    dashboardContentType === tab.id
                      ? 'border-primary/45 bg-primary/12 text-primary'
                      : 'border-border/55 bg-background/50 text-muted-foreground hover:text-foreground',
                  )}
                >
                  {tab.label}
                </button>
              ))}

              <select
                aria-label="Dashboard activity visual variant"
                value={activityVariant}
                onChange={(e) =>
                  applyCustomization({
                    activityVariant: e.target.value as typeof activityVariant,
                  })
                }
                className="rounded-lg border border-border/60 bg-background/60 px-2.5 py-1 text-[10.5px] font-semibold text-foreground"
              >
                {ACTIVITY_VARIANT_OPTIONS.map((opt) => (
                  <option key={opt.id} value={opt.id}>
                    {opt.shortLabel}
                  </option>
                ))}
              </select>

              <button
                type="button"
                onClick={() => {
                  setActiveCustomizeSection('agent-activity');
                  setIsCustomizeOpen(true);
                }}
                className="inline-flex items-center gap-1 rounded-lg border border-border/60 bg-background/60 px-2.5 py-1 text-[10.5px] font-semibold text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
                title="Customize Agent Activity options"
              >
                <SlidersHorizontal className="h-3 w-3" />
                <span>Options</span>
              </button>
            </div>
          </div>

          <AgentActivity
            steps={unifiedActivitySteps}
            isActive={isAnySessionActive}
            mode="inline-collapsible"
            defaultOpen
            variant={activityVariant}
            orbStyle={activityOrbStyle}
            density={activityDensity}
            maxHeight={activityMaxHeight}
            contentType={dashboardContentType}
            showDetails={activityShowDetails}
            title={
              isAnySessionActive
                ? 'Agent executing across active session'
                : 'Recent gateway & agent telemetry'
            }
            emptyLabel="Start a chat, trigger a subagent workflow, or open Gateway Logs to stream live steps here."
          />
        </div>
      )}

      {/* Workspace Section */}
      <div className="mt-8">
        <h2 className="text-xs font-bold uppercase tracking-wider text-muted-foreground/80 mb-3.5">
          Workspace Panel
        </h2>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <LauncherCard
            label="Agents"
            desc="Configure LLM profiles, tool sets, instructions, and default credentials."
            icon={Bot}
            accent="text-chart-1 bg-chart-1/12 border-chart-1/25"
            onClick={() => setActiveView('agents')}
          />
          <LauncherCard
            label="Skills"
            desc="Teach custom markdown skills for formatting, design systems, and integrations."
            icon={BookOpen}
            accent="text-info bg-info/12 border-info/25"
            onClick={() => setActiveView('skills')}
          />
          <LauncherCard
            label="Knowledge Graph"
            desc="Audit structural memory nodes, relationships, and context compactor graphs."
            icon={BrainCircuit}
            badge="Cognitive"
            accent="text-success bg-success/12 border-success/25"
            onClick={() => setActiveView('knowledge')}
          />
          <LauncherCard
            label="Core Inventory"
            desc="Inspect runtime paths, tools, cron jobs, channels, and shared OpenZ core metadata."
            icon={Database}
            badge={runtimeInventory ? String(runtimeInventory.counts.tools) : undefined}
            accent="text-primary bg-primary/12 border-primary/25"
            onClick={() => setActiveView('inventory')}
          />
        </div>
      </div>

      {/* System Services Section */}
      <div className="mt-8">
        <h2 className="text-xs font-bold uppercase tracking-wider text-muted-foreground/80 mb-3.5">
          System Services
        </h2>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <LauncherCard
            label="Memory"
            desc="Manage the entity-relation database store and recall facts."
            icon={Database}
            badge={(cognitiveStats.entitiesCount + cognitiveStats.factsCount) > 0 ? String(cognitiveStats.entitiesCount + cognitiveStats.factsCount) : undefined}
            accent="text-chart-2 bg-chart-2/12 border-chart-2/25"
            onClick={() => {
              setIsMemoryOpen(true);
              wsService.requestCognitiveMemory();
            }}
          />
          <LauncherCard
            label="MCP Servers"
            desc="Configure and inspect connected Model Context Protocol tool servers."
            icon={Cpu}
            badge={mcpStats.total > 0 ? `${mcpStats.loaded}/${mcpStats.total}` : undefined}
            accent="text-chart-3 bg-chart-3/12 border-chart-3/25"
            onClick={() => {
              setIsMcpsOpen(true);
              wsService.requestMcpServers();
            }}
          />
          <LauncherCard
            label="Background Bots"
            desc={capabilities.channels.length > 0
              ? 'Configure ' + capabilities.channels.map((channel) => channel.label).join(', ') + '.'
              : 'Configure background channel listeners from the gateway.'}
            icon={Server}
            badge={servers.length > 0 ? String(servers.length) : undefined}
            accent="text-chart-4 bg-chart-4/12 border-chart-4/25"
            onClick={() => {
              setIsServersOpen(true);
              wsService.requestServers();
            }}
          />
          <LauncherCard
            label="Gateway Logs"
            desc="Stream server traces and debug logs in real time."
            icon={ScrollText}
            badge={logs.length > 0 ? String(logs.length) : undefined}
            accent="text-chart-5 bg-chart-5/12 border-chart-5/25"
            onClick={() => {
              setIsLogsOpen(true);
              wsService.requestLogs();
            }}
          />
        </div>
      </div>

      {/* Toggles */}
      <div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-3">
        <ToggleRow
          label="Streaming Responses"
          desc="Deltas are broadcast as they are generated"
          on={streamingMode}
          onToggle={toggleStreamingMode}
          icon={Activity}
        />
        <ToggleRow
          label="Caveman Mode"
          desc="Terseness instruction injected into the system prompt"
          on={cavemanMode}
          onToggle={toggleCavemanMode}
          icon={Database}
        />
        <ToggleRow
          label="Dashboard Activity"
          desc="Live @beui/agent-activity feed on overview"
          on={showDashboardActivity}
          onToggle={() =>
            applyCustomization({ showDashboardActivity: !showDashboardActivity })
          }
          icon={SlidersHorizontal}
        />
      </div>

      {/* Configuration Status Overview */}
      <div className="mt-8 grid grid-cols-1 gap-6 md:grid-cols-2">
        {/* Providers card */}
        <div className="rounded-2xl border border-border/60 bg-card/45 p-6 backdrop-blur-md">
          <div className="flex items-center gap-2 mb-4 border-b border-border/40 pb-2 select-none">
            <Cpu className="h-4 w-4 text-primary" />
            <h2 className="text-sm font-bold text-foreground">LLM Provider Integrations</h2>
          </div>
          <div className="space-y-3.5 max-h-[260px] overflow-y-auto pr-1">
            {providerDescriptors.map((provider) => {
              const rawCfg = providersConfig[provider.key];
              const cfg = isJsonObject(rawCfg) ? rawCfg : undefined;
              const apiBase = cfg ? jsonString(cfg['api_base']) : undefined;
              const isConfigured = Boolean(cfg && jsonString(cfg['api_key']));
              return (
                <div key={provider.key} className="flex items-center justify-between py-1 border-b border-border/10 last:border-0 text-xs">
                  <div className="flex flex-col">
                    <span className="font-semibold text-foreground">{provider.display}</span>
                    <span className="text-[10px] text-muted-foreground font-mono truncate max-w-[200px]">
                      {apiBase || provider.apiBase || 'Default endpoint'}
                    </span>
                  </div>
                  <span className={`px-2.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider select-none ${
                    isConfigured ? 'bg-success/12 text-success' : 'bg-muted text-muted-foreground'
                  }`}>
                    {isConfigured ? 'Ready' : 'Not Setup'}
                  </span>
                </div>
              );
            })}
          </div>
        </div>

        {/* Bot Channels card */}
        <div className="rounded-2xl border border-border/60 bg-card/45 p-6 backdrop-blur-md">
          <div className="flex items-center gap-2 mb-4 border-b border-border/40 pb-2 select-none">
            <Server className="h-4 w-4 text-primary" />
            <h2 className="text-sm font-bold text-foreground">Active Bot Listeners</h2>
          </div>
          <div className="space-y-4">
            {capabilities.channels.length === 0 ? (
              <div className="text-sm text-muted-foreground">Channel capabilities are not loaded yet.</div>
            ) : capabilities.channels.map((channel) => {
              const rawChannelConfig = channelsConfig[channel.name];
              const cfg = isJsonObject(rawChannelConfig) ? rawChannelConfig : {};
              const isEnabled = jsonBool(cfg.enabled);
              const portField = channel.fields.find((field) => field.kind === 'number');
              const portValue = portField
                ? jsonNumber(cfg[portField.key]) ?? jsonNumber(channel.defaults[portField.key])
                : undefined;
              return (
                <div key={channel.name} className="flex items-center justify-between text-xs py-1 border-b border-border/10 last:border-0">
                  <div className="flex flex-col">
                    <span className="font-semibold text-foreground">{channel.label}</span>
                    <span className="text-[10px] text-muted-foreground font-mono truncate">
                      {portField && portValue !== undefined ? portField.label + ': ' + portValue : isEnabled ? 'Background listener' : 'Offline'}
                    </span>
                  </div>
                  <div className="flex items-center gap-2 select-none">
                    <span className={isEnabled ? 'px-2.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider bg-primary/12 text-primary' : 'px-2.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider bg-muted text-muted-foreground'}>
                      {isEnabled ? 'Enabled' : 'Disabled'}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
};

const ToggleRow: React.FC<{
  label: string;
  desc: string;
  on: boolean;
  onToggle: () => void;
  icon: React.ComponentType<{ className?: string }>;
}> = ({ label, desc, on, onToggle, icon: Icon }) => (
  <button
    onClick={onToggle}
    className="flex items-center justify-between rounded-xl border border-border/60 bg-card/60 p-4 text-left shadow-sm transition hover:border-primary/40 hover:bg-card"
  >
    <div className="flex items-center gap-3">
      <Icon className="h-4 w-4 text-primary" />
      <div>
        <div className="text-sm font-semibold text-foreground">{label}</div>
        <div className="text-[11px] text-muted-foreground">{desc}</div>
      </div>
    </div>
    <span
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${on ? 'bg-primary' : 'bg-muted'}`}
      aria-label={on ? 'Enabled' : 'Disabled'}
    >
      <span
        className={`absolute top-0.5 h-4 w-4 rounded-full shadow transition-all ${on ? 'left-[18px] bg-primary-foreground' : 'left-0.5 bg-card'}`}
      />
    </span>
  </button>
);
