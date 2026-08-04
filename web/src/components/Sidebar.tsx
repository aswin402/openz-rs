import React from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import type { WorkspaceView } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import { cn } from '../lib/utils';
import { useThemeStore } from '../store/useThemeStore';
import {
  Plus,
  MessageSquare,
  Trash2,
  LayoutDashboard,
  Bot,
  BookOpen,
  BrainCircuit,
  Database,
  Cpu,
  ScrollText,
  Settings,
  PanelLeftClose,
  PanelLeftOpen,
  ChevronsLeft,
  ChevronsRight,
  Sun,
  Moon,
} from 'lucide-react';

interface NavItem {
  key: WorkspaceView;
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
  const status = useOpenZStore((s) => s.status);
  const connectionStatus = useOpenZStore((s) => s.connectionStatus);
  const mcpStats = useOpenZStore((s) => s.mcpStats);
  const logs = useOpenZStore((s) => s.logs);
  const cognitiveStats = useOpenZStore((s) => s.cognitiveStats);

  const activeModel = useOpenZStore((s) => s.activeModel);
  const providers = useOpenZStore((s) => s.providers);
  const setActiveModel = useOpenZStore((s) => s.setActiveModel);

  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const resolvedTheme =
    theme === 'system'
      ? window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light'
      : theme;

  const handleThemeClick = () => {
    setTheme(resolvedTheme === 'dark' ? 'light' : 'dark');
  };

