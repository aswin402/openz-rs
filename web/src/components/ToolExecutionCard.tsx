import React, { useState } from 'react';
import type { ToolExecution } from '../types';
import { Terminal, CheckCircle2, AlertTriangle, Loader2, ChevronDown, ChevronRight, Copy, Check, ShieldAlert } from 'lucide-react';

interface ToolExecutionCardProps {
  tool: ToolExecution;
}

export const ToolExecutionCard: React.FC<ToolExecutionCardProps> = ({ tool }) => {
  const [isOpen, setIsOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    const content =
      typeof tool.args === 'string' ? tool.args : JSON.stringify(tool.args, null, 2);
    navigator.clipboard.writeText(content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const statusConfig = {
    running: { icon: Loader2, cls: 'text-amber-500', label: 'executing...', spin: true },
    success: { icon: CheckCircle2, cls: 'text-emerald-500', label: 'success', spin: false },
    error: { icon: AlertTriangle, cls: 'text-red-500', label: 'error', spin: false },
    awaiting_approval: { icon: ShieldAlert, cls: 'text-amber-500', label: 'awaiting approval', spin: false },
  }[tool.status];
  const StatusIcon = statusConfig.icon;

  return (
    <div className="my-2.5 overflow-hidden rounded-lg border border-border/60 bg-muted/30 dark:bg-card/40 transition-all hover:border-border">
      {/* Header Bar */}
      <div
        onClick={() => setIsOpen(!isOpen)}
        className="flex cursor-pointer items-center justify-between px-3.5 py-2 text-xs font-medium text-foreground select-none"
      >
        <div className="flex items-center gap-2">
          <div className="flex h-6 w-6 items-center justify-center rounded-md bg-primary/10 text-primary">
            <Terminal className="h-3.5 w-3.5" />
          </div>
          <span className="font-mono text-xs font-semibold text-foreground/90">{tool.name}</span>
          <span className={`flex items-center gap-1 text-[11px] font-normal ${statusConfig.cls}`}>
            <StatusIcon className={`h-3 w-3 ${statusConfig.spin ? 'animate-spin' : ''}`} /> {statusConfig.label}
          </span>
        </div>

        <div className="flex items-center gap-2 text-muted-foreground">
          {tool.durationMs ? <span className="text-[10px]">{tool.durationMs}ms</span> : null}
          {isOpen ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
        </div>
      </div>

      {/* Collapsible Content Body */}
      {isOpen && (
        <div className="border-t border-border/40 bg-black/5 p-3 text-xs dark:bg-black/30">
          {tool.args && (
            <div className="mb-2">
              <div className="mb-1 flex items-center justify-between text-[11px] font-medium text-muted-foreground">
                <span>Arguments</span>
                <button
                  onClick={handleCopy}
                  className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground"
                >
                  {copied ? <Check className="h-3 w-3 text-emerald-500" /> : <Copy className="h-3 w-3" />}
                  {copied ? 'Copied' : 'Copy'}
                </button>
              </div>
              <pre className="max-h-48 overflow-x-auto rounded bg-background p-2 font-mono text-[11px] text-foreground/90 border border-border/30">
                {typeof tool.args === 'string' ? tool.args : JSON.stringify(tool.args, null, 2)}
              </pre>
            </div>
          )}

          {tool.output && (
            <div>
              <div className="mb-1 text-[11px] font-medium text-muted-foreground">Result Output</div>
              <pre className="max-h-60 overflow-x-auto rounded bg-background p-2 font-mono text-[11px] text-emerald-400 dark:text-emerald-300 border border-border/30">
                {tool.output}
              </pre>
            </div>
          )}

          {tool.error && (
            <div>
              <div className="mb-1 text-[11px] font-medium text-red-400">Error Detail</div>
              <pre className="max-h-48 overflow-x-auto rounded bg-red-950/20 p-2 font-mono text-[11px] text-red-300 border border-red-900/30">
                {tool.error}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
