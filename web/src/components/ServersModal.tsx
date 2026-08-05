import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, Server, Square } from 'lucide-react';

export const ServersModal: React.FC = () => {
  const isServersOpen = useOpenZStore((s) => s.isServersOpen);
  const setIsServersOpen = useOpenZStore((s) => s.setIsServersOpen);
  const servers = useOpenZStore((s) => s.servers);
  const channels = useOpenZStore((s) => s.channels);
  const stopServer = useOpenZStore((s) => s.stopServer);

  if (!isServersOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-2xl animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between pb-4 border-b border-border/50">
          <div className="flex items-center gap-2 text-foreground font-semibold text-base">
            <Server className="h-5 w-5 text-amber-500" /> Background Bots & Servers
          </div>
          <button
            onClick={() => setIsServersOpen(false)}
            className="rounded-lg p-1 text-muted-foreground hover:text-foreground hover:bg-muted"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-4 space-y-4 text-xs max-h-[50vh] overflow-y-auto pr-1">
          {/* Active Subprocesses Section */}
          <div>
            <div className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground mb-2 select-none">
              Active Processes
            </div>
            {servers.length === 0 ? (
              <div className="py-4 text-center text-muted-foreground border border-dashed border-border/60 rounded-xl bg-muted/10">
                No active background subprocesses running.
              </div>
            ) : (
              <div className="space-y-2">
                {servers.map((server) => (
                  <div
                    key={server.id}
                    className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/30 p-3 gap-4"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="font-semibold text-foreground flex items-center gap-2">
                        <span className="text-amber-500">#{server.id}</span>
                        <span className="capitalize">{server.kind}</span>
                        <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] font-mono text-muted-foreground">
                          PID: {server.pid}
                        </span>
                      </div>
                      <div className="font-mono text-[10px] text-muted-foreground mt-1 truncate" title={server.command}>
                        {server.command}
                      </div>
                    </div>
                    <button
                      onClick={() => stopServer(server.id)}
                      className="flex h-8 items-center gap-1.5 rounded-lg border border-red-500/30 bg-red-500/10 px-2.5 py-1 text-[11px] font-semibold text-red-400 hover:bg-red-500/25 hover:text-red-300 transition"
                    >
                      <Square className="h-3 w-3 fill-current" /> Stop
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Configured Channels Section */}
          <div className="pt-3 border-t border-border/40">
            <div className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground mb-2 select-none">
              Configured Channels
            </div>
            <div className="space-y-2">
              {channels.map((chan) => (
                <div
                  key={chan.name}
                  className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/30 p-3"
                >
                  <div>
                    <div className="font-semibold text-foreground capitalize flex items-center gap-2">
                      {chan.name}
                      <span
                        className={`rounded px-1.5 py-0.5 text-[9px] font-normal ${
                          chan.enabled
                            ? 'bg-emerald-500/20 text-emerald-400'
                            : 'bg-muted text-muted-foreground'
                        }`}
                      >
                        {chan.enabled ? 'Enabled' : 'Disabled'}
                      </span>
                    </div>
                    <div className="text-[10px] text-muted-foreground mt-1">
                      {chan.token_configured ? '🔑 API Token Configured' : '⚠️ No Token Set'}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="mt-6 flex justify-between gap-2">
          {servers.length > 0 ? (
            <button
              onClick={() => stopServer('all')}
              className="flex items-center gap-1.5 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 text-xs font-semibold text-red-400 hover:bg-red-500/20"
            >
              <Square className="h-3.5 w-3.5 fill-current" /> Stop All Servers
            </button>
          ) : (
            <div />
          )}
          <button
            onClick={() => setIsServersOpen(false)}
            className="rounded-lg bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:opacity-90"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