  const handleModelChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    setActiveModel(e.target.value);
  };

  const groups = providers.filter((p) => p.models.length > 0);

  const isSidebarOpen = useOpenZStore((s) => s.isSidebarOpen);
  const setIsSidebarOpen = useOpenZStore((s) => s.setIsSidebarOpen);
  const collapsed = useOpenZStore((s) => s.isSidebarCollapsed);
  const setSidebarCollapsed = useOpenZStore((s) => s.setSidebarCollapsed);
  const setActiveView = useOpenZStore((s) => s.setActiveView);

  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const setIsMcpsOpen = useOpenZStore((s) => s.setIsMcpsOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);

  const go = (fn: () => void) => {
    fn();
    setIsSidebarOpen(false); // close the mobile drawer on pick
  };

  // Label/decoration visibility: hidden on desktop when the rail is collapsed,
  // always visible on mobile (the drawer is full-width when opened).
  const lbl = collapsed ? 'md:hidden' : '';

  const workspaceItems: NavItem[] = [
    {
      key: 'dashboard',
      label: 'Dashboard',
      icon: LayoutDashboard,
      action: () => setActiveView('dashboard'),
    },
    {
      key: 'chats',
      label: 'Chats',
      icon: MessageSquare,
      action: () => setActiveView('chats'),
      badge: sessions.length > 0 ? String(sessions.length) : undefined,
    },
    {
      key: 'agents',
      label: 'Agents',
      icon: Bot,
      action: () => setActiveView('agents'),
    },
    {
      key: 'skills',
      label: 'Skills',
      icon: BookOpen,
      action: () => setActiveView('skills'),
    },
    {
      key: 'knowledge',
      label: 'Knowledge',
      icon: BrainCircuit,
      action: () => setActiveView('knowledge'),
    },
  ];

  const systemItems: NavItem[] = [
    {
      key: 'dashboard',
      label: 'Memory',
      icon: Database,
      action: () => {
        setIsMemoryOpen(true);
        wsService.requestCognitiveMemory();
      },
      openPanel: true,
      badge: (cognitiveStats.entitiesCount + cognitiveStats.factsCount) > 0
        ? String(cognitiveStats.entitiesCount + cognitiveStats.factsCount)
        : undefined,
    },
    {
      key: 'dashboard',
      label: 'MCP Servers',
      icon: Cpu,
      action: () => {
        setIsMcpsOpen(true);
        wsService.requestMcpServers();
      },
      openPanel: true,
      badge: mcpStats.total > 0 ? `${mcpStats.loaded}/${mcpStats.total}` : undefined,
    },
    {
      key: 'dashboard',
      label: 'Logs',
      icon: ScrollText,
      action: () => {
        setIsLogsOpen(true);
        wsService.requestLogs();
      },
      openPanel: true,
      badge: logs.length > 0 ? String(logs.length) : undefined,
    },
    {
      key: 'dashboard',
      label: 'Settings',
      icon: Settings,
      action: () => setIsSettingsOpen(true),
      openPanel: true,
    },
    {
      key: 'dashboard',
      label: resolvedTheme === 'dark' ? 'Light Theme' : 'Dark Theme',
      icon: resolvedTheme === 'dark' ? Sun : Moon,
      action: handleThemeClick,
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
            'group relative flex w-full items-center text-[13px] transition-all duration-300 ease-in-out',
            collapsed ? 'md:pl-[20px] md:pr-0 md:gap-0' : 'md:pl-2.5 md:pr-2.5 md:gap-2.5',
            isActive
              ? 'bg-amber-500/15 font-medium text-amber-400'
              : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
          )}
          style={{ minHeight: 34 }}
          title={collapsed ? item.label : undefined}
        >
          <Icon className="h-4 w-4 shrink-0" />
          <span className={cn(
            'transition-all duration-300 ease-in-out inline-block truncate origin-left',
            collapsed ? 'opacity-0 max-w-0 pointer-events-none' : 'opacity-100 max-w-[150px]'
          )}>
            {item.label}
          </span>
          {item.badge && (
            <span
              className={cn(
                'ml-auto rounded-full bg-muted/70 px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground transition-all duration-300 ease-in-out',
                collapsed ? 'opacity-0 scale-0 pointer-events-none w-0 h-0 p-0 overflow-hidden' : 'opacity-100 scale-100',
              )}
            >
              {item.badge}
            </span>
          )}
          {collapsed && (
            <span className="pointer-events-none absolute left-full top-1/2 ml-2 hidden -translate-y-1/2 whitespace-nowrap rounded-md border border-border bg-popover px-2 py-1 text-[11px] font-medium text-foreground shadow-lg md:group-hover:block">
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
          className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm md:hidden"
          onClick={() => setIsSidebarOpen(false)}
        />
      )}

      <aside
        className={cn(
          'fixed inset-y-0 left-0 z-50 flex flex-col border-r border-border/60 bg-card/95 backdrop-blur-md transition-all duration-300 ease-in-out',
          'w-[272px]',
          'md:static md:z-auto md:bg-card/50',
          collapsed ? 'md:w-[68px]' : 'md:w-[264px]',
          isSidebarOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0',
        )}
      >
        {/* Brand Header */}
        <div
          className={cn(
            'flex h-14 shrink-0 items-center border-b border-border/40 px-4 transition-all duration-300 ease-in-out',
            collapsed ? 'justify-center' : 'justify-between',
          )}
        >
          <div
            className={cn(
              'flex items-center gap-1.5 overflow-hidden transition-all duration-300 ease-in-out origin-left',
              collapsed ? 'opacity-0 max-w-0 pointer-events-none' : 'opacity-100 max-w-[180px]',
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
            className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:bg-muted hover:text-foreground transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
            title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
            aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            {collapsed ? <PanelLeftOpen className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
          </button>
        </div>

        {/* New Session */}
        <div className={cn('shrink-0 p-3 transition-all duration-300', collapsed && 'md:p-2 md:px-1.5')}>
          <button
            onClick={() => go(newSession)}
            className={cn(
              'flex w-full items-center rounded-xl border border-amber-500/30 bg-amber-500/10 text-xs font-semibold text-amber-400 transition-all duration-300 ease-in-out hover:bg-amber-500/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50',
              collapsed ? 'md:pl-[20px] md:pr-0 md:gap-0 py-2.5' : 'md:pl-3 md:pr-3 md:gap-2 py-2 justify-center',
            )}
            title={collapsed ? 'New Session' : undefined}
          >
            <Plus className="h-4 w-4 shrink-0" />
            <span className={cn(
              'transition-all duration-300 ease-in-out inline-block truncate origin-left',
              collapsed ? 'opacity-0 max-w-0 pointer-events-none' : 'opacity-100 max-w-[150px]'
            )}>
              New Session
            </span>
          </button>
        </div>

        {/* Navigation */}
        <nav className={cn('shrink-0 space-y-1 px-3 transition-all duration-300', collapsed && 'md:px-1.5')}>
          <div className={cn(
            'px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground transition-all duration-300 ease-in-out origin-left',
            collapsed ? 'opacity-0 max-w-0 overflow-hidden py-0' : 'opacity-100 max-w-[200px]'
          )}>
            Workspace
          </div>
          {renderNav(workspaceItems)}
          <div className={cn(
            'px-2 pb-1 pt-3 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground transition-all duration-300 ease-in-out origin-left',
            collapsed ? 'opacity-0 max-w-0 overflow-hidden py-0' : 'opacity-100 max-w-[200px]'
          )}>
            System
          </div>
          {renderNav(systemItems)}
        </nav>

        <div className={cn('mt-3 h-px shrink-0 bg-border/40 transition-all duration-300', collapsed && 'md:mx-2')} />

        {/* Sessions (desktop-expanded / always on mobile drawer) */}
        <div className={cn('flex-1 overflow-y-auto px-2 pb-2 transition-all duration-300 ease-in-out', collapsed && 'md:opacity-0 md:pointer-events-none md:max-h-0 md:overflow-hidden')}>
          {activeView === 'chats' && (
            <>
              <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                Sessions
              </div>
              {sessions.length === 0 ? (
                <div className="px-2 py-3 text-[11px] text-muted-foreground/70">
                  No sessions yet. Start a new one to begin.
                </div>
              ) : (
                sessions.map((session) => {
                  const isActive = session.id === activeChatId;
                  return (
                    <div
                      key={session.id}
                      onClick={() => go(() => selectSession(session.id))}
                      className={cn(
                        'group flex cursor-pointer select-none items-center justify-between rounded-lg px-2.5 py-2 text-xs transition',
                        isActive
                          ? 'border border-amber-500/30 bg-amber-500/15 font-medium text-amber-400'
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
                })
              )}
            </>
          )}
        </div>
        {/* Footer */}
        <div className={cn('shrink-0 space-y-2 border-t border-border/40 p-3 transition-all duration-300', collapsed && 'md:space-y-3 md:p-2')}>
          {/* Connection pill */}
          <div className="flex items-center justify-between px-1 transition-all duration-300">
            <div className={cn(
              'flex items-center gap-1.5 transition-all duration-300 ease-in-out origin-left',
              collapsed ? 'opacity-0 max-w-0 overflow-hidden' : 'opacity-100 max-w-[150px]'
            )}>
              <span
                className={cn(
                  'h-2 w-2 rounded-full',
                  connectionStatus === 'connected'
                    ? 'bg-emerald-500 shadow-sm shadow-emerald-500/50'
                    : connectionStatus === 'connecting'
                      ? 'animate-ping bg-amber-500'
                      : 'bg-red-500',
                )}
              />
              <span className="text-[10px] font-medium capitalize text-muted-foreground">
                {connectionStatus}
              </span>
            </div>
            
            {collapsed && (
              <div className="flex w-full items-center justify-center py-1 transition-all duration-300 animate-fade-in">
                <span
                  className={cn(
                    'h-2 w-2 rounded-full',
                    connectionStatus === 'connected'
                      ? 'bg-emerald-500 shadow-sm shadow-emerald-500/50'
                      : connectionStatus === 'connecting'
                        ? 'bg-amber-500 animate-ping'
                        : 'bg-red-500',
                  )}
                  title={`Connection: ${connectionStatus}`}
                />
              </div>
            )}
            
            <span className={cn(
              'text-[10px] text-muted-foreground transition-all duration-300 ease-in-out origin-left',
              collapsed ? 'opacity-0 max-w-0 overflow-hidden' : 'opacity-100 max-w-[100px]'
            )}>
              {mcpStats.loaded}/{mcpStats.total} MCP
            </span>
          </div>

          {/* Clear session */}
          <button
            onClick={clearActiveSession}
            disabled={!activeChatId}
            className={cn(
              'flex w-full items-center rounded-lg bg-muted/40 text-[11px] text-muted-foreground transition-all duration-300 ease-in-out hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40',
              collapsed ? 'md:pl-[20px] md:pr-0 md:gap-0 py-2.5' : 'md:pl-2.5 md:pr-2.5 md:gap-1.5 p-2',
            )}
            title={collapsed ? 'Clear Active Session' : undefined}
          >
            <Trash2 className="h-3 w-3 shrink-0 text-red-400" />
            <span className={cn(
              'transition-all duration-300 ease-in-out inline-block truncate origin-left',
              collapsed ? 'opacity-0 max-w-0 pointer-events-none' : 'opacity-100 max-w-[150px]'
            )}>
              Clear Active Session
            </span>
          </button>
        </div>
      </aside>
    </>
  );
};