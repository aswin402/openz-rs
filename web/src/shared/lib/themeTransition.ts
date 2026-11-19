import { flushSync } from 'react-dom';

/**
 * GPU-composited directional & radial theme transition engine (`View Transition API` + fallback).
 *
 * Animates the entire OpenZ workspace seamlessly from one side, the clicked toggle origin,
 * the center of the screen, or a diagonal sweep without per-element layout thrashing or
 * muddy mid-transition color blending.
 */

export type ThemeTransitionStyleChoice =
  | 'circle-origin'
  | 'circle-center'
  | 'wipe-side'
  | 'diagonal-sweep'
  | 'crossfade';

export interface ThemeTransitionStyleDescriptor {
  id: ThemeTransitionStyleChoice;
  title: string;
  shortLabel: string;
  description: string;
  isDefault?: boolean;
}

export const THEME_TRANSITION_STYLE_OPTIONS: ReadonlyArray<ThemeTransitionStyleDescriptor> = [
  {
    id: 'circle-origin',
    title: 'Radial Origin Ripple (Default)',
    shortLabel: 'Origin Ripple',
    description:
      'Expands a smooth circular theme wave directly from the clicked toggle or theme card across the workspace.',
    isDefault: true,
  },
  {
    id: 'circle-center',
    title: 'Center Horizon Bloom',
    shortLabel: 'Center Bloom',
    description:
      'Expands a balanced circular reveal outward from the exact center of the viewport.',
  },
  {
    id: 'wipe-side',
    title: 'Directional Side Curtain',
    shortLabel: 'Side Wipe',
    description:
      'Sweeps the new theme smoothly across the screen from one side to the other.',
  },
  {
    id: 'diagonal-sweep',
    title: 'Diagonal Prism Sweep',
    shortLabel: 'Diagonal',
    description:
      'Glides a clean angled horizon from corner to corner across the interface.',
  },
  {
    id: 'crossfade',
    title: 'Soft Studio Crossfade',
    shortLabel: 'Crossfade',
    description:
      'Quiet 220ms opacity blend without geometric clip-path motion.',
  },
];

const VALID_TRANSITION_STYLES: Readonly<Record<ThemeTransitionStyleChoice, true>> = {
  'circle-origin': true,
  'circle-center': true,
  'wipe-side': true,
  'diagonal-sweep': true,
  crossfade: true,
};

export function normalizeThemeTransitionStyle(raw: unknown): ThemeTransitionStyleChoice {
  if (typeof raw === 'string' && raw in VALID_TRANSITION_STYLES) {
    return raw as ThemeTransitionStyleChoice;
  }
  return 'circle-origin';
}

interface PointerOrigin {
  x: number;
  y: number;
  timestamp: number;
}

let lastPointerOrigin: PointerOrigin | null = null;
let listenerInstalled = false;
let activeViewTransitionCount = 0;
let sideWipeFlip = false;

export function isThemeViewTransitionActive(): boolean {
  return activeViewTransitionCount > 0;
}

export function installThemePointerTracker(): void {
  if (listenerInstalled || typeof window === 'undefined') return;
  listenerInstalled = true;
  window.addEventListener(
    'pointerdown',
    (event: PointerEvent) => {
      if (Number.isFinite(event.clientX) && Number.isFinite(event.clientY)) {
        lastPointerOrigin = {
          x: event.clientX,
          y: event.clientY,
          timestamp: Date.now(),
        };
      }
    },
    { passive: true, capture: true },
  );
}

export function computeThemeClipKeyframes(
  style: ThemeTransitionStyleChoice,
  viewportWidth: number,
  viewportHeight: number,
  origin?: { x: number; y: number } | null,
  fromRightSide = true,
): { keyframes: PropertyIndexedKeyframes; durationMs: number; usesClipPath: boolean } {
  const w = Math.max(1, viewportWidth);
  const h = Math.max(1, viewportHeight);

  switch (style) {
    case 'circle-center': {
      const cx = Math.round(w / 2);
      const cy = Math.round(h / 2);
      const maxRadius = Math.ceil(Math.hypot(cx, cy));
      return {
        keyframes: {
          clipPath: [
            `circle(0px at ${cx}px ${cy}px)`,
            `circle(${maxRadius}px at ${cx}px ${cy}px)`,
          ],
        },
        durationMs: 520,
        usesClipPath: true,
      };
    }

    case 'wipe-side': {
      return {
        keyframes: {
          clipPath: fromRightSide
            ? ['inset(0% 0% 0% 100%)', 'inset(0% 0% 0% 0%)']
            : ['inset(0% 100% 0% 0%)', 'inset(0% 0% 0% 0%)'],
        },
        durationMs: 460,
        usesClipPath: true,
      };
    }

    case 'diagonal-sweep': {
      return {
        keyframes: {
          clipPath: [
            'polygon(100% 0%, 100% 0%, 100% 0%)',
            'polygon(100% 0%, -115% 0%, 100% 215%)',
          ],
        },
        durationMs: 540,
        usesClipPath: true,
      };
    }

    case 'crossfade': {
      return {
        keyframes: {
          opacity: [0, 1],
        },
        durationMs: 220,
        usesClipPath: false,
      };
    }

    case 'circle-origin':
    default: {
      const ox = origin ? Math.min(w, Math.max(0, Math.round(origin.x))) : Math.round(w / 2);
      const oy = origin ? Math.min(h, Math.max(0, Math.round(origin.y))) : Math.round(h / 2);
      const maxDx = Math.max(ox, w - ox);
      const maxDy = Math.max(oy, h - oy);
      const maxRadius = Math.ceil(Math.hypot(maxDx, maxDy));
      return {
        keyframes: {
          clipPath: [
            `circle(0px at ${ox}px ${oy}px)`,
            `circle(${maxRadius}px at ${ox}px ${oy}px)`,
          ],
        },
        durationMs: 520,
        usesClipPath: true,
      };
    }
  }
}

