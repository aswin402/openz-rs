import React, { useMemo, useState } from 'react';
import type { ToolExecution } from '../types';
import { cn } from '../lib/utils';
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clipboard,
  Copy,
  FileText,
  Loader2,
  ShieldAlert,
  Terminal,
} from 'lucide-react';

interface ToolExecutionCardProps {
  tool: ToolExecution;
}

type DetailTab = 'args' | 'output' | 'error';
type ToolDetailTab = {
  id: DetailTab;
  label: string;
  icon: React.ElementType;
  text: string;
  tone?: string;
};

const statusConfig = {
  running: { icon: Loader2, cls: 'text-amber-500', label: 'executing', spin: true },
  success: { icon: CheckCircle2, cls: 'text-emerald-500', label: 'success', spin: false },
  error: { icon: AlertTriangle, cls: 'text-red-500', label: 'error', spin: false },
  awaiting_approval: { icon: ShieldAlert, cls: 'text-amber-500', label: 'awaiting approval', spin: false },
};

function stringify(value: unknown): string {
  if (value === undefined || value === null) return '';
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function compactPreview(value: unknown): string {
  const text = stringify(value).replace(/s+/g, ' ').trim();
  if (!text) return 'No details available';
  return text.length > 180 ? text.slice(0, 177) + '...' : text;
}

function formatDuration(ms?: number): string | null {
  if (!ms) return null;
  if (ms < 1000) return ms + 'ms';
  return (ms / 1000).toFixed(ms < 10000 ? 1 : 0) + 's';
}

export const ToolExecutionCard: React.FC<ToolExecutionCardProps> = ({ tool }) => {
  const [isOpen, setIsOpen] = useState(tool.status === 'running' || tool.status === 'error');
  const [activeTab, setActiveTab] = useState<DetailTab>(tool.error ? 'error' : tool.output ? 'output' : 'args');
  const [copied, setCopied] = useState<DetailTab | 'all' | null>(null);

  const argsText = useMemo(() => stringify(tool.args), [tool.args]);
  const outputText = useMemo(() => stringify(tool.output), [tool.output]);
  const errorText = useMemo(() => stringify(tool.error), [tool.error]);
  const duration = formatDuration(tool.durationMs);
  const status = statusConfig[tool.status];
  const StatusIcon = status.icon;

  const availableTabs: ToolDetailTab[] = [
    { id: 'args', label: 'Args', icon: Clipboard, text: argsText },
    { id: 'output', label: 'Output', icon: FileText, text: outputText, tone: 'text-emerald-400 dark:text-emerald-300' },
    { id: 'error', label: 'Error', icon: AlertTriangle, text: errorText, tone: 'text-red-300' },
  ];
  const tabs = availableTabs.filter((tab) => tab.text);

  const selectedTab = tabs.find((tab) => tab.id === activeTab) || tabs[0];
  const preview = tool.error || tool.output || tool.args;

  const copy = (label: DetailTab | 'all', text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(label);
    setTimeout(() => setCopied(null), 1800);
  };

  return (
    <div className="my-2.5 overflow-hidden rounded-lg border border-border/60 bg-muted/30 transition-all hover:border-border dark:bg-card/40">
      <button
        type="button"
        onClick={() => setIsOpen((open) => !open)}
        className="flex w-full cursor-pointer items-center justify-between gap-3 px-3.5 py-2 text-left text-xs font-medium text-foreground select-none"
        aria-expanded={isOpen}
      >
        <div className="flex min-w-0 items-center gap-2">
          <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
            <Terminal className="h-3.5 w-3.5" />
          </div>
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              <span className="truncate font-mono text-xs font-semibold text-foreground/90">{tool.name}</span>
              <span className={cn('flex shrink-0 items-center gap-1 text-[11px] font-normal', status.cls)}>
                <StatusIcon className={cn('h-3 w-3', status.spin && 'animate-spin')} />
                {status.label}
              </span>
            </div>
            {!isOpen && <div className="mt-0.5 line-clamp-1 max-w-[520px] text-[10px] text-muted-foreground">{compactPreview(preview)}</div>}
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-2 text-muted-foreground">
          {duration ? <span className="rounded bg-muted px-1.5 py-0.5 text-[10px]">{duration}</span> : null}
          {isOpen ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
        </div>
      </button>

      {isOpen && (
        <div className="border-t border-border/40 bg-black/5 p-3 text-xs dark:bg-black/30">
          {tabs.length > 0 ? (
            <>
              <div className="mb-2 flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-1 rounded-lg bg-muted/50 p-1">
                  {tabs.map((tab) => {
                    const Icon = tab.icon;
                    return (
                      <button
                        key={tab.id}
                        type="button"
                        onClick={() => setActiveTab(tab.id)}
                        className={cn(
                          'flex items-center gap-1 rounded-md px-2 py-1 text-[10px] font-medium text-muted-foreground transition-colors hover:text-foreground',
                          selectedTab?.id === tab.id && 'bg-background text-foreground shadow-sm',
                        )}
                      >
                        <Icon className="h-3 w-3" />
                        {tab.label}
                      </button>
                    );
                  })}
                </div>
                {selectedTab && (
                  <button
                    type="button"
                    onClick={() => copy(selectedTab.id, selectedTab.text)}
                    className="flex items-center gap-1 rounded-md px-2 py-1 text-[10px] text-muted-foreground hover:bg-muted hover:text-foreground"
                  >
                    {copied === selectedTab.id ? <Check className="h-3 w-3 text-emerald-500" /> : <Copy className="h-3 w-3" />}
                    {copied === selectedTab.id ? 'Copied' : 'Copy'}
                  </button>
                )}
              </div>

              {selectedTab && (
                <pre className={cn('max-h-64 overflow-x-auto rounded border border-border/30 bg-background p-2 font-mono text-[11px] text-foreground/90', selectedTab.tone)}>
                  {selectedTab.text}
                </pre>
              )}
            </>
          ) : (
            <div className="rounded border border-dashed border-border/50 p-3 text-[11px] text-muted-foreground">No arguments or output captured for this tool.</div>
          )}
        </div>
      )}
    </div>
  );
};
