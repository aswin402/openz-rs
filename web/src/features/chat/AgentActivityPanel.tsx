import React, { useMemo, useState } from 'react';
import {
  Brain,
  Clock3,
  ExternalLink,
  PanelRightClose,
  SlidersHorizontal,
} from 'lucide-react';
import {
  ACTIVITY_DENSITY_OPTIONS,
  ACTIVITY_ORB_OPTIONS,
  ACTIVITY_VARIANT_OPTIONS,
  AgentActivity,
  BeuiActivityOrb,
  resolveActivityPhase,
  resolveStepContentType,
  type AgentActivityStep,
} from '../../components/beui';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { cn } from '../../lib/utils';
import { OrchestrationRunPanel } from '../../shared/ui/OrchestrationRunPanel';
import { useUiCustomizationStore } from '../../store/useUiCustomizationStore';
import type { OpenZMessage, OrchestrationRunState } from '../../types';
import { buildMessageActivitySteps } from '../activity/activityAdapters';

export { buildMessageActivitySteps };

interface AgentActivityPanelProps {
  messages: OpenZMessage[];
  isStreaming: boolean;
  orchestrationRuns?: OrchestrationRunState[];
  onClose?: () => void;
}

type ActivityFilter =
  | 'all'
  | 'tools'
  | 'search'
  | 'reasoning'
  | 'approvals'
  | 'notices'
  | 'errors';

const FILTER_PREDICATES: Record<
  ActivityFilter,
  (item: AgentActivityStep) => boolean
> = {
  all: () => true,
  tools: (item) =>
    item.kind === 'tool' && resolveStepContentType(item) !== 'search',
  search: (item) => resolveStepContentType(item) === 'search',
  reasoning: (item) => item.kind === 'reasoning',
  approvals: (item) => item.kind === 'security',
  notices: (item) => item.kind === 'notice',
  errors: (item) => item.status === 'error',
};

const FILTER_METADATA: ReadonlyArray<{ id: ActivityFilter; label: string }> = [
  { id: 'all', label: 'All' },
  { id: 'tools', label: 'Tools' },
  { id: 'search', label: 'Search' },
  { id: 'reasoning', label: 'Think' },
  { id: 'approvals', label: 'Guard' },
  { id: 'notices', label: 'Notes' },
  { id: 'errors', label: 'Errors' },
];

