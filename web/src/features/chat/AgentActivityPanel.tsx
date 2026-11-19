import React, { useEffect, useMemo, useState } from 'react';
import {
  Bot,
  CheckCircle2,
  Clock,
  Eye,
  Filter,
  Loader2,
  PanelRightClose,
  RefreshCw,
  ScrollText,
  Search,
  Server,
  Square,
  Terminal,
  XCircle,
} from 'lucide-react';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import {
  KokonutSmoothDrawer,
  SpectrumStatusBadge,
} from '../../components/registries/modernRegistries';
import { useUiCustomizationStore } from '../../store/useUiCustomizationStore';
import { cn } from '../../lib/utils';
import { wsService } from '../../services/websocket';
import { OrchestrationRunPanel } from '../../shared/ui/OrchestrationRunPanel';
import { useOpenZStore } from '../../store/useOpenZStore';
import type {
  LogEntry,
  OpenZMessage,
  OrchestrationRunState,
  ToolExecution,
} from '../../types';
import { buildMessageActivitySteps } from '../activity/activityAdapters';
import {
  classifyLogScope,
  prepareScopedLogs,
  presentLogItem,
} from '../logs/logPresentation';

export { buildMessageActivitySteps };

interface AgentActivityPanelProps {
  open?: boolean;
  messages?: OpenZMessage[];
  isStreaming?: boolean;
  orchestrationRuns?: OrchestrationRunState[];
  onClose?: () => void;
}

type ActivityDrawerTab = 'all' | 'jobs' | 'subagents' | 'logs';
type LogsScopeFilter = 'all' | 'session' | 'background' | 'system';

interface BackgroundJobItem {
  id: string;
  kind: 'server' | 'command' | 'cron' | 'session';
  title: string;
  command: string;
  status: 'running' | 'completed' | 'failed';
  meta?: string;
  timestamp: number;
  serverId?: string;
  sessionId?: string;
}

interface SubagentActivityItem {
  id: string;
  agent: string;
  task: string;
  status: 'running' | 'completed' | 'failed';
  durationLabel?: string;
  timestamp: number;
}

const JOB_TOOL_NAMES = new Set([
  'exec_command',
  'cargo_manager',
  'cargo_build',
  'cargo_check',
  'cargo_test',
  'manage_servers',
  'onpkg',
  'schedule_job',
]);

const SUBAGENT_TOOL_NAMES = new Set([
  'delegate_task',
  'parallel_research',
  'orchestrate_workflow',
  'evaluator_optimizer_loop',
]);

const BADGE_VARIANT_BY_LEVEL: Record<
  LogEntry['level'],
  'destructive' | 'warning' | 'success' | 'outline'
> = {
  ERROR: 'destructive',
  WARN: 'warning',
  INFO: 'success',
  DEBUG: 'outline',
  TRACE: 'outline',
};

function parseArgsObject(raw: ToolExecution['args']): Record<string, unknown> | null {
  if (!raw) return null;
  if (typeof raw === 'object' && !Array.isArray(raw)) return raw;
  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw) as unknown;
      return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
        ? (parsed as Record<string, unknown>)
        : null;
    } catch {
      return null;
    }
  }
  return null;
}