interface ViewTransitionHandle {
  ready: Promise<void>;
  finished: Promise<void>;
}

export function runThemeViewTransition(
  mutate: () => void,
  options?: {
    style?: ThemeTransitionStyleChoice;
    origin?: { x: number; y: number } | null;
  },
): void {
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    mutate();
    return;
  }

  const reducedMotion =
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  if (reducedMotion) {
    mutate();
    return;
  }

  const style = normalizeThemeTransitionStyle(options?.style);
  const recentPointer =
    lastPointerOrigin && Date.now() - lastPointerOrigin.timestamp < 1500
      ? { x: lastPointerOrigin.x, y: lastPointerOrigin.y }
      : null;
  const resolvedOrigin = options?.origin ?? recentPointer;

  const fromRightSide = resolvedOrigin
    ? resolvedOrigin.x >= window.innerWidth / 2
    : (sideWipeFlip = !sideWipeFlip);

  const doc = document as Document & {
    startViewTransition?: (cb: () => void) => ViewTransitionHandle;
  };

  const root = document.documentElement;

  if (typeof doc.startViewTransition === 'function') {
    const { keyframes, durationMs, usesClipPath } = computeThemeClipKeyframes(
      style,
      window.innerWidth,
      window.innerHeight,
      resolvedOrigin,
      fromRightSide,
    );

    if (usesClipPath) {
      root.setAttribute('data-theme-vt', 'clip');
    } else {
      root.removeAttribute('data-theme-vt');
    }

    activeViewTransitionCount += 1;
    let transition: ViewTransitionHandle;
    try {
      transition = doc.startViewTransition(() => {
        flushSync(() => {
          mutate();
        });
      });
    } catch {
      activeViewTransitionCount = Math.max(0, activeViewTransitionCount - 1);
      root.removeAttribute('data-theme-vt');
      mutate();
      return;
    }

    transition.ready
      .then(() => {
        if (typeof root.animate === 'function') {
          root.animate(keyframes, {
            duration: durationMs,
            easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
            pseudoElement: '::view-transition-new(root)',
          });
        }
      })
      .catch(() => {
        // Ignore aborted view transitions when rapidly clicking
      });

    transition.finished
      .finally(() => {
        activeViewTransitionCount = Math.max(0, activeViewTransitionCount - 1);
        if (activeViewTransitionCount === 0) {
          root.removeAttribute('data-theme-vt');
        }
      })
      .catch(() => {});
    return;
  }

  // Fallback for browsers/WebViews without View Transition API support:
  // capture current background color, mutate tokens immediately, and animate the old
  // surface away using the exact inverse clip-path so directional/radial motion still works!
  const previousBg =
    window.getComputedStyle(root).getPropertyValue('--background').trim() || '#09090b';

  root.classList.add('theme-transitioning');
  mutate();

  if (style !== 'crossfade' && typeof document.body?.appendChild === 'function') {
    const veil = document.createElement('div');
    veil.setAttribute('aria-hidden', 'true');
    veil.style.position = 'fixed';
    veil.style.inset = '0';
    veil.style.zIndex = '9998';
    veil.style.pointerEvents = 'none';
    veil.style.background = previousBg;
    veil.style.opacity = '0.92';
    document.body.appendChild(veil);

    const { keyframes, durationMs } = computeThemeClipKeyframes(
      style,
      window.innerWidth,
      window.innerHeight,
      resolvedOrigin,
      fromRightSide,
    );
    const clipFrames = Array.isArray(keyframes.clipPath)
      ? keyframes.clipPath.filter((frame): frame is string => typeof frame === 'string').reverse()
      : undefined;

    if (typeof veil.animate === 'function' && clipFrames) {
      const anim = veil.animate(
        { clipPath: clipFrames, opacity: [0.9, 0.45] },
        { duration: Math.round(durationMs * 0.85), easing: 'cubic-bezier(0.22, 1, 0.36, 1)' },
      );
      anim.onfinish = () => veil.remove();
      anim.oncancel = () => veil.remove();
    } else {
      window.setTimeout(() => veil.remove(), 320);
    }
  }

  window.setTimeout(() => {
    root.classList.remove('theme-transitioning');
  }, 320);
}
