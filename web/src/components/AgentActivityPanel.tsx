import React, { useMemo, useState } from 'react';
import type { OpenZMessage, ToolExecution } from '../types';
import { cn } from '../lib/utils';
import {
  Activity,
  AlertTriangle,
  Brain,
  CheckCircle2,
  Clock3,
  GitBranch,
  Loader2,
  PanelRightClose,
  Search,
  ShieldAlert,
  Sparkles,
  Terminal,
} from 'lucide-react';

interface AgentActivityPanelProps {
  messages: OpenZMessage[];
  isStreaming: boolean;
  onClose?: () => void;
}

type ActivityFilter = 'all' | 'tools' | 'notices' | 'approvals' | 'errors';

type ActivityItem = {
  id: string;
  kind: 'tool' | 'security' | 'reasoning' | 'notice' | 'message';
  noticeKind?: 'workflow' | 'memory' | 'research' | 'self_improvement' | 'source' | 'system';
  title: string;
  detail?: string;
  status: 'running' | 'success' | 'error' | 'pending' | 'info';
  timestamp: number;
  tool?: ToolExecution;
};

const statusStyles: Record<ActivityItem['status'], { icon: React.ElementType; cls: string; label: string }> = {
  running: { icon: Loader2, cls: 'text-amber-500', label: 'Running' },
  success: { icon: CheckCircle2, cls: 'text-emerald-500', label: 'Done' },
  error: { icon: AlertTriangle, cls: 'text-red-500', label: 'Error' },
  pending: { icon: ShieldAlert, cls: 'text-amber-500', label: 'Waiting' },
  info: { icon: Activity, cls: 'text-muted-foreground', label: 'Info' },
};

function summarizeValue(value: unknown): string | undefined {
  if (value === undefined || value === null || value === '') return undefined;
  if (typeof value === 'string') return value.length > 120 ? value.slice(0, 117) + '...' : value;
  try {
    const text = JSON.stringify(value);
    return text.length > 120 ? text.slice(0, 117) + '...' : text;
  } catch {
    return String(value);
  }
}

function buildActivity(messages: OpenZMessage[]): ActivityItem[] {
  const items: ActivityItem[] = [];

  messages.forEach((message) => {
    if (message.reasoningContent) {
      items.push({
        id: message.id + '-reasoning',
        kind: 'reasoning',
        title: message.isStreaming ? 'Reasoning stream active' : 'Reasoning captured',
        detail: message.reasoningContent.split('\n').find(Boolean),
        status: message.isStreaming ? 'running' : 'info',
        timestamp: message.timestamp,
      });
    }

    message.securityPrompts?.forEach((prompt) => {
      items.push({
        id: message.id + '-security-' + prompt.id,
        kind: 'security',
        title: prompt.toolName,
        detail: prompt.description,
        status: prompt.status === 'pending' ? 'pending' : prompt.status === 'denied' ? 'error' : 'success',
        timestamp: message.timestamp,
      });
    });

    message.activityNotices?.forEach((notice) => {
      items.push({
        id: notice.id,
        kind: 'notice',
        noticeKind: notice.kind,
        title: notice.title,
        detail: notice.detail,
        status: 'info',
        timestamp: notice.timestamp,
      });
    });

    message.toolCalls?.forEach((tool) => {
      items.push({
        id: message.id + '-tool-' + tool.id,
        kind: 'tool',
        title: tool.name,
        detail: summarizeValue(tool.args) || summarizeValue(tool.output) || summarizeValue(tool.error),
        status: tool.status === 'awaiting_approval' ? 'pending' : tool.status,
        timestamp: tool.startedAt || message.timestamp,
        tool,
      });
    });
  });

  return items.sort((a, b) => b.timestamp - a.timestamp).slice(0, 20);
}

