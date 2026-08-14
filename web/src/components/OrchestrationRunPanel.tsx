import { CheckCircle2, Circle, Clock3, GitBranch, XCircle } from 'lucide-react';
import type { OrchestrationRunState } from '../types';
import { cn } from '../lib/utils';

function statusIcon(status: string) {
  if (status === 'success') return <CheckCircle2 className="h-3.5 w-3.5 text-emerald-400" />;
  if (status === 'failed' || status === 'cancelled') return <XCircle className="h-3.5 w-3.5 text-red-400" />;
  if (status === 'running') return <Clock3 className="h-3.5 w-3.5 text-amber-400" />;
  return <Circle className="h-3.5 w-3.5 text-muted-foreground" />;
}

function statusLabel(status: string): string {
  return status.replace('_', ' ');
}

export function OrchestrationRunPanel({ run }: { run: OrchestrationRunState }) {
  return (
    <div className="rounded-lg border border-border/50 bg-card/50 p-3 text-xs transition-colors hover:border-border">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-1.5 text-[10px] font-medium uppercase text-muted-foreground">
            <GitBranch className="h-3 w-3" />
            <span className="truncate">{run.mode}</span>
          </div>
          <div className="mt-1 line-clamp-2 font-semibold leading-snug text-foreground">{run.goal || run.id}</div>
        </div>
        <div className="flex shrink-0 items-center gap-1 text-[10px] font-medium text-muted-foreground">
          {statusIcon(run.status)}
          <span className="capitalize">{statusLabel(run.status)}</span>
        </div>
      </div>

      {run.steps.length > 0 ? (
        <div className="mt-3 space-y-2">
          {run.steps.map((step) => (
            <div key={step.id} className="flex min-w-0 items-start gap-2 border-l border-border/70 pl-2">
              <div className="mt-0.5 shrink-0">{statusIcon(step.status)}</div>
              <div className="min-w-0 flex-1">
                <div className="flex min-w-0 items-center gap-1.5">
                  <span className="truncate font-mono text-[11px] font-medium text-foreground">{step.id}</span>
                  {step.agent ? <span className="min-w-0 truncate text-[10px] text-muted-foreground">by {step.agent}</span> : null}
                </div>
                {step.output ? (
                  <div className="mt-0.5 line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">{step.output}</div>
                ) : null}
                {step.error ? (
                  <div className="mt-0.5 line-clamp-2 text-[11px] leading-relaxed text-red-300">{step.error}</div>
                ) : null}
              </div>
            </div>
          ))}
        </div>
      ) : null}

      {run.summary ? (
        <div
          className={cn(
            'mt-3 line-clamp-2 text-[11px] leading-relaxed',
            run.status === 'failed' || run.status === 'cancelled' ? 'text-red-300' : 'text-muted-foreground',
          )}
        >
          {run.summary}
        </div>
      ) : null}
    </div>
  );
}
