import React, { useEffect, useMemo } from 'react';
import { useOpenZStore, type WorkspaceView } from './store/useOpenZStore';
import type { OpenZMessage, OrchestrationRunState } from './types';
import { useThemeStore } from './store/useThemeStore';
import { useResolvedTheme } from './shared/lib/theme';
import {
  resolveActivePalette,
  useUiCustomizationStore,
} from './store/useUiCustomizationStore';
import { Sidebar } from './components/Sidebar';
import { ChatMessage } from './features/chat/ChatMessage';
import { ChatInput } from './features/chat/ChatInput';
import { HeroWelcome } from './components/HeroWelcome';
import { DashboardView } from './components/DashboardView';
import { KnowledgeView } from './features/knowledge/KnowledgeView';
import { SkillsView } from './features/skills/SkillsView';
import { AgentsView } from './features/agents/AgentsView';
import { InventoryView } from './features/inventory/InventoryView';
import { CognitiveMemoryModal } from './components/CognitiveMemoryModal';
import { LogsDrawer } from './components/LogsDrawer';
import { McpServersModal } from './components/McpServersModal';
import { SettingsModal } from './features/settings/SettingsModal';
import { ServersModal } from './components/ServersModal';
import { AgentActivityPanel } from './features/chat/AgentActivityPanel';
import { UiStudioModal } from './features/customize/UiStudioModal';
import {
  BeuiCommandPalette,
  MessageScroller,
  type BeuiCommandItem,
} from './components/beui';
import { Button } from './components/ui/button';
import { cn } from './lib/utils';
import {
  Activity,
  Archive,
  BookOpen,
  Bot,
  BrainCircuit,
  Code2,
  LayoutDashboard,
  Menu,
  MessageSquare,
  Moon,
  Palette,
  PanelRightClose,
  PanelRightOpen,
  Plus,
  ScrollText,
  Settings,
  Sun,
  SunMoon,
  User,
} from 'lucide-react';

const EMPTY_MESSAGES: OpenZMessage[] = [];
const EMPTY_ORCHESTRATION_RUNS: OrchestrationRunState[] = [];

