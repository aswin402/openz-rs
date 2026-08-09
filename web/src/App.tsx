import React, { useEffect, useMemo, useRef } from 'react';
import { useOpenZStore } from './store/useOpenZStore';
import type { WorkspaceView } from './store/useOpenZStore';
import { useThemeStore } from './store/useThemeStore';
import { Sidebar } from './components/Sidebar';
import { ChatMessage } from './components/ChatMessage';
import { ChatInput } from './components/ChatInput';
import { HeroWelcome } from './components/HeroWelcome';
import { DashboardView } from './components/DashboardView';
import { KnowledgeView } from './components/KnowledgeView';
import { SkillsView } from './components/SkillsView';
import { AgentsView } from './components/AgentsView';
import { CognitiveMemoryModal } from './components/CognitiveMemoryModal';
import { LogsDrawer } from './components/LogsDrawer';
import { McpServersModal } from './components/McpServersModal';
import { SettingsModal } from './components/SettingsModal';
import { ServersModal } from './components/ServersModal';
import { AgentActivityPanel } from './components/AgentActivityPanel';
import {
  Bot,
  BookOpen,
  BrainCircuit,
  LayoutDashboard,
  Menu,
  MessageSquare,
  PanelRightClose,
  PanelRightOpen,
  Sun,
  Moon,
} from 'lucide-react';
import { cn } from './lib/utils';

export const App: React.FC = () => {
  const init = useOpenZStore((s) => s.init);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const activeView = useOpenZStore((s) => s.activeView);
  const setActiveView = useOpenZStore((s) => s.setActiveView);
  const setIsSidebarOpen = useOpenZStore((s) => s.setIsSidebarOpen);
  const isActivityPanelOpen = useOpenZStore((s) => s.isActivityPanelOpen);
  const setIsActivityPanelOpen = useOpenZStore((s) => s.setIsActivityPanelOpen);
  const toggleActivityPanel = useOpenZStore((s) => s.toggleActivityPanel);
  const messages = useOpenZStore((s) => s.messages);
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const activeMessages = useMemo(() => messages[activeChatId] || [], [messages, activeChatId]);
  const resolvedTheme =
    theme === 'system'
      ? window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light'
      : theme;

  useEffect(() => {
    init();
  }, [init]);

  // Keep track of the message count to detect additions
  const prevMsgCountRef = useRef(0);

  // 1. Scroll to bottom on initial load / chat switch
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (container && activeView === 'chats') {
      // Small timeout to allow content layout to finish
      setTimeout(() => {
        container.scrollTop = container.scrollHeight;
      }, 50);
      prevMsgCountRef.current = activeMessages.length;
    }
  }, [activeChatId, activeMessages.length, activeView]);

  // 2. Smart auto-scroll during streams or new message additions
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || activeMessages.length === 0 || activeView !== 'chats') {
      prevMsgCountRef.current = activeMessages.length;
      return;
    }

    const currentCount = activeMessages.length;
    const prevCount = prevMsgCountRef.current;
    prevMsgCountRef.current = currentCount;

    const lastMsg = activeMessages[currentCount - 1];
    let shouldScroll = false;

    if (currentCount > prevCount) {
      // Since user message and assistant placeholder are added together (incrementing length by 2),
      // we check if any of the newly added messages is a user message.
      const hasNewUserMsg = activeMessages
        .slice(prevCount, currentCount)
        .some((m) => m.role === 'user');

      if (hasNewUserMsg) {
        shouldScroll = true;
      } else {
        // If it's just a new assistant message (without a user message), scroll if near bottom
        const scrollOffset = container.scrollHeight - container.scrollTop - container.clientHeight;
        if (scrollOffset <= 150) {
          shouldScroll = true;
        }
      }
    } else if (lastMsg && lastMsg.role === 'assistant' && lastMsg.isStreaming) {
      // During stream content updates, scroll if near bottom
      const scrollOffset = container.scrollHeight - container.scrollTop - container.clientHeight;
      if (scrollOffset <= 150) {
        shouldScroll = true;
      }
    }

    if (shouldScroll) {
      container.scrollTo({
        top: container.scrollHeight,
        behavior: 'smooth',
      });
    }
  }, [activeMessages, activeView]);

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
      default:
        return null;
    }
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground font-sans antialiased selection:bg-amber-500/30 selection:text-amber-300">
      {/* Left Sidebar (collapsible rail) */}
      <Sidebar />

      {/* Main Container */}
      <div className="flex flex-1 flex-col h-full overflow-hidden relative">
        {!(activeView === 'chats' && isActivityPanelOpen) && (
          <WorkspaceQuickActions
            activeView={activeView}
            activityOpen={false}
            onViewChange={setActiveView}
            onThemeToggle={() => setTheme(resolvedTheme === 'dark' ? 'light' : 'dark')}
            themeLabel={resolvedTheme === 'dark' ? 'Light theme' : 'Dark theme'}
            themeIcon={resolvedTheme === 'dark' ? Sun : Moon}
            onActivityToggle={() => {
              if (activeView !== 'chats') {
                setActiveView('chats');
                setIsActivityPanelOpen(true);
                return;
              }
              toggleActivityPanel();
            }}
          />
        )}

        {/* Mobile Sidebar Toggle */}
        <div className="md:hidden absolute top-4 left-4 z-10">
          <button
            onClick={() => setIsSidebarOpen(true)}
            className="flex h-10 w-10 items-center justify-center rounded-full bg-card/80 backdrop-blur-md shadow-lg border border-border/60 text-foreground focus:outline-none focus:ring-2 focus:ring-amber-500/50"
            aria-label="Open Sidebar"
          >
            <Menu className="h-5 w-5" />
          </button>
        </div>


        {activeView === 'chats' ? (
          <div className="relative flex flex-1 overflow-hidden">
            <div className="relative min-w-0 flex-1 overflow-hidden">
              {/* Message Stream Scroll Area */}
              <div ref={scrollContainerRef} className="h-full overflow-y-auto px-4 py-6 pb-36">
              <div className="mx-auto max-w-3xl space-y-4">
                {activeMessages.length === 0 ? (
                  <HeroWelcome />
                ) : (
                  activeMessages.map((msg) => <ChatMessage key={msg.id} message={msg} />)
                )}
                <div ref={messagesEndRef} />
              </div>
            </div>

            {/* Floating Bottom Input Bar — floats over messages, transparent sides */}
              <div className="absolute bottom-0 left-0 right-0">
                <ChatInput />
                {/* Shadow strip in the gap below the input card */}
                <div className="mx-auto max-w-3xl px-6">
                  <div className="h-5 rounded-b-2xl bg-black/60 blur-2xl -mt-2 pointer-events-none" />
                </div>
              </div>
            </div>
            {isActivityPanelOpen && (
              <AgentActivityPanel
                messages={activeMessages}
                isStreaming={activeMessages.some((msg) => !!msg.isStreaming)}
                onClose={() => setIsActivityPanelOpen(false)}
              />
            )}
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto">{renderWorkspace()}</div>
        )}
      </div>

      {/* Modals & Drawers */}
      <CognitiveMemoryModal />
      <LogsDrawer />
      <McpServersModal />
      <SettingsModal />
      <ServersModal />
    </div>
  );
};

