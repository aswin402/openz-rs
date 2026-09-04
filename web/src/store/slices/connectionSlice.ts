import type { StateCreator } from 'zustand';
import type { ConnectionStatus } from '../../types';
import { WS_STORAGE_KEYS } from '../../config/runtime';
import { defaultWebSocketUrl, wsService } from '../../services/websocket';
import type { OpenZState } from '../useOpenZStore';

export interface ConnectionSlice {
  connectionStatus: ConnectionStatus;
  wsUrl: string;
  wsToken: string;
  setWsConfig: (url: string, token: string) => void;
}

export const createConnectionSlice: StateCreator<
  OpenZState,
  [],
  [],
  ConnectionSlice
> = (set) => ({
  connectionStatus: 'disconnected',
  wsUrl: defaultWebSocketUrl(),
  wsToken: localStorage.getItem(WS_STORAGE_KEYS.token) || '',

  setWsConfig: (url, token) => {
    set({ wsUrl: url, wsToken: token });
    wsService.setConfig(url, token);
  },
});
