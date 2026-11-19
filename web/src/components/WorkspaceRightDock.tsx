import React, { useMemo } from 'react';
import {
  Archive,
  BookOpen,
  Bot,
  BrainCircuit,
  LayoutDashboard,
  MessageSquare,
  Palette,
  PanelRightClose,
  PanelRightOpen,
} from 'lucide-react';
import type { WorkspaceView } from '../store/useOpenZStore';
import { useUiCustomizationStore } from '../store/useUiCustomizationStore';
import {
  ClassicThemeToggleIcon,
  KokonutToolbarDock,
  RareNotificationBell,
  type KokonutToolbarItem,
} from './registries/modernRegistries';
import { cn } from '../lib/utils';

export interface WorkspaceRightDockProps {
  activeView: WorkspaceView;
  activityOpen: boolean;
  notificationsOpen: boolean;
  toastEnabled: boolean;
  unreadCount: number;
  isDarkTheme: boolean;
  themeLabel: string;
  onViewChange: (view: WorkspaceView) => void;
  onCustomizeOpen: () => void;
  onNotificationsToggle: () => void;
  onThemeToggle: () => void;
  onActivityToggle: () => void;
  className?: string;
}

/**
 * Floating Right-Side Workspace Dock powered by:
 * - `@kokonutui/toolbar` spring-gliding selection pill & micro-physics
 * - `swamimalode07/rare-ui/notification-bell` damped pendulum bell + live unread counter
 * - `@toggles/classic` morphing sun-to-moon SVG toggle
 */
export const WorkspaceRightDock: React.FC<WorkspaceRightDockProps> = ({
  activeView,
  activityOpen,
  notificationsOpen,
  toastEnabled,
  unreadCount,
  isDarkTheme,
  themeLabel,
  onViewChange,
  onCustomizeOpen,
  onNotificationsToggle,
  onThemeToggle,
  onActivityToggle,
  className,
}) => {
  const dockToolbarStyle = useUiCustomizationStore((s) => s.dockToolbarStyle);

  const items = useMemo<KokonutToolbarItem[]>(
    () => [
      {
        id: 'dashboard',
        label: 'Dashboard',
        active: activeView === 'dashboard',
        onClick: () => onViewChange('dashboard'),
        renderIcon: () => <LayoutDashboard className="size-4" />,
      },
      {
        id: 'chats',
        label: 'Chat',
        active: activeView === 'chats',
        onClick: () => onViewChange('chats'),
        renderIcon: () => <MessageSquare className="size-4" />,
      },
      {
        id: 'agents',
        label: 'Agents',
        active: activeView === 'agents',
        onClick: () => onViewChange('agents'),
        renderIcon: () => <Bot className="size-4" />,
      },
      {
        id: 'skills',
        label: 'Skills',
        active: activeView === 'skills',
        onClick: () => onViewChange('skills'),
        renderIcon: () => <BookOpen className="size-4" />,
      },
      {
        id: 'knowledge',
        label: 'Knowledge',
        active: activeView === 'knowledge',
        onClick: () => onViewChange('knowledge'),
        renderIcon: () => <BrainCircuit className="size-4" />,
      },
      {
        id: 'inventory',
        label: 'Inventory',
        active: activeView === 'inventory',
        onClick: () => onViewChange('inventory'),
        renderIcon: () => <Archive className="size-4" />,
      },
      {
        id: 'notifications',
        label: toastEnabled ? 'Notifications' : 'Notifications (muted)',
        active: notificationsOpen,
        dividerBefore: true,
        onClick: onNotificationsToggle,
        renderIcon: () => (
          <RareNotificationBell
            unreadCount={unreadCount}
            muted={!toastEnabled}
            active={notificationsOpen}
          />
        ),
      },
      {
        id: 'customize',
        label: 'Customize UI',
        active: false,
        onClick: onCustomizeOpen,
        renderIcon: () => <Palette className="size-4" />,
      },
      {
        id: 'theme',
        label: themeLabel,
        active: false,
        onClick: onThemeToggle,
        renderIcon: () => <ClassicThemeToggleIcon isDark={isDarkTheme} size={16} />,
      },
      {
        id: 'activity',
        label: activityOpen ? 'Hide activity' : 'Show activity',
        active: activityOpen,
        onClick: onActivityToggle,
        renderIcon: () =>
          activityOpen ? (
            <PanelRightClose className="size-4" />
          ) : (
            <PanelRightOpen className="size-4" />
          ),
      },
    ],
    [
      activeView,
      activityOpen,
      isDarkTheme,
      notificationsOpen,
      onActivityToggle,
      onCustomizeOpen,
      onNotificationsToggle,
      onThemeToggle,
      onViewChange,
      themeLabel,
      toastEnabled,
      unreadCount,
    ],
  );

  return (
    <div
      className={cn(
        'absolute top-3 right-3 z-30 md:top-4 md:right-4',
        className,
      )}
    >
      <KokonutToolbarDock
        items={items}
        variant={dockToolbarStyle}
        orientation="vertical"
        layoutGroupId="openz-workspace-right-dock"
      />
    </div>
  );
};
