import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { OpenZBrandLogo } from './OpenZFoxIcon';
import { cn } from '../lib/utils';
import {
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Settings,
  Trash2,
} from 'lucide-react';

export const Sidebar: React.FC = () => {
  const sessions = useOpenZStore((s) => s.sessions);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const activeTurnIds = useOpenZStore((s) => s.activeTurnIds);
  const activeView = useOpenZStore((s) => s.activeView);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const selectSession = useOpenZStore((s) => s.selectSession);
  const newSession = useOpenZStore((s) => s.newSession);
  const deleteSession = useOpenZStore((s) => s.deleteSession);
  const clearActiveSession = useOpenZStore((s) => s.clearActiveSession);
  const connectionStatus = useOpenZStore((s) => s.connectionStatus);
  const mcpStats = useOpenZStore((s) => s.mcpStats);
  const setIsMcpsOpen = useOpenZStore((s) => s.setIsMcpsOpen);

  const isSidebarOpen = useOpenZStore((s) => s.isSidebarOpen);
  const setIsSidebarOpen = useOpenZStore((s) => s.setIsSidebarOpen);
  const collapsed = useOpenZStore((s) => s.isSidebarCollapsed);
  const setSidebarCollapsed = useOpenZStore((s) => s.setSidebarCollapsed);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);

  const go = (fn: () => void) => {
    fn();
    setIsSidebarOpen(false);
  };

  return (
    <>
      {isSidebarOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/45 backdrop-blur-md dark:bg-black/60 md:hidden"
          onClick={() => setIsSidebarOpen(false)}
        />
      )}

      <aside
        className={cn(
          'fixed inset-y-0 left-0 z-50 flex flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-[width,transform] duration-200 will-change-[width]',
          'w-[272px] md:static md:z-auto',
          collapsed ? 'md:w-[68px]' : 'md:w-[264px]',
          isSidebarOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0',
        )}
      >
        {/* Brand Header (Inline OpenZ Agent + Fox Silhouette Logo) */}
        <div className="flex h-14 shrink-0 items-center justify-between border-b border-border/45 px-3.5 overflow-hidden">
          {!collapsed && (
            <button
              type="button"
              onClick={() => go(() => setActiveView('chats'))}
              className="flex min-w-0 items-center whitespace-nowrap text-left"
            >
              <OpenZBrandLogo size="sm" />
            </button>
          )}

          <button
            type="button"
            onClick={() => {
              if (window.innerWidth >= 768) {
                setSidebarCollapsed(!collapsed);
              } else {
                setIsSidebarOpen(false);
              }
            }}
            className={cn(
              'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground',
              collapsed && 'mx-auto',
            )}
            title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
            aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            {collapsed ? (
              <PanelLeftOpen className="h-4 w-4" />
            ) : (
              <PanelLeftClose className="h-4 w-4" />
            )}
          </button>
        </div>

        {/* + New Session Button */}
        <div className="shrink-0 p-3">
          <button
            type="button"
            onClick={() => go(newSession)}
            className={cn(
              'group relative flex h-9 w-full items-center rounded-xl border border-primary/35 bg-primary/10 px-2.5 text-xs font-semibold text-primary transition-colors hover:bg-primary/20 overflow-hidden',
              collapsed && 'justify-center px-0',
            )}
            title={collapsed ? 'New Session' : undefined}
          >
            <div className="flex h-5 w-5 shrink-0 items-center justify-center">
              <Plus className="h-4 w-4 shrink-0 transition-transform duration-200 group-hover:scale-110" />
            </div>
            {!collapsed && <span className="ml-2 truncate">New Session</span>}
          </button>
        </div>

        {/* Sessions List */}
        <div
          className={cn(
            'flex-1 overflow-y-auto px-3 py-1',
            collapsed && 'md:pointer-events-none md:opacity-0',
          )}
        >
          <div className="px-2 pb-1.5 text-[10px] font-bold uppercase tracking-wider text-muted-foreground/65">
            Sessions
          </div>

          {sessions.length === 0 ? (
            <div className="px-2 py-3 text-[11px] text-muted-foreground/60">
              No sessions yet.
            </div>
          ) : (
            <div className="space-y-0.5">
              {sessions.map((session) => {
                const isActive = session.id === activeChatId && activeView === 'chats';
                const isRunning = Boolean(activeTurnIds[session.id]);
                return (
                  <div
                    key={session.id}
                    onClick={() => go(() => selectSession(session.id))}
                    className={cn(
                      'group flex cursor-pointer select-none items-center justify-between rounded-lg px-2.5 py-2 text-xs transition-colors',
                      isActive
                        ? 'bg-primary/15 font-medium text-primary'
                        : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                    )}
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      {isRunning ? (
                        <span className="size-2 shrink-0 animate-pulse rounded-full bg-primary" />
                      ) : (
                        <MessageSquare className="size-3.5 shrink-0" />
                      )}
                      <span className="truncate">{session.title}</span>
                    </div>
                    {!session.isDraft && (
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          deleteSession(session.id);
                        }}
                        className="shrink-0 rounded p-0.5 opacity-0 transition hover:text-destructive group-hover:opacity-100"
                        title="Delete session"
                        aria-label={`Delete ${session.title}`}
                      >
                        <Trash2 className="size-3" />
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Minimal Footer: Settings, Status & Clear Active Session */}
        <div className="shrink-0 border-t border-border/45 p-3">
          <nav className="mb-2.5 space-y-1">
            <button
              type="button"
              onClick={() => go(() => setIsSettingsOpen(true))}
              title={collapsed ? 'Settings' : undefined}
              className={cn(
                'flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-[13px] text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground',
                collapsed && 'justify-center px-0',
              )}
            >
              <Settings className="size-4 shrink-0" />
              {!collapsed && <span className="truncate">Settings</span>}
            </button>
          </nav>

          <button
            type="button"
            onClick={() => setIsMcpsOpen(true)}
            className={cn(
              'mb-2 flex h-7 w-full items-center justify-between rounded-md px-1.5 text-[10px] text-muted-foreground transition-colors hover:text-foreground',
              collapsed && 'justify-center',
            )}
            title="Inspect Gateway & MCP servers"
          >
            <span className="flex items-center gap-2">
              <span
                className={cn(
                  'size-2.5 shrink-0 rounded-full',
                  connectionStatus === 'connected'
                    ? 'bg-success'
                    : connectionStatus === 'connecting'
                      ? 'animate-pulse bg-warning'
                      : 'bg-destructive',
                )}
              />
              {!collapsed && <span className="font-medium capitalize">{connectionStatus}</span>}
            </span>
            {!collapsed && (
              <span>
                {mcpStats.loaded}/{mcpStats.total} MCP
              </span>
            )}
          </button>

          <button
            type="button"
            onClick={clearActiveSession}
            disabled={!activeChatId}
            title={collapsed ? 'Clear Active Session' : undefined}
            className={cn(
              'group flex h-8 w-full items-center gap-2.5 rounded-lg bg-muted/40 px-2.5 text-[11px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40',
              collapsed && 'justify-center px-0',
            )}
          >
            <Trash2 className="size-3 shrink-0 text-destructive/80 group-hover:text-destructive" />
            {!collapsed && <span className="truncate">Clear Active Session</span>}
          </button>
        </div>
      </aside>
    </>
  );
};
