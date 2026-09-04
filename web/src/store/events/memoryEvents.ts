import type { StoreEventContext } from '../eventContext';
import { wsService } from '../../services/websocket';

export function registerMemoryEvents({ set }: StoreEventContext) {
  wsService.on('cognitive_memory', (payload) => {
    if (payload.stats) {
      set({
        cognitiveStats: {
          entitiesCount: payload.stats.entitiesCount ?? 0,
          relationsCount: payload.stats.relationsCount ?? 0,
          factsCount: payload.stats.factsCount ?? 0,
          workingMemoryKeys: Array.isArray(payload.stats.workingMemoryKeys)
            ? payload.stats.workingMemoryKeys
            : [],
          nodes: Array.isArray(payload.nodes) ? payload.nodes : [],
          edges: Array.isArray(payload.edges)
            ? payload.edges
                .filter((edge: Record<string, unknown>) => edge && typeof edge === 'object')
                .map((edge: Record<string, unknown>) => ({
                  from_name: typeof edge.from_name === 'string' ? edge.from_name : '',
                  to_name: typeof edge.to_name === 'string' ? edge.to_name : '',
                  relation_type: typeof edge.relation_type === 'string' ? edge.relation_type : '',
                  ...(typeof edge.confidence === 'number' ? { confidence: edge.confidence } : {}),
                  ...(typeof edge.valid_from === 'string' ? { valid_from: edge.valid_from } : {}),
                  ...(typeof edge.provenance === 'string' ? { provenance: edge.provenance } : {}),
                  ...(typeof edge.source === 'string' ? { source: edge.source } : {}),
                }))
                .filter((edge: { from_name: string; to_name: string }) => edge.from_name.length > 0 && edge.to_name.length > 0)
            : [],
          facts: Array.isArray(payload.facts) ? payload.facts : [],
          paths: payload.paths && typeof payload.paths === 'object'
            ? {
                memoryDb: typeof payload.paths.memoryDb === 'string' ? payload.paths.memoryDb : '',
                graphDb: typeof payload.paths.graphDb === 'string' ? payload.paths.graphDb : '',
              }
            : undefined,
        },
      });
    }
  });

  wsService.on('mcp_servers', (payload) => {
    if (Array.isArray(payload.servers)) {
      set({ mcpServers: payload.servers });
    }
    if (payload.stats) {
      set({
        mcpStats: {
          loaded: payload.stats.loaded ?? 0,
          failed: payload.stats.failed ?? 0,
          total: payload.stats.total ?? 0,
        },
      });
    }
  });

  wsService.on('logs_data', (payload) => {
    if (Array.isArray(payload.logs)) {
      set({ logs: payload.logs });
    }
  });
}
