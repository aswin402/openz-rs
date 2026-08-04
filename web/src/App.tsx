import React, { useEffect, useRef } from 'react';
import { useOpenZStore } from './store/useOpenZStore';
import { Sidebar } from './components/Sidebar';
import { ChatMessage } from './components/ChatMessage';
import { ChatInput } from './components/ChatInput';
import { HeroWelcome } from './components/HeroWelcome';
import { DashboardView } from './components/DashboardView';
import { KnowledgeView } from './components/KnowledgeView';
import { WorkspacePlaceholder } from './components/WorkspacePlaceholder';
import { CognitiveMemoryModal } from './components/CognitiveMemoryModal';
import { LogsDrawer } from './components/LogsDrawer';
import { McpServersModal } from './components/McpServersModal';
import { SettingsModal } from './components/SettingsModal';
import { Menu } from 'lucide-react';

export const App: React.FC = () => {
  const init = useOpenZStore((s) => s.init);
  const activeChatId = useOpenZStore((s) => s.activeChatId);
  const activeView = useOpenZStore((s) => s.activeView);
  const setIsSidebarOpen = useOpenZStore((s) => s.setIsSidebarOpen);
  const messages = useOpenZStore((s) => s.messages);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const activeMessages = messages[activeChatId] || [];

  useEffect(() => {
    init();
  }, [init]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [activeMessages]);

  const renderWorkspace = () => {
    switch (activeView) {
      case 'dashboard':
        return <DashboardView />;
      case 'knowledge':
        return <KnowledgeView />;
      case 'agents':
        return <WorkspacePlaceholder kind="agents" />;
      case 'skills':
        return <WorkspacePlaceholder kind="skills" />;
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
          <>
            {/* Message Stream Scroll Area */}
            <div className="flex-1 overflow-y-auto px-4 py-6">
              <div className="mx-auto max-w-3xl space-y-4">
                {activeMessages.length === 0 ? (
                  <HeroWelcome />
                ) : (
                  activeMessages.map((msg) => <ChatMessage key={msg.id} message={msg} />)
                )}
                <div ref={messagesEndRef} />
              </div>
            </div>

            {/* Floating Bottom Input Bar */}
            <ChatInput />
          </>
        ) : (
          <div className="flex-1 overflow-y-auto">{renderWorkspace()}</div>
        )}
      </div>

      {/* Modals & Drawers */}
      <CognitiveMemoryModal />
      <LogsDrawer />
      <McpServersModal />
      <SettingsModal />
    </div>
  );
};

export default App;
