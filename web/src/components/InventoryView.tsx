import React, { useMemo, useState } from 'react';
import {
  Archive,
  Bot,
  CheckCircle2,
  Clock3,
  FileText,
  Database,
  FolderTree,
  Pause,
  Play,
  RefreshCw,
  Search,
  Server,
  ShieldCheck,
  Trash2,
  Wrench,
  X,
} from 'lucide-react';
import { useOpenZStore } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import type { CronRunRecord, RuntimeInventory } from '../types';
import { cn } from '../lib/utils';

type InventoryTab = 'core' | 'tools' | 'cron' | 'paths' | 'channels';

const tabs: Array<{ id: InventoryTab; label: string; icon: React.ComponentType<{ className?: string }> }> = [
  { id: 'core', label: 'Core', icon: Archive },
  { id: 'tools', label: 'Tools', icon: Wrench },
  { id: 'cron', label: 'Cron', icon: Clock3 },
  { id: 'paths', label: 'Paths', icon: FolderTree },
  { id: 'channels', label: 'Channels', icon: Server },
];

export const InventoryView: React.FC = () => {
  const inventory = useOpenZStore((s) => s.runtimeInventory);
  const cronLogs = useOpenZStore((s) => s.cronLogs);
  const pauseCronJob = useOpenZStore((s) => s.pauseCronJob);
  const resumeCronJob = useOpenZStore((s) => s.resumeCronJob);
  const deleteCronJob = useOpenZStore((s) => s.deleteCronJob);
  const requestCronLogs = useOpenZStore((s) => s.requestCronLogs);
  const notice = useOpenZStore((s) => s.workspaceNotice);
  const clearWorkspaceNotice = useOpenZStore((s) => s.clearWorkspaceNotice);
  const [activeTab, setActiveTab] = useState<InventoryTab>('core');
  const [toolSearch, setToolSearch] = useState('');

  const filteredTools = useMemo(() => {
    const query = toolSearch.trim().toLowerCase();
    const tools = inventory?.tools ?? [];
    if (!query) return tools;
    return tools.filter((tool) => {
      return (
        tool.name.toLowerCase().includes(query) ||
        tool.domain.toLowerCase().includes(query) ||
        tool.risk.toLowerCase().includes(query) ||
        tool.description.toLowerCase().includes(query)
      );
    });
  }, [inventory?.tools, toolSearch]);

  return (
    <div className="mx-auto max-w-6xl px-4 py-8">
      <div className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <Archive className="h-5 w-5 text-amber-500" />
            Core Inventory
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            One OpenZ core exposed to every UI and channel from the gateway.
          </p>
        </div>
        <button
          type="button"
          onClick={() => wsService.requestRuntimeInventory()}
          className="flex items-center justify-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs font-semibold text-amber-400 transition hover:bg-amber-500/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
        >
          <RefreshCw className="h-4 w-4" /> Refresh
        </button>
      </div>

      {notice?.scope === 'inventory' && (
        <div
          className={cn(
            'mb-4 flex items-center justify-between gap-3 rounded-xl border px-4 py-3 text-sm',
            notice.type === 'error'
              ? 'border-red-500/30 bg-red-500/10 text-red-300'
              : notice.type === 'success'
                ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300'
                : 'border-amber-500/30 bg-amber-500/10 text-amber-300',
          )}
        >
          <span>{notice.message}</span>
          <button
            type="button"
            onClick={() => clearWorkspaceNotice('inventory')}
            className="rounded p-1 opacity-80 transition hover:bg-background/40 hover:opacity-100"
            aria-label="Dismiss inventory notice"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      )}

      {!inventory ? (
        <div className="rounded-2xl border border-border/60 bg-card/45 p-8 text-sm text-muted-foreground">
          Waiting for gateway inventory. Use Refresh if the gateway is already connected.
        </div>
      ) : (
        <>
          <InventorySummary inventory={inventory} />

          <div className="mt-6 flex flex-wrap gap-2 border-b border-border/50 pb-3">
            {tabs.map((tab) => {
              const Icon = tab.icon;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setActiveTab(tab.id)}
                  className={cn(
                    'flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50',
                    activeTab === tab.id
                      ? 'bg-amber-500/15 text-amber-400'
                      : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                  )}
                >
                  <Icon className="h-4 w-4" /> {tab.label}
                </button>
              );
            })}
          </div>

          <div className="mt-6">
            {activeTab === 'core' && <CoreTab inventory={inventory} />}
            {activeTab === 'tools' && (
              <ToolsTab tools={filteredTools} totalTools={inventory.tools.length} search={toolSearch} onSearch={setToolSearch} />
            )}
            {activeTab === 'cron' && (
              <CronTab
                inventory={inventory}
                cronLogs={cronLogs}
                onPause={pauseCronJob}
                onResume={resumeCronJob}
                onDelete={deleteCronJob}
                onLoadLogs={requestCronLogs}
              />
            )}
            {activeTab === 'paths' && <PathsTab inventory={inventory} />}
            {activeTab === 'channels' && <ChannelsTab inventory={inventory} />}
          </div>
        </>
      )}
    </div>
  );
};

