"use client";

import * as React from "react";
import { motion, useReducedMotion } from "motion/react";
import { cn } from "@/lib/utils";

export type OrbPhase = "reasoning" | "search" | "tool" | "security" | "complete" | "error";

export type AicssOrbVariant = "morph" | "helix" | "ring" | "lens";

export interface ThinkingOrbsCanvasProps {
  phase?: OrbPhase;
  isActive?: boolean;
  size?: number;
  className?: string;
}

/**
 * `thinking-orbs` — Lightweight 2D HTML5 Canvas harmonic phase orb for AI agent states.
 * Reacts smoothly to `reasoning`, `search`, `tool`, `security`, and `complete` without WebGL overhead.
 */
export const ThinkingOrbsCanvas: React.FC<ThinkingOrbsCanvasProps> = ({
  phase = "reasoning",
  isActive = true,
  size = 20,
  className,
}) => {
  const canvasRef = React.useRef<HTMLCanvasElement | null>(null);
  const reducedMotion = useReducedMotion();

  React.useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return undefined;
    const ctx = canvas.getContext("2d");
    if (!ctx) return undefined;

    const dpr = typeof window !== "undefined" ? Math.min(window.devicePixelRatio || 1, 2) : 1;
    canvas.width = Math.round(size * dpr);
    canvas.height = Math.round(size * dpr);
    ctx.scale(dpr, dpr);

    const resolvePrimaryColor = (): string => {
      if (typeof window === "undefined") return "#ff5a1f";
      const computed = window.getComputedStyle(canvas).color;
      return computed || "#ff5a1f";
    };

    let frameId = 0;
    const start = performance.now();

    const speedMultiplier =
      phase === "tool"
        ? 1.65
        : phase === "search"
          ? 1.35
          : phase === "security"
            ? 0.9
            : 1.1;

    const draw = (now: number) => {
      const t = isActive && !reducedMotion ? ((now - start) / 1000) * speedMultiplier : 0.4;
      const cx = size / 2;
      const cy = size / 2;
      const baseR = size * 0.34;
      const strokeColor = resolvePrimaryColor();

      ctx.clearRect(0, 0, size, size);

      // Soft outer halo
      ctx.save();
      ctx.globalAlpha = isActive ? 0.18 : 0.1;
      ctx.beginPath();
      ctx.arc(cx, cy, baseR * (1.12 + 0.08 * Math.sin(t * 2.2)), 0, Math.PI * 2);
      ctx.fillStyle = strokeColor;
      ctx.fill();
      ctx.restore();

      // 3 harmonic orbital lobes (`thinking-orbs` signature 2D wave rings)
      const rings = phase === "complete" ? 1 : 3;
      for (let r = 0; r < rings; r += 1) {
        ctx.save();
        ctx.beginPath();
        const steps = 40;
        const phaseShift = (r * Math.PI * 2) / 3 + t * (r % 2 === 0 ? 1.4 : -1.1);
        const wobble = isActive && phase !== "complete" ? 0.16 : 0.02;
        for (let i = 0; i <= steps; i += 1) {
          const theta = (i / steps) * Math.PI * 2;
          const harmonic =
            1 +
            wobble * Math.sin(theta * (2 + r) + phaseShift) +
            wobble * 0.5 * Math.cos(theta * 3 - t * 1.8);
          const rad = baseR * harmonic * (1 - r * 0.08);
          const x = cx + Math.cos(theta) * rad;
          const y = cy + Math.sin(theta) * rad;
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.closePath();
        ctx.strokeStyle = strokeColor;
        ctx.lineWidth = Math.max(1.15, size * 0.075);
        ctx.globalAlpha = r === 0 ? 0.92 : r === 1 ? 0.55 : 0.32;
        ctx.stroke();
        ctx.restore();
      }

      // Core nucleus dot
      ctx.save();
      ctx.beginPath();
      ctx.arc(
        cx,
        cy,
        Math.max(1.6, size * (phase === "complete" ? 0.16 : 0.11 + 0.025 * Math.sin(t * 3))),
        0,
        Math.PI * 2,
      );
      ctx.fillStyle = strokeColor;
      ctx.globalAlpha = 0.95;
      ctx.fill();
      ctx.restore();

      if (isActive && !reducedMotion) {
        frameId = window.requestAnimationFrame(draw);
      }
    };

    draw(start);
    return () => {
      if (frameId) window.cancelAnimationFrame(frameId);
    };
  }, [phase, isActive, size, reducedMotion]);

  return (
    <canvas
      ref={canvasRef}
      aria-hidden="true"
      style={{ width: size, height: size }}
      className={cn("inline-block shrink-0 text-primary", className)}
    />
  );
};

