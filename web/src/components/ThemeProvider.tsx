import React, { useEffect, useLayoutEffect, useRef } from 'react';
import { useThemeStore } from '../store/useThemeStore';
import { useResolvedTheme } from '../shared/lib/theme';
import {
  computeThemeClipKeyframes,
  installThemePointerTracker,
  isThemeViewTransitionActive,
} from '../shared/lib/themeTransition';
import {
  resolveActivePalette,
  useUiCustomizationStore,
} from '../store/useUiCustomizationStore';
import { syncDocumentFavicon } from './OpenZFoxIcon';

interface ThemeProviderProps {
  children: React.ReactNode;
  inlineTheme?: Record<string, string>;
}

export function ThemeProvider({
  children,
  inlineTheme,
}: ThemeProviderProps) {
  const theme = useThemeStore((state) => state.theme);
  const resolvedSystemOrMode = useResolvedTheme(theme);
  const activeThemeId = useUiCustomizationStore((s) => s.activeThemeId);
  const themeTransitionStyle = useUiCustomizationStore((s) => s.themeTransitionStyle);
  const badgeStyle = useUiCustomizationStore((s) => s.badgeStyle);

  const isFirstMount = useRef(true);
  const prevSignatureRef = useRef<string | null>(null);
  const transitionTimerRef = useRef<number | null>(null);

  useEffect(() => {
    installThemePointerTracker();
  }, []);

  useLayoutEffect(() => {
    const root = window.document.documentElement;
    const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');

    const palette = resolveActivePalette(activeThemeId, resolvedSystemOrMode);

    const applyTokens = () => {
      root.classList.remove('light', 'dark');
      root.classList.add(resolvedSystemOrMode);
      root.setAttribute('data-theme', resolvedSystemOrMode);
      root.setAttribute('data-theme-family', palette.id);
      root.setAttribute('data-badge-style', badgeStyle);
      root.style.colorScheme = resolvedSystemOrMode;

      const vars: Record<string, string> = {
        '--background': palette.backgroundHex,
        '--foreground': palette.foregroundHex,
        '--card': palette.cardHex,
        '--card-foreground': palette.foregroundHex,
        '--popover': palette.cardHex,
        '--popover-foreground': palette.foregroundHex,
        '--primary': palette.primaryHex,
        '--primary-foreground': palette.primaryForegroundHex,
        '--brand-accent': palette.accentHex,
        '--secondary': palette.mutedHex,
        '--secondary-foreground': palette.foregroundHex,
        '--muted': palette.mutedHex,
        '--muted-foreground': palette.mutedForegroundHex,
        '--accent': palette.mutedHex,
        '--accent-foreground': palette.foregroundHex,
        '--border': palette.borderHex,
        '--input': palette.borderHex,
        '--ring': palette.primaryHex,
        '--sidebar': palette.sidebarHex,
        '--sidebar-foreground': palette.foregroundHex,
        '--sidebar-primary': palette.primaryHex,
        '--sidebar-primary-foreground': palette.primaryForegroundHex,
        '--sidebar-accent': palette.mutedHex,
        '--sidebar-accent-foreground': palette.foregroundHex,
        '--sidebar-border': palette.borderHex,
        '--sidebar-ring': palette.primaryHex,
        '--success': palette.successHex,
        '--success-foreground': palette.successForegroundHex,
        '--warning': palette.warningHex,
        '--warning-foreground': palette.warningForegroundHex,
        '--info': palette.infoHex,
        '--info-foreground': palette.infoForegroundHex,
        '--destructive': palette.destructiveHex,
        '--destructive-foreground': palette.destructiveForegroundHex,
        '--syntax-kw': palette.syntaxKeywordHex,
        '--syntax-str': palette.syntaxStringHex,
        '--syntax-num': palette.syntaxNumberHex,
        '--syntax-fn': palette.syntaxFunctionHex,
        '--syntax-cm': palette.syntaxCommentHex,
        '--syntax-type': palette.syntaxTypeHex,
        '--diff-add': palette.diffAddHex,
        '--diff-del': palette.diffDelHex,
        '--chart-1': palette.chartPalette[0],
        '--chart-2': palette.chartPalette[1],
        '--chart-3': palette.chartPalette[2],
        '--chart-4': palette.chartPalette[3],
        '--chart-5': palette.chartPalette[4],
      };

      for (const [key, val] of Object.entries(vars)) {
        root.style.setProperty(key, val);
      }

      if (window.document.title !== 'OpenZ') {
        window.document.title = 'OpenZ';
      }
      syncDocumentFavicon(palette.primaryHex);
      let metaTheme = window.document.querySelector('meta[name="theme-color"]');
      if (!metaTheme) {
        metaTheme = window.document.createElement('meta');
        metaTheme.setAttribute('name', 'theme-color');
        window.document.head.appendChild(metaTheme);
      }
      metaTheme.setAttribute('content', palette.backgroundHex);
    };

    const signature = `${resolvedSystemOrMode}:${palette.id}`;
    const paletteChanged =
      prevSignatureRef.current !== null && prevSignatureRef.current !== signature;
    prevSignatureRef.current = signature;

    if (isFirstMount.current) {
      applyTokens();
      isFirstMount.current = false;
      return;
    }

    // When `runThemeViewTransition` already wrapped React's `flushSync` inside
    // `document.startViewTransition`, apply the new CSS tokens synchronously now
    // so the browser captures the exact post-update frame without nested transitions.
    if (isThemeViewTransitionActive() || !paletteChanged || motionQuery.matches) {
      applyTokens();
      return;
    }

    const doc = document as Document & {
      startViewTransition?: (cb: () => void) => {
        ready: Promise<void>;
        finished: Promise<void>;
      };
    };
    if (typeof doc.startViewTransition === 'function') {
      const { keyframes, durationMs, usesClipPath } = computeThemeClipKeyframes(
        themeTransitionStyle,
        window.innerWidth,
        window.innerHeight,
        null,
        resolvedSystemOrMode === 'dark',
      );
      if (usesClipPath) {
        root.setAttribute('data-theme-vt', 'clip');
      } else {
        root.removeAttribute('data-theme-vt');
      }
      const vt = doc.startViewTransition(() => {
        applyTokens();
      });
      vt.ready
        .then(() => {
          if (typeof root.animate === 'function') {
            root.animate(keyframes, {
              duration: durationMs,
              easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
              pseudoElement: '::view-transition-new(root)',
            });
          }
        })
        .catch(() => {});
      vt.finished
        .finally(() => {
          root.removeAttribute('data-theme-vt');
        })
        .catch(() => {});
      return;
    }

    if (transitionTimerRef.current !== null) {
      window.clearTimeout(transitionTimerRef.current);
    }

    root.classList.add('theme-transitioning');
    applyTokens();

    transitionTimerRef.current = window.setTimeout(() => {
      root.classList.remove('theme-transitioning');
      transitionTimerRef.current = null;
    }, 280);

    return () => {
      if (transitionTimerRef.current !== null) {
        window.clearTimeout(transitionTimerRef.current);
        transitionTimerRef.current = null;
      }
      root.classList.remove('theme-transitioning');
    };
  }, [resolvedSystemOrMode, activeThemeId, themeTransitionStyle, badgeStyle]);

  return (
    <div style={inlineTheme as React.CSSProperties}>
      {children}
    </div>
  );
}
