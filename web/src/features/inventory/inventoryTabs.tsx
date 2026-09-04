import React, { useMemo, useState } from 'react';
import {
  Archive,
  Bot,
  CheckCircle2,
  Clock3,
  Copy,
  Database,
  ExternalLink,
  FileText,
  FolderTree,
  MessageSquare,
  Monitor,
  Pause,
  Play,
  Search,
  Server,
  ShieldCheck,
  Trash2,
  Wrench,
} from 'lucide-react';
import type { CronRunRecord, RuntimeInventory } from '../../types';
import { cn } from '../../lib/utils';
import { Badge } from '../../shared/ui/Badge';
import { KeyValue, Panel } from '../../shared/ui/Panel';
import { MetricCard } from '../../shared/ui/MetricCard';

export type InventoryTab = 'core' | 'sessions' | 'tools' | 'cron' | 'paths' | 'channels';

export const tabs: Array<{ id: InventoryTab; label: string; icon: React.ComponentType<{ className?: string }> }> = [
  { id: 'core', label: 'Core', icon: Archive },
  { id: 'sessions', label: 'Sessions', icon: Monitor },
  { id: 'tools', label: 'Tools', icon: Wrench },
  { id: 'cron', label: 'Cron', icon: Clock3 },
  { id: 'paths', label: 'Paths', icon: FolderTree },
  { id: 'channels', label: 'Channels', icon: Server },
];

export const InventorySummary: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
  <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-5">
    <MetricCard label="Version" value={`v${inventory.version}`} icon={ShieldCheck} />
    <MetricCard label="Subagents" value={`${inventory.counts.subagents}`} detail={`${inventory.counts.coreSubagents} core / ${inventory.counts.customSubagents} custom`} icon={Bot} />
    <MetricCard label="Tools" value={`${inventory.counts.tools}`} detail="Backend tool registry" icon={Wrench} />
    <MetricCard
      label="Cron Jobs"
      value={`${inventory.counts.runningCronJobs}/${inventory.counts.cronJobs}`}
      detail={`${inventory.counts.activeCronJobs} enabled · ${inventory.cron.recentRuns} recorded runs`}
      icon={Clock3}
    />
    <MetricCard label="Sessions" value={`${inventory.counts.activeUiSessions}/${inventory.counts.sessions}`} detail="Active UI / recent" icon={Monitor} />
  </div>
);

