import React, { useMemo, useState } from 'react';
import type { OpenZMessage, OrchestrationRunState } from '../../types';
import { cn } from '../../lib/utils';
import { formatDuration } from '../../shared/lib/format';
import { OrchestrationRunPanel } from '../../shared/ui/OrchestrationRunPanel';
import { AgentActivity, type AgentActivityStep } from '../../components/beui';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import {
  Activity,
  Brain,
  Clock3,
  PanelRightClose,
} from 'lucide-react';

interface AgentActivityPanelProps {
  messages: OpenZMessage[];
  isStreaming: boolean;
  orchestrationRuns?: OrchestrationRunState[];
  onClose?: () => void;
}

type ActivityFilter = 'all' | 'tools' | 'notices' | 'approvals' | 'errors';

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

export function buildMessageActivitySteps(messages: OpenZMessage[]): AgentActivityStep[] {
  const items: AgentActivityStep[] = [];

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
        status:
          prompt.status === 'pending'
            ? 'pending'
            : prompt.status === 'denied'
              ? 'error'
              : 'success',
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
        detail:
          summarizeValue(tool.args) ||
          summarizeValue(tool.output) ||
          summarizeValue(tool.error),
        status: tool.status === 'awaiting_approval' ? 'pending' : tool.status,
        durationLabel: formatDuration(tool.durationMs) || undefined,
        timestamp: tool.startedAt || message.timestamp,
      });
    });
  });

  return items.sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0)).slice(0, 24);
}

function matchesFilter(item: AgentActivityStep, filter: ActivityFilter): boolean {
  if (filter === 'all') return true;
  if (filter === 'tools') return item.kind === 'tool';
  if (filter === 'notices') return item.kind === 'notice';
  if (filter === 'approvals') return item.kind === 'security';
  return item.status === 'error';
}

export const AgentActivityPanel: React.FC<AgentActivityPanelProps> = ({
  messages,
  isStreaming,
  orchestrationRuns = [],
  onClose,
}) => {
  const [filter, setFilter] = useState<ActivityFilter>('all');
  const activity = useMemo(() => buildMessageActivitySteps(messages), [messages]);
  const filteredActivity = useMemo(
    () => activity.filter((item) => matchesFilter(item, filter)),
    [activity, filter],
  );
  const runningTools = activity.filter(
    (item) => item.kind === 'tool' && item.status === 'running',
  ).length;
  const noticeCount = activity.filter((item) => item.kind === 'notice').length;
  const visibleRuns = orchestrationRuns.slice(-5).reverse();
  const failedTools = activity.filter(
    (item) => item.kind === 'tool' && item.status === 'error',
  ).length;
  const lastAssistant = [...messages].reverse().find((message) => message.role === 'assistant');
  const hasReasoning = Boolean(lastAssistant?.reasoningContent);

  const filters: Array<{ id: ActivityFilter; label: string; count: number }> = [
    { id: 'all', label: 'All', count: activity.length },
    {
      id: 'tools',
      label: 'Tools',
      count: activity.filter((item) => item.kind === 'tool').length,
    },
    { id: 'notices', label: 'Notes', count: noticeCount },
    {
      id: 'approvals',
      label: 'Approvals',
      count: activity.filter((item) => item.kind === 'security').length,
    },
    {
      id: 'errors',
      label: 'Errors',
      count: activity.filter((item) => item.status === 'error').length,
    },
  ];

  return (
    <aside className="hidden h-full w-[328px] shrink-0 border-l border-border/60 bg-sidebar/85 xl:flex xl:flex-col">
      <div className="border-b border-border/50 px-4 py-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="flex size-8 items-center justify-center rounded-lg border border-primary/25 bg-primary/10 text-primary">
              <Activity className="size-4" />
            </div>
            <div>
              <div className="text-sm font-semibold text-foreground">Agent Activity</div>
              <div className="text-[11px] text-muted-foreground">
                {isStreaming ? 'Live turn in progress' : 'Latest turn timeline'}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-1.5">
            <Badge variant={isStreaming ? 'warning' : 'outline'} className="gap-1.5">
              <span
                className={cn(
                  'size-1.5 rounded-full',
                  isStreaming ? 'animate-pulse bg-warning' : 'bg-muted-foreground',
                )}
              />
              <span>{isStreaming ? 'Active' : 'Idle'}</span>
            </Badge>
            {onClose && (
              <Button
                type="button"
                variant="outline"
                size="icon-sm"
                onClick={onClose}
                title="Hide activity panel"
                aria-label="Hide activity panel"
              >
                <PanelRightClose className="size-3.5" />
              </Button>
            )}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-2 border-b border-border/50 p-3">
        <Metric label="Running" value={runningTools} tone={runningTools ? 'amber' : 'muted'} />
        <Metric label="Notes" value={noticeCount} tone={noticeCount ? 'amber' : 'muted'} />
        <Metric label="Errors" value={failedTools} tone={failedTools ? 'red' : 'muted'} />
      </div>

      <div className="border-b border-border/50 px-4 py-3">
        <div className="mb-2 flex items-center justify-between text-[11px] font-medium text-muted-foreground">
          <span>Current Focus</span>
          {hasReasoning ? (
            <Brain className="size-3.5 text-primary" />
          ) : (
            <Clock3 className="size-3.5" />
          )}
        </div>
        <div className="rounded-lg border border-border/60 bg-card/75 p-2.5 text-xs text-foreground/90">
          {lastAssistant?.isStreaming &&
          lastAssistant.toolCalls?.some((tool) => tool.status === 'running') ? (
            <span>
              {lastAssistant.toolCalls.find((tool) => tool.status === 'running')?.name} is executing
            </span>
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
        <div className="grid grid-cols-5 gap-1 rounded-lg bg-muted/50 p-1">
          {filters.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setFilter(item.id)}
              className={cn(
                'min-w-0 rounded-md px-1.5 py-1.5 text-[10px] font-medium text-muted-foreground transition-colors hover:text-foreground',
                filter === item.id && 'bg-background text-foreground shadow-xs',
              )}
            >
              <span className="block truncate">{item.label}</span>
              <span className="block text-[9px] opacity-70">{item.count}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {visibleRuns.length > 0 ? (
          <div className="mb-3 space-y-2">
            {visibleRuns.map((run) => (
              <OrchestrationRunPanel key={run.id} run={run} />
            ))}
          </div>
        ) : null}

        {filteredActivity.length === 0 ? (
          <div className="flex h-full min-h-[140px] items-center justify-center rounded-xl border border-dashed border-border/60 px-4 text-center text-xs text-muted-foreground">
            {activity.length === 0
              ? 'Tool calls, workflow matches, memory saves, research context, approvals, and reasoning markers appear here during a turn.'
              : 'No activity matches this filter.'}
          </div>
        ) : (
          <AgentActivity steps={filteredActivity} isActive={isStreaming} mode="timeline" />
        )}
      </div>
    </aside>
  );
};

const Metric: React.FC<{ label: string; value: number; tone: 'amber' | 'red' | 'muted' }> = ({
  label,
  value,
  tone,
}) => (
  <div className="rounded-lg border border-border/60 bg-card/65 px-2.5 py-2">
    <div
      className={cn(
        'text-base font-semibold leading-none',
        tone === 'amber' && 'text-primary',
        tone === 'red' && 'text-destructive',
        tone === 'muted' && 'text-foreground',
      )}
    >
      {value}
    </div>
    <div className="mt-1 truncate text-[10px] text-muted-foreground">{label}</div>
  </div>
);