export interface AicssOrbProps {
  variant?: AicssOrbVariant;
  isActive?: boolean;
  size?: number;
  className?: string;
}

/**
 * `aicss.dev/r/orbs.json` — Animated AI agent state orbs (`morph`, `helix`, `ring`, `lens`).
 */
export const AicssOrb: React.FC<AicssOrbProps> = ({
  variant = "morph",
  isActive = true,
  size = 18,
  className,
}) => {
  const reducedMotion = useReducedMotion();
  const animate = isActive && !reducedMotion;

  if (variant === "ring") {
    return (
      <span
        aria-hidden="true"
        style={{ width: size, height: size }}
        className={cn("relative inline-flex shrink-0 items-center justify-center text-primary", className)}
      >
        <motion.span
          animate={animate ? { rotate: 360 } : undefined}
          transition={{ duration: 2.2, repeat: Infinity, ease: "linear" }}
          className="absolute inset-0 rounded-full border-[1.5px] border-current border-t-transparent opacity-90"
        />
        <motion.span
          animate={animate ? { scale: [0.78, 1.08, 0.78], opacity: [0.45, 0.95, 0.45] } : undefined}
          transition={{ duration: 1.6, repeat: Infinity, ease: "easeInOut" }}
          className="size-[42%] rounded-full bg-current"
        />
      </span>
    );
  }

  if (variant === "helix") {
    return (
      <svg
        viewBox="0 0 24 24"
        width={size}
        height={size}
        aria-hidden="true"
        className={cn("shrink-0 text-primary", className)}
      >
        {[0, 1, 2, 3].map((idx) => (
          <React.Fragment key={idx}>
            <motion.circle
              cx={6 + idx * 4}
              cy={12}
              r={1.7}
              fill="currentColor"
              animate={
                animate
                  ? {
                      cy: [7, 17, 7],
                      opacity: [0.35, 1, 0.35],
                    }
                  : undefined
              }
              transition={{
                duration: 1.4,
                delay: idx * 0.16,
                repeat: Infinity,
                ease: "easeInOut",
              }}
            />
            <motion.circle
              cx={6 + idx * 4}
              cy={12}
              r={1.4}
              fill="currentColor"
              animate={
                animate
                  ? {
                      cy: [17, 7, 17],
                      opacity: [0.95, 0.3, 0.95],
                    }
                  : undefined
              }
              transition={{
                duration: 1.4,
                delay: idx * 0.16,
                repeat: Infinity,
                ease: "easeInOut",
              }}
            />
          </React.Fragment>
        ))}
      </svg>
    );
  }

  if (variant === "lens") {
    return (
      <span
        aria-hidden="true"
        style={{ width: size, height: size }}
        className={cn(
          "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full border border-primary/40 bg-primary/10 text-primary",
          className,
        )}
      >
        <motion.span
          animate={
            animate
              ? {
                  x: ["-35%", "35%", "-35%"],
                  scale: [0.85, 1.15, 0.85],
                }
              : undefined
          }
          transition={{ duration: 1.9, repeat: Infinity, ease: "easeInOut" }}
          className="size-[62%] rounded-full bg-current opacity-80 blur-[1px]"
        />
      </span>
    );
  }

  // Default: `morph` (fluid organic border-radius + rotation morph)
  return (
    <span
      aria-hidden="true"
      style={{ width: size, height: size }}
      className={cn("relative inline-flex shrink-0 items-center justify-center text-primary", className)}
    >
      <motion.span
        animate={
          animate
            ? {
                borderRadius: [
                  "48% 52% 62% 38% / 42% 48% 52% 58%",
                  "62% 38% 46% 54% / 56% 62% 38% 44%",
                  "42% 58% 52% 48% / 48% 40% 60% 52%",
                  "48% 52% 62% 38% / 42% 48% 52% 58%",
                ],
                rotate: [0, 120, 240, 360],
                scale: [0.92, 1.04, 0.96, 0.92],
              }
            : undefined
        }
        transition={{ duration: 3.2, repeat: Infinity, ease: "easeInOut" }}
        className="size-[84%] border border-current bg-primary/20 shadow-[inset_0_0_6px_currentColor]"
      />
    </span>
  );
};

export interface RareAiStatusProps {
  label?: string;
  detail?: string;
  isActive?: boolean;
  compact?: boolean;
  className?: string;
}

