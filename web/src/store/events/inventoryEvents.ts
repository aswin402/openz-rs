import { normalizeCronRuns, normalizeRuntimeInventory } from '../../types/protocol';
import { wsService } from '../../services/websocket';
import type { StoreEventContext } from '../eventContext';

export function registerInventoryEvents({ set }: StoreEventContext) {
  wsService.on('servers_list', (payload) => {
    if (Array.isArray(payload.servers)) {
      set({ servers: payload.servers });
    }
    if (Array.isArray(payload.channels)) {
      set({ channels: payload.channels });
    }
  });

  wsService.on('server_stopped', () => {
    wsService.requestServers();
  });

  wsService.on('runtime_inventory', (payload) => {
    const inventory = normalizeRuntimeInventory(payload.inventory);
    if (inventory) set({ runtimeInventory: inventory });
  });

  wsService.on('cron_jobs_updated', (payload) => {
    const id = typeof payload.id === 'string' ? payload.id : 'cron job';
    const status = typeof payload.status === 'string' ? payload.status : 'updated';
    const inventory = normalizeRuntimeInventory(payload.inventory);
    if (inventory) set({ runtimeInventory: inventory });
    set({
      workspaceNotice: {
        scope: 'inventory',
        type: 'success',
        message: `Cron job ${id} ${status}.`,
        timestamp: Date.now(),
      },
    });
  });

  wsService.on('cron_logs', (payload) => {
    if (Array.isArray(payload.runs)) set({ cronLogs: normalizeCronRuns(payload.runs) });
  });
}
