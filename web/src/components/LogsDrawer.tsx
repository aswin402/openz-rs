import React, { useEffect, useMemo, useState } from 'react';
import {
  Clock,
  Eye,
  Filter,
  MessageSquare,
  RefreshCw,
  ScrollText,
  Search,
  Server,
  X,
} from 'lucide-react';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { KokonutSmoothDrawer } from './registries/modernRegistries';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import {
  classifyLogScope,
  normalizeWsSessionId,
  prepareScopedLogs,
  presentLogItem,
} from '../features/logs/logPresentation';
import { cn } from '../lib/utils';
import { wsService } from '../services/websocket';
import { useOpenZStore } from '../store/useOpenZStore';
import type { LogEntry } from '../types';

type LogsScopeTab = 'session' | 'background' | 'system';
type LevelFilter = 'ALL' | LogEntry['level'];
type CategoryFilter =
  | 'ALL'
  | 'session'
  | 'tool'
  | 'cron'
  | 'subagent'
  | 'curator'
  | 'channel'
  | 'security'
  | 'system';

const LEVEL_FILTERS_ESSENTIAL: ReadonlyArray<LevelFilter> = [
  'ALL',
  'ERROR',
  'WARN',
  'INFO',
];

const LEVEL_FILTERS_VERBOSE: ReadonlyArray<LevelFilter> = [
  'ALL',
  'ERROR',
  'WARN',
  'INFO',
  'DEBUG',
];

const CATEGORIES_BY_SCOPE: Record<
  LogsScopeTab,
  ReadonlyArray<{ id: CategoryFilter; label: string }>
> = {
  session: [
    { id: 'ALL', label: 'All' },
    { id: 'tool', label: 'Tools' },
    { id: 'session', label: 'Turn' },
    { id: 'subagent', label: 'Subagents' },
    { id: 'security', label: 'Security' },
  ],
  background: [
    { id: 'ALL', label: 'All' },
    { id: 'cron', label: 'Cron Jobs' },
    { id: 'curator', label: 'Curator' },
  ],
  system: [
    { id: 'ALL', label: 'All' },
    { id: 'channel', label: 'Gateway' },
    { id: 'security', label: 'Security' },
    { id: 'system', label: 'System' },
    { id: 'tool', label: 'Tools' },
    { id: 'session', label: 'Sessions' },
  ],
};

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

