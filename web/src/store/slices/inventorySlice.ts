import type { StateCreator } from 'zustand';
import type {
  BackgroundServerInfo,
  ChannelConfigInfo,
  CronRunRecord,
  RuntimeInventory,
} from '../../types';
import { wsService } from '../../services/websocket';
import type { OpenZState } from '../useOpenZStore';

export interface InventorySlice {
  runtimeInventory: RuntimeInventory | null;
  cronLogs: CronRunRecord[];
  servers: BackgroundServerInfo[];
  channels: ChannelConfigInfo[];
  requestServers: () => void;
  stopServer: (id: string) => void;
  pauseCronJob: (id: string) => void;
  resumeCronJob: (id: string) => void;
  deleteCronJob: (id: string) => void;
  requestCronLogs: (id?: string, limit?: number) => void;
}

export const createInventorySlice: StateCreator<
  OpenZState,
  [],
  [],
  InventorySlice
> = (set) => ({
  runtimeInventory: null,
  cronLogs: [],
  servers: [],
  channels: [],

  requestServers: () => {
    wsService.requestServers();
  },

  stopServer: (id) => {
    wsService.stopServer(id);
  },

  pauseCronJob: (id) => {
    set({
      workspaceNotice: {
        scope: 'inventory',
        type: 'info',
        message: `Pausing cron job ${id}.`,
        timestamp: Date.now(),
      },
    });
    wsService.pauseCronJob(id);
  },

  resumeCronJob: (id) => {
    set({
      workspaceNotice: {
        scope: 'inventory',
        type: 'info',
        message: `Resuming cron job ${id}.`,
        timestamp: Date.now(),
      },
    });
    wsService.resumeCronJob(id);
  },

  deleteCronJob: (id) => {
    set({
      workspaceNotice: {
        scope: 'inventory',
        type: 'info',
        message: `Deleting cron job ${id}.`,
        timestamp: Date.now(),
      },
    });
    wsService.deleteCronJob(id);
  },

  requestCronLogs: (id, limit = 20) => {
    wsService.requestCronLogs(id, limit);
  },
});
