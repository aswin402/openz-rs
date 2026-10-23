import type { StateCreator } from 'zustand';
import type {
  BackgroundServerInfo,
  ChannelConfigInfo,
  CronRunRecord,
  RuntimeInventory,
} from '../../types';
import { wsService } from '../../services/websocket';
import { createWorkspaceNotice } from '../chatUtils';
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
      workspaceNotice: createWorkspaceNotice('inventory', 'info', `Pausing cron job ${id}.`),
    });
    wsService.pauseCronJob(id);
  },

  resumeCronJob: (id) => {
    set({
      workspaceNotice: createWorkspaceNotice('inventory', 'info', `Resuming cron job ${id}.`),
    });
    wsService.resumeCronJob(id);
  },

  deleteCronJob: (id) => {
    set({
      workspaceNotice: createWorkspaceNotice('inventory', 'info', `Deleting cron job ${id}.`),
    });
    wsService.deleteCronJob(id);
  },

  requestCronLogs: (id, limit = 20) => {
    wsService.requestCronLogs(id, limit);
  },
});