function formatDurationMs(ms?: number): string | undefined {
  if (typeof ms !== 'number' || !Number.isFinite(ms) || ms <= 0) return undefined;
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function extractCommandLabel(tool: ToolExecution): string {
  const args = parseArgsObject(tool.args);
  if (args) {
    const cmd =
      args.command ??
      args.command_line ??
      args.CommandLine ??
      args.cmd ??
      args.subcommand ??
      args.action;
    if (typeof cmd === 'string' && cmd.trim()) {
      const pkg = typeof args.package === 'string' && args.package ? ` -p ${args.package}` : '';
      return tool.name.startsWith('cargo') && !cmd.startsWith('cargo')
        ? `cargo ${cmd.trim()}${pkg}`
        : cmd.trim();
    }
  }
  return tool.name;
}

export const AgentActivityPanel: React.FC<AgentActivityPanelProps> = ({
  open = true,
  onClose,
}) => {
  const [activeTab, setActiveTab] = useState<ActivityDrawerTab>('all');
  const [logScope, setLogScope] = useState<LogsScopeFilter>('all');
  const [essentialOnly, setEssentialOnly] = useState<boolean>(true);
  const [query, setQuery] = useState('');
  const drawerMotionStyle = useUiCustomizationStore((s) => s.drawerMotionStyle);

  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const sessions = useOpenZStore((s) => s.sessions);
  const selectSession = useOpenZStore((s) => s.selectSession);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const messagesByChat = useOpenZStore((s) => s.messages);
  const orchestrationRunsByChat = useOpenZStore((s) => s.orchestrationRuns);
  const activeTurnIds = useOpenZStore((s) => s.activeTurnIds);
  const servers = useOpenZStore((s) => s.servers);
  const logs = useOpenZStore((s) => s.logs);
  const cronLogs = useOpenZStore((s) => s.cronLogs);
  const runtimeInventory = useOpenZStore((s) => s.runtimeInventory);

  const refreshAll = React.useCallback(() => {
    wsService.requestLogs(activeChatId);
    wsService.requestServers();
    wsService.requestRuntimeInventory();
  }, [activeChatId]);

  useEffect(() => {
    if (open) {
      refreshAll();
    }
  }, [open, refreshAll]);

  useEffect(() => {
    if (!open || !onClose) return undefined;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open, onClose]);

  const registeredSubagentNames = useMemo(
    () => new Set((runtimeInventory?.subagents ?? []).map((a) => a.name.toLowerCase())),
    [runtimeInventory?.subagents],
  );

  const allOrchestrationRuns = useMemo(() => {
    return Object.values(orchestrationRunsByChat)
      .flat()
      .sort((a, b) => b.startedAt - a.startedAt)
      .slice(0, 12);
  }, [orchestrationRunsByChat]);

  const { backgroundJobs, subagentItems } = useMemo(() => {
    const jobs: BackgroundJobItem[] = [];
    const subagents: SubagentActivityItem[] = [];
    const now = Date.now();

    // 1. Live managed background servers (dev servers, watchers, processes)
    for (const srv of servers) {
      jobs.push({
        id: `srv-${srv.id}`,
        kind: 'server',
        title: srv.kind || 'Background Server',
        command: srv.command,
        status: 'running',
        meta: `PID ${srv.pid}`,
        timestamp: now + 10_000,
        serverId: srv.id,
      });
    }

    // 2. Concurrent background turns running in non-active sessions
    for (const sid of Object.keys(activeTurnIds)) {
      if (!activeTurnIds[sid] || sid === activeChatId) continue;
      const match = sessions.find((s) => s.id === sid);
      jobs.push({
        id: `bg-session-${sid}`,
        kind: 'session',
        title: match?.title || 'Background Session Turn',
        command: sid,
        status: 'running',
        meta: 'Async session',
        timestamp: now + 5_000,
        sessionId: sid,
      });
    }

    // 3. Running & recent scheduled Cron jobs
    for (const cronJob of runtimeInventory?.cron.jobs ?? []) {
      const lowerStatus = cronJob.status.toLowerCase();
      if (lowerStatus === 'running') {
        jobs.push({
          id: `cron-live-${cronJob.id}`,
          kind: 'cron',
          title: `Cron · ${cronJob.id}`,
          command: cronJob.prompt,
          status: 'running',
          meta: cronJob.schedule,
          timestamp: now + 2_000,
        });
      }
    }

    const recentCronRecords = Object.values(cronLogs)
      .flat()
      .slice(0, 10);
    for (const rec of recentCronRecords) {
      const ts = Date.parse(rec.started_at) || 0;
      const lower = rec.status.toLowerCase();
      jobs.push({
        id: `cron-run-${rec.run_id}`,
        kind: 'cron',
        title: `Cron · ${rec.job_id}`,
        command: rec.summary || rec.error || rec.schedule,
        status:
          lower === 'running'
            ? 'running'
            : lower === 'failed' || lower === 'error'
              ? 'failed'
              : 'completed',
        meta: rec.schedule,
        timestamp: ts,
      });
    }

    // 4. Shell/build/dev commands (`bun build`, `cargo build`, `dev`, etc.) & delegated subagents across sessions
    const allMessages = Object.values(messagesByChat).flat();
    for (const msg of allMessages) {
      if (!msg.toolCalls || msg.toolCalls.length === 0) continue;
      for (const tc of msg.toolCalls) {
        const lower = tc.name.toLowerCase();
        const args = parseArgsObject(tc.args);
        const ts = tc.startedAt ?? msg.timestamp;
        const status: 'running' | 'completed' | 'failed' =
          tc.status === 'running' || tc.status === 'awaiting_approval'
            ? 'running'
            : tc.status === 'error'
              ? 'failed'
              : 'completed';

        const isSkillFork =
          lower === 'curate_skill' &&
          typeof args?.action === 'string' &&
          args.action === 'fork';

        if (
          SUBAGENT_TOOL_NAMES.has(lower) ||
          isSkillFork ||
          registeredSubagentNames.has(lower)
        ) {
          const agentName =
            (typeof args?.profile === 'string' && args.profile) ||
            (typeof args?.agent === 'string' && args.agent) ||
            (typeof args?.name === 'string' && args.name) ||
            tc.name;
          const taskText =
            (typeof args?.task === 'string' && args.task) ||
            (typeof args?.goal === 'string' && args.goal) ||
            (typeof args?.prompt === 'string' && args.prompt) ||
            'Delegated specialist execution';

          subagents.push({
            id: `sub-${tc.id}`,
            agent: agentName,
            task: taskText,
            status,
            durationLabel: formatDurationMs(tc.durationMs),
            timestamp: ts,
          });
          continue;
        }

        if (JOB_TOOL_NAMES.has(lower) || lower.startsWith('cargo_')) {
          const cmd = extractCommandLabel(tc);
          jobs.push({
            id: `cmd-${tc.id}`,
            kind: 'command',
            title: tc.name,
            command: cmd,
            status,
            meta: formatDurationMs(tc.durationMs),
            timestamp: ts,
          });
        }
      }
    }

    jobs.sort((a, b) => {
      if (a.status === 'running' && b.status !== 'running') return -1;
      if (b.status === 'running' && a.status !== 'running') return 1;
      return b.timestamp - a.timestamp;
    });

    subagents.sort((a, b) => {
      if (a.status === 'running' && b.status !== 'running') return -1;
      if (b.status === 'running' && a.status !== 'running') return 1;
      return b.timestamp - a.timestamp;
    });

    return {
      backgroundJobs: jobs.slice(0, 30),
      subagentItems: subagents.slice(0, 24),
    };
  }, [
    servers,
    activeTurnIds,
    activeChatId,
    sessions,
    runtimeInventory?.cron.jobs,
    cronLogs,
    messagesByChat,
    registeredSubagentNames,
  ]);

  const filteredLogs = useMemo(() => {
    const scoped = logs.filter((entry) => {
      if (logScope === 'all') return true;
      const bucket = classifyLogScope(entry, activeChatId);
      if (logScope === 'session') return bucket === 'session';
      if (logScope === 'background') return bucket === 'background';
      return bucket === 'system' || bucket === 'other_session';
    });

    const { items } = prepareScopedLogs(scoped, { essentialOnly });
    const q = query.trim().toLowerCase();
    if (!q) return items.slice(0, 80);
    return items
      .filter(({ entry, baseMessage, fields }) => {
        return (
          entry.message.toLowerCase().includes(q) ||
          baseMessage.toLowerCase().includes(q) ||
          entry.target.toLowerCase().includes(q) ||
          (entry.category ?? '').toLowerCase().includes(q) ||
          Object.values(fields).some((v) => v.toLowerCase().includes(q))
        );
      })
      .slice(0, 80);
  }, [logs, logScope, activeChatId, essentialOnly, query]);

  const qLower = query.trim().toLowerCase();
  const visibleJobs = useMemo(
    () =>
      !qLower
        ? backgroundJobs
        : backgroundJobs.filter(
            (j) =>
              j.title.toLowerCase().includes(qLower) ||
              j.command.toLowerCase().includes(qLower),
          ),
    [backgroundJobs, qLower],
  );

  const visibleSubagents = useMemo(
    () =>
      !qLower
        ? subagentItems
        : subagentItems.filter(
            (s) =>
              s.agent.toLowerCase().includes(qLower) ||
              s.task.toLowerCase().includes(qLower),
          ),
    [subagentItems, qLower],
  );

  const visibleOrchRuns = useMemo(
    () =>
      !qLower
        ? allOrchestrationRuns
        : allOrchestrationRuns.filter(
            (r) =>
              r.goal.toLowerCase().includes(qLower) ||
              r.steps.some((st) => st.agent.toLowerCase().includes(qLower)),
          ),
    [allOrchestrationRuns, qLower],
  );

  const runningCount =
    backgroundJobs.filter((j) => j.status === 'running').length +
    subagentItems.filter((s) => s.status === 'running').length +
    allOrchestrationRuns.filter((r) => r.status === 'running').length;

  const totalSubagentCount = visibleOrchRuns.length + visibleSubagents.length;

  return (
    <KokonutSmoothDrawer
      open={open}
      onClose={onClose}
      side="right"
      styleVariant={drawerMotionStyle}
      showBackdrop={Boolean(onClose)}
      backdropClassName="z-30 bg-background/55 backdrop-blur-xs xl:hidden"
      ariaLabel="Runtime activity drawer"
      className="fixed inset-y-0 right-0 z-40 flex h-full w-full max-w-[380px] shrink-0 flex-col border-l border-border/65 bg-sidebar/95 shadow-2xl backdrop-blur-xl xl:static xl:z-auto xl:w-[368px]"
    >
        {/* Header */}
        <div className="flex items-center justify-between gap-2 border-b border-border/50 px-3.5 py-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="truncate text-sm font-semibold text-foreground">
                Activity
              </span>
              <SpectrumStatusBadge
                tone={runningCount > 0 ? 'running' : 'idle'}
                label={runningCount > 0 ? `${runningCount} Running` : 'Idle'}
              />
            </div>
            <div className="truncate text-[11px] text-muted-foreground">
              Background jobs, subagents &amp; live logs
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-1">
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={refreshAll}
              title="Refresh jobs, subagents & logs"
              aria-label="Refresh activity"
            >
              <RefreshCw className="size-3.5" />
            </Button>

            {onClose && (
              <Button
                type="button"
                variant="outline"
                size="icon-sm"
                onClick={onClose}
                title="Close activity (restore dock)"
                aria-label="Close activity panel"
              >
                <PanelRightClose className="size-3.5" />
              </Button>
            )}
          </div>
        </div>

        {/* Section Tabs */}
        <div className="grid grid-cols-4 gap-1 border-b border-border/50 bg-muted/25 p-1.5">
          {(
            [
              {
                id: 'all',
                label: 'All',
                count: visibleJobs.length + totalSubagentCount + filteredLogs.length,
              },
              { id: 'jobs', label: 'Jobs', count: visibleJobs.length },
              { id: 'subagents', label: 'Subagents', count: totalSubagentCount },
              { id: 'logs', label: 'Logs', count: filteredLogs.length },
            ] as const
          ).map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                'flex items-center justify-center gap-1 rounded-lg px-2 py-1.5 text-[11px] font-medium transition-colors',
                activeTab === tab.id
                  ? 'bg-background text-foreground shadow-2xs ring-1 ring-border/60'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <span className="truncate">{tab.label}</span>
              <span className="font-mono text-[9.5px] opacity-70">{tab.count}</span>
            </button>
          ))}
        </div>

        {/* Search + Log Signal Filter Bar */}
        <div className="space-y-1.5 border-b border-border/50 px-3 py-2">
          <div className="flex items-center gap-2 rounded-lg border border-border/60 bg-background/75 px-2.5 py-1">
            <Search className="size-3.5 shrink-0 text-muted-foreground" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Filter commands, subagents, or logs…"
              aria-label="Filter activity items"
              className="min-w-0 flex-1 bg-transparent text-xs text-foreground placeholder:text-muted-foreground focus:outline-none"
            />
            {query && (
              <button
                type="button"
                onClick={() => setQuery('')}
                className="text-[10px] text-muted-foreground hover:text-foreground"
              >
                Clear
              </button>
            )}
          </div>

          {(activeTab === 'logs' || activeTab === 'all') && (
            <div className="flex items-center justify-between gap-1 text-[10px]">
              <div className="flex items-center gap-1">
                {(
                  [
                    { id: 'all', label: 'All' },
                    { id: 'background', label: 'Bg & Cron' },
                    { id: 'session', label: 'Session' },
                    { id: 'system', label: 'Gateway' },
                  ] as const
                ).map((sc) => (
                  <button
                    key={sc.id}
                    type="button"
                    onClick={() => setLogScope(sc.id)}
                    className={cn(
                      'rounded-md border px-1.5 py-0.5 font-medium transition-colors',
                      logScope === sc.id
                        ? 'border-primary/40 bg-primary/12 text-primary'
                        : 'border-border/50 bg-background/50 text-muted-foreground hover:text-foreground',
                    )}
                  >
                    {sc.label}
                  </button>
                ))}
              </div>

              <button
                type="button"
                onClick={() => setEssentialOnly((prev) => !prev)}
                className={cn(
                  'inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 font-medium transition-colors',
                  essentialOnly
                    ? 'border-primary/40 bg-primary/12 text-primary'
                    : 'border-border/50 bg-background/50 text-muted-foreground hover:text-foreground',
                )}
                title="Toggle essential vs verbose logs"
              >
                {essentialOnly ? (
                  <>
                    <Filter className="size-2.5" />
                    <span>Essential</span>
                  </>
                ) : (
                  <>
                    <Eye className="size-2.5" />
                    <span>Verbose</span>
                  </>
                )}
              </button>
            </div>
          )}
        </div>

        {/* Scrollable Content */}
        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-3">
          {/* 1. Background Jobs (bun build, cargo build, dev servers, cron) */}
          {(activeTab === 'all' || activeTab === 'jobs') && (
            <section className="space-y-2">
              {activeTab === 'all' && (
                <div className="flex items-center justify-between text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
                  <span className="inline-flex items-center gap-1.5">
                    <Terminal className="size-3 text-primary" />
                    <span>Background Jobs &amp; Commands</span>
                  </span>
                  <span className="font-mono">{visibleJobs.length}</span>
                </div>
              )}

              {visibleJobs.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border/60 px-3 py-4 text-center text-[11px] text-muted-foreground">
                  No active background builds, dev servers, or shell jobs right now.
                </div>
              ) : (
                <div className="space-y-1.5">
                  {(activeTab === 'all' ? visibleJobs.slice(0, 8) : visibleJobs).map(
                    (job) => (
                      <div
                        key={job.id}
                        onClick={
                          job.sessionId
                            ? () => {
                                selectSession(job.sessionId!);
                                setActiveView('chats');
                              }
                            : undefined
                        }
                        className={cn(
                          'rounded-xl border border-border/60 bg-card/75 px-3 py-2 text-xs',
                          job.sessionId && 'cursor-pointer hover:border-primary/40',
                        )}
                      >
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex min-w-0 items-center gap-1.5">
                            {job.status === 'running' ? (
                              <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
                            ) : job.status === 'failed' ? (
                              <XCircle className="size-3.5 shrink-0 text-destructive" />
                            ) : (
                              <CheckCircle2 className="size-3.5 shrink-0 text-success" />
                            )}
                            <span className="truncate font-medium text-foreground">
                              {job.kind === 'server' ? (
                                <span className="inline-flex items-center gap-1">
                                  <Server className="size-3 text-primary" />
                                  <span>{job.title}</span>
                                </span>
                              ) : job.kind === 'cron' ? (
                                <span className="inline-flex items-center gap-1">
                                  <Clock className="size-3 text-warning" />
                                  <span>{job.title}</span>
                                </span>
                              ) : (
                                job.title
                              )}
                            </span>
                          </div>

                          <div className="flex shrink-0 items-center gap-1.5">
                            {job.meta && (
                              <span className="font-mono text-[10px] text-muted-foreground">
                                {job.meta}
                              </span>
                            )}
                            {job.serverId && (
                              <button
                                type="button"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  wsService.stopServer(job.serverId!);
                                }}
                                className="inline-flex items-center gap-1 rounded-md border border-destructive/35 bg-destructive/10 px-1.5 py-0.5 text-[10px] font-medium text-destructive hover:bg-destructive/20"
                                title="Stop background server"
                              >
                                <Square className="size-2.5" />
                                <span>Stop</span>
                              </button>
                            )}
                          </div>
                        </div>

                        <div className="mt-1 truncate rounded bg-background/70 px-2 py-1 font-mono text-[10.5px] text-foreground/85">
                          $ {job.command}
                        </div>
                      </div>
                    ),
                  )}
                </div>
              )}
            </section>
          )}

          {/* 2. Subagents Activity */}
          {(activeTab === 'all' || activeTab === 'subagents') && (
            <section className="space-y-2">
              {activeTab === 'all' && (
                <div className="flex items-center justify-between text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
                  <span className="inline-flex items-center gap-1.5">
                    <Bot className="size-3 text-primary" />
                    <span>Subagents Activity</span>
                  </span>
                  <span className="font-mono">{totalSubagentCount}</span>
                </div>
              )}

              {totalSubagentCount === 0 ? (
                <div className="rounded-xl border border-dashed border-border/60 px-3 py-4 text-center text-[11px] text-muted-foreground">
                  No active or recent subagent runs recorded yet.
                </div>
              ) : (
                <div className="space-y-2">
                  {visibleOrchRuns.map((run) => (
                    <OrchestrationRunPanel key={run.id} run={run} />
                  ))}

                  {(activeTab === 'all' ? visibleSubagents.slice(0, 6) : visibleSubagents).map(
                    (sub) => (
                      <div
                        key={sub.id}
                        className="rounded-xl border border-border/60 bg-card/75 px-3 py-2 text-xs"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex min-w-0 items-center gap-1.5">
                            {sub.status === 'running' ? (
                              <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
                            ) : sub.status === 'failed' ? (
                              <XCircle className="size-3.5 shrink-0 text-destructive" />
                            ) : (
                              <Bot className="size-3.5 shrink-0 text-success" />
                            )}
                            <span className="truncate font-mono text-[11px] font-semibold text-foreground">
                              {sub.agent}
                            </span>
                          </div>
                          <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                            {sub.status === 'running'
                              ? 'running'
                              : sub.durationLabel || sub.status}
                          </span>
                        </div>
                        <p className="mt-1 line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">
                          {sub.task}
                        </p>
                      </div>
                    ),
                  )}
                </div>
              )}
            </section>
          )}

          {/* 3. Live Logs */}
          {(activeTab === 'all' || activeTab === 'logs') && (
            <section className="space-y-2">
              {activeTab === 'all' && (
                <div className="flex items-center justify-between text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
                  <span className="inline-flex items-center gap-1.5">
                    <ScrollText className="size-3 text-primary" />
                    <span>Live Logs</span>
                  </span>
                  <span className="font-mono">{filteredLogs.length}</span>
                </div>
              )}

              {filteredLogs.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border/60 px-3 py-4 text-center text-[11px] text-muted-foreground">
                  No logs match the current scope or filter.
                </div>
              ) : (
                <div className="space-y-1.5 font-mono text-[10.5px]">
                  {(activeTab === 'all' ? filteredLogs.slice(0, 20) : filteredLogs).map(
                    (item) => {
                      const { entry } = item;
                      const presented = presentLogItem(item);
                      return (
                        <div
                          key={entry.id}
                          className="rounded-lg border border-border/50 bg-card/65 px-2.5 py-2"
                        >
                          <div className="mb-1 flex items-center justify-between gap-2 text-[9.5px] text-muted-foreground">
                            <div className="flex min-w-0 items-center gap-1.5">
                              <span className="shrink-0">{entry.timestamp}</span>
                              {entry.category && (
                                <span className="truncate rounded bg-foreground/[0.05] px-1.5 py-0 text-[8.5px] uppercase text-foreground/75">
                                  {entry.category}
                                </span>
                              )}
                            </div>
                            <Badge
                              variant={BADGE_VARIANT_BY_LEVEL[entry.level]}
                              className="px-1.5 py-0 text-[8.5px]"
                            >
                              {entry.level}
                            </Badge>
                          </div>

                          {essentialOnly ? (
                            <div className="space-y-0.5 font-sans text-[11px]">
                              <div className="font-medium text-foreground">
                                {presented.title}
                              </div>
                              {presented.detail && presented.detail !== presented.title && (
                                <div className="break-words font-mono text-[10px] text-muted-foreground">
                                  {presented.detail}
                                </div>
                              )}
                            </div>
                          ) : (
                            <div className="break-words text-foreground/90">
                              {entry.message}
                            </div>
                          )}
                        </div>
                      );
                    },
                  )}
                </div>
              )}
            </section>
          )}
        </div>
    </KokonutSmoothDrawer>
  );
};
