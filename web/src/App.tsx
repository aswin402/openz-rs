import React, { useEffect, useMemo } from 'react';
import { useOpenZStore } from './store/useOpenZStore';
import type { OpenZMessage, OrchestrationRunState } from './types';
import { useThemeStore } from './store/useThemeStore';
import { useResolvedTheme } from './shared/lib/theme';
import { runThemeViewTransition } from './shared/lib/themeTransition';
import {
  resolveActivePalette,
  useUiCustomizationStore,
} from './store/useUiCustomizationStore';
import { useToastStore } from './store/useToastStore';
import { Sidebar } from './components/Sidebar';
import { FloatingComposerDock } from './components/FloatingComposerDock';
import { WorkspaceRightDock } from './components/WorkspaceRightDock';
import { ChatMessage } from './features/chat/ChatMessage';
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
import { ThemedHintLayer } from './components/ThemedHintLayer';
import { AgentActivityPanel } from './features/chat/AgentActivityPanel';
import { UiStudioModal } from './features/customize/UiStudioModal';
import {
  AnimatedToastStack,
  BeuiCommandPalette,
  BeuiNotificationCenterPopover,
  MessageScroller,
  type BeuiCommandItem,
} from './components/beui';
import { Button } from './components/ui/button';
import { cn } from './lib/utils';
import {
  Activity,
  Archive,
  Bell,
  BookOpen,
  Bot,
  BrainCircuit,
  Code2,
  LayoutDashboard,
  Menu,
  MessageSquare,
  Palette,
  PanelRightClose,
  PanelRightOpen,
  Plus,
  ScrollText,
  Settings,
  Sparkles,
  SunMoon,
  User,
  Zap,
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
  const isLogsOpen = useOpenZStore((s) => s.isLogsOpen);
  const setIsLogsOpen = useOpenZStore((s) => s.setIsLogsOpen);
  const isActivityPanelOpen = useOpenZStore((s) => s.isActivityPanelOpen);
  const setIsActivityPanelOpen = useOpenZStore((s) => s.setIsActivityPanelOpen);
  const toggleActivityPanel = useOpenZStore((s) => s.toggleActivityPanel);
  const activeMessages = useOpenZStore((s) => s.messages[s.activeChatId] ?? EMPTY_MESSAGES);
  const activeTurnIds = useOpenZStore((s) => s.activeTurnIds);
  const activeOrchestrationRuns = useOpenZStore(
    (s) => s.orchestrationRuns[s.activeChatId] ?? EMPTY_ORCHESTRATION_RUNS,
  );
  const themePreference = useThemeStore((s) => s.theme);
  const resolvedMode = useResolvedTheme(themePreference);
  const setTheme = useThemeStore((s) => s.setTheme);

  const activeThemeId = useUiCustomizationStore((s) => s.activeThemeId);
  const themeTransitionStyle = useUiCustomizationStore((s) => s.themeTransitionStyle);
  const scrollerBehavior = useUiCustomizationStore((s) => s.scrollerBehavior);
  const toastEnabled = useUiCustomizationStore((s) => s.toastEnabled);
  const toastVariant = useUiCustomizationStore((s) => s.toastVariant);
  const toastPosition = useUiCustomizationStore((s) => s.toastPosition);
  const toastDuration = useUiCustomizationStore((s) => s.toastDuration);
  const toastMaxVisible = useUiCustomizationStore((s) => s.toastMaxVisible);
  const backgroundInboxStyle = useUiCustomizationStore((s) => s.backgroundInboxStyle);
  const deleteButtonStyle = useUiCustomizationStore((s) => s.deleteButtonStyle);
  const notificationCenterStyle = useUiCustomizationStore((s) => s.notificationCenterStyle);
  const drawerMotionStyle = useUiCustomizationStore((s) => s.drawerMotionStyle);
  const applyCustomization = useUiCustomizationStore((s) => s.applyCustomization);
  const setIsCustomizeOpen = useUiCustomizationStore((s) => s.setIsCustomizeOpen);
  const setActiveCustomizeSection = useUiCustomizationStore((s) => s.setActiveCustomizeSection);
  const setActiveGatewaySection = useUiCustomizationStore((s) => s.setActiveGatewaySection);
  const isCommandPaletteOpen = useUiCustomizationStore((s) => s.isCommandPaletteOpen);
  const setIsCommandPaletteOpen = useUiCustomizationStore((s) => s.setIsCommandPaletteOpen);

  const activeToasts = useToastStore((s) => s.toasts);
  const toastHistory = useToastStore((s) => s.history);
  const toastUnreadCount = useToastStore((s) => s.unreadCount);
  const isToastCenterOpen = useToastStore((s) => s.isCenterOpen);
  const dismissToast = useToastStore((s) => s.dismissToast);
  const clearActiveToasts = useToastStore((s) => s.clearActiveToasts);
  const clearToastHistory = useToastStore((s) => s.clearHistory);
  const setIsToastCenterOpen = useToastStore((s) => s.setIsCenterOpen);
  const toggleToastCenter = useToastStore((s) => s.toggleCenter);

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
    runThemeViewTransition(
      () => {
        setTheme(nextMode);
      },
      { style: themeTransitionStyle },
    );
  }, [palette.mode, setTheme, themeTransitionStyle]);

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
        subtitle: 'Choose from 4 live stream styles, row density, and viewport height',
        icon: Activity,
        onSelect: () => {
          setActiveCustomizeSection('agent-activity');
          setIsCustomizeOpen(true);
        },
      },
      {
        id: 'customize-toast-stack',
        group: 'Customize UI (@beui)',
        label: 'Customize Toast Stack (@beui/animated-toast-stack)',
        subtitle: 'Choose from 12 layered toast component styles with spring fan-out',
        icon: Bell,
        onSelect: () => {
          setActiveCustomizeSection('animated-toast-stack');
          setIsCustomizeOpen(true);
        },
      },
      {
        id: 'customize-telemetry-badges',
        group: 'Customize UI (@beui)',
        label: 'Customize Telemetry Badges (@beui/telemetry-strip)',
        subtitle: 'Configure inline chat telemetry pills and per-category toast triggers',
        icon: Sparkles,
        onSelect: () => {
          setActiveCustomizeSection('telemetry-badges');
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
        label: isActivityPanelOpen ? 'Hide Activity Drawer' : 'Open Activity Drawer',
        subtitle: 'Inspect background jobs (builds/servers), subagent runs, and live logs',
        icon: isActivityPanelOpen ? PanelRightClose : PanelRightOpen,
        onSelect: () => {
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
        onSelect: () => {
          setActiveGatewaySection('agent');
          setIsSettingsOpen(true);
        },
      },
      {
        id: 'open-caveman-settings',
        group: 'Actions',
        label: 'Caveman Mode Settings',
        subtitle: 'Enable or disable caveman mode and configure Lite, Full, Ultra, or Symbolic modes',
        icon: Zap,
        onSelect: () => {
          setActiveGatewaySection('caveman');
          setIsSettingsOpen(true);
        },
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
    setActiveGatewaySection,
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

        {/* Right-Side Minimal Icons Dock — automatically hidden while Activity or Logs drawer is open */}
        {!isActivityPanelOpen && !isLogsOpen && (
          <WorkspaceRightDock
            activeView={activeView}
            activityOpen={isActivityPanelOpen}
            notificationsOpen={isToastCenterOpen}
            toastEnabled={toastEnabled}
            unreadCount={toastUnreadCount}
            isDarkTheme={palette.mode === 'dark'}
            themeLabel={palette.mode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
            onViewChange={setActiveView}
            onCustomizeOpen={() => setIsCustomizeOpen(true)}
            onNotificationsToggle={toggleToastCenter}
            onThemeToggle={handleQuickThemeToggle}
            onActivityToggle={() => toggleActivityPanel()}
          />
        )}

        <div className="relative flex flex-1 overflow-hidden">
          {activeView === 'chats' ? (
            <div className="relative flex h-full min-w-0 flex-1 flex-col overflow-hidden">
              <MessageScroller
                sessionKey={activeChatId}
                itemCount={activeMessages.length}
                isStreaming={isAnyStreaming}
                behaviorMode={scrollerBehavior}
                contentClassName="space-y-3 pb-36 md:pb-40"
                jumpPillClassName="bottom-32 md:bottom-36"
              >
                {activeMessages.length === 0 ? (
                  <HeroWelcome />
                ) : (
                  activeMessages.map((msg) => <ChatMessage key={msg.id} message={msg} />)
                )}
              </MessageScroller>

              <FloatingComposerDock />
            </div>
          ) : (
            <div
              className={cn(
                'min-w-0 flex-1',
                activeView === 'knowledge' ? 'h-full overflow-hidden' : 'overflow-y-auto',
              )}
            >
              {renderWorkspace()}
            </div>
          )}

          <AgentActivityPanel
            open={isActivityPanelOpen}
            messages={activeMessages}
            isStreaming={isAnyStreaming}
            orchestrationRuns={activeOrchestrationRuns}
            onClose={() => setIsActivityPanelOpen(false)}
          />
        </div>
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
      <ThemedHintLayer />

      <BeuiNotificationCenterPopover
        open={isToastCenterOpen}
        onClose={() => setIsToastCenterOpen(false)}
        history={toastHistory}
        backgroundTasks={Object.keys(activeTurnIds)
          .filter((sid) => Boolean(activeTurnIds[sid]) && sid !== activeChatId)
          .map((sid) => {
            const match = sessions.find((s) => s.id === sid);
            return {
              id: `bg-${sid}`,
              title: match?.title || sid,
              detail: 'Running asynchronously in background',
              status: 'running' as const,
              onInspect: () => {
                selectSession(sid);
                setActiveView('chats');
                setIsToastCenterOpen(false);
              },
            };
          })}
        backgroundInboxStyle={backgroundInboxStyle}
        deleteButtonStyle={deleteButtonStyle}
        notificationCenterStyle={notificationCenterStyle}
        drawerMotionStyle={drawerMotionStyle}
        toastEnabled={toastEnabled}
        onToggleEnabled={(enabled) => applyCustomization({ toastEnabled: enabled })}
        onClearHistory={clearToastHistory}
      />

      {toastEnabled && (
        <AnimatedToastStack
          toasts={activeToasts}
          onDismiss={dismissToast}
          onClearAll={clearActiveToasts}
          onOpenOptions={() => {
            setActiveCustomizeSection('animated-toast-stack');
            setIsCustomizeOpen(true);
          }}
          variant={toastVariant}
          position={toastPosition}
          durationPreset={toastDuration}
          maxVisible={toastMaxVisible}
        />
      )}
    </div>
  );
};

export default App;
