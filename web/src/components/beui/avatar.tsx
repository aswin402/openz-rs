import * as React from "react"
import {
  Bot,
  BrainCircuit,
  Code2,
  Flame,
  Shield,
  Sparkles,
  Terminal,
  User,
  Zap,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { OpenZFoxIcon } from "@/components/OpenZFoxIcon"

export type BeuiAgentAvatarVariant =
  | "fox-classic"
  | "fox-pure"
  | "fox-red-orange"
  | "fox-solid-badge"
  | "fox-outline"
  | "bot-droid"
  | "sparkles-ai"
  | "neural-core"
  | "rust-terminal"
  | "ember-flame"
  | "async-bolt"
  | "monogram-z"
  | "none"

export type BeuiUserAvatarVariant =
  | "user-default"
  | "user-you"
  | "user-dev"
  | "user-terminal"
  | "user-shield"
  | "none"

export interface BeuiAvatarOptionMeta<T extends string> {
  id: T
  title: string
  description: string
  isDefault?: boolean
}

export const AGENT_AVATAR_OPTIONS: BeuiAvatarOptionMeta<BeuiAgentAvatarVariant>[] = [
  {
    id: "fox-classic",
    title: "OpenZ Fox Silhouette (Default)",
    description: "Signature solid OpenZ Fox head silhouette inside a crisp subtle badge.",
    isDefault: true,
  },
  {
    id: "fox-pure",
    title: "Pure Minimal Fox Silhouette",
    description: "Borderless solid OpenZ Fox silhouette with zero ring or background.",
  },
  {
    id: "fox-red-orange",
    title: "Faceted Fox Avatar",
    description: "Sculpted theme-adaptive faceted Fox emblem.",
  },
  {
    id: "fox-solid-badge",
    title: "Solid Fox Crest",
    description: "Solid circular brand-gradient Fox crest avatar.",
  },
  {
    id: "fox-outline",
    title: "Wireframe Fox",
    description: "Sharp geometric stroke Fox silhouette avatar.",
  },
  {
    id: "bot-droid",
    title: "BEUI Bot Droid",
    description: "Clean autonomous AI bot assistant avatar.",
  },
  {
    id: "sparkles-ai",
    title: "Sparkles AI Star",
    description: "Minimalist generative star badge.",
  },
  {
    id: "neural-core",
    title: "Neural Circuit Core",
    description: "Cognitive graph & reasoning circuit emblem.",
  },
  {
    id: "rust-terminal",
    title: "Rust CLI Terminal",
    description: "Developer shell prompt (>_ ) agent badge.",
  },
  {
    id: "ember-flame",
    title: "Ember Flame",
    description: "Warm high-energy flame badge.",
  },
  {
    id: "async-bolt",
    title: "Async Lightning Bolt",
    description: "Low-latency Tokio lightning emblem.",
  },
  {
    id: "monogram-z",
    title: "OpenZ 'Z' Monogram",
    description: "Bold typographic Z crest in primary accent.",
  },
  {
    id: "none",
    title: "Hidden (No Avatar)",
    description: "Hide outer assistant avatar for a pure text column.",
  },
]

export const USER_AVATAR_OPTIONS: BeuiAvatarOptionMeta<BeuiUserAvatarVariant>[] = [
  {
    id: "user-default",
    title: "Minimal User",
    description: "Clean shadcn/ui user silhouette circle.",
    isDefault: true,
  },
  {
    id: "user-you",
    title: "'U' Monogram",
    description: "Bold typographic U badge in secondary surface.",
  },
  {
    id: "user-dev",
    title: "Code Developer",
    description: "Clean < /> developer badge.",
  },
  {
    id: "user-terminal",
    title: "CLI Operator",
    description: "Terminal prompt operator badge.",
  },
  {
    id: "user-shield",
    title: "Security Operator",
    description: "Shielded operator badge.",
  },
  {
    id: "none",
    title: "Hidden (No Avatar)",
    description: "Hide outer right user avatar.",
  },
]

export interface BeuiAgentAvatarProps {
  variant?: BeuiAgentAvatarVariant
  size?: "sm" | "md" | "lg"
  className?: string
}

/**
 * @beui/avatar — Multi-avatar renderer for OpenZ Agent (defaulting to the signature
 * solid OpenZ Fox silhouette, plus sculpted Fox variants, Bot, Sparkles, Neural Core,
 * Terminal, Flame, Bolt, and Z Monogram).
 */
export const BeuiAgentAvatar: React.FC<BeuiAgentAvatarProps> = ({
  variant = "fox-classic",
  size = "md",
  className,
}) => {
  if (variant === "none") return null

  const dims =
    size === "sm"
      ? { box: "size-7", iconClass: "size-3.5", px: 16, text: "text-xs" }
      : size === "lg"
        ? { box: "size-11", iconClass: "size-5", px: 25, text: "text-base" }
        : { box: "size-8", iconClass: "size-4", px: 18, text: "text-xs" }

  if (variant === "fox-pure") {
    return (
      <div
        className={cn(
          "inline-flex shrink-0 select-none items-center justify-center",
          dims.box,
          className
        )}
      >
        <OpenZFoxIcon size={dims.px + 2} variant="classic" />
      </div>
    )
  }

  if (variant === "fox-solid-badge") {
    return (
      <div
        className={cn(
          "inline-flex shrink-0 select-none items-center justify-center rounded-full shadow-2xs",
          dims.box,
          className
        )}
      >
        <OpenZFoxIcon size={dims.px + 12} variant="solid-badge" />
      </div>
    )
  }

  if (
    variant === "fox-classic" ||
    variant === "fox-red-orange" ||
    variant === "fox-outline"
  ) {
    const foxVar =
      variant === "fox-red-orange"
        ? "full-red-orange"
        : variant === "fox-outline"
          ? "outline"
          : "classic"
    return (
      <div
        className={cn(
          "inline-flex shrink-0 select-none items-center justify-center rounded-full border border-primary/35 bg-primary/12 shadow-2xs",
          dims.box,
          className
        )}
      >
        <OpenZFoxIcon size={dims.px} variant={foxVar} />
      </div>
    )
  }

  if (variant === "monogram-z") {
    return (
      <div
        className={cn(
          "inline-flex shrink-0 select-none items-center justify-center rounded-full border border-primary/40 bg-primary text-primary-foreground font-mono font-extrabold shadow-2xs",
          dims.box,
          dims.text,
          className
        )}
      >
        Z
      </div>
    )
  }

  const IconMap: Record<
    Exclude<
      BeuiAgentAvatarVariant,
      | "fox-classic"
      | "fox-pure"
      | "fox-red-orange"
      | "fox-solid-badge"
      | "fox-outline"
      | "monogram-z"
      | "none"
    >,
    React.ComponentType<{ className?: string }>
  > = {
    "bot-droid": Bot,
    "sparkles-ai": Sparkles,
    "neural-core": BrainCircuit,
    "rust-terminal": Terminal,
    "ember-flame": Flame,
    "async-bolt": Zap,
  }

  const IconComp = IconMap[variant]

  return (
    <div
      className={cn(
        "inline-flex shrink-0 select-none items-center justify-center rounded-full border border-primary/35 bg-primary/12 text-primary shadow-2xs",
        dims.box,
        className
      )}
    >
      <IconComp className={dims.iconClass} />
    </div>
  )
}

export interface BeuiUserAvatarProps {
  variant?: BeuiUserAvatarVariant
  size?: "sm" | "md" | "lg"
  className?: string
}

export const BeuiUserAvatar: React.FC<BeuiUserAvatarProps> = ({
  variant = "user-default",
  size = "md",
  className,
}) => {
  if (variant === "none") return null

  const dims =
    size === "sm"
      ? { box: "size-7", iconClass: "size-3.5", text: "text-[10px]" }
      : size === "lg"
        ? { box: "size-11", iconClass: "size-5", text: "text-sm" }
        : { box: "size-8", iconClass: "size-4", text: "text-xs" }

  if (variant === "user-you") {
    return (
      <div
        className={cn(
          "inline-flex shrink-0 select-none items-center justify-center rounded-full border border-border/80 bg-secondary font-mono font-bold text-secondary-foreground shadow-2xs",
          dims.box,
          dims.text,
          className
        )}
      >
        U
      </div>
    )
  }

  const UserIcon =
    variant === "user-dev"
      ? Code2
      : variant === "user-terminal"
        ? Terminal
        : variant === "user-shield"
          ? Shield
          : User

  return (
    <div
      className={cn(
        "inline-flex shrink-0 select-none items-center justify-center rounded-full border border-border/80 bg-secondary text-secondary-foreground shadow-2xs",
        dims.box,
        className
      )}
    >
      <UserIcon className={dims.iconClass} />
    </div>
  )
}