export const AgentActivityPanel: React.FC<AgentActivityPanelProps> = ({
  messages,
  isStreaming,
  orchestrationRuns = [],
  onClose,
}) => {
  const [filter, setFilter] = useState<ActivityFilter>('all');
  const [showQuickOptions, setShowQuickOptions] = useState(false);

  const activityVariant = useUiCustomizationStore((s) => s.activityVariant);
  const activityOrbStyle = useUiCustomizationStore((s) => s.activityOrbStyle);
  const activityDensity = useUiCustomizationStore((s) => s.activityDensity);
  const activityShowDetails = useUiCustomizationStore((s) => s.activityShowDetails);
  const applyCustomization = useUiCustomizationStore((s) => s.applyCustomization);
  const setActiveCustomizeSection = useUiCustomizationStore(
    (s) => s.setActiveCustomizeSection,
  );
  const setIsCustomizeOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);

  const activity = useMemo(
    () => buildMessageActivitySteps(messages, { limit: 40 }),
    [messages],
  );

  const filteredActivity = useMemo(
    () => activity.filter(FILTER_PREDICATES[filter]),
    [activity, filter],
  );

  const livePhase = useMemo(
    () => resolveActivityPhase(activity, isStreaming),
    [activity, isStreaming],
  );

  const runningCount = activity.filter(
    (item) => item.status === 'running' || item.status === 'pending',
  ).length;
  const searchCount = activity.filter(
    (item) => resolveStepContentType(item) === 'search',
  ).length;
  const failedCount = activity.filter((item) => item.status === 'error').length;

  const visibleRuns = orchestrationRuns.slice(-5).reverse();
  const lastAssistant = [...messages]
    .reverse()
    .find((message) => message.role === 'assistant');
  const hasReasoning = Boolean(lastAssistant?.reasoningContent);

  const activeRunningTool = lastAssistant?.toolCalls?.find(
    (tool) => tool.status === 'running',
  );

  const currentFocusText =
    lastAssistant?.isStreaming && activeRunningTool
      ? `${activeRunningTool.name} is executing`
      : lastAssistant?.isStreaming && hasReasoning
        ? 'Reading context and planning the next step'
        : lastAssistant?.isStreaming
          ? 'Composing response stream'
          : activity[0]?.title ?? 'No activity recorded for this chat yet';

  const filtersWithCounts = useMemo(
    () =>
      FILTER_METADATA.map((meta) => ({
        ...meta,
        count: activity.filter(FILTER_PREDICATES[meta.id]).length,
      })),
    [activity],
  );

  return (
    <>
      {onClose && (
        <button
          type="button"
          aria-label="Close activity drawer backdrop"
          onClick={onClose}
          className="fixed inset-0 z-30 bg-background/55 backdrop-blur-xs xl:hidden"
        />
      )}

      <aside className="fixed inset-y-0 right-0 z-40 flex h-full w-full max-w-[356px] shrink-0 flex-col border-l border-border/60 bg-sidebar/95 shadow-2xl xl:static xl:z-auto xl:w-[344px] xl:bg-sidebar/85 xl:shadow-none">
        <div className="border-b border-border/50 px-4 py-3">
          <div className="flex items-center justify-between gap-2">
            <div className="flex min-w-0 items-center gap-2.5">
              <BeuiActivityOrb
                phase={livePhase}
                isActive={isStreaming || runningCount > 0}
                styleVariant={activityOrbStyle}
                size="lg"
              />
              <div className="min-w-0">
                <div className="truncate text-sm font-semibold text-foreground">
                  Agent Activity
                </div>
                <div className="truncate text-[11px] text-muted-foreground">
                  {isStreaming ? 'Live adaptive stream' : 'Latest session telemetry'}
                </div>
              </div>
            </div>

            <div className="flex shrink-0 items-center gap-1">
              <Badge
                variant={isStreaming || runningCount > 0 ? 'warning' : 'outline'}
                className="gap-1.5"
              >
                <span
                  className={cn(
                    'size-1.5 rounded-full',
                    isStreaming || runningCount > 0
                      ? 'animate-pulse bg-warning'
                      : 'bg-muted-foreground',
                  )}
                />
                <span>{isStreaming || runningCount > 0 ? 'Active' : 'Idle'}</span>
              </Badge>

              <Button
                type="button"
                variant={showQuickOptions ? 'secondary' : 'ghost'}
                size="icon-sm"
                onClick={() => setShowQuickOptions((prev) => !prev)}
                title="Activity view options"
                aria-label="Activity view options"
              >
                <SlidersHorizontal className="size-3.5" />
              </Button>

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

          {showQuickOptions && (
            <div className="mt-3 space-y-2.5 rounded-xl border border-border/65 bg-card/85 p-2.5 text-[11px]">
              <div>
                <div className="mb-1 flex items-center justify-between text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                  <span>Stream Variant</span>
                  <button
                    type="button"
                    onClick={() => {
                      setActiveCustomizeSection('agent-activity');
                      setIsCustomizeOpen(true);
                    }}
                    className="inline-flex items-center gap-1 font-sans font-semibold normal-case text-primary hover:underline"
                  >
                    <span>All Studio Options</span>
                    <ExternalLink className="size-2.5" />
                  </button>
                </div>
                <div className="grid grid-cols-4 gap-1">
                  {ACTIVITY_VARIANT_OPTIONS.map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      onClick={() => applyCustomization({ activityVariant: opt.id })}
                      className={cn(
                        'rounded-md border px-1.5 py-1 text-center text-[10px] font-medium transition-colors',
                        activityVariant === opt.id
                          ? 'border-primary/50 bg-primary/12 text-primary'
                          : 'border-border/50 bg-background/50 text-muted-foreground hover:text-foreground',
                      )}
                    >
                      {opt.shortLabel}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <div>
                  <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    Orb
                  </div>
                  <div className="grid grid-cols-3 gap-1">
                    {ACTIVITY_ORB_OPTIONS.map((opt) => (
                      <button
                        key={opt.id}
                        type="button"
                        onClick={() => applyCustomization({ activityOrbStyle: opt.id })}
                        className={cn(
                          'truncate rounded-md border px-1 py-1 text-[9.5px] font-medium transition-colors',
                          activityOrbStyle === opt.id
                            ? 'border-primary/50 bg-primary/12 text-primary'
                            : 'border-border/50 bg-background/50 text-muted-foreground hover:text-foreground',
                        )}
                        title={opt.title}
                      >
                        {opt.shortLabel.split(' ')[0]}
                      </button>
                    ))}
                  </div>
                </div>

                <div>
                  <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    Density
                  </div>
                  <div className="grid grid-cols-2 gap-1">
                    {ACTIVITY_DENSITY_OPTIONS.map((opt) => (
                      <button
                        key={opt.id}
                        type="button"
                        onClick={() => applyCustomization({ activityDensity: opt.id })}
                        className={cn(
                          'truncate rounded-md border px-1.5 py-1 text-[9.5px] font-medium transition-colors',
                          activityDensity === opt.id
                            ? 'border-primary/50 bg-primary/12 text-primary'
                            : 'border-border/50 bg-background/50 text-muted-foreground hover:text-foreground',
                        )}
                      >
                        {opt.shortLabel}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>

        <div className="grid grid-cols-3 gap-2 border-b border-border/50 p-3">
          <Metric
            label="Active"
            value={runningCount}
            tone={runningCount ? 'amber' : 'muted'}
          />
          <Metric
            label="Searches"
            value={searchCount}
            tone={searchCount ? 'info' : 'muted'}
          />
          <Metric
            label="Errors"
            value={failedCount}
            tone={failedCount ? 'red' : 'muted'}
          />
        </div>

        <div className="border-b border-border/50 px-4 py-2.5">
          <div className="mb-1.5 flex items-center justify-between text-[11px] font-medium text-muted-foreground">
            <span>Current Focus</span>
            {hasReasoning ? (
              <Brain className="size-3.5 text-primary" />
            ) : (
              <Clock3 className="size-3.5" />
            )}
          </div>
          <div className="truncate rounded-lg border border-border/60 bg-card/75 px-2.5 py-2 text-xs text-foreground/90">
            {currentFocusText}
          </div>
        </div>

        <div className="border-b border-border/50 p-2.5">
          <div className="flex flex-wrap gap-1 rounded-lg bg-muted/50 p-1">
            {filtersWithCounts.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setFilter(item.id)}
                className={cn(
                  'flex flex-1 items-center justify-center gap-1 rounded-md px-1.5 py-1 text-[10px] font-medium text-muted-foreground transition-colors hover:text-foreground',
                  filter === item.id && 'bg-background text-foreground shadow-xs',
                )}
              >
                <span>{item.label}</span>
                <span className="font-mono text-[9px] opacity-70">{item.count}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          {visibleRuns.length > 0 && (
            <div className="mb-3 space-y-2">
              {visibleRuns.map((run) => (
                <OrchestrationRunPanel key={run.id} run={run} />
              ))}
            </div>
          )}

          {filteredActivity.length === 0 ? (
            <div className="flex h-full min-h-[140px] items-center justify-center rounded-xl border border-dashed border-border/60 px-4 text-center text-xs text-muted-foreground">
              {activity.length === 0
                ? 'Tool calls, searches, workflow matches, memory saves, approvals, and reasoning markers stream here in real time.'
                : 'No activity matches the selected filter.'}
            </div>
          ) : (
            <AgentActivity
              steps={filteredActivity}
              isActive={isStreaming}
              mode="timeline"
              variant={activityVariant}
              orbStyle={activityOrbStyle}
              density={activityDensity}
              maxHeight="unlimited"
              showDetails={activityShowDetails}
            />
          )}
        </div>
      </aside>
    </>
  );
};

const TONE_VALUE_CLASSES: Record<'amber' | 'info' | 'red' | 'muted', string> = {
  amber: 'text-primary',
  info: 'text-info',
  red: 'text-destructive',
  muted: 'text-foreground',
};

const Metric: React.FC<{
  label: string;
  value: number;
  tone: 'amber' | 'info' | 'red' | 'muted';
}> = ({ label, value, tone }) => (
  <div className="rounded-lg border border-border/60 bg-card/65 px-2.5 py-2">
    <div className={cn('text-base font-semibold leading-none', TONE_VALUE_CLASSES[tone])}>
      {value}
    </div>
    <div className="mt-1 truncate text-[10px] text-muted-foreground">{label}</div>
  </div>
);
