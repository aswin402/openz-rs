import React, { useLayoutEffect, useRef } from 'react';
import { useThemeStore } from '../store/useThemeStore';

interface ThemeProviderProps {
  children: React.ReactNode;
  inlineTheme?: Record<string, string>;
}

export function ThemeProvider({
  children,
  inlineTheme,
}: ThemeProviderProps) {
  const { theme } = useThemeStore();
  const isFirstMount = useRef(true);
  const transitionTimerRef = useRef<number | null>(null);

  useLayoutEffect(() => {
    const root = window.document.documentElement;
    const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');

    const updateTheme = () => {
      root.classList.remove('light', 'dark');

      if (theme === 'system') {
        const systemTheme = window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light';

        root.classList.add(systemTheme);
        root.style.colorScheme = systemTheme;
        return;
      }

      root.classList.add(theme);
      root.style.colorScheme = theme;
    };

    if (isFirstMount.current) {
      updateTheme();
      isFirstMount.current = false;
      return;
    }

    if (motionQuery.matches) {
      updateTheme();
      return;
    }

    if (transitionTimerRef.current !== null) {
      window.clearTimeout(transitionTimerRef.current);
    }

    root.classList.add('theme-transitioning');
    updateTheme();

    transitionTimerRef.current = window.setTimeout(() => {
      root.classList.remove('theme-transitioning');
      transitionTimerRef.current = null;
    }, 220);

    return () => {
      if (transitionTimerRef.current !== null) {
        window.clearTimeout(transitionTimerRef.current);
        transitionTimerRef.current = null;
      }
      root.classList.remove('theme-transitioning');
    };
  }, [theme]);

  return (
    <div style={inlineTheme as React.CSSProperties}>
      {children}
    </div>
  );
}