export const App: React.FC = () => {
  const init = useOpenZStore((s) => s.init);
  const sessions = useOpenZStore((s) => s.sessions);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const selectSession = useOpenZStore((s) => s.selectSession);
  const newSession = useOpenZStore((s) => s.newSession);
  const activeView = useOpenZStore((s) => s.activeView);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const setIsSidebarOpen = useOpenZStore((s) => s.setIsSidebarOpen);
  const setIsSettingsOpen = useOpenZStore((s) => s.setIsSettingsOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const isActivityPanelOpen = useOpenZStore((s) => s.isActivityPanelOpen);
  const setIsActivityPanelOpen = useOpenZStore((s) => s.setIsActivityPanelOpen);
  const toggleActivityPanel = useOpenZStore((s) => s.toggleActivityPanel);
  const activeMessages = useOpenZStore((s) => s.messages[s.activeChatId] ?? EMPTY_MESSAGES);
  const activeOrchestrationRuns = useOpenZStore(
    (s) => s.orchestrationRuns[s.activeChatId] ?? EMPTY_ORCHESTRATION_RUNS,
  );
  const themePreference = useThemeStore((s) => s.theme);
  const resolvedMode = useResolvedTheme(themePreference);
  const setTheme = useThemeStore((s) => s.setTheme);

  const activeThemeId = useUiCustomizationStore((s) => s.activeThemeId);
  const scrollerBehavior = useUiCustomizationStore((s) => s.scrollerBehavior);
  const setIsCustomizeOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);
  const setActiveCustomizeSection = useUiCustomizationStore((s) => s.setActiveCustomizeSection);
  const isCommandPaletteOpen = useUiCustomizationStore((s) => s.isCommandPaletteOpen);
  const setIsCommandPaletteOpen = useUiCustomizationStore((s) => s.setIsCommandPaletteOpen);

  const palette = useMemo(
    () => resolveActivePalette(activeThemeId, resolvedMode),
    [activeThemeId, resolvedMode],
  );

  useEffect(() => {
    init();
  }, [init]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setIsCommandPaletteOpen(!isCommandPaletteOpen);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [isCommandPaletteOpen, setIsCommandPaletteOpen]);

  const isAnyStreaming = useMemo(
    () => activeMessages.some((msg) => Boolean(msg.isStreaming)),
    [activeMessages],
  );

  const handleQuickThemeToggle = React.useCallback(() => {
    const nextMode = palette.mode === 'dark' ? 'light' : 'dark';
    setTheme(nextMode);
  }, [palette.mode, setTheme]);

  const commandItems = useMemo<BeuiCommandItem[]>(() => {
    const base: BeuiCommandItem[] = [
      {
        id: 'new-chat',
        group: 'Actions',
        label: 'New Chat Session',
        subtitle: 'Start a fresh OpenZ conversation',
        icon: Plus,
        onSelect: () => newSession(),
      },
      {
        id: 'customize-ui',
        group: 'Customize UI (@beui)',
        label: 'Open Customize UI Studio',
        subtitle: 'Preview and select @beui component variants & dual-mode global themes',
        icon: Palette,
        onSelect: () => setIsCustomizeOpen(true),
      },
      {
        id: 'customize-avatars',
        group: 'Customize UI (@beui)',
        label: 'Choose Agent & Fox Avatars',
        subtitle: 'Pick from 13 Agent & Fox avatars + User avatars',
        icon: User,
        onSelect: () => {
          setActiveCustomizeSection('avatars');
          setIsCustomizeOpen(true);
        },
      },
      {
        id: 'customize-activity',
        group: 'Customize UI (@beui)',
        label: 'Customize Agent Activity (@beui/agent-activity)',
        subtitle: 'Configure Thought Orb, 4 stream variants, density, maxHeight & multi-surface feeds',
        icon: Activity,
        onSelect: () => {
          setActiveCustomizeSection('agent-activity');
          setIsCustomizeOpen(true);
        },
      },
      {
        id: 'customize-code',
        group: 'Customize UI (@beui)',
        label: 'Customize Code Block & File Diff',
        subtitle: '@beui/code-block & @beui/file-diff variants',
        icon: Code2,
        onSelect: () => {
          setActiveCustomizeSection('code-block');
          setIsCustomizeOpen(true);
        },
      },
      {
        id: 'toggle-activity-drawer',
        group: 'Actions',
        label: isActivityPanelOpen ? 'Hide Agent Activity Drawer' : 'Open Agent Activity Drawer',
        subtitle: 'Inspect live session tool calls, searches, approvals, and subagent runs',
        icon: isActivityPanelOpen ? PanelRightClose : PanelRightOpen,
        onSelect: () => {
          setActiveView('chats');
          toggleActivityPanel();
        },
      },
      {
        id: 'toggle-theme',
        group: 'Actions',
        label: `Switch ${palette.name} to ${palette.mode === 'dark' ? 'Light' : 'Dark'} Mode`,
        subtitle: 'Toggle Light / Dark mode while keeping your active theme family',
        icon: SunMoon,
        onSelect: handleQuickThemeToggle,
      },
      {
        id: 'open-settings',
        group: 'Actions',
        label: 'Gateway Settings',
        subtitle: 'Providers, API keys, channels & runtime limits',
        icon: Settings,
        onSelect: () => setIsSettingsOpen(true),
      },
      {
        id: 'open-logs',
        group: 'Actions',
        label: 'Structured Logs Drawer',
        subtitle: 'Inspect live OpenZ runtime logs',
        icon: ScrollText,
        onSelect: () => setIsLogsOpen(true),
      },
      {
        id: 'view-chats',
        group: 'Workspace Views',
        label: 'Chat',
        icon: MessageSquare,
        onSelect: () => setActiveView('chats'),
      },
      {
        id: 'view-agents',
        group: 'Workspace Views',
        label: 'Agents & Subagent Profiles',
        icon: Bot,
        onSelect: () => setActiveView('agents'),
      },
      {
        id: 'view-skills',
        group: 'Workspace Views',
        label: 'Procedural Skills',
        icon: BookOpen,
        onSelect: () => setActiveView('skills'),
      },
      {
        id: 'view-knowledge',
        group: 'Workspace Views',
        label: 'Knowledge Graph & Memory',
        icon: BrainCircuit,
        onSelect: () => setActiveView('knowledge'),
      },
      {
        id: 'view-inventory',
        group: 'Workspace Views',
        label: 'Tool & Runtime Inventory',
        icon: Archive,
        onSelect: () => setActiveView('inventory'),
      },
      {
        id: 'view-dashboard',
        group: 'Workspace Views',
        label: 'System Dashboard',
        icon: LayoutDashboard,
        onSelect: () => setActiveView('dashboard'),
      },
    ];

    const sessionCommands: BeuiCommandItem[] = sessions.slice(0, 12).map((s) => ({
      id: `session-${s.id}`,
      group: 'Recent Sessions',
      label: s.title,
      icon: MessageSquare,
      onSelect: () => selectSession(s.id),
    }));

    return [...base, ...sessionCommands];
  }, [
    palette.mode,
    palette.name,
    handleQuickThemeToggle,
    sessions,
    newSession,
    setIsCustomizeOpen,
    setActiveCustomizeSection,
    setIsSettingsOpen,
    setIsLogsOpen,
    setActiveView,
    selectSession,
    isActivityPanelOpen,
    toggleActivityPanel,
  ]);

  const renderWorkspace = () => {
    switch (activeView) {
      case 'dashboard':
        return <DashboardView />;
      case 'knowledge':
        return <KnowledgeView />;
      case 'agents':
        return <AgentsView />;
      case 'skills':
        return <SkillsView />;
      case 'inventory':
        return <InventoryView />;
      default:
        return null;
    }
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background font-sans text-foreground antialiased selection:bg-primary/20 selection:text-primary">
      <Sidebar />

      <div className="relative flex h-full flex-1 flex-col overflow-hidden bg-background">
        {/* Mobile Sidebar Button */}
        <Button
          type="button"
          variant="outline"
          size="icon-sm"
          onClick={() => setIsSidebarOpen(true)}
          className="absolute left-3 top-3 z-20 bg-card md:hidden"
          aria-label="Open Sidebar"
        >
          <Menu className="size-4" />
        </Button>

        {/* Right-Side Minimal Icons Menu (Workspace Quick Actions) */}
        <WorkspaceQuickActions
          activeView={activeView}
          activityOpen={isActivityPanelOpen}
          onViewChange={setActiveView}
          onCustomizeOpen={() => setIsCustomizeOpen(true)}
          onThemeToggle={handleQuickThemeToggle}
          themeLabel={palette.mode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
          themeIcon={palette.mode === 'dark' ? Sun : Moon}
          onActivityToggle={() => {
            if (activeView !== 'chats') {
              setActiveView('chats');
              setIsActivityPanelOpen(true);
            } else {
              toggleActivityPanel();
            }
          }}
        />

        {activeView === 'chats' ? (
          <div className="relative flex flex-1 overflow-hidden">
            <div className="flex h-full min-w-0 flex-1 flex-col overflow-hidden">
              <MessageScroller
                sessionKey={activeChatId}
                itemCount={activeMessages.length}
                isStreaming={isAnyStreaming}
                behaviorMode={scrollerBehavior}
                contentClassName="space-y-3"
              >
                {activeMessages.length === 0 ? (
                  <HeroWelcome />
                ) : (
                  activeMessages.map((msg) => <ChatMessage key={msg.id} message={msg} />)
                )}
              </MessageScroller>

              <div className="shrink-0 bg-background pr-[6px] pt-1">
                <ChatInput />
              </div>
            </div>

            {isActivityPanelOpen && (
              <AgentActivityPanel
                messages={activeMessages}
                isStreaming={isAnyStreaming}
                orchestrationRuns={activeOrchestrationRuns}
                onClose={() => setIsActivityPanelOpen(false)}
              />
            )}
          </div>
        ) : (
          <div
            className={cn(
              'flex-1',
              activeView === 'knowledge' ? 'h-full overflow-hidden' : 'overflow-y-auto',
            )}
          >
            {renderWorkspace()}
          </div>
        )}
      </div>

      {/* BEUI Command Palette & Full-Screen Studios */}
      <BeuiCommandPalette
        open={isCommandPaletteOpen}
        onOpenChange={setIsCommandPaletteOpen}
        items={commandItems}
      />
      <CognitiveMemoryModal />
      <LogsDrawer />
      <McpServersModal />
      <SettingsModal />
      <ServersModal />
      <UiStudioModal />
    </div>
  );
};

interface QuickActionItem {
  id: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  action: () => void;
  active: boolean;
}

const WorkspaceQuickActions: React.FC<{
  activeView: WorkspaceView;
  activityOpen: boolean;
  onViewChange: (view: WorkspaceView) => void;
  onCustomizeOpen: () => void;
  onThemeToggle: () => void;
  themeLabel: string;
  themeIcon: React.ComponentType<{ className?: string }>;
  onActivityToggle: () => void;
}> = ({
  activeView,
  activityOpen,
  onViewChange,
  onCustomizeOpen,
  onThemeToggle,
  themeLabel,
  themeIcon,
  onActivityToggle,
}) => {
  const actions: QuickActionItem[] = [
    {
      id: 'dashboard',
      label: 'Dashboard',
      icon: LayoutDashboard,
      action: () => onViewChange('dashboard'),
      active: activeView === 'dashboard',
    },
    {
      id: 'chats',
      label: 'Chat',
      icon: MessageSquare,
      action: () => onViewChange('chats'),
      active: activeView === 'chats',
    },
    {
      id: 'agents',
      label: 'Agents',
      icon: Bot,
      action: () => onViewChange('agents'),
      active: activeView === 'agents',
    },
    {
      id: 'skills',
      label: 'Skills',
      icon: BookOpen,
      action: () => onViewChange('skills'),
      active: activeView === 'skills',
    },
    {
      id: 'knowledge',
      label: 'Knowledge',
      icon: BrainCircuit,
      action: () => onViewChange('knowledge'),
      active: activeView === 'knowledge',
    },
    {
      id: 'inventory',
      label: 'Inventory',
      icon: Archive,
      action: () => onViewChange('inventory'),
      active: activeView === 'inventory',
    },
    {
      id: 'customize',
      label: 'Customize UI',
      icon: Palette,
      action: onCustomizeOpen,
      active: false,
    },
    {
      id: 'theme',
      label: themeLabel,
      icon: themeIcon,
      action: onThemeToggle,
      active: false,
    },
    {
      id: 'activity',
      label: activityOpen ? 'Hide activity' : 'Show activity',
      icon: activityOpen ? PanelRightClose : PanelRightOpen,
      action: onActivityToggle,
      active: activeView === 'chats' && activityOpen,
    },
  ];

  return (
    <div
      className={cn(
        'absolute top-3 z-30 flex flex-col items-center gap-1 rounded-xl border border-border/60 bg-background/75 p-1 shadow-lg shadow-foreground/5 backdrop-blur-md transition-[right] duration-200 md:top-4',
        activeView === 'chats' && activityOpen
          ? 'right-3 lg:right-[332px]'
          : 'right-3 md:right-4',
      )}
    >
      {actions.map((item) => {
        const Icon = item.icon;
        return (
          <button
            key={item.id}
            type="button"
            onClick={item.action}
            className={cn(
              'group relative flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground transition hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50',
              item.active && 'bg-primary/15 text-primary',
            )}
            title={item.label}
            aria-label={item.label}
          >
            <Icon className="h-4 w-4" />
            <span className="pointer-events-none absolute right-full top-1/2 mr-2 hidden -translate-y-1/2 whitespace-nowrap rounded-md border border-border bg-popover px-2 py-1 text-[11px] font-medium text-foreground shadow-lg group-hover:block">
              {item.label}
            </span>
          </button>
        );
      })}
    </div>
  );
};

export default App;