const InventorySummary: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
  <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
    <Metric label="Version" value={`v${inventory.version}`} icon={ShieldCheck} />
    <Metric label="Subagents" value={`${inventory.counts.subagents}`} sub={`${inventory.counts.coreSubagents} core / ${inventory.counts.customSubagents} custom`} icon={Bot} />
    <Metric label="Tools" value={`${inventory.counts.tools}`} sub="Backend tool registry" icon={Wrench} />
    <Metric label="Cron" value={`${inventory.counts.activeCronJobs}/${inventory.counts.cronJobs}`} sub="Active jobs" icon={Clock3} />
  </div>
);

const CoreTab: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
  <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
    <Panel title="Runtime Defaults" icon={Archive}>
      <KeyValue label="Model" value={inventory.defaults.model} />
      <KeyValue label="Provider" value={inventory.defaults.provider} />
      <KeyValue label="Streaming" value={inventory.defaults.streaming ? 'Enabled' : 'Disabled'} />
      <KeyValue label="Caveman Mode" value={inventory.defaults.cavemanMode ? 'Enabled' : 'Disabled'} />
      <KeyValue label="Max Messages" value={String(inventory.defaults.maxMessages)} />
      <KeyValue label="Max Tool Iterations" value={String(inventory.defaults.maxToolIterations)} />
      <KeyValue label="Tool Timeout" value={`${inventory.defaults.toolTimeoutSecs}s`} />
    </Panel>
    <Panel title="Core Counts" icon={Database}>
      <KeyValue label="Skills" value={String(inventory.counts.skills)} />
      <KeyValue label="Channels" value={`${inventory.counts.enabledChannels}/${inventory.counts.channels} enabled`} />
      <KeyValue label="Memory DB" value={inventory.memory.memoryDb.exists ? 'Found' : 'Missing'} />
      <KeyValue label="Graph DB" value={inventory.memory.graphDb.exists ? 'Found' : 'Missing'} />
      <KeyValue label="Workspace" value={inventory.paths.workspace} mono />
    </Panel>
  </div>
);

const ToolsTab: React.FC<{
  tools: RuntimeInventory['tools'];
  totalTools: number;
  search: string;
  onSearch: (value: string) => void;
}> = ({ tools, totalTools, search, onSearch }) => (
  <Panel title={`Tools (${tools.length}/${totalTools})`} icon={Wrench}>
    <div className="mb-4 flex items-center gap-2 rounded-xl border border-border/60 bg-background/50 px-3 py-2">
      <Search className="h-4 w-4 text-muted-foreground" />
      <input
        value={search}
        onChange={(event) => onSearch(event.target.value)}
        placeholder="Search tools, domains, risk, or descriptions"
        className="min-w-0 flex-1 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none"
      />
    </div>
    <div className="max-h-[560px] space-y-2 overflow-y-auto pr-1">
      {tools.map((tool) => (
        <div key={tool.name} className="rounded-xl border border-border/50 bg-background/35 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-xs font-semibold text-foreground">{tool.name}</span>
            <Badge>{tool.domain}</Badge>
            <Badge tone={tool.risk === 'safe' ? 'green' : tool.risk === 'dangerous' ? 'red' : 'amber'}>{tool.risk}</Badge>
            {tool.requiresApproval && <Badge tone="amber">approval</Badge>}
            {tool.usesNetwork && <Badge>network</Badge>}
            {tool.writesDisk && <Badge>disk write</Badge>}
            {tool.spawnsProcess && <Badge>process</Badge>}
          </div>
          <p className="mt-2 line-clamp-2 text-xs text-muted-foreground">{tool.description || 'No description.'}</p>
        </div>
      ))}
      {tools.length === 0 && <div className="py-8 text-center text-sm text-muted-foreground">No tools match this filter.</div>}
    </div>
  </Panel>
);

