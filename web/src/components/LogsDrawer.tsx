import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, ScrollText } from 'lucide-react';
import { Badge } from './ui/badge';
import { Button } from './ui/button';

export const LogsDrawer: React.FC = () => {
  const isLogsOpen = useOpenZStore((s) => s.isLogsOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const logs = useOpenZStore((s) => s.logs);

  if (!isLogsOpen) return null;

  return (
    <div className="fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col border-l border-border bg-card shadow-2xl">
      <div className="flex h-13 items-center justify-between border-b border-border/50 px-4">
        <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
          <ScrollText className="size-4 text-primary" />
          <span>OpenZ Structured Logs</span>
        </div>
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

      <div className="flex-1 space-y-2 overflow-y-auto p-4 font-mono text-[11px]">
        {logs.length === 0 ? (
          <div className="py-12 text-center text-muted-foreground">No logs recorded yet.</div>
        ) : (
          logs.map((log) => (
            <div key={log.id} className="rounded-lg border border-border/50 bg-muted/30 p-2.5">
              <div className="mb-1 flex items-center justify-between text-[10px] text-muted-foreground">
                <span>{log.timestamp}</span>
                <Badge
                  variant={
                    log.level === 'ERROR'
                      ? 'destructive'
                      : log.level === 'WARN'
                        ? 'warning'
                        : 'success'
                  }
                  className="px-1.5 py-0 text-[9px]"
                >
                  {log.level}
                </Badge>
              </div>
              <div className="break-all text-foreground/90">{log.message}</div>
              <div className="mt-1 text-[9px] text-muted-foreground/65">{log.target}</div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
