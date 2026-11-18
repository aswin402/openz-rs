import React, { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

interface ActiveHintState {
  text: string;
  rect: DOMRect;
}

const SVG_NS = 'http://www.w3.org/2000/svg';
const HOVER_DELAY_MS = 140;

export function hoistNativeTitlesInAncestry(start: Element | null): HTMLElement | null {
  let cur: Element | null = start;
  let nearestHintEl: HTMLElement | null = null;

  while (cur) {
    if (cur.namespaceURI !== SVG_NS && cur instanceof HTMLElement) {
      const rawTitle = cur.getAttribute('title');
      if (rawTitle !== null) {
        const trimmed = rawTitle.trim();
        if (trimmed) {
          cur.setAttribute('data-openz-hint', trimmed);
          if (
            !cur.hasAttribute('aria-label') &&
            (cur.tagName === 'BUTTON' || cur.tagName === 'A')
          ) {
            cur.setAttribute('aria-label', trimmed);
          }
        }
        cur.removeAttribute('title');
      }

      if (!nearestHintEl && cur.hasAttribute('data-openz-hint')) {
        nearestHintEl = cur;
      }
    }
    cur = cur.parentElement;
  }

  return nearestHintEl;
}

export function hasDedicatedCustomTooltip(el: HTMLElement): boolean {
  if (el.hasAttribute('data-openz-custom-hint')) return true;
  if (el.querySelector('[data-openz-custom-hint]')) return true;
  if (el.closest('[data-slot="tooltip-trigger"]')) return true;
  return false;
}

function computeHintPositionStyle(rect: DOMRect): React.CSSProperties {
  const vw = typeof window !== 'undefined' ? window.innerWidth : 1280;
  const vh = typeof window !== 'undefined' ? window.innerHeight : 800;

  if (rect.left < 84 && rect.right + 180 < vw) {
    return {
      left: `${Math.round(rect.right + 8)}px`,
      top: `${Math.round(rect.top + rect.height / 2)}px`,
      transform: 'translateY(-50%)',
    };
  }

  if (vw - rect.right < 84 && rect.left > 180) {
    return {
      right: `${Math.round(vw - rect.left + 8)}px`,
      top: `${Math.round(rect.top + rect.height / 2)}px`,
      transform: 'translateY(-50%)',
    };
  }

  const centerX = Math.min(vw - 16, Math.max(16, Math.round(rect.left + rect.width / 2)));
  if (rect.top > 44) {
    return {
      left: `${centerX}px`,
      bottom: `${Math.round(vh - rect.top + 7)}px`,
      transform: 'translateX(-50%)',
    };
  }

  return {
    left: `${centerX}px`,
    top: `${Math.round(rect.bottom + 7)}px`,
    transform: 'translateX(-50%)',
  };
}

/**
 * Global single-hint theme layer:
 * Intercepts native HTML `title` attributes across the entire WebUI so the browser
 * never renders a second native OS tooltip alongside OpenZ's themed UI, and displays
 * one crisp theme-aware glass hint pill for elements without their own custom tooltip.
 */
export const ThemedHintLayer: React.FC = () => {
  const [hint, setHint] = useState<ActiveHintState | null>(null);
  const activeAnchorRef = useRef<HTMLElement | null>(null);
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    const clearTimer = () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };

    const hideHint = () => {
      clearTimer();
      activeAnchorRef.current = null;
      setHint(null);
    };

    const showForTarget = (rawTarget: EventTarget | null) => {
      const targetEl = rawTarget instanceof Element ? rawTarget : null;
      const anchor = hoistNativeTitlesInAncestry(targetEl);

      if (!anchor || hasDedicatedCustomTooltip(anchor)) {
        hideHint();
        return;
      }

      const text = anchor.getAttribute('data-openz-hint')?.trim() ?? '';
      if (!text) {
        hideHint();
        return;
      }

      if (activeAnchorRef.current === anchor) return;

      clearTimer();
      activeAnchorRef.current = anchor;
      timerRef.current = window.setTimeout(() => {
        if (!anchor.isConnected) {
          hideHint();
          return;
        }
        setHint({
          text,
          rect: anchor.getBoundingClientRect(),
        });
      }, HOVER_DELAY_MS);
    };

    const handlePointerOver = (e: PointerEvent) => {
      if (e.pointerType === 'touch') return;
      showForTarget(e.target);
    };

    const handlePointerOut = (e: PointerEvent) => {
      const current = activeAnchorRef.current;
      if (!current) return;
      const next = e.relatedTarget instanceof Node ? e.relatedTarget : null;
      if (next && current.contains(next)) return;
      hideHint();
    };

    document.addEventListener('pointerover', handlePointerOver, true);
    document.addEventListener('pointerout', handlePointerOut, true);
    document.addEventListener('pointerdown', hideHint, true);
    document.addEventListener('keydown', hideHint, true);
    window.addEventListener('scroll', hideHint, true);
    window.addEventListener('resize', hideHint);
    window.addEventListener('blur', hideHint);

    return () => {
      clearTimer();
      document.removeEventListener('pointerover', handlePointerOver, true);
      document.removeEventListener('pointerout', handlePointerOut, true);
      document.removeEventListener('pointerdown', hideHint, true);
      document.removeEventListener('keydown', hideHint, true);
      window.removeEventListener('scroll', hideHint, true);
      window.removeEventListener('resize', hideHint);
      window.removeEventListener('blur', hideHint);
    };
  }, []);

  if (!hint || typeof document === 'undefined') return null;

  return createPortal(
    <div
      role="tooltip"
      style={computeHintPositionStyle(hint.rect)}
      className="pointer-events-none fixed z-[100] max-w-[280px] truncate rounded-lg border border-border/80 bg-popover/95 px-2.5 py-1 text-[11px] font-medium text-popover-foreground shadow-xl backdrop-blur-md"
    >
      {hint.text}
    </div>,
    document.body,
  );
};
