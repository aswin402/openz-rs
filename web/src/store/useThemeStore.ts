import { create } from 'zustand';
import { createJSONStorage, persist, type StateStorage } from 'zustand/middleware';

export type Theme = 'light' | 'dark' | 'system';

export const THEME_STORAGE_KEY = 'theme-storage';
export const THEME_BACKUP_STORAGE_KEY = 'theme-storage-backup';

const memoryThemeMap = new Map<string, string>();

const safeThemeStorage: StateStorage = {
  getItem: (name) => {
    if (typeof globalThis.localStorage !== 'undefined') {
      const primary = globalThis.localStorage.getItem(name);
      if (primary !== null) return primary;
      const backup = globalThis.localStorage.getItem(THEME_BACKUP_STORAGE_KEY);
      if (backup !== null) {
        globalThis.localStorage.setItem(name, backup);
        return backup;
      }
      return null;
    }
    return memoryThemeMap.get(name) ?? memoryThemeMap.get(THEME_BACKUP_STORAGE_KEY) ?? null;
  },
  setItem: (name, value) => {
    if (typeof globalThis.localStorage !== 'undefined') {
      globalThis.localStorage.setItem(name, value);
      globalThis.localStorage.setItem(THEME_BACKUP_STORAGE_KEY, value);
      return;
    }
    memoryThemeMap.set(name, value);
    memoryThemeMap.set(THEME_BACKUP_STORAGE_KEY, value);
  },
  removeItem: (name) => {
    if (typeof globalThis.localStorage !== 'undefined') {
      globalThis.localStorage.removeItem(name);
      return;
    }
    memoryThemeMap.delete(name);
  },
};

type ThemeSyncListener = (theme: Theme) => void;
let themeSyncListener: ThemeSyncListener | null = null;

export function registerThemeSyncListener(
  listener: ThemeSyncListener | null,
): () => void {
  themeSyncListener = listener;
  return () => {
    if (themeSyncListener === listener) {
      themeSyncListener = null;
    }
  };
}

interface ThemeState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  hydrateTheme: (theme: Theme) => void;
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: 'system',
      setTheme: (theme) => {
        const changed = get().theme !== theme;
        set({ theme });
        if (changed && themeSyncListener) {
          themeSyncListener(theme);
        }
      },
      hydrateTheme: (theme) => {
        if (get().theme !== theme) {
          set({ theme });
        }
      },
    }),
    {
      name: THEME_STORAGE_KEY,
      storage: createJSONStorage(() => safeThemeStorage),
      partialize: (state) => ({ theme: state.theme }),
    },
  ),
);
