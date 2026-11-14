import React, { useMemo, useState } from 'react';
import {
  Activity,
  RefreshCw,
  ScrollText,
  Search,
  SlidersHorizontal,
  Terminal,
  X,
} from 'lucide-react';
import {
  ACTIVITY_VARIANT_OPTIONS,
  AgentActivity,
} from './beui';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { buildLogActivitySteps } from '../features/activity/activityAdapters';
import { cn } from '../lib/utils';
import { wsService } from '../services/websocket';
import { useOpenZStore } from '../store/useOpenZStore';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import type { LogEntry } from '../types';

type LogsViewMode = 'activity' | 'raw';
type LevelFilter = 'ALL' | LogEntry['level'];

const LEVEL_FILTERS: ReadonlyArray<LevelFilter> = [
  'ALL',
  'ERROR',
  'WARN',
  'INFO',
  'DEBUG',
];

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
  const isStreaming = useOpenZStore((s) => s.isStreaming);

  const activityVariant = useUiCustomizationStore((s) => s.activityVariant);
  const activityOrbStyle = useUiCustomizationStore((s) => s.activityOrbStyle);
  const activityDensity = useUiCustomizationStore((s) => s.activityDensity);
  const activityShowDetails = useUiCustomizationStore((s) => s.activityShowDetails);
  const applyCustomization = useUiCustomizationStore((s) => s.applyCustomization);
  const setActiveCustomizeSection = useUiCustomizationStore(
    (s) => s.setActiveCustomizeSection,
  );
  const setIsCustomizeOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);

  const [viewMode, setViewMode] = useState<LogsViewMode>('activity');
  const [levelFilter, setLevelFilter] = useState<LevelFilter>('ALL');
  const [query, setQuery] = useState('');

  const filteredRawLogs = useMemo(() => {
    const q = query.trim().toLowerCase();
    return logs.filter((entry) => {
      if (levelFilter !== 'ALL' && entry.level !== levelFilter) return false;
      if (!q) return true;
      return (
        entry.message.toLowerCase().includes(q) ||
        entry.target.toLowerCase().includes(q) ||
        entry.level.toLowerCase().includes(q)
      );
    });
  }, [logs, levelFilter, query]);

  const activitySteps = useMemo(
    () => buildLogActivitySteps(logs, { levelFilter, query, limit: 100 }),
    [logs, levelFilter, query],
  );

  if (!isLogsOpen) return null;

  return (
    <div className="fixed inset-y-0 right-0 z-50 flex w-full max-w-lg flex-col border-l border-border bg-card shadow-2xl">
      <div className="flex h-13 items-center justify-between border-b border-border/50 px-4">
        <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
          <ScrollText className="size-4 text-primary" />
          <span>OpenZ Structured Logs</span>
          <Badge variant="outline" className="font-mono text-[10px]">
            {filteredRawLogs.length}
          </Badge>
        </div>

        <div className="flex items-center gap-1">
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => wsService.requestLogs()}
            title="Refresh gateway logs"
            aria-label="Refresh gateway logs"
          >
            <RefreshCw className="size-3.5" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => {
              setActiveCustomizeSection('agent-activity');
              setIsCustomizeOpen(true);
            }}
            title="Customize Agent Activity options"
            aria-label="Customize Agent Activity options"
          >
            <SlidersHorizontal className="size-3.5" />
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

      {/* View Switcher + Filter Controls */}
      <div className="space-y-2 border-b border-border/50 bg-muted/20 p-3">
        <div className="flex items-center justify-between gap-2">
          <div className="grid grid-cols-2 gap-1 rounded-lg bg-muted/65 p-0.5">
            <button
              type="button"
              onClick={() => setViewMode('activity')}
              className={cn(
                'inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-semibold transition-colors',
                viewMode === 'activity'
                  ? 'bg-background text-foreground shadow-2xs'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Activity className="size-3 text-primary" />
              <span>Activity Stream</span>
            </button>
            <button
              type="button"
              onClick={() => setViewMode('raw')}
              className={cn(
                'inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-semibold transition-colors',
                viewMode === 'raw'
                  ? 'bg-background text-foreground shadow-2xs'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Terminal className="size-3" />
              <span>Raw Console</span>
            </button>
          </div>

          {viewMode === 'activity' && (
            <select
              aria-label="Log stream variant"
              value={activityVariant}
              onChange={(e) =>
                applyCustomization({
                  activityVariant: e.target.value as typeof activityVariant,
                })
              }
              className="rounded-lg border border-border/60 bg-background px-2 py-1 text-[10.5px] font-semibold text-foreground"
            >
              {ACTIVITY_VARIANT_OPTIONS.map((opt) => (
                <option key={opt.id} value={opt.id}>
                  {opt.shortLabel}
                </option>
              ))}
            </select>
          )}
        </div>

        <div className="flex items-center gap-2 rounded-lg border border-border/60 bg-background px-2.5 py-1.5">
          <Search className="size-3.5 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter logs by module, level, or message…"
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

        <div className="flex flex-wrap gap-1">
          {LEVEL_FILTERS.map((lvl) => (
            <button
              key={lvl}
              type="button"
              onClick={() => setLevelFilter(lvl)}
              className={cn(
                'rounded-md border px-2 py-0.5 font-mono text-[10px] font-semibold transition-colors',
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

      <div className="flex-1 overflow-y-auto p-3">
        {viewMode === 'activity' ? (
          <AgentActivity
            steps={activitySteps}
            isActive={isStreaming}
            mode="timeline"
            variant={activityVariant}
            orbStyle={activityOrbStyle}
            density={activityDensity}
            maxHeight="unlimited"
            showDetails={activityShowDetails}
            title="Gateway Structured Trace"
            emptyLabel={
              logs.length === 0
                ? 'No gateway logs recorded yet. Click Refresh to fetch recent entries.'
                : 'No log entries match the active level or search filter.'
            }
          />
        ) : filteredRawLogs.length === 0 ? (
          <div className="py-12 text-center font-mono text-xs text-muted-foreground">
            {logs.length === 0
              ? 'No logs recorded yet.'
              : 'No logs match the active filter.'}
          </div>
        ) : (
          <div className="space-y-2 font-mono text-[11px]">
            {filteredRawLogs.map((log) => (
              <div
                key={log.id}
                className="rounded-lg border border-border/50 bg-muted/30 p-2.5"
              >
                <div className="mb-1 flex items-center justify-between text-[10px] text-muted-foreground">
                  <span>{log.timestamp}</span>
                  <Badge
                    variant={BADGE_VARIANT_BY_LEVEL[log.level]}
                    className="px-1.5 py-0 text-[9px]"
                  >
                    {log.level}
                  </Badge>
                </div>
                <div className="break-all text-foreground/90">{log.message}</div>
                <div className="mt-1 text-[9px] text-muted-foreground/65">
                  {log.target}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
