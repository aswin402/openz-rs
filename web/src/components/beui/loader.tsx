import * as React from "react"
import {
  Loader as MotionLoader,
  loaderMessages,
  type LoaderMessages,
  type LoaderProps as MotionLoaderProps,
  type LoaderVariant as MotionLoaderVariant,
} from "@/components/motion/loader"
import {
  AicssOrb,
  BeautifulLoadingState,
  KokonutAiLoading,
  ThinkingOrbsCanvas,
} from "@/components/registries/thinkingOrbs"

export { loaderMessages, type LoaderMessages }

export type ExtendedLoaderVariant =
  | MotionLoaderVariant
  | "beautiful-loading"
  | "beautiful-orbit"
  | "kokonut-ai-loading"
  | "aicss-orb-loader"
  | "thinking-orbs-mini"

export type BeuiLoaderVariant = ExtendedLoaderVariant

export interface BeuiLoaderProps extends Omit<MotionLoaderProps, "variant"> {
  variant?: BeuiLoaderVariant
}

export const BeuiLoader: React.FC<BeuiLoaderProps> = ({
  variant = "dot-matrix",
  size = 16,
  className,
  ...rest
}) => {
  if (variant === "beautiful-loading") {
    return <BeautifulLoadingState variant="pixel-timer" size={size} showElapsed className={className} />
  }
  if (variant === "beautiful-orbit") {
    return <BeautifulLoadingState variant="orbit" size={size} className={className} />
  }
  if (variant === "kokonut-ai-loading") {
    return <KokonutAiLoading size={size} className={className} />
  }
  if (variant === "aicss-orb-loader") {
    return <AicssOrb variant="morph" isActive size={size} className={className} />
  }
  if (variant === "thinking-orbs-mini") {
    return <ThinkingOrbsCanvas phase="reasoning" isActive size={size} className={className} />
  }
  return <MotionLoader variant={variant} size={size} className={className} {...rest} />
}

export interface BeuiLoaderOptionDescriptor {
  id: BeuiLoaderVariant
  title: string
  shortLabel: string
  category: "geometric" | "matrix" | "terminal" | "physics"
  description: string
  isDefault?: boolean
}