type QuickAction = {
  id: string;
  view?: WorkspaceView;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  action: () => void;
  active: boolean;
};

const WorkspaceQuickActions: React.FC<{
  activeView: WorkspaceView;
  activityOpen: boolean;
  onViewChange: (view: WorkspaceView) => void;
  onThemeToggle: () => void;
  themeLabel: string;
  themeIcon: React.ComponentType<{ className?: string }>;
  onActivityToggle: () => void;
}> = ({ activeView, activityOpen, onViewChange, onThemeToggle, themeLabel, themeIcon, onActivityToggle }) => {
  const actions: QuickAction[] = [
    { id: 'dashboard', view: 'dashboard', label: 'Dashboard', icon: LayoutDashboard, action: () => onViewChange('dashboard'), active: activeView === 'dashboard' },
    { id: 'chats', view: 'chats', label: 'Chat', icon: MessageSquare, action: () => onViewChange('chats'), active: activeView === 'chats' },
    { id: 'agents', view: 'agents', label: 'Agents', icon: Bot, action: () => onViewChange('agents'), active: activeView === 'agents' },
    { id: 'skills', view: 'skills', label: 'Skills', icon: BookOpen, action: () => onViewChange('skills'), active: activeView === 'skills' },
    { id: 'knowledge', view: 'knowledge', label: 'Knowledge', icon: BrainCircuit, action: () => onViewChange('knowledge'), active: activeView === 'knowledge' },
    { id: 'theme', label: themeLabel, icon: themeIcon, action: onThemeToggle, active: false },
    { id: 'activity', label: activityOpen ? 'Hide activity' : 'Show activity', icon: activityOpen ? PanelRightClose : PanelRightOpen, action: onActivityToggle, active: activeView === 'chats' && activityOpen },
  ];

  return (
    <div className="absolute right-3 top-3 z-20 flex flex-col items-center gap-1 rounded-xl border border-border/60 bg-background/70 p-1 shadow-lg shadow-black/10 backdrop-blur-md md:right-4 md:top-4">
      {actions.map((item) => {
        const Icon = item.icon;
        return (
          <button
            key={item.id}
            type="button"
            onClick={item.action}
            className={cn(
              'group relative flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground transition hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/50',
              item.active && 'bg-amber-500/15 text-amber-400',
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
