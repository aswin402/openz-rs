import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, Cpu } from 'lucide-react';
import type { McpServerInfo } from '../types/openz';

const STATUS_BADGE: Record<McpServerInfo['status'], { label: string; cls: string; dot: string }> = {
  connected: { label: 'Connected', cls: 'bg-emerald-500/20 text-emerald-400', dot: 'bg-emerald-400' },
  starting: { label: 'Starting', cls: 'bg-amber-500/20 text-amber-300', dot: 'bg-amber-400' },
  disabled: { label: 'Disabled', cls: 'bg-muted text-muted-foreground', dot: 'bg-muted-foreground' },
  error: { label: 'Error', cls: 'bg-red-500/20 text-red-400', dot: 'bg-red-400' },
};

export const McpServersModal: React.FC = () => {
  const isMcpsOpen = useOpenZStore((s) => s.isMcpsOpen);
  const setIsMcpsOpen = useOpenZStore((s) => s.setIsMcpsOpen);
  const mcpServers = useOpenZStore((s) => s.mcpServers);

  if (!isMcpsOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-2xl animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between pb-4 border-b border-border/50">
          <div className="flex items-center gap-2 text-foreground font-semibold text-base">
            <Cpu className="h-5 w-5 text-amber-500" /> Connected MCP Servers & Tools
          </div>
          <button
            onClick={() => setIsMcpsOpen(false)}
            className="rounded-lg p-1 text-muted-foreground hover:text-foreground hover:bg-muted"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-4 space-y-2.5 text-xs">
          {mcpServers.map((server) => (
            <div
              key={server.name}
              className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/30 p-3"
            >
              <div>
                <div className="font-semibold text-foreground flex items-center gap-2">
                  {server.name}
                  <span
                    className={`flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-normal ${STATUS_BADGE[server.status].cls}`}
                  >
                    <span className={`h-1.5 w-1.5 rounded-full ${STATUS_BADGE[server.status].dot} ${server.status === 'starting' ? 'animate-pulse' : ''}`} />
                    {STATUS_BADGE[server.status].label}
                  </span>
                </div>
                <div className="font-mono text-[11px] text-muted-foreground mt-1">{server.command}</div>
              </div>
              <div className="text-right">
                <span className="font-bold text-amber-500 text-sm">{server.toolsCount}</span>
                <div className="text-[10px] text-muted-foreground">Registered Tools</div>
              </div>
            </div>
          ))}
        </div>

        <div className="mt-6 flex justify-end">
          <button
            onClick={() => setIsMcpsOpen(false)}
            className="rounded-lg bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:opacity-90"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
