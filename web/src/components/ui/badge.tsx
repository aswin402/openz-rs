import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

export type GlobalBadgeStyle =
  | "minimal"
  | "subtle-pill"
  | "tinted"
  | "mono-tag"
  | "uiarc-badge"
  | "spectrum-status"

export interface BadgeStyleDescriptor {
  id: GlobalBadgeStyle
  title: string
  description: string
  isDefault?: boolean
}

export const GLOBAL_BADGE_STYLE_OPTIONS: ReadonlyArray<BadgeStyleDescriptor> = [
  {
    id: "minimal",
    title: "Minimal Hairline (Default)",
    description:
      "Ultra-clean low-contrast hairline surface with subtle typography and zero heavy fills.",
    isDefault: true,
  },
  {
    id: "uiarc-badge",
    title: "UIArc Specular Glass Badge",
    description:
      "@uiarc/badge — Frosted specular top-edge highlight with crisp subtle border across status pills.",
  },
  {
    id: "spectrum-status",
    title: "SpectrumUI Live Status Pill",
    description:
      "@spectrumui/status-badge — Rounded capsule status pill with semantic leading indicator dot.",
  },
  {
    id: "subtle-pill",
    title: "Soft Rounded Capsule",
    description:
      "Smooth rounded-full capsule pills harmonized with your active theme surface.",
  },
  {
    id: "mono-tag",
    title: "Precision Monospace Tag",
    description:
      "Compact architectural monospace tags inspired by Linear and Vercel Geist.",
  },
  {
    id: "tinted",
    title: "Tinted Accent Surface",
    description:
      "Soft primary-tinted badges for higher visual contrast across status labels.",
  },
]

const badgeVariants = cva(
  "openz-badge inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-md border px-1.5 py-0.5 text-[10.5px] font-medium tracking-tight whitespace-nowrap transition-colors [&>svg]:pointer-events-none [&>svg]:size-3",
  {
    variants: {
      variant: {
        default:
          "border-primary/25 bg-primary/14 text-primary",
        secondary:
          "border-border/50 bg-foreground/[0.045] text-muted-foreground",
        destructive:
          "border-destructive/25 bg-destructive/10 text-destructive",
        outline:
          "border-border/60 bg-transparent text-muted-foreground",
        minimal:
          "border-border/45 bg-transparent text-muted-foreground",
        ghost:
          "border-transparent bg-foreground/[0.045] text-muted-foreground",
        pill:
          "rounded-full border-border/55 bg-foreground/[0.04] px-2 text-foreground/85",
        success:
          "border-success/25 bg-success/10 text-success",
        warning:
          "border-warning/25 bg-warning/10 text-warning",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Badge({
  className,
  variant,
  asChild = false,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot.Root : "span"

  return (
    <Comp
      data-slot="badge"
      data-badge-variant={variant ?? "default"}
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