const CronTab: React.FC<{
  inventory: RuntimeInventory;
  cronLogs: CronRunRecord[];
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onDelete: (id: string) => void;
  onLoadLogs: (id?: string, limit?: number) => void;
}> = ({ inventory, cronLogs, onPause, onResume, onDelete, onLoadLogs }) => (
  <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_380px]">
    <Panel title={`Cron Jobs (${inventory.cron.jobs.length})`} icon={Clock3}>
      <div className="space-y-2">
        {inventory.cron.jobs.map((job) => (
          <div key={job.id} className="rounded-xl border border-border/50 bg-background/35 p-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="font-mono text-xs font-semibold text-foreground">Job {job.id}</div>
              <div className="flex flex-wrap gap-1.5">
                <Badge tone={job.enabled ? 'green' : 'muted'}>{job.enabled ? 'enabled' : 'disabled'}</Badge>
                <Badge tone={job.status === 'failed' ? 'red' : job.status === 'running' ? 'amber' : 'muted'}>{job.status}</Badge>
                {job.quiet && <Badge>quiet</Badge>}
                {job.runOnce && <Badge>run once</Badge>}
              </div>
            </div>
            <div className="mt-3 grid grid-cols-1 gap-2 text-xs sm:grid-cols-2">
              <KeyValue label="Schedule" value={job.schedule} mono compact />
              <KeyValue label="Notify" value={job.notifyOn} compact />
              <KeyValue label="Runs" value={`${job.runCount} total / ${job.failureCount} failed`} compact />
              <KeyValue label="Last Run" value={job.lastRun || job.lastFinishedAt || 'Never'} compact />
              <KeyValue label="Last Log" value={job.lastLogPath || 'None'} mono compact />
              <KeyValue label="Last Error" value={job.lastError || 'None'} compact />
            </div>
            <div className="mt-3 flex flex-wrap gap-2 border-t border-border/30 pt-3">
              {job.enabled ? (
                <CronAction icon={Pause} label="Pause" onClick={() => onPause(job.id)} />
              ) : (
                <CronAction icon={Play} label="Resume" onClick={() => onResume(job.id)} />
              )}
              <CronAction icon={FileText} label="Logs" onClick={() => onLoadLogs(job.id, 20)} />
              <CronAction
                icon={Trash2}
                label="Delete"
                danger
                onClick={() => {
                  if (window.confirm(`Delete cron job ${job.id}?`)) onDelete(job.id);
                }}
              />
            </div>
          </div>
        ))}
        {inventory.cron.jobs.length === 0 && <div className="py-8 text-center text-sm text-muted-foreground">No cron jobs registered.</div>}
      </div>
    </Panel>
    <div className="space-y-4">
      <Panel title="Cron Storage" icon={FolderTree}>
        <KeyValue label="Jobs File" value={inventory.cron.jobsFile} mono />
        <KeyValue label="Runs File" value={inventory.cron.runsFile} mono />
        <KeyValue label="Recent Runs Loaded" value={String(inventory.cron.recentRuns)} />
        <button
          type="button"
          onClick={() => onLoadLogs(undefined, 20)}
          className="mt-3 flex w-full items-center justify-center gap-2 rounded-lg border border-border/60 bg-background/40 px-3 py-2 text-xs font-semibold text-muted-foreground transition hover:border-amber-500/40 hover:text-foreground"
        >
          <FileText className="h-4 w-4" /> Load Recent Runs
        </button>
      </Panel>
      <Panel title={`Loaded Runs (${cronLogs.length})`} icon={FileText}>
        <div className="max-h-[360px] space-y-2 overflow-y-auto pr-1">
          {cronLogs.map((run) => (
            <div key={run.run_id} className="rounded-lg border border-border/40 bg-background/30 p-3">
              <div className="flex items-center justify-between gap-2">
                <span className="font-mono text-[11px] font-semibold text-foreground">{run.job_id}</span>
                <Badge tone={run.status === 'failed' ? 'red' : run.status === 'success' ? 'green' : 'muted'}>{run.status}</Badge>
              </div>
              <div className="mt-2 space-y-1 text-[11px] text-muted-foreground">
                <div>{run.started_at}</div>
                {run.log_path && <div className="break-words font-mono">{run.log_path}</div>}
                {run.error && <div className="break-words text-red-400">{run.error}</div>}
                {run.summary && <div className="line-clamp-3 break-words">{run.summary}</div>}
              </div>
            </div>
          ))}
          {cronLogs.length === 0 && <div className="py-6 text-center text-sm text-muted-foreground">No run logs loaded.</div>}
        </div>
      </Panel>
    </div>
  </div>
);

