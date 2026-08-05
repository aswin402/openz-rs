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

  useLayoutEffect(() => {
    const root = window.document.documentElement;

    const updateTheme = () => {
      root.classList.remove('light', 'dark');

      if (theme === 'system') {
        const systemTheme = window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light';

        root.classList.add(systemTheme);
        return;
      }

      root.classList.add(theme);
    };

    if (isFirstMount.current) {
      updateTheme();
      isFirstMount.current = false;
      return;
    }

    const supportsViewTransition = (document as any).startViewTransition !== undefined;
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    if (!supportsViewTransition || prefersReducedMotion) {
      updateTheme();
      return;
    }

    (document as any).startViewTransition(() => {
      updateTheme();
    });
  }, [theme]);

  return (
    <div style={inlineTheme as React.CSSProperties}>
      {children}
    </div>
  );
}