export const LogsDrawer: React.FC = () => {
  const isLogsOpen = useOpenZStore((s) => s.isLogsOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const logs = useOpenZStore((s) => s.logs);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const sessions = useOpenZStore((s) => s.sessions);
  const drawerMotionStyle = useUiCustomizationStore((s) => s.drawerMotionStyle);

  const [scopeTab, setScopeTab] = useState<LogsScopeTab>('session');
  const [essentialOnly, setEssentialOnly] = useState<boolean>(true);
  const [includeOtherSessions, setIncludeOtherSessions] = useState<boolean>(false);
  const [selectedCronJob, setSelectedCronJob] = useState<string>('ALL');
  const [levelFilter, setLevelFilter] = useState<LevelFilter>('ALL');
  const [categoryFilter, setCategoryFilter] = useState<CategoryFilter>('ALL');
  const [query, setQuery] = useState('');

  useEffect(() => {
    if (isLogsOpen) {
      wsService.requestLogs(activeChatId);
    }
  }, [isLogsOpen, activeChatId]);

  const normalizedSessionKey = useMemo(
    () => normalizeWsSessionId(activeChatId),
    [activeChatId],
  );

  const currentSessionTitle = useMemo(() => {
    const found = sessions.find(
      (s) => s.id === activeChatId || s.id === normalizedSessionKey,
    );
    return found?.title || normalizedSessionKey || 'No active session';
  }, [sessions, activeChatId, normalizedSessionKey]);

  const partitioned = useMemo(() => {
    const sessionLogs: LogEntry[] = [];
    const backgroundLogs: LogEntry[] = [];
    const systemLogs: LogEntry[] = [];
    const otherSessionLogs: LogEntry[] = [];

    for (const entry of logs) {
      const bucket = classifyLogScope(entry, activeChatId);
      if (bucket === 'session') {
        sessionLogs.push(entry);
      } else if (bucket === 'background') {
        backgroundLogs.push(entry);
      } else if (bucket === 'other_session') {
        otherSessionLogs.push(entry);
      } else {
        systemLogs.push(entry);
      }
    }

    return { sessionLogs, backgroundLogs, systemLogs, otherSessionLogs };
  }, [logs, activeChatId]);

  const scopeCounts = useMemo(() => {
    const sessionCount = prepareScopedLogs(partitioned.sessionLogs, {
      essentialOnly,
    }).items.length;
    const backgroundCount = prepareScopedLogs(partitioned.backgroundLogs, {
      essentialOnly,
    }).items.length;
    const sysSource = includeOtherSessions
      ? [...partitioned.systemLogs, ...partitioned.otherSessionLogs]
      : partitioned.systemLogs;
    const systemCount = prepareScopedLogs(sysSource, {
      essentialOnly,
    }).items.length;
    return { sessionCount, backgroundCount, systemCount };
  }, [partitioned, essentialOnly, includeOtherSessions]);

  const availableCronJobs = useMemo(() => {
    const jobs = new Set<string>();
    for (const entry of partitioned.backgroundLogs) {
      const fromField = entry.fields?.job_id?.trim();
      if (fromField) {
        jobs.add(fromField);
      } else if (entry.session?.startsWith('cron:') && entry.session.length > 5) {
        jobs.add(entry.session.slice(5));
      }
    }
    return Array.from(jobs).sort();
  }, [partitioned.backgroundLogs]);

  const activeScopeBaseLogs = useMemo(() => {
    if (scopeTab === 'session') {
      return partitioned.sessionLogs;
    }
    if (scopeTab === 'background') {
      if (selectedCronJob === 'ALL') return partitioned.backgroundLogs;
      return partitioned.backgroundLogs.filter((entry) => {
        const jobId =
          entry.fields?.job_id?.trim() ||
          (entry.session?.startsWith('cron:') ? entry.session.slice(5) : '');
        return jobId === selectedCronJob;
      });
    }
    return includeOtherSessions
      ? logs.filter((entry) => {
          const b = classifyLogScope(entry, activeChatId);
          return b === 'system' || b === 'other_session';
        })
      : partitioned.systemLogs;
  }, [
    scopeTab,
    partitioned,
    selectedCronJob,
    includeOtherSessions,
    logs,
    activeChatId,
  ]);

  const categoryScopedLogs = useMemo(
    () =>
      activeScopeBaseLogs.filter(
        (entry) =>
          categoryFilter === 'ALL' || (entry.category ?? 'system') === categoryFilter,
      ),
    [activeScopeBaseLogs, categoryFilter],
  );

  const { preparedItems, hiddenBoilerplateCount } = useMemo(() => {
    const { items, hiddenCount } = prepareScopedLogs(categoryScopedLogs, {
      essentialOnly,
    });
    const q = query.trim().toLowerCase();
    const filtered = items.filter(({ entry, baseMessage, fields }) => {
      const levelOk = levelFilter === 'ALL' || entry.level === levelFilter;
      const queryOk =
        !q ||
        entry.message.toLowerCase().includes(q) ||
        baseMessage.toLowerCase().includes(q) ||
        entry.target.toLowerCase().includes(q) ||
        entry.level.toLowerCase().includes(q) ||
        (entry.category ?? '').toLowerCase().includes(q) ||
        (entry.session ?? '').toLowerCase().includes(q) ||
        Object.values(fields).some((v) => v.toLowerCase().includes(q));
      return levelOk && queryOk;
    });
    return { preparedItems: filtered, hiddenBoilerplateCount: hiddenCount };
  }, [categoryScopedLogs, essentialOnly, levelFilter, query]);

  const handleScopeChange = (nextScope: LogsScopeTab) => {
    setScopeTab(nextScope);
    const validCategories = CATEGORIES_BY_SCOPE[nextScope];
    if (!validCategories.some((c) => c.id === categoryFilter)) {
      setCategoryFilter('ALL');
    }
  };

  const levelOptions = essentialOnly ? LEVEL_FILTERS_ESSENTIAL : LEVEL_FILTERS_VERBOSE;
  const scopeCategories = CATEGORIES_BY_SCOPE[scopeTab];

  const emptyMessage = (() => {
    if (scopeTab === 'session') {
      if (!normalizedSessionKey) {
        return 'Select or start a chat session to view isolated session logs.';
      }
      if (partitioned.sessionLogs.length === 0) {
        return 'No logs recorded for this chat session yet. Send a message to see tool calls and turn events here — background cron jobs are kept separate.';
      }
      if (essentialOnly && hiddenBoilerplateCount > 0) {
        return `All ${hiddenBoilerplateCount} internal plumbing logs were hidden. Switch to Verbose to inspect low-level trace steps.`;
      }
      return 'No session logs match the active filter.';
    }
    if (scopeTab === 'background') {
      return partitioned.backgroundLogs.length === 0
        ? 'No background cron or curator logs recorded yet.'
        : 'No background logs match the active filter.';
    }
    return 'No gateway or system logs match the active filter.';
  })();

  return (
    <KokonutSmoothDrawer
      open={isLogsOpen}
      onClose={() => setIsLogsOpen(false)}
      side="right"
      styleVariant={drawerMotionStyle}
      showBackdrop
      ariaLabel="Runtime and session logs drawer"
      className="fixed inset-y-0 right-0 z-50 flex w-full max-w-lg flex-col border-l border-border/75 bg-card/95 shadow-2xl backdrop-blur-2xl"
    >
      {/* Header */}
      <div className="flex h-13 items-center justify-between border-b border-border/50 px-4">
        <div className="flex min-w-0 items-center gap-2 text-sm font-semibold text-foreground">
          <ScrollText className="size-4 shrink-0 text-primary" />
          <span className="truncate">Runtime &amp; Session Logs</span>
          <Badge variant="outline" className="font-mono text-[10px]">
            {preparedItems.length}
          </Badge>
        </div>

        <div className="flex items-center gap-1">
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => wsService.requestLogs(activeChatId)}
            title="Refresh logs"
            aria-label="Refresh logs"
          >
            <RefreshCw className="size-3.5" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => setIsLogsOpen(false)}
            aria-label="Close logs drawer"
          >
            <X className="size-4" />
          </Button>
        </div>
      </div>

      {/* Top-Level Scope Separation Tabs */}
      <div className="grid grid-cols-3 gap-1 border-b border-border/50 bg-muted/30 px-3 py-2">
        <button
          type="button"
          onClick={() => handleScopeChange('session')}
          className={cn(
            'flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[11px] font-semibold transition-colors',
            scopeTab === 'session'
              ? 'bg-background text-foreground shadow-2xs ring-1 ring-border/60'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          <MessageSquare className="size-3 text-primary" />
          <span className="truncate">This Session</span>
          <span className="rounded-full bg-primary/12 px-1.5 py-0 font-mono text-[9.5px] text-primary">
            {scopeCounts.sessionCount}
          </span>
        </button>

        <button
          type="button"
          onClick={() => handleScopeChange('background')}
          className={cn(
            'flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[11px] font-semibold transition-colors',
            scopeTab === 'background'
              ? 'bg-background text-foreground shadow-2xs ring-1 ring-border/60'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          <Clock className="size-3 text-warning" />
          <span className="truncate">Cron &amp; Bg</span>
          <span className="rounded-full bg-muted px-1.5 py-0 font-mono text-[9.5px] text-muted-foreground">
            {scopeCounts.backgroundCount}
          </span>
        </button>

        <button
          type="button"
          onClick={() => handleScopeChange('system')}
          className={cn(
            'flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[11px] font-semibold transition-colors',
            scopeTab === 'system'
              ? 'bg-background text-foreground shadow-2xs ring-1 ring-border/60'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          <Server className="size-3 text-muted-foreground" />
          <span className="truncate">Gateway</span>
          <span className="rounded-full bg-muted px-1.5 py-0 font-mono text-[9.5px] text-muted-foreground">
            {scopeCounts.systemCount}
          </span>
        </button>
      </div>

      {/* Scope Context + Signal Controls */}
      <div className="space-y-2 border-b border-border/50 bg-muted/15 p-3">
        {/* Active Session / Cron Job Sub-Header */}
        <div className="flex items-center justify-between gap-2 text-[10.5px]">
          {scopeTab === 'session' && (
            <div className="flex min-w-0 items-center gap-1.5 text-muted-foreground">
              <span className="truncate font-medium text-foreground/90">
                {currentSessionTitle}
              </span>
              {normalizedSessionKey && (
                <span className="shrink-0 rounded bg-muted/70 px-1.5 py-0.5 font-mono text-[9.5px] text-muted-foreground">
                  {normalizedSessionKey.slice(0, 18)}
                </span>
              )}
            </div>
          )}

          {scopeTab === 'background' && (
            <div className="text-muted-foreground">
              Scheduled cron jobs &amp; background curator runs
            </div>
          )}

          {scopeTab === 'system' && (
            <div className="flex items-center gap-2 text-muted-foreground">
              <span>Gateway, MCP &amp; channels</span>
              {partitioned.otherSessionLogs.length > 0 && (
                <button
                  type="button"
                  onClick={() => setIncludeOtherSessions((prev) => !prev)}
                  className={cn(
                    'rounded border px-1.5 py-0.5 text-[9.5px] font-semibold transition-colors',
                    includeOtherSessions
                      ? 'border-primary/45 bg-primary/12 text-primary'
                      : 'border-border/60 bg-background text-muted-foreground hover:text-foreground',
                  )}
                >
                  + Other Chats ({partitioned.otherSessionLogs.length})
                </button>
              )}
            </div>
          )}

          {/* Essential vs Verbose Signal Toggle */}
          <div
            className="inline-flex shrink-0 items-center rounded-md border border-border/60 bg-background p-0.5"
            title="Essential hides internal restore/save/request boilerplate and merges tool calls"
          >
            <button
              type="button"
              onClick={() => {
                setEssentialOnly(true);
                if (levelFilter === 'DEBUG') setLevelFilter('ALL');
              }}
              className={cn(
                'inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-semibold transition-colors',
                essentialOnly
                  ? 'bg-primary/15 text-primary'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Filter className="size-2.5" />
              <span>Essential</span>
            </button>
            <button
              type="button"
              onClick={() => setEssentialOnly(false)}
              className={cn(
                'inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-semibold transition-colors',
                !essentialOnly
                  ? 'bg-muted text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Eye className="size-2.5" />
              <span>Verbose</span>
            </button>
          </div>
        </div>

        {/* Cron Job Selector Pills when viewing Background & Cron */}
        {scopeTab === 'background' && availableCronJobs.length > 0 && (
          <div className="flex flex-wrap gap-1">
            <button
              type="button"
              onClick={() => setSelectedCronJob('ALL')}
              className={cn(
                'rounded-md border px-2 py-0.5 font-mono text-[9.5px] font-semibold transition-colors',
                selectedCronJob === 'ALL'
                  ? 'border-primary/45 bg-primary/12 text-primary'
                  : 'border-border/50 bg-background/60 text-muted-foreground hover:text-foreground',
              )}
            >
              All Jobs
            </button>
            {availableCronJobs.map((jobId) => (
              <button
                key={jobId}
                type="button"
                onClick={() => setSelectedCronJob(jobId)}
                className={cn(
                  'rounded-md border px-2 py-0.5 font-mono text-[9.5px] font-semibold transition-colors',
                  selectedCronJob === jobId
                    ? 'border-primary/45 bg-primary/12 text-primary'
                    : 'border-border/50 bg-background/60 text-muted-foreground hover:text-foreground',
                )}
              >
                {jobId}
              </button>
            ))}
          </div>
        )}

        {/* Search Input */}
        <div className="flex items-center gap-2 rounded-lg border border-border/60 bg-background px-2.5 py-1.5">
          <Search className="size-3.5 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={
              scopeTab === 'session'
                ? 'Search tools, files, or errors in this session…'
                : scopeTab === 'background'
                  ? 'Search cron jobs or curator events…'
                  : 'Search gateway or channel logs…'
            }
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

        {/* Contextual Category & Severity Filters */}
        <div className="flex flex-wrap items-center justify-between gap-1.5">
          <div className="flex flex-wrap gap-1">
            {scopeCategories.map((cat) => (
              <button
                key={cat.id}
                type="button"
                onClick={() => setCategoryFilter(cat.id)}
                className={cn(
                  'rounded-md border px-2 py-0.5 text-[10px] font-semibold transition-colors',
                  categoryFilter === cat.id
                    ? 'border-primary/45 bg-primary/12 text-primary'
                    : 'border-border/50 bg-background/60 text-muted-foreground hover:text-foreground',
                )}
              >
                {cat.label}
              </button>
            ))}
          </div>

          <div className="flex flex-wrap gap-1">
            {levelOptions.map((lvl) => (
              <button
                key={lvl}
                type="button"
                onClick={() => setLevelFilter(lvl)}
                className={cn(
                  'rounded-md border px-1.5 py-0.5 font-mono text-[9.5px] font-semibold transition-colors',
                  levelFilter === lvl
                    ? 'border-primary/45 bg-primary/12 text-primary'
                    : 'border-border/50 bg-background/60 text-muted-foreground hover:text-foreground',
                )}
              >
                {lvl}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Main Log Body */}
      <div className="flex-1 overflow-y-auto p-3">
        {preparedItems.length === 0 ? (
          <div className="py-12 text-center font-mono text-xs text-muted-foreground">
            {emptyMessage}
          </div>
        ) : (
          <div className="space-y-2 font-mono text-[11px]">
            {preparedItems.map((item) => {
              const { entry } = item;
              const presented = presentLogItem(item);
              return (
                <div
                  key={entry.id}
                  className="rounded-lg border border-border/50 bg-muted/30 p-2.5"
                >
                  <div className="mb-1 flex items-center justify-between gap-2 text-[10px] text-muted-foreground">
                    <div className="flex items-center gap-1.5">
                      <span>{entry.timestamp}</span>
                      {entry.category && (
                        <Badge
                          variant="outline"
                          className="px-1.5 py-0 text-[8.5px] uppercase"
                        >
                          {entry.category}
                        </Badge>
                      )}
                      {presented.badge && (
                        <span className="rounded bg-background/80 px-1.5 py-0 text-[9px] text-foreground/80">
                          {presented.badge}
                        </span>
                      )}
                    </div>
                    <Badge
                      variant={BADGE_VARIANT_BY_LEVEL[entry.level]}
                      className="px-1.5 py-0 text-[9px]"
                    >
                      {entry.level}
                    </Badge>
                  </div>

                  {essentialOnly ? (
                    <div className="space-y-1">
                      <div className="font-semibold text-foreground">
                        {presented.title}
                      </div>
                      {presented.detail && presented.detail !== presented.title && (
                        <div className="break-words text-foreground/80">
                          {presented.detail}
                        </div>
                      )}
                    </div>
                  ) : (
                    <div className="break-all text-foreground/90">{entry.message}</div>
                  )}

                  {!essentialOnly && (
                    <div className="mt-1 flex items-center justify-between gap-2 text-[9px] text-muted-foreground/65">
                      <span className="truncate">{entry.target}</span>
                      {entry.session && (
                        <span className="truncate font-mono">{entry.session}</span>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {essentialOnly && hiddenBoilerplateCount > 0 && (
          <div className="mt-3 flex items-center justify-between rounded-lg border border-border/50 bg-muted/20 px-3 py-2 text-[11px] text-muted-foreground">
            <span>
              Hid {hiddenBoilerplateCount} low-signal internal event
              {hiddenBoilerplateCount === 1 ? '' : 's'}
            </span>
            <button
              type="button"
              onClick={() => setEssentialOnly(false)}
              className="font-semibold text-primary hover:underline"
            >
              Show Verbose
            </button>
          </div>
        )}
      </div>
    </KokonutSmoothDrawer>
  );
};
