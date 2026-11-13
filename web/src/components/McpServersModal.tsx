import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { Cpu, X } from 'lucide-react';
import type { McpServerInfo } from '../types/openz';
import { BeuiAlert } from './beui';
import { Badge } from './ui/badge';
import { Button } from './ui/button';

const STATUS_VARIANT: Record<
  McpServerInfo['status'],
  'success' | 'warning' | 'secondary' | 'destructive'
> = {
  connected: 'success',
  starting: 'warning',
  disabled: 'secondary',
  error: 'destructive',
};

export const McpServersModal: React.FC = () => {
  const isMcpsOpen = useOpenZStore((s) => s.isMcpsOpen);
  const setIsMcpsOpen = useOpenZStore((s) => s.setIsMcpsOpen);
  const mcpServers = useOpenZStore((s) => s.mcpServers);

  if (!isMcpsOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-foreground/45 p-4 backdrop-blur-xs">
      <div className="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-2xl">
        <div className="flex items-center justify-between border-b border-border/50 pb-4">
          <div className="flex items-center gap-2 text-base font-semibold text-foreground">
            <Cpu className="size-5 text-primary" />
            <span>Connected MCP Servers &amp; Native Tools</span>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => setIsMcpsOpen(false)}
            aria-label="Close MCP modal"
          >
            <X className="size-4" />
          </Button>
        </div>

        <div className="mt-4 space-y-2.5 text-xs">
          {mcpServers.length === 0 ? (
            <BeuiAlert tone="info" title="Native Rust Tooling Active">
              No external stdio MCP servers configured. OpenZ executes its 240+ native tools
              (filesystem, shell, semantic search, cognitive memory, browser) directly in-process.
            </BeuiAlert>
          ) : (
            mcpServers.map((server) => (
              <div
                key={server.name}
                className="flex items-center justify-between rounded-xl border border-border/65 bg-muted/25 p-3"
              >
                <div>
                  <div className="flex items-center gap-2 font-semibold text-foreground">
                    <span>{server.name}</span>
                    <Badge variant={STATUS_VARIANT[server.status]} className="capitalize">
                      {server.status}
                    </Badge>
                  </div>
                  <div className="mt-1 font-mono text-[11px] text-muted-foreground">
                    {server.command}
                  </div>
                </div>
                <div className="text-right">
                  <span className="text-sm font-bold text-primary">{server.toolsCount}</span>
                  <div className="text-[10px] text-muted-foreground">Tools</div>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="mt-6 flex justify-end">
          <Button type="button" size="sm" onClick={() => setIsMcpsOpen(false)}>
            Close
          </Button>
        </div>
      </div>
    </div>
  );
};
