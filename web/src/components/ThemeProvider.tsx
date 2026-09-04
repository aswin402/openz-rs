import React, { useLayoutEffect, useRef } from 'react';
import { useThemeStore } from '../store/useThemeStore';
import { useResolvedTheme } from '../shared/lib/theme';

interface ThemeProviderProps {
  children: React.ReactNode;
  inlineTheme?: Record<string, string>;
}

export function ThemeProvider({
  children,
  inlineTheme,
}: ThemeProviderProps) {
  const theme = useThemeStore((state) => state.theme);
  const resolvedTheme = useResolvedTheme(theme);
  const isFirstMount = useRef(true);
  const transitionTimerRef = useRef<number | null>(null);

  useLayoutEffect(() => {
    const root = window.document.documentElement;
    const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');

    const updateTheme = () => {
      root.classList.remove('light', 'dark');
      root.classList.add(resolvedTheme);
      root.style.colorScheme = resolvedTheme;
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

    // Use native View Transitions API if supported for ultra-smooth GPU crossfade
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const doc = document as any;
    if (typeof doc.startViewTransition === 'function') {
      doc.startViewTransition(() => {
        updateTheme();
      });
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
    }, 260);

    return () => {
      if (transitionTimerRef.current !== null) {
        window.clearTimeout(transitionTimerRef.current);
        transitionTimerRef.current = null;
      }
      root.classList.remove('theme-transitioning');
    };
  }, [resolvedTheme]);

  return (
    <div style={inlineTheme as React.CSSProperties}>
      {children}
    </div>
  );
}
