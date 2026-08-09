import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import type { JsonObject, JsonValue } from '../types';
import {
  Activity,
  Cpu,
  Database,
  MessageSquare,
  Network,
  Plus,
  ScrollText,
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

interface StatCardProps {
  label: string;
  value: string;
  sub?: string;
  icon: React.ComponentType<{ className?: string }>;
  accent?: string;
  onClick?: () => void;
}

const StatCard: React.FC<StatCardProps> = ({ label, value, sub, icon: Icon, accent = 'text-amber-500', onClick }) => (
  <button
    onClick={onClick}
    disabled={!onClick}
    className={cnCard(onClick)}
    title={onClick ? `Open ${label}` : undefined}
  >
    <div className="flex items-center justify-between">
      <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">{label}</span>
      <Icon className={`h-4 w-4 ${accent}`} />
    </div>
    <div className="mt-1.5 truncate text-xl font-extrabold tracking-tight text-foreground">{value}</div>
    {sub && <div className="mt-0.5 truncate text-[11px] text-muted-foreground">{sub}</div>}
  </button>
);

const cnCard = (onClick?: () => void) =>
  [
    'rounded-xl border border-border/60 bg-card/60 p-4 text-left shadow-sm transition',
    onClick ? 'cursor-pointer hover:border-amber-500/40 hover:bg-card' : 'cursor-default',
  ].join(' ');

interface LauncherCardProps {
  label: string;
  desc: string;
  badge?: string;
  icon: React.ComponentType<{ className?: string }>;
  accent?: string;
  onClick: () => void;
}

const LauncherCard: React.FC<LauncherCardProps> = ({ label, desc, badge, icon: Icon, accent = 'text-amber-500 bg-amber-500/10 border-amber-500/20', onClick }) => (
  <button
    onClick={onClick}
    className="group flex flex-col justify-between rounded-2xl border border-border/60 bg-card/40 p-5 text-left shadow-sm backdrop-blur-md transition-all duration-300 hover:-translate-y-1 hover:border-amber-500/40 hover:bg-card hover:shadow-md focus:outline-none focus:ring-2 focus:ring-amber-500/50"
  >
    <div className="w-full">
      <div className="flex items-start justify-between">
        <div className={`rounded-xl p-2.5 transition-colors duration-300 group-hover:scale-110 ${accent}`}>
          <Icon className="h-5 w-5" />
        </div>
        {badge && (
          <span className="rounded-full bg-amber-500/10 px-2.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-amber-500">
            {badge}
          </span>
        )}
      </div>
      <h3 className="mt-4 text-sm font-bold text-foreground transition-colors group-hover:text-amber-500">{label}</h3>
      <p className="mt-1.5 text-xs text-muted-foreground leading-relaxed">{desc}</p>
    </div>
    <div className="mt-4 flex items-center gap-1 text-[10px] font-bold text-amber-500 opacity-0 transition-opacity duration-300 group-hover:opacity-100">
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
  const sessions = useOpenZStore((s) => s.sessions);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
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
            <Sparkles className="h-5 w-5 text-amber-500 animate-pulse" />
            Dashboard
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Live system overview — every value streams from the gateway in realtime
            {status?.version ? ` · v${status.version}` : ''}.
          </p>
        </div>
        <button
          onClick={newSession}
          className="flex items-center justify-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs font-semibold text-amber-400 transition hover:bg-amber-500/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
        >
          <Plus className="h-4 w-4" /> New Session
        </button>
      </div>

      {/* Stat grid */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        <StatCard
          label="Connection"
          value={connectionStatus}
          sub={wsUrl}
          icon={Network}
          accent={connectionStatus === 'connected' ? 'text-emerald-500' : connectionStatus === 'connecting' ? 'text-amber-500' : 'text-red-500'}
          onClick={() => setIsSettingsOpen(true)}
        />
        <StatCard
          label="Active Model"
          value={activeModel || '—'}
          sub={settings?.provider || 'provider unknown'}
          icon={Zap}
          onClick={() => setIsSettingsOpen(true)}
        />
        <StatCard
          label="Sessions"
          value={String(sessions.length)}
          sub={activeSession?.title ? `Active: ${activeSession.title}` : 'No active session'}
          icon={MessageSquare}
          onClick={() => setActiveView('chats')}
        />
      </div>

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
            accent="text-indigo-400 bg-indigo-500/10 border-indigo-500/20"
            onClick={() => setActiveView('agents')}
          />
          <LauncherCard
            label="Skills"
            desc="Teach custom markdown skills for formatting, design systems, and integrations."
            icon={BookOpen}
            accent="text-sky-400 bg-sky-500/10 border-sky-500/20"
            onClick={() => setActiveView('skills')}
          />
          <LauncherCard
            label="Knowledge Graph"
            desc="Audit structural memory nodes, relationships, and context compactor graphs."
            icon={BrainCircuit}
            badge="Cognitive"
            accent="text-emerald-400 bg-emerald-500/10 border-emerald-500/20"
            onClick={() => setActiveView('knowledge')}
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
            accent="text-amber-500 bg-amber-500/10 border-amber-500/20"
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
            accent="text-rose-400 bg-rose-500/10 border-rose-500/20"
            onClick={() => {
              setIsMcpsOpen(true);
              wsService.requestMcpServers();
            }}
          />
          <LauncherCard
            label="Background Bots"
            desc="Configure Slack, Discord, Telegram, and WhatsApp bot listeners."
            icon={Server}
            badge={servers.length > 0 ? String(servers.length) : undefined}
            accent="text-cyan-400 bg-cyan-500/10 border-cyan-500/20"
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
            accent="text-fuchsia-400 bg-fuchsia-500/10 border-fuchsia-500/20"
            onClick={() => {
              setIsLogsOpen(true);
              wsService.requestLogs();
            }}
          />
        </div>
      </div>

      {/* Toggles */}
      <div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2">
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
      </div>

      {/* Configuration Status Overview */}
      <div className="mt-8 grid grid-cols-1 gap-6 md:grid-cols-2">
        {/* Providers card */}
        <div className="rounded-2xl border border-border/60 bg-card/45 p-6 backdrop-blur-md">
          <div className="flex items-center gap-2 mb-4 border-b border-border/40 pb-2 select-none">
            <Cpu className="h-4 w-4 text-amber-500" />
            <h2 className="text-sm font-bold text-foreground">LLM Provider Integrations</h2>
          </div>
          <div className="space-y-3.5 max-h-[260px] overflow-y-auto pr-1">
            {['openai', 'anthropic', 'openrouter', 'deepseek', 'groq', 'ollama', 'minimax', 'mistral', 'z_ai', 'nvidia', 'opencode_zen', 'cerebras', 'google_ai_studio'].map((provKey) => {
              const cfg = isJsonObject(providersConfig[provKey]) ? providersConfig[provKey] : {};
              const apiBase = jsonString(cfg.api_base);
              const isConfigured = Boolean(jsonString(cfg.api_key));
              const displayName = provKey === 'google_ai_studio' ? 'Google AI Studio' : provKey === 'z_ai' ? 'z.ai' : provKey === 'opencode_zen' ? 'OpenCode Zen' : provKey.toUpperCase();
              return (
                <div key={provKey} className="flex items-center justify-between py-1 border-b border-border/10 last:border-0 text-xs">
                  <div className="flex flex-col">
                    <span className="font-semibold text-foreground capitalize">{displayName}</span>
                    <span className="text-[10px] text-muted-foreground font-mono truncate max-w-[200px]">
                      {apiBase || 'Default endpoint'}
                    </span>
                  </div>
                  <span className={`px-2.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider select-none ${
                    isConfigured ? 'bg-emerald-500/10 text-emerald-400' : 'bg-muted text-muted-foreground'
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
            <Server className="h-4 w-4 text-amber-500" />
            <h2 className="text-sm font-bold text-foreground">Active Bot Listeners</h2>
          </div>
          <div className="space-y-4">
            {['telegram', 'discord', 'whatsapp'].map((chanKey) => {
              const cfg = isJsonObject(channelsConfig[chanKey]) ? channelsConfig[chanKey] : {};
              const webhookPort = jsonNumber(cfg.webhook_port) || 8090;
              const isEnabled = jsonBool(cfg.enabled);
              return (
                <div key={chanKey} className="flex items-center justify-between text-xs py-1 border-b border-border/10 last:border-0">
                  <div className="flex flex-col">
                    <span className="font-semibold text-foreground capitalize">{chanKey} Listener</span>
                    <span className="text-[10px] text-muted-foreground font-mono">
                      {chanKey === 'whatsapp' ? `Webhook Port: ${webhookPort}` : isEnabled ? 'Background polling' : 'Offline'}
                    </span>
                  </div>
                  <div className="flex items-center gap-2 select-none">
                    <span className={`px-2.5 py-0.5 rounded-full text-[9px] font-bold tracking-wider ${
                      isEnabled ? 'bg-amber-500/10 text-amber-400' : 'bg-muted text-muted-foreground'
                    }`}>
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
    className="flex items-center justify-between rounded-xl border border-border/60 bg-card/60 p-4 text-left shadow-sm transition hover:border-amber-500/40 hover:bg-card"
  >
    <div className="flex items-center gap-3">
      <Icon className="h-4 w-4 text-amber-500" />
      <div>
        <div className="text-sm font-semibold text-foreground">{label}</div>
        <div className="text-[11px] text-muted-foreground">{desc}</div>
      </div>
    </div>
    <span
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${on ? 'bg-amber-500' : 'bg-muted'}`}
      aria-label={on ? 'Enabled' : 'Disabled'}
    >
      <span
        className={`absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all ${on ? 'left-[18px]' : 'left-0.5'}`}
      />
    </span>
  </button>
);