import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { X, Server, Square } from 'lucide-react';
import { Badge } from './ui/badge';
import { Button } from './ui/button';

export const ServersModal: React.FC = () => {
  const isServersOpen = useOpenZStore((s) => s.isServersOpen);
  const setIsServersOpen = useOpenZStore((s) => s.setIsServersOpen);
  const servers = useOpenZStore((s) => s.servers);
  const channels = useOpenZStore((s) => s.channels);
  const stopServer = useOpenZStore((s) => s.stopServer);

  if (!isServersOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-foreground/45 p-4 backdrop-blur-xs">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-2xl">
        <div className="flex items-center justify-between border-b border-border/50 pb-4">
          <div className="flex items-center gap-2 text-base font-semibold text-foreground">
            <Server className="size-5 text-primary" />
            <span>Background Bots &amp; Servers</span>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => setIsServersOpen(false)}
            aria-label="Close servers modal"
          >
            <X className="size-4" />
          </Button>
        </div>

        <div className="mt-4 max-h-[50vh] space-y-4 overflow-y-auto pr-1 text-xs">
          <div>
            <div className="mb-2 select-none text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
              Active Processes
            </div>
            {servers.length === 0 ? (
              <div className="rounded-xl border border-dashed border-border/60 bg-muted/15 py-4 text-center text-muted-foreground">
                No active background subprocesses running.
              </div>
            ) : (
              <div className="space-y-2">
                {servers.map((server) => (
                  <div
                    key={server.id}
                    className="flex items-center justify-between gap-4 rounded-xl border border-border/60 bg-muted/25 p-3"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2 font-semibold text-foreground">
                        <span className="text-primary">#{server.id}</span>
                        <span className="capitalize">{server.kind}</span>
                        <Badge variant="outline" className="font-mono text-[10px]">
                          PID: {server.pid}
                        </Badge>
                      </div>
                      <div
                        className="mt-1 truncate font-mono text-[10px] text-muted-foreground"
                        title={server.command}
                      >
                        {server.command}
                      </div>
                    </div>
                    <Button
                      type="button"
                      variant="destructive"
                      size="xs"
                      onClick={() => stopServer(server.id)}
                    >
                      <Square className="size-3 fill-current" />
                      <span>Stop</span>
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="border-t border-border/40 pt-3">
            <div className="mb-2 select-none text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
              Configured Channels
            </div>
            <div className="space-y-2">
              {channels.map((chan) => (
                <div
                  key={chan.name}
                  className="flex items-center justify-between rounded-xl border border-border/60 bg-muted/25 p-3"
                >
                  <div>
                    <div className="flex items-center gap-2 font-semibold capitalize text-foreground">
                      <span>{chan.name}</span>
                      <Badge variant={chan.enabled ? 'success' : 'secondary'}>
                        {chan.enabled ? 'Enabled' : 'Disabled'}
                      </Badge>
                    </div>
                    <div className="mt-1 text-[10px] text-muted-foreground">
                      {chan.token_configured ? 'API Token Configured' : 'No Token Set'}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="mt-6 flex justify-between gap-2">
          {servers.length > 0 ? (
            <Button
              type="button"
              variant="destructive"
              size="sm"
              onClick={() => stopServer('all')}
            >
              <Square className="size-3.5 fill-current" />
              <span>Stop All Servers</span>
            </Button>
          ) : (
            <div />
          )}
          <Button type="button" size="sm" onClick={() => setIsServersOpen(false)}>
            Close
          </Button>
        </div>
      </div>
    </div>
  );
};
