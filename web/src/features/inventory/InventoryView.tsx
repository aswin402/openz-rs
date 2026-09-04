import React, { useEffect, useMemo, useState } from 'react';
import { Archive, RefreshCw, X } from 'lucide-react';
import { useOpenZStore } from '../../store/useOpenZStore';
import { wsService } from '../../services/websocket';
import { TIMING_MS } from '../../config/runtime';
import { cn } from '../../lib/utils';
import {
  ChannelsTab,
  CoreTab,
  CronTab,
  InventorySummary,
  SessionsTab,
  tabs,
  ToolsTab,
  PathsTab,
  type InventoryTab,
} from './inventoryTabs';

export const InventoryView: React.FC = () => {
  const inventory = useOpenZStore((s) => s.runtimeInventory);
  const cronLogs = useOpenZStore((s) => s.cronLogs);
  const pauseCronJob = useOpenZStore((s) => s.pauseCronJob);
  const resumeCronJob = useOpenZStore((s) => s.resumeCronJob);
  const deleteCronJob = useOpenZStore((s) => s.deleteCronJob);
  const requestCronLogs = useOpenZStore((s) => s.requestCronLogs);
  const selectSession = useOpenZStore((s) => s.selectSession);
  const archiveSession = useOpenZStore((s) => s.archiveSession);
  const deleteSession = useOpenZStore((s) => s.deleteSession);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const setWorkspaceNotice = useOpenZStore((s) => s.setWorkspaceNotice);
  const notice = useOpenZStore((s) => s.workspaceNotice);
  const clearWorkspaceNotice = useOpenZStore((s) => s.clearWorkspaceNotice);
  const [activeTab, setActiveTab] = useState<InventoryTab>('core');
  const [toolSearch, setToolSearch] = useState('');

  // Cron status is persisted by a background scheduler, so refresh inventory
  // while this page is open instead of showing the connection-time snapshot.
  useEffect(() => {
    wsService.requestRuntimeInventory();
    const timer = window.setInterval(() => wsService.requestRuntimeInventory(), TIMING_MS.inventoryPoll);
    return () => window.clearInterval(timer);
  }, []);

  const filteredTools = useMemo(() => {
    const query = toolSearch.trim().toLowerCase();
    const tools = inventory?.tools ?? [];
    if (!query) return tools;
    return tools.filter((tool) => {
      return (
        tool.name.toLowerCase().includes(query) ||
        tool.domain.toLowerCase().includes(query) ||
        tool.risk.toLowerCase().includes(query) ||
        tool.description.toLowerCase().includes(query)
      );
    });
  }, [inventory?.tools, toolSearch]);

  return (
    <div className="mx-auto max-w-6xl px-4 py-8">
      <div className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-extrabold tracking-tight text-foreground">
            <Archive className="h-5 w-5 text-amber-500" />
            Core Inventory
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            One OpenZ core exposed to every UI and channel from the gateway.
          </p>
        </div>
        <button
          type="button"
          onClick={() => wsService.requestRuntimeInventory()}
          className="flex items-center justify-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs font-semibold text-amber-400 transition hover:bg-amber-500/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
        >
          <RefreshCw className="h-4 w-4" /> Refresh
        </button>
      </div>

      {notice?.scope === 'inventory' && (
        <div
          className={cn(
            'mb-4 flex items-center justify-between gap-3 rounded-xl border px-4 py-3 text-sm',
            notice.type === 'error'
              ? 'border-red-500/30 bg-red-500/10 text-red-300'
              : notice.type === 'success'
                ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300'
                : 'border-amber-500/30 bg-amber-500/10 text-amber-300',
          )}
        >
          <span>{notice.message}</span>
          <button
            type="button"
            onClick={() => clearWorkspaceNotice('inventory')}
            className="rounded p-1 opacity-80 transition hover:bg-background/40 hover:opacity-100"
            aria-label="Dismiss inventory notice"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      )}

      {!inventory ? (
        <div className="rounded-2xl border border-border/60 bg-card/45 p-8 text-sm text-muted-foreground">
          Waiting for gateway inventory. Use Refresh if the gateway is already connected.
        </div>
      ) : (
        <>
          <InventorySummary inventory={inventory} />

          <div className="mt-6 flex flex-wrap gap-2 border-b border-border/50 pb-3">
            {tabs.map((tab) => {
              const Icon = tab.icon;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setActiveTab(tab.id)}
                  className={cn(
                    'flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50',
                    activeTab === tab.id
                      ? 'bg-amber-500/15 text-amber-400'
                      : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                  )}
                >
                  <Icon className="h-4 w-4" /> {tab.label}
                </button>
              );
            })}
          </div>

          <div className="mt-6">
            {activeTab === 'core' && <CoreTab inventory={inventory} />}
            {activeTab === 'sessions' && (
              <SessionsTab
                inventory={inventory}
                activeSessionKey={activeChatId}
                onOpen={(sessionKey) => {
                  selectSession(sessionKey);
                  wsService.requestRuntimeInventory();
                }}
                onCopy={(sessionKey) => {
                  void navigator.clipboard?.writeText(sessionKey);
                  setWorkspaceNotice({ scope: 'inventory', type: 'success', message: `Copied ${sessionKey}.` });
                  wsService.requestRuntimeInventory();
                }}
                onArchive={archiveSession}
                onDelete={deleteSession}
              />
            )}
            {activeTab === 'tools' && (
              <ToolsTab tools={filteredTools} totalTools={inventory.tools.length} search={toolSearch} onSearch={setToolSearch} />
            )}
            {activeTab === 'cron' && (
              <CronTab
                inventory={inventory}
                cronLogs={cronLogs}
                onPause={pauseCronJob}
                onResume={resumeCronJob}
                onDelete={deleteCronJob}
                onLoadLogs={requestCronLogs}
              />
            )}
            {activeTab === 'paths' && <PathsTab inventory={inventory} />}
            {activeTab === 'channels' && <ChannelsTab inventory={inventory} />}
          </div>
        </>
      )}
    </div>
  );
};