function matchesFilter(item: ActivityItem, filter: ActivityFilter): boolean {
  if (filter === 'all') return true;
  if (filter === 'tools') return item.kind === 'tool';
  if (filter === 'notices') return item.kind === 'notice';
  if (filter === 'approvals') return item.kind === 'security';
  return item.status === 'error';
}

function formatDuration(ms?: number): string | null {
  if (!ms) return null;
  if (ms < 1000) return ms + 'ms';
  return (ms / 1000).toFixed(ms < 10000 ? 1 : 0) + 's';
}

export const AgentActivityPanel: React.FC<AgentActivityPanelProps> = ({ messages, isStreaming, onClose }) => {
  const [filter, setFilter] = useState<ActivityFilter>('all');
  const activity = useMemo(() => buildActivity(messages), [messages]);
  const filteredActivity = useMemo(() => activity.filter((item) => matchesFilter(item, filter)), [activity, filter]);
  const runningTools = activity.filter((item) => item.kind === 'tool' && item.status === 'running').length;
  const noticeCount = activity.filter((item) => item.kind === 'notice').length;
  const failedTools = activity.filter((item) => item.kind === 'tool' && item.status === 'error').length;
  const lastAssistant = [...messages].reverse().find((message) => message.role === 'assistant');
  const hasReasoning = Boolean(lastAssistant?.reasoningContent);
  const filters: Array<{ id: ActivityFilter; label: string; count: number }> = [
    { id: 'all', label: 'All', count: activity.length },
    { id: 'tools', label: 'Tools', count: activity.filter((item) => item.kind === 'tool').length },
    { id: 'notices', label: 'Notes', count: noticeCount },
    { id: 'approvals', label: 'Approvals', count: activity.filter((item) => item.kind === 'security').length },
    { id: 'errors', label: 'Errors', count: activity.filter((item) => item.status === 'error').length },
  ];

  return (
    <aside className="hidden h-full w-[320px] shrink-0 border-l border-border/50 bg-background/80 xl:flex xl:flex-col">
      <div className="border-b border-border/50 px-4 py-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-card text-amber-500">
              <Activity className="h-4 w-4" />
            </div>
            <div>
              <div className="text-sm font-semibold text-foreground">Agent Activity</div>
              <div className="text-[11px] text-muted-foreground">
                {isStreaming ? 'Live turn in progress' : 'Latest turn timeline'}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-1.5">
            <span
              className={cn(
                'flex items-center gap-1.5 rounded-md border px-2 py-1 text-[10px] font-medium',
                isStreaming
                  ? 'border-amber-500/30 bg-amber-500/10 text-amber-500'
                  : 'border-border/60 bg-muted/40 text-muted-foreground',
              )}
            >
              <span className={cn('h-1.5 w-1.5 rounded-full', isStreaming ? 'animate-pulse bg-amber-500' : 'bg-muted-foreground')} />
              {isStreaming ? 'Active' : 'Idle'}
            </span>
            {onClose && (
              <button
                type="button"
                onClick={onClose}
                className="flex h-7 w-7 items-center justify-center rounded-md border border-border/60 bg-muted/30 text-muted-foreground transition hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
                title="Hide activity panel"
                aria-label="Hide activity panel"
              >
                <PanelRightClose className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-2 border-b border-border/50 p-3">
        <Metric label="Tools" value={runningTools} tone={runningTools ? 'amber' : 'muted'} />
        <Metric label="Notes" value={noticeCount} tone={noticeCount ? 'amber' : 'muted'} />
        <Metric label="Errors" value={failedTools} tone={failedTools ? 'red' : 'muted'} />
      </div>

      <div className="border-b border-border/50 px-4 py-3">
        <div className="mb-2 flex items-center justify-between text-[11px] font-medium text-muted-foreground">
          <span>Current Focus</span>
          {hasReasoning ? <Brain className="h-3.5 w-3.5 text-amber-500" /> : <Clock3 className="h-3.5 w-3.5" />}
        </div>
        <div className="rounded-lg border border-border/50 bg-card/60 p-3 text-xs text-foreground/90">
          {lastAssistant?.isStreaming && lastAssistant.toolCalls?.some((tool) => tool.status === 'running') ? (
            <span>{lastAssistant.toolCalls.find((tool) => tool.status === 'running')?.name} is executing</span>
          ) : lastAssistant?.isStreaming && hasReasoning ? (
            <span>Reading context and planning the next step</span>
          ) : lastAssistant?.isStreaming ? (
            <span>Composing response</span>
          ) : activity.length > 0 ? (
            <span>{activity[0].title}</span>
          ) : (
            <span>No activity recorded for this chat yet</span>
          )}
        </div>
      </div>

      <div className="border-b border-border/50 p-3">
        <div className="grid grid-cols-5 gap-1 rounded-lg bg-muted/40 p-1">
          {filters.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setFilter(item.id)}
              className={cn(
                'min-w-0 rounded-md px-1.5 py-1.5 text-[10px] font-medium text-muted-foreground transition-colors hover:text-foreground',
                filter === item.id && 'bg-background text-foreground shadow-sm',
              )}
            >
              <span className="block truncate">{item.label}</span>
              <span className="block text-[9px] opacity-70">{item.count}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {filteredActivity.length === 0 ? (
          <div className="flex h-full items-center justify-center rounded-lg border border-dashed border-border/60 px-4 text-center text-xs text-muted-foreground">
            {activity.length === 0 ? 'Tool calls, workflow matches, memory saves, research context, approvals, and reasoning markers will appear here during a turn.' : 'No activity matches this filter.'}
          </div>
        ) : (
          <div className="space-y-2">
            {filteredActivity.map((item) => (
              <ActivityRow key={item.id} item={item} />
            ))}
          </div>
        )}
      </div>
    </aside>
  );
};

const Metric: React.FC<{ label: string; value: number; tone: 'amber' | 'red' | 'muted' }> = ({ label, value, tone }) => (
  <div className="rounded-lg border border-border/50 bg-card/50 px-2 py-2">
    <div
      className={cn(
        'text-base font-semibold leading-none',
        tone === 'amber' && 'text-amber-500',
        tone === 'red' && 'text-red-500',
        tone === 'muted' && 'text-foreground',
      )}
    >
      {value}
    </div>
    <div className="mt-1 truncate text-[10px] text-muted-foreground">{label}</div>
  </div>
);

const ActivityRow: React.FC<{ item: ActivityItem }> = ({ item }) => {
  const styles = statusStyles[item.status];
  const Icon = item.kind === 'tool'
    ? Terminal
    : item.kind === 'security'
      ? ShieldAlert
      : item.kind === 'reasoning'
        ? Brain
        : item.kind === 'notice'
          ? item.noticeKind === 'workflow'
            ? GitBranch
            : item.noticeKind === 'memory'
              ? Brain
              : item.noticeKind === 'research' || item.noticeKind === 'source'
                ? Search
                : item.noticeKind === 'self_improvement'
                  ? Sparkles
                  : Activity
          : styles.icon;
  const duration = formatDuration(item.tool?.durationMs);

  return (
    <div className="rounded-lg border border-border/50 bg-card/50 p-3 transition-colors hover:border-border">
      <div className="flex items-start gap-2">
        <div className={cn('mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-muted/60', styles.cls)}>
          <Icon className={cn('h-3.5 w-3.5', item.status === 'running' && 'animate-spin')} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center justify-between gap-2">
            <span className="min-w-0 truncate font-mono text-xs font-semibold text-foreground/90">{item.title}</span>
            <span className={cn('shrink-0 text-[10px] font-medium', styles.cls)}>{styles.label}</span>
          </div>
          {item.detail && <div className="mt-1 line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">{item.detail}</div>}
          {duration ? <div className="mt-2 text-[10px] text-muted-foreground">{duration}</div> : null}
        </div>
      </div>
    </div>
  );
};