export const CoreTab: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
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
      <KeyValue label="Subagents" value={`${inventory.counts.subagents} (${inventory.counts.coreSubagents} core / ${inventory.counts.customSubagents} custom)`} />
      <KeyValue label="Skills" value={String(inventory.counts.skills)} />
      <KeyValue label="Channels" value={`${inventory.counts.enabledChannels}/${inventory.counts.channels} enabled`} />
      <KeyValue label="Tools" value={String(inventory.counts.tools)} />
      <KeyValue label="Cron Jobs" value={`${inventory.counts.runningCronJobs} running / ${inventory.counts.activeCronJobs} enabled / ${inventory.counts.cronJobs} total`} />
      <KeyValue label="Active UI Sessions" value={String(inventory.counts.activeUiSessions)} />
      <KeyValue label="Recent Sessions" value={String(inventory.counts.sessions)} />
      <KeyValue label="Memory DB" value={inventory.memory.memoryDb.exists ? 'Found' : 'Missing'} />
      <KeyValue label="Graph DB" value={inventory.memory.graphDb.exists ? 'Found' : 'Missing'} />
      <KeyValue label="Workspace" value={inventory.paths.workspace} mono />
    </Panel>
    <Panel title={`Cron Jobs (${inventory.cron.jobs.length})`} icon={Clock3}>
      <div className="space-y-2">
        {inventory.cron.jobs.map((job) => (
          <div key={job.id} className="rounded-lg border border-border/40 bg-background/30 p-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-mono text-xs font-semibold text-foreground">{job.id}</span>
              <div className="flex gap-1.5">
                <Badge tone={job.enabled ? 'green' : 'muted'}>{job.enabled ? 'enabled' : 'disabled'}</Badge>
                <Badge tone={job.status === 'failed' ? 'red' : job.status === 'running' ? 'amber' : 'muted'}>{job.status}</Badge>
              </div>
            </div>
            <div className="mt-2 space-y-1 text-[11px] text-muted-foreground">
              <div><span className="font-semibold text-foreground">Schedule:</span> <span className="font-mono">{job.schedule}</span></div>
              <div className="line-clamp-2 break-words"><span className="font-semibold text-foreground">Prompt:</span> {job.prompt || 'No prompt recorded.'}</div>
              <div>Next: {job.nextRun || 'Not scheduled'} · Last: {job.lastRun || job.lastFinishedAt || 'Never'}</div>
              <div>Runs: {job.runCount} total / {job.failureCount} failed</div>
              {job.lastError && <div className="break-words text-red-400">{job.lastError}</div>}
            </div>
          </div>
        ))}
        {inventory.cron.jobs.length === 0 && (
          <div className="rounded-lg border border-dashed border-border/50 p-4 text-sm text-muted-foreground">
            No jobs were read from <span className="font-mono text-[11px]">{inventory.cron.jobsFile}</span>.
          </div>
        )}
      </div>
    </Panel>
    <Panel title="Loaded Components" icon={Wrench}>
      <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-3">
        <KeyValue label="Skills" value={String(inventory.skills.length)} compact />
        <KeyValue label="Subagents" value={String(inventory.subagents.length)} compact />
        <KeyValue label="Tools" value={String(inventory.tools.length)} compact />
        <KeyValue label="Channels" value={String(inventory.channels.length)} compact />
        <KeyValue label="Sessions" value={String(inventory.sessions.recentSessions.length)} compact />
        <KeyValue label="Cron Runs" value={String(inventory.cron.recentRuns)} compact />
      </div>
      <div className="mt-3 space-y-1 text-[11px] text-muted-foreground">
        <div className="break-words font-mono">Skills: {inventory.paths.skillsDir}</div>
        <div className="break-words font-mono">Subagents: {inventory.paths.subagentsFile}</div>
      </div>
    </Panel>
    <Panel title={`Subagents (${inventory.subagents.length})`} icon={Bot}>
      <div className="max-h-[360px] space-y-2 overflow-y-auto pr-1">
        {inventory.subagents.map((agent) => (
          <div key={agent.name} className="rounded-lg border border-border/40 bg-background/30 p-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-mono text-xs font-semibold text-foreground">{agent.name}</span>
              <div className="flex gap-1.5">
                <Badge tone={agent.isCore ? 'amber' : 'muted'}>{agent.isCore ? 'core' : 'custom'}</Badge>
                {agent.supportsVision && <Badge>vision</Badge>}
              </div>
            </div>
            <div className="mt-2 text-[11px] text-muted-foreground">
              {agent.effectiveProvider}/{agent.effectiveModel} · {agent.capabilities.join(', ') || 'general'}
            </div>
            {agent.lastError && <div className="mt-1 break-words text-[11px] text-red-400">{agent.lastError}</div>}
          </div>
        ))}
        {inventory.subagents.length === 0 && <div className="py-6 text-center text-sm text-muted-foreground">No subagent profiles loaded.</div>}
      </div>
    </Panel>
    <Panel title={`Skills (${inventory.skills.length})`} icon={FileText}>
      <div className="max-h-[360px] space-y-2 overflow-y-auto pr-1">
        {inventory.skills.map((skill) => (
          <div key={`${skill.scope || 'global'}:${skill.name}`} className="rounded-lg border border-border/40 bg-background/30 p-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-mono text-xs font-semibold text-foreground">{skill.name}</span>
              <div className="flex gap-1.5">
                <Badge>{skill.scope || 'global'}</Badge>
                {skill.isProtected && <Badge tone="amber">read-only</Badge>}
                {(skill.validationErrors?.length || 0) > 0 && <Badge tone="red">invalid</Badge>}
              </div>
            </div>
            <div className="mt-2 line-clamp-2 text-[11px] text-muted-foreground">{skill.content || 'No content recorded.'}</div>
            <div className="mt-1 text-[10px] text-muted-foreground/70">Uses: {skill.useCount ?? 0} · Source: {skill.source || 'unknown'}</div>
          </div>
        ))}
        {inventory.skills.length === 0 && <div className="py-6 text-center text-sm text-muted-foreground">No skills loaded.</div>}
      </div>
    </Panel>
  </div>
);

