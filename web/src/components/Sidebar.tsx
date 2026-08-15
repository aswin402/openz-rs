import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { cn } from '../lib/utils';
import {
  Plus,
  MessageSquare,
  Trash2,
  Settings,
  PanelLeftClose,
  PanelLeftOpen,
} from 'lucide-react';

interface NavItem {
  key: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  action: () => void;
  badge?: string;
  openPanel?: boolean;
}

export const Sidebar: React.FC = () => {
  const sessions = useOpenZStore((s) => s.sessions);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const activeView = useOpenZStore((s) => s.activeView);
  const selectSession = useOpenZStore((s) => s.selectSession);
  const newSession = useOpenZStore((s) => s.newSession);
  const deleteSession = useOpenZStore((s) => s.deleteSession);
  const clearActiveSession = useOpenZStore((s) => s.clearActiveSession);
  const connectionStatus = useOpenZStore((s) => s.connectionStatus);
  const mcpStats = useOpenZStore((s) => s.mcpStats);

  const isSidebarOpen = useOpenZStore((s) => s.isSidebarOpen);
  const setIsSidebarOpen = useOpenZStore((s) => s.setIsSidebarOpen);
  const collapsed = useOpenZStore((s) => s.isSidebarCollapsed);
  const setSidebarCollapsed = useOpenZStore((s) => s.setSidebarCollapsed);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);

  const go = (fn: () => void) => {
    fn();
    setIsSidebarOpen(false); // close the mobile drawer on pick
  };

  const bottomItems: NavItem[] = [
    {
      key: 'settings',
      label: 'Settings',
      icon: Settings,
      action: () => setIsSettingsOpen(true),
      openPanel: true,
    },
  ];

  const renderNav = (items: NavItem[]) =>
    items.map((item) => {
      const Icon = item.icon;
      const isActive = !item.openPanel && activeView === item.key;
      return (
        <button
          key={item.label}
          onClick={() => go(item.action)}
          className={cn(
            'group relative flex h-9 w-full items-center rounded-lg px-2.5 text-[13px] transition-all duration-300 ease-[cubic-bezier(0.2,0,0,1)] overflow-hidden',
            isActive
              ? 'bg-amber-500/15 font-medium text-amber-400'
              : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
          )}
          title={collapsed ? item.label : undefined}
        >
          <div className="flex h-5 w-5 shrink-0 items-center justify-center">
            <Icon className="h-4 w-4" />
          </div>
          <span
            className={cn(
              'ml-2.5 inline-block truncate whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
              collapsed
                ? 'max-w-0 opacity-0 -translate-x-2 pointer-events-none'
                : 'max-w-[150px] opacity-100 translate-x-0',
            )}
          >
            {item.label}
          </span>
          {item.badge && (
            <span
              className={cn(
                'ml-auto rounded-full bg-muted/70 px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground transition-all duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
                collapsed ? 'opacity-0 scale-0 pointer-events-none w-0 h-0 p-0 overflow-hidden' : 'opacity-100 scale-100',
              )}
            >
              {item.badge}
            </span>
          )}
          {collapsed && (
            <span className="pointer-events-none absolute left-full top-1/2 ml-2 hidden -translate-y-1/2 whitespace-nowrap rounded-md border border-border bg-popover px-2 py-1 text-[11px] font-medium text-foreground shadow-lg md:group-hover:block z-50 animate-in fade-in zoom-in-95 duration-150">
              {item.label}
            </span>
          )}
        </button>
      );
    });

  return (
    <>
      {/* Mobile backdrop */}
      {isSidebarOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm md:hidden animate-in fade-in duration-200"
          onClick={() => setIsSidebarOpen(false)}
        />
      )}

      <aside
        className={cn(
          'fixed inset-y-0 left-0 z-50 flex flex-col border-r border-border/60 bg-card/95 backdrop-blur-md transition-[width,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)] will-change-[width]',
          'w-[272px]',
          'md:static md:z-auto md:bg-card/50',
          collapsed ? 'md:w-[68px]' : 'md:w-[264px]',
          isSidebarOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0',
        )}
      >
        {/* Brand Header */}
        <div className="flex h-14 shrink-0 items-center justify-between border-b border-border/40 px-3.5 transition-all duration-300 overflow-hidden">
          <div
            className={cn(
              'flex items-center gap-1.5 overflow-hidden whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
              collapsed
                ? 'max-w-0 opacity-0 -translate-x-3 pointer-events-none'
                : 'max-w-[180px] opacity-100 translate-x-0',
            )}
          >
            <span className="text-sm font-extrabold tracking-tight text-foreground whitespace-nowrap">
              OpenZ <span className="bg-gradient-to-r from-amber-500 to-orange-500 bg-clip-text text-transparent">Agent 🦊</span>
            </span>
          </div>
          <button
            onClick={() => {
              if (window.innerWidth >= 768) {
                setSidebarCollapsed(!collapsed);
              } else {
                setIsSidebarOpen(false);
              }
            }}
            className={cn(
              'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50',
              collapsed && 'mx-auto'
            )}
            title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
            aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            {collapsed ? <PanelLeftOpen className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
          </button>
        </div>

        {/* New Session Button */}
        <div className="shrink-0 p-3 transition-all duration-300">
          <button
            onClick={() => go(newSession)}
            className="group relative flex h-9 w-full items-center rounded-xl border border-amber-500/30 bg-amber-500/10 px-2.5 text-xs font-semibold text-amber-400 transition-all duration-300 ease-[cubic-bezier(0.2,0,0,1)] hover:bg-amber-500/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50 overflow-hidden"
            title={collapsed ? 'New Session' : undefined}
          >
            <div className="flex h-5 w-5 shrink-0 items-center justify-center">
              <Plus className="h-4 w-4 shrink-0 transition-transform duration-200 group-hover:scale-110" />
            </div>
            <span
              className={cn(
                'ml-2.5 inline-block truncate whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
                collapsed
                  ? 'max-w-0 opacity-0 -translate-x-2 pointer-events-none'
                  : 'max-w-[150px] opacity-100 translate-x-0',
              )}
            >
              New Session
            </span>
            {collapsed && (
              <span className="pointer-events-none absolute left-full top-1/2 ml-2 hidden -translate-y-1/2 whitespace-nowrap rounded-md border border-border bg-popover px-2 py-1 text-[11px] font-medium text-foreground shadow-lg md:group-hover:block z-50 animate-in fade-in zoom-in-95 duration-150">
                New Session
              </span>
            )}
          </button>
        </div>

        {/* Sessions List */}
        <div
          className={cn(
            'flex-1 overflow-y-auto overflow-x-hidden px-2 pb-2 transition-[opacity,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
            collapsed
              ? 'md:opacity-0 md:pointer-events-none md:scale-95'
              : 'md:opacity-100 md:scale-100',
          )}
        >
          <div className="px-2 py-1.5 text-[10px] font-bold uppercase tracking-wider text-muted-foreground/60">
            Sessions
          </div>
          {sessions.length === 0 ? (
            <div className="px-2.5 py-3 text-[11px] text-muted-foreground/50 italic">
              No sessions yet.
            </div>
          ) : (
            <div className="space-y-1">
              {sessions.map((session) => {
                const isActive = session.id === activeChatId && activeView === 'chats';
                return (
                  <div
                    key={session.id}
                    onClick={() => go(() => selectSession(session.id))}
                    className={cn(
                      'group flex cursor-pointer select-none items-center justify-between rounded-lg px-2.5 py-2 text-xs transition-colors duration-200',
                      isActive
                        ? 'border border-amber-500/30 bg-amber-500/15 font-semibold text-amber-400'
                        : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                    )}
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <MessageSquare className="h-3.5 w-3.5 shrink-0" />
                      <span className="truncate">{session.title}</span>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        deleteSession(session.id);
                      }}
                      className="shrink-0 p-1 opacity-0 transition hover:text-red-400 group-hover:opacity-100"
                      title="Delete session"
                    >
                      <Trash2 className="h-3 w-3" />
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer & Bottom Navigation */}
        <div className="shrink-0 border-t border-border/40 p-3 transition-all duration-300">
          {/* Bottom Items Grid */}
          <nav className="space-y-1 mb-2.5">
            {renderNav(bottomItems)}
          </nav>

          {/* Connection and telemetry */}
          <div className="flex h-7 items-center justify-between px-1.5 mb-2 transition-all duration-300 overflow-hidden">
            <div className="flex items-center gap-2 shrink-0">
              <span
                className={cn(
                  'h-2.5 w-2.5 rounded-full shrink-0 transition-colors duration-300',
                  connectionStatus === 'connected'
                    ? 'bg-emerald-500 shadow-sm shadow-emerald-500/50'
                    : connectionStatus === 'connecting'
                      ? 'animate-pulse bg-amber-500'
                      : 'bg-red-500',
                )}
                title={`Status: ${connectionStatus}`}
              />
              <span
                className={cn(
                  'text-[10px] font-medium capitalize text-muted-foreground whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
                  collapsed
                    ? 'max-w-0 opacity-0 -translate-x-2 pointer-events-none'
                    : 'max-w-[120px] opacity-100 translate-x-0',
                )}
              >
                {connectionStatus}
              </span>
            </div>

            <span
              className={cn(
                'text-[10px] text-muted-foreground whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
                collapsed
                  ? 'max-w-0 opacity-0 -translate-x-2 pointer-events-none'
                  : 'max-w-[90px] opacity-100 translate-x-0',
              )}
            >
              {mcpStats.loaded}/{mcpStats.total} MCP
            </span>
          </div>

          {/* Clear session */}
          <button
            onClick={clearActiveSession}
            disabled={!activeChatId}
            className={cn(
              'group relative flex h-8 w-full items-center rounded-lg bg-muted/40 px-2.5 text-[11px] text-muted-foreground transition-all duration-300 ease-[cubic-bezier(0.2,0,0,1)] hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 overflow-hidden',
            )}
            title={collapsed ? 'Clear Active Session' : undefined}
          >
            <div className="flex h-4 w-4 shrink-0 items-center justify-center">
              <Trash2 className="h-3 w-3 text-red-400/80 group-hover:text-red-400 transition-colors" />
            </div>
            <span
              className={cn(
                'ml-2.5 inline-block truncate whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-[cubic-bezier(0.2,0,0,1)]',
                collapsed
                  ? 'max-w-0 opacity-0 -translate-x-2 pointer-events-none'
                  : 'max-w-[150px] opacity-100 translate-x-0',
              )}
            >
              Clear Active Session
            </span>
            {collapsed && (
              <span className="pointer-events-none absolute left-full top-1/2 ml-2 hidden -translate-y-1/2 whitespace-nowrap rounded-md border border-border bg-popover px-2 py-1 text-[11px] font-medium text-foreground shadow-lg md:group-hover:block z-50 animate-in fade-in zoom-in-95 duration-150">
                Clear Active Session
              </span>
            )}
          </button>
        </div>
      </aside>
    </>
  );
};