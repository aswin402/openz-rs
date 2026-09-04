import type { StoreApi } from 'zustand';
import type { OpenZState } from './useOpenZStore';

export type StoreSet = StoreApi<OpenZState>['setState'];
export type StoreGet = StoreApi<OpenZState>['getState'];

export interface StoreEventContext {
  set: StoreSet;
  get: StoreGet;
}
