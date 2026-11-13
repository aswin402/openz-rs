import * as React from "react"
import { cn } from "@/lib/utils"

export type FoxIconVariant = "classic" | "full-red-orange" | "solid-badge" | "outline"

/**
 * Exact symmetric Bezier silhouette of the OpenZ Fox head (ears, temple pinch,
 * dual cheek fur tufts, sweeping jawline, and rounded snout dip) in 0..100 space.
 */
export const FOX_SILHOUETTE_PATH =
  "M 25.5 8.0 " +
  "C 24.0 8.0, 22.8 9.2, 22.2 11.5 " +
  "C 19.6 19.5, 19.8 29.0, 22.2 40.0 " +
  "C 19.2 47.0, 13.5 58.0, 8.0 65.5 " +
  "L 10.6 67.3 " +
  "L 8.0 69.6 " +
  "C 15.0 75.5, 29.0 82.0, 40.2 84.6 " +
  "C 42.5 89.2, 46.5 90.4, 50.0 90.4 " +
  "C 53.5 90.4, 57.5 89.2, 59.8 84.6 " +
  "C 71.0 82.0, 85.0 75.5, 92.0 69.6 " +
  "L 89.4 67.3 " +
  "L 92.0 65.5 " +
  "C 86.5 58.0, 80.8 47.0, 77.8 40.0 " +
  "C 80.2 29.0, 80.4 19.5, 77.8 11.5 " +
  "C 77.2 9.2, 76.0 8.0, 74.5 8.0 " +
  "C 72.8 8.0, 70.6 9.8, 68.5 12.8 " +
  "C 64.2 18.8, 60.2 24.5, 57.5 26.8 " +
  "C 53.5 26.2, 46.5 26.2, 42.5 26.8 " +
  "C 39.8 24.5, 35.8 18.8, 31.5 12.8 " +
  "C 29.4 9.8, 27.2 8.0, 25.5 8.0 Z"

export interface OpenZFoxIconProps {
  size?: number
  variant?: FoxIconVariant
  /** Optional explicit CSS color override (e.g. for isolated theme swatch cards). */
  tintColor?: string
  accentColor?: string
  className?: string
}

/**
 * OpenZ Fox Icon — Uses the signature solid Fox head silhouette (Image 2) by default
 * (`variant="classic"`), driven 100% by theme CSS variables (`--primary`, `--brand-accent`)
 * with zero hardcoded hex colors.
 */
export const OpenZFoxIcon: React.FC<OpenZFoxIconProps> = ({
  size = 18,
  variant = "classic",
  tintColor,
  accentColor,
  className,
}) => {
  const gradId = React.useId()
  const primaryStop = tintColor ?? "var(--primary)"
  const accentStop = accentColor ?? tintColor ?? "var(--brand-accent)"

  if (variant === "classic") {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 100 100"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        role="img"
        aria-label="OpenZ Fox"
        className={cn("inline-block shrink-0 select-none", className)}
      >
        <defs>
          <linearGradient id={gradId} x1="12" y1="10" x2="88" y2="90" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor={primaryStop} />
            <stop offset="100%" stopColor={accentStop} />
          </linearGradient>
        </defs>
        <path d={FOX_SILHOUETTE_PATH} fill={`url(#${gradId})`} />
      </svg>
    )
  }

  if (variant === "outline") {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 100 100"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className={cn("inline-block shrink-0 select-none text-primary", className)}
        style={tintColor ? { color: tintColor } : undefined}
        aria-hidden="true"
      >
        <path
          d={FOX_SILHOUETTE_PATH}
          stroke="currentColor"
          strokeWidth="6.5"
          strokeLinejoin="round"
        />
      </svg>
    )
  }

  if (variant === "solid-badge") {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 100 100"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className={cn("inline-block shrink-0 select-none", className)}
        aria-hidden="true"
      >
        <defs>
          <linearGradient id={gradId} x1="8" y1="8" x2="92" y2="92" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stopColor={primaryStop} />
            <stop offset="100%" stopColor={accentStop} />
          </linearGradient>
        </defs>
        <circle cx="50" cy="50" r="46" fill={`url(#${gradId})`} />
        <g transform="translate(17 17) scale(0.66)">
          <path d={FOX_SILHOUETTE_PATH} fill="var(--primary-foreground)" />
        </g>
      </svg>
    )
  }

  // "full-red-orange" — Faceted geometric Fox silhouette driven by theme variables
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={cn("inline-block shrink-0 select-none", className)}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={gradId} x1="12" y1="8" x2="88" y2="92" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor={primaryStop} />
          <stop offset="100%" stopColor={accentStop} />
        </linearGradient>
      </defs>
      <path d={FOX_SILHOUETTE_PATH} fill={`url(#${gradId})`} />
      <path
        d="M 42.5 26.8 L 50 68 L 57.5 26.8 Z"
        fill="var(--background)"
        fillOpacity="0.16"
      />
      <circle cx="37" cy="54" r="3.8" fill="var(--background)" fillOpacity="0.72" />
      <circle cx="63" cy="54" r="3.8" fill="var(--background)" fillOpacity="0.72" />
      <path
        d="M 45.5 76 L 50 82.5 L 54.5 76 Z"
        fill="var(--background)"
        fillOpacity="0.72"
      />
    </svg>
  )
}

export interface OpenZBrandLogoProps {
  size?: "sm" | "md" | "lg"
  className?: string
}

/**
 * OpenZ Brand Wordmark (Image 1):
 * Renders `OpenZ Agent <FoxSilhouette>` inline on a single row with no leading icon.
 */
export const OpenZBrandLogo: React.FC<OpenZBrandLogoProps> = ({
  size = "md",
  className,
}) => {
  const spec =
    size === "lg"
      ? { text: "text-3xl sm:text-4xl", icon: 36, gap: "gap-2" }
      : size === "sm"
        ? { text: "text-sm", icon: 17, gap: "gap-1.5" }
        : { text: "text-lg", icon: 21, gap: "gap-1.5" }

  return (
    <span
      className={cn(
        "inline-flex items-center font-extrabold tracking-tight text-foreground select-none",
        spec.text,
        spec.gap,
        className
      )}
    >
      <span>
        OpenZ <span className="text-brand-gradient">Agent</span>
      </span>
      <OpenZFoxIcon size={spec.icon} variant="classic" className="-translate-y-[0.5px]" />
    </span>
  )
}

export const OpenZFoxAvatarBadge: React.FC<{
  variant?: FoxIconVariant
  size?: "sm" | "md" | "lg"
  className?: string
}> = ({ variant = "classic", size = "md", className }) => {
  const dims =
    size === "sm"
      ? { box: "h-7 w-7", icon: 16 }
      : size === "lg"
        ? { box: "h-11 w-11", icon: 25 }
        : { box: "h-8 w-8", icon: 18 }

  if (variant === "solid-badge") {
    return (
      <div
        className={cn(
          "inline-flex shrink-0 select-none items-center justify-center rounded-full shadow-xs",
          dims.box,
          className
        )}
      >
        <OpenZFoxIcon size={dims.icon + 11} variant="solid-badge" />
      </div>
    )
  }

  return (
    <div
      className={cn(
        "inline-flex shrink-0 select-none items-center justify-center rounded-full border border-primary/35 bg-primary/12 shadow-xs",
        dims.box,
        className
      )}
    >
      <OpenZFoxIcon size={dims.icon} variant={variant} />
    </div>
  )
}