export const SessionsTab: React.FC<{
  inventory: RuntimeInventory;
  activeSessionKey: string;
  onOpen: (sessionKey: string) => void;
  onCopy: (sessionKey: string) => void;
  onArchive: (sessionKey: string) => void;
  onDelete: (sessionKey: string) => void;
}> = ({ inventory, activeSessionKey, onOpen, onCopy, onArchive, onDelete }) => {
  const [sessionSearch, setSessionSearch] = useState('');
  const [channelFilter, setChannelFilter] = useState('all');
  const [activeOnly, setActiveOnly] = useState(false);

  const channelOptions = useMemo(() => {
    const channels = new Set(inventory.sessions.recentSessions.map((session) => session.channel));
    inventory.sessions.activeUiSessions.forEach((session) => channels.add(session.channel));
    return ['all', ...Array.from(channels).sort()];
  }, [inventory.sessions.activeUiSessions, inventory.sessions.recentSessions]);

  const filteredSessions = useMemo(() => {
    const query = sessionSearch.trim().toLowerCase();
    return inventory.sessions.recentSessions.filter((session) => {
      if (channelFilter !== 'all' && session.channel !== channelFilter) return false;
      if (activeOnly && !session.active && session.key !== activeSessionKey) return false;
      if (!query) return true;
      return [session.key, session.channel, session.title, session.filePath]
        .join(' ')
        .toLowerCase()
        .includes(query);
    });
  }, [activeOnly, activeSessionKey, channelFilter, inventory.sessions.recentSessions, sessionSearch]);

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_380px]">
      <Panel title={`Active UI Sessions (${inventory.sessions.activeUiSessions.length})`} icon={Monitor}>
        <div className="space-y-2">
          {inventory.sessions.activeUiSessions.map((session) => (
            <div key={`${session.sessionKey}-${session.pid}`} className="rounded-xl border border-border/50 bg-background/35 p-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="font-mono text-xs font-semibold text-foreground">{session.sessionKey}</span>
                <div className="flex flex-wrap gap-1.5">
                  <Badge tone="green">active</Badge>
                  <Badge>{session.channel}</Badge>
                </div>
              </div>
              <div className="mt-3 grid grid-cols-1 gap-2 text-xs sm:grid-cols-2">
                <KeyValue label="Model" value={session.model || 'default'} compact />
                <KeyValue label="Provider" value={session.provider || 'auto'} compact />
                <KeyValue label="PID" value={String(session.pid)} compact />
                <KeyValue label="Last Seen" value={session.lastSeenAt} compact />
                <KeyValue label="CWD" value={session.cwd} mono compact />
                <KeyValue label="Preview" value={session.preview || 'No user prompt yet'} compact />
              </div>
            </div>
          ))}
          {inventory.sessions.activeUiSessions.length === 0 && (
            <div className="py-8 text-center text-sm text-muted-foreground">No active TUI/Ratatui sessions reported.</div>
          )}
        </div>
      </Panel>
      <div className="space-y-4">
        <Panel title="WebUI Control Center" icon={Server}>
          <KeyValue label="Connected Clients" value={String(inventory.sessions.webuiControlCenter.connectedClients)} />
          <KeyValue
            label="Attached Chats"
            value={
              inventory.sessions.webuiControlCenter.attachedChats.length > 0
                ? inventory.sessions.webuiControlCenter.attachedChats.join(', ')
                : 'None'
            }
            mono
          />
        </Panel>
        <Panel title="Session Channels" icon={MessageSquare}>
          <div className="space-y-2">
            {inventory.sessions.channelCounts.map((item) => (
              <div key={item.channel} className="flex items-center justify-between rounded-lg border border-border/40 bg-background/30 px-3 py-2">
                <span className="text-xs font-semibold capitalize text-foreground">{item.channel}</span>
                <Badge>{item.count}</Badge>
              </div>
            ))}
            {inventory.sessions.channelCounts.length === 0 && <div className="text-sm text-muted-foreground">No persisted sessions found.</div>}
          </div>
        </Panel>
        <Panel title={`Recent Sessions (${filteredSessions.length}/${inventory.sessions.recentSessions.length})`} icon={Archive}>
          <div className="mb-3 space-y-2">
            <div className="flex items-center gap-2 rounded-xl border border-border/60 bg-background/50 px-3 py-2">
              <Search className="h-4 w-4 text-muted-foreground" />
              <input
                value={sessionSearch}
                onChange={(event) => setSessionSearch(event.target.value)}
                placeholder="Search sessions by key, title, channel, or path"
                className="min-w-0 flex-1 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none"
              />
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <select
                value={channelFilter}
                onChange={(event) => setChannelFilter(event.target.value)}
                className="rounded-lg border border-border/60 bg-background/50 px-3 py-2 text-xs font-semibold text-foreground focus:outline-none focus:ring-2 focus:ring-amber-500/40"
              >
                {channelOptions.map((channel) => (
                  <option key={channel} value={channel}>
                    {channel === 'all' ? 'All channels' : channel}
                  </option>
                ))}
              </select>
              <label className="flex items-center gap-2 rounded-lg border border-border/60 bg-background/40 px-3 py-2 text-xs font-semibold text-muted-foreground">
                <input
                  type="checkbox"
                  checked={activeOnly}
                  onChange={(event) => setActiveOnly(event.target.checked)}
                  className="h-3.5 w-3.5 accent-amber-500"
                />
                Active only
              </label>
            </div>
          </div>
          <div className="max-h-[420px] space-y-2 overflow-y-auto pr-1">
            {filteredSessions.map((session) => {
              const isOpened = session.key === activeSessionKey;
              return (
                <div key={session.key} className="rounded-lg border border-border/40 bg-background/30 p-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="font-mono text-[11px] font-semibold text-foreground">{session.key}</span>
                    <div className="flex flex-wrap gap-1.5">
                      {isOpened && <Badge tone="amber">opened</Badge>}
                      {session.active && <Badge tone="green">active</Badge>}
                      <Badge>{session.channel}</Badge>
                    </div>
                  </div>
                  <div className="mt-2 space-y-1 text-[11px] text-muted-foreground">
                    <div className="line-clamp-2 text-foreground">{session.title}</div>
                    <div>{session.messageCount} messages | {session.updatedAt}</div>
                    <div className="break-words font-mono">{session.filePath}</div>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2 border-t border-border/30 pt-3">
                    <CronAction icon={ExternalLink} label="Open" onClick={() => onOpen(session.key)} />
                    <CronAction icon={Copy} label="Copy Key" onClick={() => onCopy(session.key)} />
                    <CronAction
                      icon={Archive}
                      label={isOpened ? 'Opened' : 'Archive'}
                      disabled={isOpened}
                      onClick={() => {
                        if (window.confirm(`Archive session ${session.key}?`)) onArchive(session.key);
                      }}
                    />
                    <CronAction
                      icon={Trash2}
                      label={isOpened ? 'Opened' : 'Delete'}
                      danger
                      disabled={isOpened}
                      onClick={() => {
                        if (window.confirm(`Delete session ${session.key}?`)) onDelete(session.key);
                      }}
                    />
                  </div>
                </div>
              );
            })}
            {filteredSessions.length === 0 && <div className="py-6 text-center text-sm text-muted-foreground">No sessions match this filter.</div>}
          </div>
        </Panel>
      </div>
    </div>
  );
};