/**
 * `swamimalode07/rare-ui/ai-status` — Subtle inline AI status indicator with smooth spring dot & text transition.
 */
export const RareAiStatus: React.FC<RareAiStatusProps> = ({
  label = "Thinking…",
  detail,
  isActive = true,
  compact = false,
  className,
}) => {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2 rounded-full border border-border/55 bg-foreground/[0.035] px-2.5 py-0.5 text-xs text-foreground/85",
        compact && "border-transparent bg-transparent px-0 py-0",
        className,
      )}
    >
      <span className="relative inline-flex size-2 shrink-0 items-center justify-center">
        {isActive && (
          <span className="absolute inset-0 animate-ping rounded-full bg-primary/55" />
        )}
        <span
          className={cn(
            "relative size-1.5 rounded-full",
            isActive ? "bg-primary" : "bg-muted-foreground/65",
          )}
        />
      </span>
      <span className="truncate font-medium tracking-tight">{label}</span>
      {detail && (
        <span className="truncate text-[11px] text-muted-foreground">· {detail}</span>
      )}
    </span>
  );
};

export interface BeautifulLoadingStateProps {
  variant?: "pixel-timer" | "orbit";
  label?: string;
  size?: number;
  showElapsed?: boolean;
  className?: string;
}

/**
 * `beautifului.dev/r/loading-state.json` — Craft-forward pixel-grid & orbit loader with live elapsed timer.
 */
export const BeautifulLoadingState: React.FC<BeautifulLoadingStateProps> = ({
  variant = "pixel-timer",
  size = 16,
  showElapsed = false,
  className,
}) => {
  const [tenths, setTenths] = React.useState(12);

  React.useEffect(() => {
    if (!showElapsed) return undefined;
    const id = window.setInterval(() => {
      setTenths((prev) => (prev + 1) % 999);
    }, 100);
    return () => window.clearInterval(id);
  }, [showElapsed]);

  if (variant === "orbit") {
    return (
      <span
        style={{ width: size, height: size }}
        className={cn("relative inline-flex shrink-0 items-center justify-center text-primary", className)}
      >
        <span className="absolute inset-0 rounded-full border border-current opacity-25" />
        <motion.span
          animate={{ rotate: 360 }}
          transition={{ duration: 1.35, repeat: Infinity, ease: "linear" }}
          className="absolute inset-0"
        >
          <span className="absolute -top-[2px] left-1/2 size-1.5 -translate-x-1/2 rounded-full bg-current" />
        </motion.span>
        <span className="size-1 rounded-full bg-current opacity-70" />
      </span>
    );
  }

  return (
    <span className={cn("inline-flex shrink-0 items-center gap-1.5 text-primary", className)}>
      <span
        style={{ width: size, height: size }}
        className="grid shrink-0 grid-cols-3 gap-[1.5px] p-[1px]"
      >
        {[0, 1, 2, 5, 8, 7, 6, 3, 4].map((order, cell) => (
          <motion.span
            key={cell}
            animate={{ opacity: [0.18, 1, 0.18] }}
            transition={{
              duration: 1.1,
              delay: order * 0.08,
              repeat: Infinity,
              ease: "easeInOut",
            }}
            className="rounded-[1px] bg-current"
          />
        ))}
      </span>
      {showElapsed && (
        <span className="font-mono text-[10px] tabular-nums text-muted-foreground">
          {(tenths / 10).toFixed(1)}s
        </span>
      )}
    </span>
  );
};

export interface KokonutAiLoadingProps {
  size?: number;
  className?: string;
}

/**
 * `@kokonutui/ai-loading` — Equalizer + shimmer pulse indicator for active AI agent turns.
 */
export const KokonutAiLoading: React.FC<KokonutAiLoadingProps> = ({
  size = 16,
  className,
}) => {
  return (
    <span
      style={{ height: size }}
      className={cn("inline-flex shrink-0 items-center gap-[2.5px] text-primary", className)}
    >
      {[0.45, 0.95, 0.65, 1, 0.5].map((peak, idx) => (
        <motion.span
          key={idx}
          style={{ width: Math.max(2, Math.round(size * 0.13)) }}
          animate={{ height: [`${Math.round(peak * 38)}%`, `${Math.round(peak * 95)}%`, `${Math.round(peak * 38)}%`] }}
          transition={{
            duration: 0.95,
            delay: idx * 0.11,
            repeat: Infinity,
            ease: "easeInOut",
          }}
          className="rounded-full bg-current"
        />
      ))}
    </span>
  );
};
