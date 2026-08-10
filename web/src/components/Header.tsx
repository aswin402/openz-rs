import React, { useMemo } from 'react';
import { useOpenZStore } from '../store/useOpenZStore';
import { wsService } from '../services/websocket';
import { Brain, ScrollText, Cpu, Settings, Menu, Sun, Moon } from 'lucide-react';
import { useThemeStore } from '../store/useThemeStore';

export const Header: React.FC = () => {
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const resolvedTheme =
    theme === 'system'
      ? window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light'
      : theme;
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const sessions = useOpenZStore((s) => s.sessions);
  const activeModel = useOpenZStore((s) => s.activeModel);
  const activeProvider = useOpenZStore((s) => s.activeProvider);
  const providers = useOpenZStore((s) => s.providers);
  const setActiveModel = useOpenZStore((s) => s.setActiveModel);
  const connectionStatus = useOpenZStore((s) => s.connectionStatus);
  const isSidebarOpen = useOpenZStore((s) => s.isSidebarOpen);
  const setIsSidebarOpen = useOpenZStore((s) => s.setIsSidebarOpen);
  const isSidebarCollapsed = useOpenZStore((s) => s.isSidebarCollapsed);
  const setSidebarCollapsed = useOpenZStore((s) => s.setSidebarCollapsed);

  const setIsMemoryOpen = useOpenZStore((s) => s.setIsMemoryOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const setIsMcpsOpen = useOpenZStore((s) => s.setIsMcpsOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);

  const currentSession = sessions.find((s) => s.id === activeChatId);

  const groups = providers.filter((p) => p.models.length > 0);
  const activeModelValue = useMemo(() => {
    const activeGroup = groups.find((g) => g.name === activeProvider && g.models.includes(activeModel))
      || groups.find((g) => g.models.includes(activeModel));
    return activeGroup ? `${activeGroup.name}::${activeModel}` : activeModel || '';
  }, [activeModel, activeProvider, groups]);

  const handleModelChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const [provider, ...modelParts] = e.target.value.split('::');
    const model = modelParts.join('::');
    if (!provider || !model) return;
    setActiveModel(model, provider);
  };

  const handleMenuClick = () => {
    if (window.innerWidth >= 768) {
      setSidebarCollapsed(!isSidebarCollapsed);
    } else {
      setIsSidebarOpen(true);
    }
  };

  const handleThemeClick = () => {
    setTheme(resolvedTheme === 'dark' ? 'light' : 'dark');
  };

  return (
    <header className="flex h-14 w-full items-center justify-between border-b border-border/60 bg-card/70 px-4 backdrop-blur-md z-20">
      {/* Title & Connection Status */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleMenuClick}
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:text-foreground hover:bg-muted transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
          title={isSidebarOpen ? 'Close session list' : 'Toggle sidebar'}
          aria-label="Toggle sidebar"
        >
          <Menu className="h-4 w-4" />
        </button>
        <div className="font-semibold text-sm text-foreground truncate max-w-xs">
          {currentSession?.title || 'OpenZ Agent Session'}
        </div>
        <div className="flex items-center gap-1.5 text-[11px] font-medium">
          <span
            className={`h-2 w-2 rounded-full ${
              connectionStatus === 'connected'
                ? 'bg-emerald-500 shadow-sm shadow-emerald-500/50'
                : connectionStatus === 'connecting'
                ? 'bg-amber-500 animate-ping'
                : 'bg-red-500'
            }`}
          />
          <span className="text-muted-foreground capitalize text-[10px] hidden sm:inline">
            {connectionStatus}
          </span>
        </div>
      </div>

      {/* Quick Action Tools */}
      <div className="flex items-center gap-2">
        {/* Model Dropdown — populated from real models_list event */}
        <div className="relative hidden sm:block">
          <select
            value={activeModelValue}
            onChange={handleModelChange}
            className="h-8 max-w-[200px] rounded-lg border border-border/60 bg-muted/40 px-2.5 py-1 text-xs font-medium text-foreground focus:outline-none focus:ring-1 focus:ring-amber-500/40"
            aria-label="Active model"
          >
            {groups.length === 0 && <option value="">No models loaded</option>}
            {groups.map((group) => (
              <optgroup key={group.name} label={group.display || group.name}>
                {group.models.map((model) => (
                  <option key={`${group.name}::${model}`} value={`${group.name}::${model}`}>
                    {model}{group.available === false ? ' (not configured)' : ''}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
        </div>

        <button
          onClick={() => {
            setIsMemoryOpen(true);
            wsService.requestCognitiveMemory();
          }}
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:text-foreground hover:bg-muted transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
          title="Cognitive Memory & Working Memory"
        >
          <Brain className="h-4 w-4" />
        </button>

        <button
          onClick={() => {
            setIsMcpsOpen(true);
            wsService.requestMcpServers();
          }}
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:text-foreground hover:bg-muted transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
          title="Connected MCP Servers"
        >
          <Cpu className="h-4 w-4" />
        </button>

        <button
          onClick={() => {
            setIsLogsOpen(true);
            wsService.requestLogs();
          }}
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:text-foreground hover:bg-muted transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
          title="Live Runtime Logs"
        >
          <ScrollText className="h-4 w-4" />
        </button>

        <button
          onClick={handleThemeClick}
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:text-foreground hover:bg-muted transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
          title={resolvedTheme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
        >
          {resolvedTheme === 'dark' ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </button>

        <button
          onClick={() => setIsSettingsOpen(true)}
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-border/60 bg-muted/40 text-muted-foreground hover:text-foreground hover:bg-muted transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50"
          title="Gateway & Agent Settings"
        >
          <Settings className="h-4 w-4" />
        </button>
      </div>
    </header>
  );
};