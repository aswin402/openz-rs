import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, ScrollText } from 'lucide-react';

export const LogsDrawer: React.FC = () => {
  const isLogsOpen = useOpenZStore((s) => s.isLogsOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const logs = useOpenZStore((s) => s.logs);

  if (!isLogsOpen) return null;

  return (
    <div className="fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col border-l border-border bg-card shadow-2xl animate-in slide-in-from-right duration-200">
      <div className="flex h-14 items-center justify-between px-4 border-b border-border/50">
        <div className="flex items-center gap-2 font-semibold text-sm text-foreground">
          <ScrollText className="h-4 w-4 text-amber-500" /> OpenZ Structured Logs
        </div>
        <button
          onClick={() => setIsLogsOpen(false)}
          className="rounded-lg p-1 text-muted-foreground hover:text-foreground hover:bg-muted"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-2 font-mono text-[11px]">
        {logs.length === 0 ? (
          <div className="text-center py-12 text-muted-foreground">No logs recorded yet.</div>
        ) : (
          logs.map((log) => (
            <div key={log.id} className="rounded bg-muted/40 p-2 border border-border/30">
              <div className="flex items-center justify-between text-[10px] text-muted-foreground mb-1">
                <span>{log.timestamp}</span>
                <span
                  className={`font-semibold ${
                    log.level === 'ERROR'
                      ? 'text-red-400'
                      : log.level === 'WARN'
                      ? 'text-amber-400'
                      : 'text-emerald-400'
                  }`}
                >
                  {log.level}
                </span>
              </div>
              <div className="text-foreground/90 break-all">{log.message}</div>
              <div className="text-[9px] text-muted-foreground/60 mt-1">{log.target}</div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