export const BEUI_LOADER_OPTIONS: ReadonlyArray<BeuiLoaderOptionDescriptor> = [
  {
    id: "dot-matrix",
    title: "3×3 Diagonal Dot Matrix",
    shortLabel: "Matrix",
    category: "matrix",
    description: "Official @beui/loader 3×3 dot matrix rippling diagonally across cells.",
    isDefault: true,
  },
  {
    id: "spinner",
    title: "Smooth Arc Spinner",
    shortLabel: "Spinner",
    category: "geometric",
    description: "Crisp orbital SVG arc with subtle track ring.",
  },
  {
    id: "dots",
    title: "Spring Wave Dots",
    shortLabel: "Dots",
    category: "geometric",
    description: "Three staggered dots bouncing smoothly in phase.",
  },
  {
    id: "bars",
    title: "Equalizer Pulse Bars",
    shortLabel: "Bars",
    category: "geometric",
    description: "Four rhythmic vertical bars scaling smoothly from baseline.",
  },
  {
    id: "dither",
    title: "4×4 Bayer Dither Sweep",
    shortLabel: "Dither",
    category: "matrix",
    description: "Retro-modern 4×4 ordered Bayer dither threshold animation.",
  },
  {
    id: "morph",
    title: "Polygon Shape Morph",
    shortLabel: "Morph",
    category: "geometric",
    description: "Point-to-point SVG morphing across circle, square, triangle, hexagon, and diamond.",
  },
  {
    id: "comet",
    title: "Orbital Comet Trail",
    shortLabel: "Comet",
    category: "physics",
    description: "Six-particle tapered comet orbiting smoothly with motion trail.",
  },
  {
    id: "metaballs",
    title: "Liquid Gooey Metaballs",
    shortLabel: "Metaballs",
    category: "physics",
    description: "SVG Gaussian-threshold liquid droplets merging and separating.",
  },
  {
    id: "newton",
    title: "Newton's Cradle",
    shortLabel: "Newton",
    category: "physics",
    description: "Five-sphere momentum transfer cradle with crisp end-ball impulse.",
  },
  {
    id: "helix",
    title: "Double-Strand DNA Helix",
    shortLabel: "Helix",
    category: "physics",
    description: "Counter-oscillating 3D depth-scaled particle strands.",
  },
  {
    id: "ascii",
    title: "CLI Braille Spinner",
    shortLabel: "CLI Dots",
    category: "terminal",
    description: "Classic 10-frame terminal agent Braille spinner (⠋ ⠙ ⠹ ⠸).",
  },
  {
    id: "ascii-braille",
    title: "Dense Braille Orbit",
    shortLabel: "Braille",
    category: "terminal",
    description: "8-cell high-density Braille perimeter sweep (⣾ ⣽ ⣻ ⢿).",
  },
  {
    id: "ascii-blocks",
    title: "Unicode Block Meter",
    shortLabel: "Blocks",
    category: "terminal",
    description: "Monospace rising and falling block-level wave (▁ ▂ ▃ ▄ ▅ ▆ ▇ █).",
  },
  {
    id: "ascii-bounce",
    title: "Single-Dot Braille Bounce",
    shortLabel: "Bounce",
    category: "terminal",
    description: "Minimalist single-dot Braille figure-eight trace.",
  },
  {
    id: "ascii-line",
    title: "UNIX Pipe Line Spinner",
    shortLabel: "ASCII |/-\\",
    category: "terminal",
    description: "Retro UNIX shell rotary character spinner (| / - \\).",
  },
  {
    id: "scramble",
    title: "Cipher Text Scramble",
    shortLabel: "Scramble",
    category: "terminal",
    description: "Cryptographic glyph decoder resolving progressively into THINKING.",
  },
  {
    id: "percent",
    title: "Live Telemetry Percent Bar",
    shortLabel: "Percent",
    category: "matrix",
    description: "Tabular monospace percentage counter paired with a micro progress track.",
  },
  {
    id: "beautiful-loading",
    title: "BeautifulUI Pixel Timer",
    shortLabel: "Pixel Timer",
    category: "matrix",
    description: "beautifului.dev/r/loading-state — Perimeter pixel shimmer with live elapsed seconds.",
  },
  {
    id: "beautiful-orbit",
    title: "BeautifulUI Satellite Orbit",
    shortLabel: "Orbit",
    category: "geometric",
    description: "beautifului.dev/r/loading-state — Smooth satellite dot orbiting a central nucleus.",
  },
  {
    id: "kokonut-ai-loading",
    title: "KokonutUI Harmonic Bars",
    shortLabel: "Kokonut",
    category: "geometric",
    description: "@kokonutui/ai-loading — Five-bar fluid harmonic wave equalizer.",
  },
  {
    id: "aicss-orb-loader",
    title: "AICSS Organic Morph Orb",
    shortLabel: "AICSS Orb",
    category: "physics",
    description: "aicss.dev/r/orbs — Fluid organic contour morphing loader.",
  },
  {
    id: "thinking-orbs-mini",
    title: "Thinking-Orbs Canvas Wave",
    shortLabel: "Canvas Orb",
    category: "physics",
    description: "thinking-orbs — 2D HTML5 Canvas multi-harmonic wave ring.",
  },
]

const VALID_LOADER_VARIANTS: Readonly<Record<BeuiLoaderVariant, true>> = {
  spinner: true,
  dots: true,
  bars: true,
  "dot-matrix": true,
  dither: true,
  ascii: true,
  "ascii-line": true,
  "ascii-braille": true,
  "ascii-blocks": true,
  "ascii-bounce": true,
  morph: true,
  comet: true,
  scramble: true,
  metaballs: true,
  newton: true,
  helix: true,
  percent: true,
  "beautiful-loading": true,
  "beautiful-orbit": true,
  "kokonut-ai-loading": true,
  "aicss-orb-loader": true,
  "thinking-orbs-mini": true,
}

export function isBeuiLoaderVariant(raw: unknown): raw is BeuiLoaderVariant {
  return typeof raw === "string" && raw in VALID_LOADER_VARIANTS
}

export function normalizeBeuiLoaderVariant(raw: unknown): BeuiLoaderVariant {
  if (isBeuiLoaderVariant(raw)) return raw
  return "dot-matrix"
}

export const ThinkingLoaderBadge: React.FC<{
  variant?: BeuiLoaderVariant
  size?: number
  className?: string
}> = ({ variant = "dot-matrix", size = 16, className }) => {
  const effectiveSize =
    variant === "scramble" || variant === "percent"
      ? Math.max(22, size + 6)
      : size
  return (
    <BeuiLoader
      variant={variant}
      size={effectiveSize}
      label="Thinking"
      labels={{ label: "Thinking", scramble: "THINKING" }}
      className={className}
    />
  )
}
