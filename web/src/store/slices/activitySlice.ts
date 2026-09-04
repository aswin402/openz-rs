import type { StateCreator } from 'zustand';
import type { WorkspaceNotice } from '../../types';
import { UI_STORAGE_KEYS } from '../../config/runtime';
import type { OpenZState, WorkspaceView } from '../useOpenZStore';

export interface ActivitySlice {
  isSidebarOpen: boolean;
  isSidebarCollapsed: boolean;
  isActivityPanelOpen: boolean;
  activeView: WorkspaceView;
  isMemoryOpen: boolean;
  isLogsOpen: boolean;
  isMcpsOpen: boolean;
  isSettingsOpen: boolean;
  isServersOpen: boolean;
  workspaceNotice: WorkspaceNotice | null;
  setIsSidebarOpen: (open: boolean) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setIsActivityPanelOpen: (open: boolean) => void;
  toggleActivityPanel: () => void;
  setActiveView: (view: WorkspaceView) => void;
  setIsMemoryOpen: (open: boolean) => void;
  setIsLogsOpen: (open: boolean) => void;
  setIsMcpsOpen: (open: boolean) => void;
  setIsSettingsOpen: (open: boolean) => void;
  setIsServersOpen: (open: boolean) => void;
  setWorkspaceNotice: (notice: Omit<WorkspaceNotice, 'timestamp'>) => void;
  clearWorkspaceNotice: (scope?: WorkspaceNotice['scope']) => void;
}

export const createActivitySlice: StateCreator<
  OpenZState,
  [],
  [],
  ActivitySlice
> = (set, get) => ({
  isSidebarOpen: false,
  isSidebarCollapsed: localStorage.getItem(UI_STORAGE_KEYS.sidebarCollapsed) === '1',
  isActivityPanelOpen: localStorage.getItem(UI_STORAGE_KEYS.activityPanelOpen) !== '0',
  activeView: 'chats',
  isMemoryOpen: false,
  isLogsOpen: false,
  isMcpsOpen: false,
  isSettingsOpen: false,
  isServersOpen: false,
  workspaceNotice: null,

  setIsSidebarOpen: (open) => set({ isSidebarOpen: open }),
  setSidebarCollapsed: (collapsed) => {
    localStorage.setItem(UI_STORAGE_KEYS.sidebarCollapsed, collapsed ? '1' : '0');
    set({ isSidebarCollapsed: collapsed });
  },
  setIsActivityPanelOpen: (open) => {
    localStorage.setItem(UI_STORAGE_KEYS.activityPanelOpen, open ? '1' : '0');
    set({ isActivityPanelOpen: open });
  },
  toggleActivityPanel: () => {
    const open = !get().isActivityPanelOpen;
    localStorage.setItem(UI_STORAGE_KEYS.activityPanelOpen, open ? '1' : '0');
    set({ isActivityPanelOpen: open });
  },
  setActiveView: (view) => set({ activeView: view, workspaceNotice: null }),
  setIsMemoryOpen: (open) => set({ isMemoryOpen: open }),
  setIsLogsOpen: (open) => set({ isLogsOpen: open }),
  setIsMcpsOpen: (open) => set({ isMcpsOpen: open }),
  setIsSettingsOpen: (open) => set({ isSettingsOpen: open, workspaceNotice: open ? null : get().workspaceNotice }),
  setIsServersOpen: (open) => set({ isServersOpen: open }),
  setWorkspaceNotice: (notice) => set({ workspaceNotice: { ...notice, timestamp: Date.now() } }),
  clearWorkspaceNotice: (scope) => {
    const current = get().workspaceNotice;
    if (!scope || current?.scope === scope) set({ workspaceNotice: null });
  },
});
