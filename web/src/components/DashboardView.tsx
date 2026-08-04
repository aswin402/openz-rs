import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import {
  Activity,
  Brain,
  Cpu,
  Database,
  MessageSquare,
  Network,
  Plus,
  Settings,
  ScrollText,
  SlashSquare,
  Sparkles,
  TerminalSquare,
  Zap,
} from 'lucide-react';

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

export const DashboardView: React.FC = () => {
  const connectionStatus = useOpenZStore((s) => s.connectionStatus);
  const wsUrl = useOpenZStore((s) => s.wsUrl);
  const activeModel = useOpenZStore((s) => s.activeModel);
  const settings = useOpenZStore((s) => s.settings);
  const status = useOpenZStore((s) => s.status);
  const sessions = useOpenZStore((s) => s.sessions);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const mcpStats = useOpenZStore((s) => s.mcpStats);
  const mcpServers = useOpenZStore((s) => s.mcpServers);
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);
  const slashCommands = useOpenZStore((s) => s.slashCommands);
  const logs = useOpenZStore((s) => s.logs);
  const streamingMode = useOpenZStore((s) => s.streamingMode);
  const cavemanMode = useOpenZStore((s) => s.cavemanMode);
  const toggleStreamingMode = useOpenZStore((s) => s.toggleStreamingMode);
  const toggleCavemanMode = useOpenZStore((s) => s.toggleCavemanMode);

  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setIsMcpsOpen = useOpenZStore((s) => s.setIsMcpsOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);
  const newSession = useOpenZStore((s) => s.newSession);

  const activeSession = sessions.find((s) => s.id === activeChatId);
  const failedMcps = mcpServers.filter((s) => s.status === 'error' || s.status === 'disabled').length;

  return (
    <div className="mx-auto max-w-5xl px-4 py-6">
      {/* Hero strip */}
      <div className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <Sparkles className="h-5 w-5 text-amber-500" />
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
        />
        <StatCard
          label="Sessions"
          value={String(sessions.length)}
          sub={activeSession?.title ? `Active: ${activeSession.title}` : 'No active session'}
          icon={MessageSquare}
        />
        <StatCard
          label="MCP Servers"
          value={`${mcpStats.loaded}/${mcpStats.total}`}
          sub={failedMcps > 0 ? `${failedMcps} failing` : 'all healthy'}
          icon={Cpu}
          accent={failedMcps > 0 ? 'text-red-400' : 'text-amber-500'}
          onClick={() => {
            setIsMcpsOpen(true);
            wsService.requestMcpServers();
          }}
        />
        <StatCard
          label="Cognitive Memory"
          value={`${cognitiveStats.entitiesCount} entities`}
          sub={`${cognitiveStats.relationsCount} relations · ${cognitiveStats.factsCount} facts`}
          icon={Brain}
          onClick={() => {
            setIsMemoryOpen(true);
            wsService.requestCognitiveMemory();
          }}
        />
        <StatCard
          label="Slash Commands"
          value={String(slashCommands.length)}
          sub="loaded from gateway"
          icon={SlashSquare}
        />
        <StatCard
          label="Runtime Logs"
          value={logs.length > 0 ? `${logs.length} entries` : 'live stream'}
          sub="live stream available"
          icon={ScrollText}
          onClick={() => {
            setIsLogsOpen(true);
            wsService.requestLogs();
          }}
        />
        <StatCard
          label="Agent Defaults"
          value={`${settings?.max_tokens ?? '—'} tokens`}
          sub={`${settings?.max_messages ?? '—'} max messages`}
          icon={TerminalSquare}
          onClick={() => setIsSettingsOpen(true)}
        />
        <StatCard
          label="Activity"
          value={status?.version ? 'running' : 'unknown'}
          sub="background curator active"
          icon={Activity}
        />
      </div>

      {/* Toggles */}
      <div className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2">
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

      {/* Quick actions */}
      <div className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
        <QuickAction label="Memory" icon={Brain} onClick={() => { setIsMemoryOpen(true); wsService.requestCognitiveMemory(); }} />
        <QuickAction label="MCP Servers" icon={Cpu} onClick={() => { setIsMcpsOpen(true); wsService.requestMcpServers(); }} />
        <QuickAction label="Logs" icon={ScrollText} onClick={() => { setIsLogsOpen(true); wsService.requestLogs(); }} />
        <QuickAction label="Settings" icon={Settings} onClick={() => setIsSettingsOpen(true)} />
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

const QuickAction: React.FC<{
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  onClick: () => void;
}> = ({ label, icon: Icon, onClick }) => (
  <button
    onClick={onClick}
    className="flex flex-col items-center gap-1.5 rounded-xl border border-border/60 bg-card/60 py-4 text-xs font-semibold text-muted-foreground shadow-sm transition hover:border-amber-500/40 hover:bg-card hover:text-foreground"
  >
    <Icon className="h-4 w-4 text-amber-500" />
    {label}
  </button>
);