const PathsTab: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
  <Panel title="Runtime Paths" icon={FolderTree}>
    <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
      {Object.entries(inventory.paths).map(([key, value]) => (
        <KeyValue key={key} label={humanize(key)} value={value} mono />
      ))}
      <KeyValue label="Memory DB" value={inventory.memory.memoryDb.path} mono />
      <KeyValue label="Graph DB" value={inventory.memory.graphDb.path} mono />
    </div>
  </Panel>
);

const ChannelsTab: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
  <Panel title={`Channels (${inventory.counts.enabledChannels}/${inventory.counts.channels} enabled)`} icon={Server}>
    <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
      {inventory.channels.map((channel) => (
        <div key={channel.name} className="rounded-xl border border-border/50 bg-background/35 p-4">
          <div className="flex items-center justify-between gap-3">
            <div className="font-semibold capitalize text-foreground">{channel.name}</div>
            {channel.enabled ? <CheckCircle2 className="h-4 w-4 text-emerald-400" /> : <span className="h-2.5 w-2.5 rounded-full bg-muted" />}
          </div>
          <div className="mt-3 flex flex-wrap gap-1.5">
            <Badge tone={channel.enabled ? 'green' : 'muted'}>{channel.enabled ? 'enabled' : 'disabled'}</Badge>
            <Badge tone={channel.configured ? 'green' : 'muted'}>{channel.configured ? 'configured' : 'not configured'}</Badge>
          </div>
        </div>
      ))}
    </div>
  </Panel>
);

const CronAction: React.FC<{
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  danger?: boolean;
  onClick: () => void;
}> = ({ icon: Icon, label, danger, onClick }) => (
  <button
    type="button"
    onClick={onClick}
    className={cn(
      'flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[11px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50',
      danger
        ? 'border-red-500/30 bg-red-500/10 text-red-400 hover:bg-red-500/15'
        : 'border-border/60 bg-background/40 text-muted-foreground hover:border-amber-500/40 hover:text-foreground',
    )}
  >
    <Icon className="h-3.5 w-3.5" /> {label}
  </button>
);

const Metric: React.FC<{
  label: string;
  value: string;
  sub?: string;
  icon: React.ComponentType<{ className?: string }>;
}> = ({ label, value, sub, icon: Icon }) => (
  <div className="rounded-xl border border-border/60 bg-card/60 p-4 shadow-sm">
    <div className="flex items-center justify-between">
      <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">{label}</span>
      <Icon className="h-4 w-4 text-amber-500" />
    </div>
    <div className="mt-1.5 truncate text-xl font-extrabold tracking-tight text-foreground">{value}</div>
    {sub && <div className="mt-0.5 truncate text-[11px] text-muted-foreground">{sub}</div>}
  </div>
);

const Panel: React.FC<{
  title: string;
  icon: React.ComponentType<{ className?: string }>;
  children: React.ReactNode;
}> = ({ title, icon: Icon, children }) => (
  <section className="rounded-2xl border border-border/60 bg-card/45 p-5 shadow-sm backdrop-blur-md">
    <div className="mb-4 flex items-center gap-2 border-b border-border/40 pb-2">
      <Icon className="h-4 w-4 text-amber-500" />
      <h2 className="text-sm font-bold text-foreground">{title}</h2>
    </div>
    {children}
  </section>
);

const KeyValue: React.FC<{ label: string; value: string; mono?: boolean; compact?: boolean }> = ({ label, value, mono, compact }) => (
  <div className={cn('rounded-lg border border-border/30 bg-background/30 px-3 py-2', compact && 'border-0 bg-transparent px-0 py-0')}>
    <div className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">{label}</div>
    <div className={cn('mt-0.5 break-words text-xs text-foreground', mono && 'font-mono')}>{value}</div>
  </div>
);

const Badge: React.FC<{ children: React.ReactNode; tone?: 'amber' | 'green' | 'red' | 'muted' }> = ({ children, tone = 'muted' }) => {
  const classes = {
    amber: 'bg-amber-500/10 text-amber-400',
    green: 'bg-emerald-500/10 text-emerald-400',
    red: 'bg-red-500/10 text-red-400',
    muted: 'bg-muted/70 text-muted-foreground',
  }[tone];
  return <span className={cn('rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider', classes)}>{children}</span>;
};

function humanize(value: string): string {
  return value.replace(/([A-Z])/g, ' $1').replace(/^./, (char) => char.toUpperCase());
}
