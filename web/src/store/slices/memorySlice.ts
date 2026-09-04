import type { StateCreator } from 'zustand';
import type {
  CognitiveMemoryStats,
  LogEntry,
  McpServerInfo,
  McpStats,
} from '../../types';
import type { OpenZState } from '../useOpenZStore';

const EMPTY_MEMORY: CognitiveMemoryStats = {
  entitiesCount: 0,
  relationsCount: 0,
  factsCount: 0,
  workingMemoryKeys: [],
  nodes: [],
  edges: [],
  facts: [],
};

export const EMPTY_MCP_STATS: McpStats = { loaded: 0, failed: 0, total: 0 };

export interface MemorySlice {
  cognitiveStats: CognitiveMemoryStats;
  mcpServers: McpServerInfo[];
  mcpStats: McpStats;
  logs: LogEntry[];
}

export const createMemorySlice: StateCreator<
  OpenZState,
  [],
  [],
  MemorySlice
> = () => ({
  cognitiveStats: EMPTY_MEMORY,
  mcpServers: [],
  mcpStats: EMPTY_MCP_STATS,
  logs: [],
});
