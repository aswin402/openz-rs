import { create } from 'zustand';
import { wsService } from '../services/websocket';
import { createConnectionSlice, type ConnectionSlice } from './slices/connectionSlice';
import { createConfigSlice, type ConfigSlice } from './slices/configSlice';
import { createInventorySlice, type InventorySlice } from './slices/inventorySlice';
import { createMemorySlice, type MemorySlice } from './slices/memorySlice';
import { createActivitySlice, type ActivitySlice } from './slices/activitySlice';
import { createChatSlice, type ChatSlice } from './slices/chatSlice';
import { registerChatTurnEvents } from './events/chatTurnEvents';
import { registerConfigEvents } from './events/configEvents';
import { registerInventoryEvents } from './events/inventoryEvents';
import { registerMemoryEvents } from './events/memoryEvents';
import { registerSessionEvents } from './events/sessionEvents';

/** Workspace views available from the left navigation rail. */
export type WorkspaceView = 'dashboard' | 'chats' | 'agents' | 'skills' | 'knowledge' | 'inventory';

export interface OpenZState
  extends ConnectionSlice,
    ConfigSlice,
    InventorySlice,
    MemorySlice,
    ActivitySlice,
    ChatSlice {
  init: () => void;
}

// Guards against duplicate listener registration (React StrictMode double-invokes
// effects in dev, which would otherwise register every WS handler twice).
let hasInitialized = false;

export const useOpenZStore = create<OpenZState>((set, get, api) => ({
  ...createConnectionSlice(set, get, api),
  ...createConfigSlice(set, get, api),
  ...createInventorySlice(set, get, api),
  ...createMemorySlice(set, get, api),
  ...createActivitySlice(set, get, api),
  ...createChatSlice(set, get, api),

  init: () => {
    if (hasInitialized) return;
    hasInitialized = true;

    wsService.setStatusCallback((status) => {
      set({ connectionStatus: status });
    });

    // Register event groups once after the store slices are available.
    registerChatTurnEvents({ set, get });

    wsService.on('command_queued', (payload) => {
      if (payload.command !== 'set_config') return;
      set({
        workspaceNotice: {
          scope: 'settings',
          type: 'info',
          message: 'Settings queued until the gateway reconnects.',
          timestamp: Date.now(),
        },
      });
    });

    wsService.on('command_ack', (payload) => {
      if (payload.command === 'set_config' && payload.status === 'accepted') {
        set({
          workspaceNotice: {
            scope: 'settings',
            type: 'info',
            message: 'Settings update accepted; waiting for gateway confirmation.',
            timestamp: Date.now(),
          },
        });
        return;
      }
      if (payload.status !== 'rejected') return;
      set({
        workspaceNotice: {
          scope: payload.command === 'set_config' ? 'settings' : 'global',
          type: 'error',
          message: payload.detail || `Gateway rejected ${payload.command}.`,
          timestamp: Date.now(),
        },
      });
    });

    wsService.on('config_update_rejected', (payload) => {
      set({
        workspaceNotice: {
          scope: 'settings',
          type: 'error',
          message: typeof payload.reason === 'string' ? payload.reason : 'Gateway rejected the settings update.',
          timestamp: Date.now(),
        },
      });
    });

    registerSessionEvents({ set, get });
    registerMemoryEvents({ set, get });
    registerInventoryEvents({ set, get });
    registerConfigEvents({ set, get });

    wsService.connect();
  },
}));

// Type-only marker so TS keeps the store shape exportable without runtime effect.
export type OpenZStoreState = OpenZState;