export const ToolsTab: React.FC<{
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

export const CronTab: React.FC<{
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
              <KeyValue label="Prompt" value={job.prompt || 'No prompt recorded.'} compact />
              <KeyValue label="Notify" value={job.notifyOn} compact />
              <KeyValue label="Runs" value={`${job.runCount} total / ${job.failureCount} failed`} compact />
              <KeyValue label="Next Run" value={job.nextRun || 'Not scheduled'} compact />
              <KeyValue label="Last Run" value={job.lastRun || job.lastFinishedAt || 'Never'} compact />
              <KeyValue label="Last Log" value={job.lastLogPath || 'None'} mono compact />
              <KeyValue label="Last Error" value={job.lastError || 'None'} compact />
              <KeyValue label="Updated" value={job.updatedAt || job.createdAt || 'Unknown'} compact />
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

export const PathsTab: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
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

export const ChannelsTab: React.FC<{ inventory: RuntimeInventory }> = ({ inventory }) => (
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
  disabled?: boolean;
  onClick: () => void;
}> = ({ icon: Icon, label, danger, disabled, onClick }) => (
  <button
    type="button"
    onClick={onClick}
    disabled={disabled}
    className={cn(
      'flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[11px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50 disabled:cursor-not-allowed disabled:opacity-50',
      danger
        ? 'border-red-500/30 bg-red-500/10 text-red-400 hover:bg-red-500/15 disabled:hover:bg-red-500/10'
        : 'border-border/60 bg-background/40 text-muted-foreground hover:border-amber-500/40 hover:text-foreground disabled:hover:border-border/60 disabled:hover:text-muted-foreground',
    )}
  >
    <Icon className="h-3.5 w-3.5" /> {label}
  </button>
);

function humanize(value: string): string {
  return value.replace(/([A-Z])/g, ' $1').replace(/^./, (char) => char.toUpperCase());
}
