"use client";

import * as React from "react";
import {
  Bell,
  BellOff,
  Bookmark,
  Brain,
  Check,
  Sparkles,
  Zap,
} from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { cn } from "@/lib/utils";
import type { ActivityNotice, CavemanLevelSlug, ToolExecution } from "@/types";
import { CAVEMAN_LEVEL_OPTIONS } from "@/shared/lib/cavemanLevels";

/* -------------------------------------------------------------------------- */
/* 1. Rare-UI Notification Bell (`swamimalode07/rare-ui/notification-bell`)   */
/* -------------------------------------------------------------------------- */

export interface RareNotificationBellProps {
  unreadCount?: number;
  muted?: boolean;
  active?: boolean;
  className?: string;
}

export const RareNotificationBell: React.FC<RareNotificationBellProps> = ({
  unreadCount = 0,
  muted = false,
  active = false,
  className,
}) => {
  const reduce = useReducedMotion() ?? false;
  const [ringCycle, setRingCycle] = React.useState(0);
  const prevUnreadRef = React.useRef(unreadCount);

  React.useEffect(() => {
    const grew = unreadCount > prevUnreadRef.current;
    prevUnreadRef.current = unreadCount;
    if (grew) {
      setRingCycle((c) => c + 1);
    }
  }, [unreadCount]);

  const hasUnread = unreadCount > 0 && !muted;

  return (
    <span
      onMouseEnter={() => {
        if (!muted && !reduce) setRingCycle((c) => c + 1);
      }}
      className={cn(
        "relative inline-flex size-5 items-center justify-center",
        className,
      )}
    >
      {muted ? (
        <BellOff className="size-4 text-muted-foreground/75" />
      ) : (
        <motion.span
          key={ringCycle}
          initial={{ rotate: 0 }}
          animate={
            ringCycle > 0 && !reduce
              ? { rotate: [0, -18, 16, -12, 9, -5, 0] }
              : { rotate: 0 }
          }
          transition={{ duration: 0.58, ease: "easeOut" }}
          style={{ transformOrigin: "50% 14%" }}
          className="inline-flex items-center justify-center"
        >
          <Bell
            className={cn(
              "size-4 transition-colors",
              active || hasUnread ? "text-primary" : "text-current",
            )}
          />
        </motion.span>
      )}

      {hasUnread && (
        <span className="pointer-events-none absolute -top-1 -right-1 flex items-center justify-center">
          {!reduce && (
            <span className="absolute size-3 animate-ping rounded-full bg-primary/50" />
          )}
          <span className="relative inline-flex min-w-[14px] items-center justify-center rounded-full bg-primary px-1 py-0 font-mono text-[8.5px] leading-[13px] font-bold text-primary-foreground ring-2 ring-background">
            {unreadCount > 9 ? "9+" : unreadCount}
          </span>
        </span>
      )}
    </span>
  );
};

/* -------------------------------------------------------------------------- */
/* 2. Classic Morphing Theme Toggle (`@toggles/classic`)                      */
/* -------------------------------------------------------------------------- */

export interface ClassicThemeToggleIconProps {
  isDark: boolean;
  size?: number;
  className?: string;
}

const SUN_RAY_ANGLES = [0, 45, 90, 135, 180, 225, 270, 315] as const;

export const ClassicThemeToggleIcon: React.FC<ClassicThemeToggleIconProps> = ({
  isDark,
  size = 16,
  className,
}) => {
  const maskId = React.useId();
  const reduce = useReducedMotion() ?? false;
  const spring = reduce
    ? { duration: 0 }
    : { type: "spring" as const, stiffness: 320, damping: 24 };

  return (
    <motion.svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      animate={{ rotate: isDark ? 40 : 90 }}
      transition={spring}
      className={cn("shrink-0", className)}
    >
      <mask id={maskId}>
        <rect x="0" y="0" width="24" height="24" fill="white" />
        <motion.circle
          r="6"
          fill="black"
          initial={false}
          animate={{
            cx: isDark ? 15.2 : 26,
            cy: isDark ? 8.8 : 2,
          }}
          transition={spring}
        />
      </mask>

      <motion.circle
        cx="12"
        cy="12"
        fill="currentColor"
        mask={`url(#${maskId})`}
        initial={false}
        animate={{ r: isDark ? 8.2 : 5 }}
        transition={spring}
      />

      <motion.g
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        initial={false}
        animate={{
          opacity: isDark ? 0 : 1,
          scale: isDark ? 0.45 : 1,
        }}
        style={{ originX: "12px", originY: "12px" }}
        transition={spring}
      >
        {SUN_RAY_ANGLES.map((deg) => {
          const rad = (deg * Math.PI) / 180;
          const x1 = 12 + Math.cos(rad) * 8.1;
          const y1 = 12 + Math.sin(rad) * 8.1;
          const x2 = 12 + Math.cos(rad) * 10.2;
          const y2 = 12 + Math.sin(rad) * 10.2;
          return (
            <line
              key={deg}
              x1={x1.toFixed(2)}
              y1={y1.toFixed(2)}
              x2={x2.toFixed(2)}
              y2={y2.toFixed(2)}
            />
          );
        })}
      </motion.g>
    </motion.svg>
  );
};

/* -------------------------------------------------------------------------- */
/* 3. Smooth Spring Drawer (`@kokonutui/smooth-drawer`)                       */
/* -------------------------------------------------------------------------- */

export type DrawerMotionStyleChoice =
  | "kokonut-smooth"
  | "snappy-slide"
  | "instant";

export interface DrawerMotionStyleDescriptor {
  id: DrawerMotionStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const DRAWER_MOTION_STYLE_OPTIONS: ReadonlyArray<DrawerMotionStyleDescriptor> =
  [
    {
      id: "kokonut-smooth",
      title: "KokonutUI Smooth Spring Drawer (Default)",
      description:
        "@kokonutui/smooth-drawer — Fluid damped spring physics with subtle scale-in and frosted backdrop transition.",
      isDefault: true,
    },
    {
      id: "snappy-slide",
      title: "Crisp Linear Edge Slide",
      description:
        "Fast 180ms cubic-out edge translation with zero overshoot for high-frequency inspection.",
    },
    {
      id: "instant",
      title: "Zero-Motion Instant Mount",
      description:
        "Disables drawer entrance/exit transitions for maximum keyboard velocity.",
    },
  ];

export interface KokonutSmoothDrawerProps {
  open: boolean;
  onClose?: () => void;
  side?: "right" | "left";
  styleVariant?: DrawerMotionStyleChoice;
  showBackdrop?: boolean;
  backdropClassName?: string;
  ariaLabel?: string;
  className?: string;
  children: React.ReactNode;
}

export const KokonutSmoothDrawer: React.FC<KokonutSmoothDrawerProps> = ({
  open,
  onClose,
  side = "right",
  styleVariant = "kokonut-smooth",
  showBackdrop = false,
  backdropClassName,
  ariaLabel,
  className,
  children,
}) => {
  const reduce = useReducedMotion() ?? false;
  const effectiveStyle: DrawerMotionStyleChoice = reduce ? "instant" : styleVariant;
  const offset = side === "right" ? 36 : -36;

  const transition =
    effectiveStyle === "instant"
      ? { duration: 0 }
      : effectiveStyle === "snappy-slide"
        ? { duration: 0.18, ease: [0.22, 1, 0.36, 1] as const }
        : { type: "spring" as const, stiffness: 350, damping: 32, mass: 0.85 };

  return (
    <AnimatePresence initial={false}>
      {open && (
        <>
          {showBackdrop && (
            <motion.div
              key="smooth-drawer-backdrop"
              initial={effectiveStyle === "instant" ? false : { opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: effectiveStyle === "instant" ? 0 : 0.18 }}
              onClick={onClose}
              className={cn(
                "fixed inset-0 z-40 bg-black/45 backdrop-blur-xs",
                backdropClassName,
              )}
            />
          )}
          <motion.aside
            key="smooth-drawer-panel"
            aria-label={ariaLabel}
            initial={
              effectiveStyle === "instant"
                ? false
                : {
                    opacity: 0,
                    x: offset,
                    scale: effectiveStyle === "kokonut-smooth" ? 0.985 : 1,
                  }
            }
            animate={{ opacity: 1, x: 0, scale: 1 }}
            exit={
              effectiveStyle === "instant"
                ? { opacity: 0 }
                : {
                    opacity: 0,
                    x: offset,
                    scale: effectiveStyle === "kokonut-smooth" ? 0.985 : 1,
                  }
            }
            transition={transition}
            className={className}
          >
            {children}
          </motion.aside>
        </>
      )}
    </AnimatePresence>
  );
};

/* -------------------------------------------------------------------------- */
/* 4. KokonutUI Animated Toolbar Dock (`@kokonutui/toolbar`)                  */
/* -------------------------------------------------------------------------- */

export type DockToolbarStyleChoice =
  | "kokonut-spring"
  | "kokonut-expandable"
  | "minimal-glass";

export interface DockToolbarStyleDescriptor {
  id: DockToolbarStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const DOCK_TOOLBAR_STYLE_OPTIONS: ReadonlyArray<DockToolbarStyleDescriptor> =
  [
    {
      id: "kokonut-spring",
      title: "KokonutUI Spring Pill Dock (Default)",
      description:
        "@kokonutui/toolbar — Shared-layout spring selection pill glides smoothly across icons with tactile press micro-physics.",
      isDefault: true,
    },
    {
      id: "kokonut-expandable",
      title: "KokonutUI Expandable Label Dock",
      description:
        "@kokonutui/toolbar — Expands a compact inline label pill beside the active or hovered icon.",
    },
    {
      id: "minimal-glass",
      title: "Minimal Hairline Glass Rail",
      description:
        "Whisper-quiet translucent floating icon rail without layout-glide motion.",
    },
  ];

export interface KokonutToolbarItem {
  id: string;
  label: string;
  active?: boolean;
  onClick: () => void;
  renderIcon: () => React.ReactNode;
  dividerBefore?: boolean;
}

export interface KokonutToolbarDockProps {
  items: ReadonlyArray<KokonutToolbarItem>;
  variant?: DockToolbarStyleChoice;
  orientation?: "vertical" | "horizontal";
  layoutGroupId?: string;
  className?: string;
}

export const KokonutToolbarDock: React.FC<KokonutToolbarDockProps> = ({
  items,
  variant = "kokonut-spring",
  orientation = "vertical",
  layoutGroupId = "openz-kokonut-dock",
  className,
}) => {
  const reduce = useReducedMotion() ?? false;
  const [hoveredId, setHoveredId] = React.useState<string | null>(null);
  const useMotion = !reduce && variant !== "minimal-glass";

  return (
    <div
      className={cn(
        "flex items-center gap-1 rounded-2xl border border-border/60 bg-background/75 p-1.5 shadow-lg shadow-foreground/5 backdrop-blur-xl",
        orientation === "vertical" ? "flex-col" : "flex-row",
        className,
      )}
    >
      {items.map((item) => {
        const showInlineExpand =
          variant === "kokonut-expandable" &&
          orientation === "horizontal" &&
          (item.active || hoveredId === item.id);

        return (
          <React.Fragment key={item.id}>
            {item.dividerBefore && (
              <span
                aria-hidden="true"
                className={
                  orientation === "vertical"
                    ? "my-0.5 h-px w-5 bg-border/55"
                    : "mx-0.5 h-5 w-px bg-border/55"
                }
              />
            )}
            <motion.button
              type="button"
              onClick={item.onClick}
              onMouseEnter={() => setHoveredId(item.id)}
              onMouseLeave={() =>
                setHoveredId((prev) => (prev === item.id ? null : prev))
              }
              whileHover={useMotion ? { scale: 1.07 } : undefined}
              whileTap={useMotion ? { scale: 0.91 } : undefined}
              transition={{ type: "spring", stiffness: 440, damping: 24 }}
              aria-label={item.label}
              className={cn(
                "group relative flex h-8 min-w-8 items-center justify-center gap-1.5 rounded-xl px-2 text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50",
                item.active && "text-primary",
                !item.active && hoveredId === item.id && "bg-foreground/[0.045]",
              )}
            >
              {item.active &&
                (useMotion ? (
                  <motion.span
                    layoutId={`${layoutGroupId}-active`}
                    transition={{
                      type: "spring",
                      stiffness: 380,
                      damping: 29,
                      mass: 0.75,
                    }}
                    className="pointer-events-none absolute inset-0 rounded-xl border border-primary/30 bg-primary/15 shadow-xs"
                  />
                ) : (
                  <span className="pointer-events-none absolute inset-0 rounded-xl border border-primary/30 bg-primary/15" />
                ))}

              <span className="relative z-10 inline-flex items-center justify-center">
                {item.renderIcon()}
              </span>

              <AnimatePresence initial={false}>
                {showInlineExpand && (
                  <motion.span
                    key="expand-label"
                    initial={{ width: 0, opacity: 0 }}
                    animate={{ width: "auto", opacity: 1 }}
                    exit={{ width: 0, opacity: 0 }}
                    transition={{ duration: 0.18, ease: "easeOut" }}
                    className="relative z-10 overflow-hidden whitespace-nowrap text-[11px] font-medium"
                  >
                    {item.label}
                  </motion.span>
                )}
              </AnimatePresence>

              {orientation === "vertical" && (
                <AnimatePresence>
                  {hoveredId === item.id && (
                    <motion.span
                      key="dock-tooltip"
                      data-openz-custom-hint="true"
                      initial={
                        variant === "kokonut-expandable"
                          ? { opacity: 0, x: 6, scale: 0.94 }
                          : { opacity: 0, x: 4 }
                      }
                      animate={{ opacity: 1, x: 0, scale: 1 }}
                      exit={{ opacity: 0, x: 4, scale: 0.96 }}
                      transition={{ duration: 0.14, ease: "easeOut" }}
                      className={cn(
                        "pointer-events-none absolute right-full top-1/2 mr-2.5 -translate-y-1/2 whitespace-nowrap rounded-xl border border-border/75 bg-popover/95 px-2.5 py-1 text-[11px] font-medium text-popover-foreground shadow-lg backdrop-blur-xl",
                        variant === "kokonut-expandable" &&
                          "border-primary/35 text-foreground",
                      )}
                    >
                      {item.label}
                    </motion.span>
                  )}
                </AnimatePresence>
              )}
            </motion.button>
          </React.Fragment>
        );
      })}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 5. KokonutUI AI Token Alternatives (`@kokonutui/ai-token-alternatives`)    */
/* -------------------------------------------------------------------------- */

export type TokenAlternativesStyleChoice =
  | "compact-popover"
  | "kokonut-pills"
  | "hidden";

export interface TokenAlternativesStyleDescriptor {
  id: TokenAlternativesStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const TOKEN_ALTERNATIVES_STYLE_OPTIONS: ReadonlyArray<TokenAlternativesStyleDescriptor> =
  [
    {
      id: "compact-popover",
      title: "KokonutUI Compact Alternatives Trigger (Default)",
      description:
        "@kokonutui/ai-token-alternatives — Expands token compression & response-density alternatives on click from the composer bar.",
      isDefault: true,
    },
    {
      id: "kokonut-pills",
      title: "KokonutUI Inline Token Strip",
      description:
        "@kokonutui/ai-token-alternatives — Displays animated spring-sliding token density pills directly inside the composer.",
    },
    {
      id: "hidden",
      title: "Single Toggle Button Only",
      description:
        "Shows only the classic Caveman ON/OFF button without inline alternative pills.",
    },
  ];

export interface KokonutTokenAlternativesProps {
  enabled: boolean;
  activeLevel: CavemanLevelSlug;
  savingsBadge?: string | null;
  variant?: TokenAlternativesStyleChoice;
  onToggleEnabled: () => void;
  onSelectLevel: (level: CavemanLevelSlug) => void;
  className?: string;
}

const QUICK_TOKEN_TIERS: ReadonlyArray<{
  slug: CavemanLevelSlug;
  short: string;
  est: string;
}> = [
  { slug: "lite", short: "Lite", est: "~31%" },
  { slug: "full", short: "Full", est: "~57%" },
  { slug: "ultra", short: "Ultra", est: "~72%" },
  { slug: "symbolic_full", short: "Symbolic", est: "~78%" },
];

export const KokonutTokenAlternatives: React.FC<
  KokonutTokenAlternativesProps
> = ({
  enabled,
  activeLevel,
  savingsBadge,
  variant = "compact-popover",
  onToggleEnabled,
  onSelectLevel,
  className,
}) => {
  const [open, setOpen] = React.useState(false);
  const groupId = React.useId();
  const levelInfo = React.useMemo(
    () =>
      CAVEMAN_LEVEL_OPTIONS.find((opt) => opt.slug === activeLevel) ??
      CAVEMAN_LEVEL_OPTIONS[1],
    [activeLevel],
  );

  if (variant === "hidden") {
    return (
      <button
        type="button"
        onClick={onToggleEnabled}
        className={cn(
          "inline-flex h-6 items-center gap-1 rounded-full border px-2.5 text-[11px] font-medium transition-colors",
          enabled
            ? "border-primary/40 bg-primary/12 text-primary hover:bg-primary/18"
            : "border-border/55 bg-transparent text-muted-foreground hover:text-foreground",
          className,
        )}
      >
        <Zap className="size-3" />
        <span>
          {enabled
            ? savingsBadge
              ? `Caveman · ${savingsBadge}`
              : `Caveman · ${levelInfo?.shortBadge ?? "Full"}`
            : "Caveman OFF"}
        </span>
      </button>
    );
  }

  if (variant === "kokonut-pills") {
    return (
      <div
        className={cn(
          "inline-flex items-center gap-0.5 rounded-full border border-border/65 bg-foreground/[0.03] p-0.5 text-[10.5px]",
          className,
        )}
      >
        <button
          type="button"
          onClick={onToggleEnabled}
          className={cn(
            "relative inline-flex h-5 items-center gap-1 rounded-full px-2 font-medium transition-colors",
            !enabled
              ? "bg-card text-foreground shadow-2xs"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          <Zap className={cn("size-2.5", enabled && "text-primary")} />
          <span>{enabled ? savingsBadge || "ON" : "OFF"}</span>
        </button>
        {QUICK_TOKEN_TIERS.map((tier) => {
          const isSelected = enabled && activeLevel === tier.slug;
          return (
            <button
              key={tier.slug}
              type="button"
              onClick={() => {
                if (!enabled) onToggleEnabled();
                onSelectLevel(tier.slug);
              }}
              className={cn(
                "relative inline-flex h-5 items-center gap-1 rounded-full px-2 font-medium transition-colors",
                isSelected
                  ? "text-primary"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {isSelected && (
                <motion.span
                  layoutId={`${groupId}-token-pill`}
                  transition={{ type: "spring", stiffness: 380, damping: 28 }}
                  className="pointer-events-none absolute inset-0 rounded-full border border-primary/35 bg-primary/14"
                />
              )}
              <span className="relative z-10">{tier.short}</span>
            </button>
          );
        })}
      </div>
    );
  }

  // Default: `compact-popover`
  return (
    <div className={cn("relative inline-flex items-center", className)}>
      <div className="inline-flex h-6 items-center overflow-hidden rounded-full border border-border/60 bg-foreground/[0.03]">
        <button
          type="button"
          onClick={onToggleEnabled}
          className={cn(
            "inline-flex h-full items-center gap-1 px-2.5 text-[11px] font-medium transition-colors",
            enabled
              ? "bg-primary/12 text-primary hover:bg-primary/18"
              : "text-muted-foreground hover:text-foreground",
          )}
          title="Toggle Caveman token compression"
        >
          <Zap className="size-3" />
          <span>
            {enabled
              ? savingsBadge
                ? `Caveman · ${savingsBadge}`
                : `Caveman · ${levelInfo?.shortBadge ?? "Full"}`
              : "Caveman OFF"}
          </span>
        </button>
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          aria-label="Choose token compression alternative"
          aria-expanded={open}
          className="inline-flex h-full items-center border-l border-border/55 px-1.5 font-mono text-[9.5px] text-muted-foreground transition-colors hover:bg-foreground/[0.05] hover:text-foreground"
        >
          {levelInfo?.shortBadge ?? "Full"}
        </button>
      </div>

      <AnimatePresence>
        {open && (
          <>
            <button
              type="button"
              aria-label="Close token alternatives"
              onClick={() => setOpen(false)}
              className="fixed inset-0 z-30 bg-transparent"
            />
            <motion.div
              initial={{ opacity: 0, y: 6, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 4, scale: 0.96 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className="absolute bottom-full left-0 z-40 mb-2 w-60 rounded-2xl border border-border/75 bg-popover/95 p-2 text-xs shadow-xl backdrop-blur-xl"
            >
              <div className="mb-1.5 flex items-center justify-between px-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                <span>Token Alternatives</span>
                <span className="font-mono text-primary">
                  {savingsBadge || "opencaveman"}
                </span>
              </div>
              <div className="space-y-1">
                {CAVEMAN_LEVEL_OPTIONS.map((opt) => {
                  const isCurrent = enabled && activeLevel === opt.slug;
                  return (
                    <button
                      key={opt.slug}
                      type="button"
                      onClick={() => {
                        if (!enabled) onToggleEnabled();
                        onSelectLevel(opt.slug);
                        setOpen(false);
                      }}
                      className={cn(
                        "flex w-full items-center justify-between gap-2 rounded-xl px-2.5 py-1.5 text-left transition-colors",
                        isCurrent
                          ? "border border-primary/30 bg-primary/12 text-foreground"
                          : "hover:bg-muted/60 text-muted-foreground hover:text-foreground",
                      )}
                    >
                      <div className="min-w-0">
                        <div className="font-medium text-foreground">
                          {opt.label}
                        </div>
                        <div className="truncate text-[10px] text-muted-foreground">
                          {opt.summary}
                        </div>
                      </div>
                      {isCurrent && (
                        <Check className="size-3.5 shrink-0 text-primary" />
                      )}
                    </button>
                  );
                })}
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 6. SpectrumUI Navigation List Card (`@spectrumui/nav-list-card`)           */
/* -------------------------------------------------------------------------- */

export type NavMenuStyleChoice =
  | "spectrum-card"
  | "minimal-rail"
  | "pill-list";

export interface NavMenuStyleDescriptor {
  id: NavMenuStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const NAV_MENU_STYLE_OPTIONS: ReadonlyArray<NavMenuStyleDescriptor> = [
  {
    id: "spectrum-card",
    title: "SpectrumUI Nav List Card (Default)",
    description:
      "@spectrumui/nav-list-card — Framed card navigation rows with spring active accent bar, icon well, and metadata badge.",
    isDefault: true,
  },
  {
    id: "minimal-rail",
    title: "Minimal Hairline Left Rail",
    description:
      "Clean left-border indicator list optimized for high-density studio sidebars.",
  },
  {
    id: "pill-list",
    title: "Soft Capsule Navigation Pills",
    description:
      "Rounded capsule rows with subtle primary tint on the active section.",
  },
];

export interface SpectrumNavItem {
  id: string;
  label: string;
  subtitle?: string;
  badge?: string;
  icon: React.ComponentType<{ className?: string }>;
}

export interface SpectrumNavListCardProps {
  items: ReadonlyArray<SpectrumNavItem>;
  activeId: string;
  onSelect: (id: string) => void;
  variant?: NavMenuStyleChoice;
  layoutGroupId?: string;
  className?: string;
}

export const SpectrumNavListCard: React.FC<SpectrumNavListCardProps> = ({
  items,
  activeId,
  onSelect,
  variant = "spectrum-card",
  layoutGroupId = "spectrum-nav",
  className,
}) => {
  const reduce = useReducedMotion() ?? false;

  return (
    <div
      className={cn(
        variant === "spectrum-card"
          ? "space-y-1 rounded-2xl border border-border/60 bg-card/55 p-1.5"
          : variant === "minimal-rail"
            ? "space-y-0.5 border-l border-border/55 pl-1.5"
            : "space-y-1",
        className,
      )}
    >
      {items.map((item) => {
        const Icon = item.icon;
        const active = item.id === activeId;
        return (
          <button
            key={item.id}
            type="button"
            onClick={() => onSelect(item.id)}
            className={cn(
              "group relative flex w-full items-center gap-2.5 text-left text-xs transition-colors",
              variant === "spectrum-card" && "rounded-xl px-2.5 py-2",
              variant === "minimal-rail" && "rounded-r-lg px-2.5 py-1.5",
              variant === "pill-list" && "rounded-full px-3 py-1.5",
              active
                ? "font-semibold text-foreground"
                : "text-muted-foreground hover:bg-foreground/[0.04] hover:text-foreground",
            )}
          >
            {active &&
              (variant === "spectrum-card" ? (
                reduce ? (
                  <span className="pointer-events-none absolute inset-0 rounded-xl border border-primary/30 bg-primary/12 shadow-2xs" />
                ) : (
                  <motion.span
                    layoutId={`${layoutGroupId}-card-bg`}
                    transition={{ type: "spring", stiffness: 380, damping: 30 }}
                    className="pointer-events-none absolute inset-0 rounded-xl border border-primary/30 bg-primary/12 shadow-2xs"
                  />
                )
              ) : variant === "minimal-rail" ? (
                <span className="pointer-events-none absolute -left-[7px] top-1.5 bottom-1.5 w-[2.5px] rounded-full bg-primary" />
              ) : (
                <span className="pointer-events-none absolute inset-0 rounded-full border border-primary/30 bg-primary/14" />
              ))}

            {variant === "spectrum-card" && active && (
              <span className="pointer-events-none absolute left-1 top-2 bottom-2 z-10 w-[3px] rounded-full bg-primary" />
            )}

            <span
              className={cn(
                "relative z-10 inline-flex shrink-0 items-center justify-center transition-colors",
                variant === "spectrum-card"
                  ? cn(
                      "size-6 rounded-lg border",
                      active
                        ? "border-primary/35 bg-primary/15 text-primary"
                        : "border-border/55 bg-background/65 text-muted-foreground group-hover:text-foreground",
                    )
                  : cn("size-4", active && "text-primary"),
              )}
            >
              <Icon className="size-3.5" />
            </span>

            <span className="relative z-10 min-w-0 flex-1">
              <span className="block truncate">{item.label}</span>
              {item.subtitle && variant === "spectrum-card" && (
                <span className="block truncate font-mono text-[9.5px] font-normal text-muted-foreground/80">
                  {item.subtitle}
                </span>
              )}
            </span>

            {item.badge && (
              <span className="relative z-10 shrink-0 rounded-full border border-border/55 bg-background/70 px-1.5 py-0 font-mono text-[9.5px] text-muted-foreground">
                {item.badge}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 7. SpectrumUI Status Badge (`@spectrumui/status-badge`)                    */
/* -------------------------------------------------------------------------- */

export type SpectrumStatusTone =
  | "online"
  | "running"
  | "warning"
  | "error"
  | "idle";

const STATUS_TONE_CLASSES: Record<
  SpectrumStatusTone,
  { wrap: string; dot: string; pulse: boolean }
> = {
  online: {
    wrap: "border-emerald-500/30 bg-emerald-500/10 text-emerald-500",
    dot: "bg-emerald-500",
    pulse: true,
  },
  running: {
    wrap: "border-primary/35 bg-primary/12 text-primary",
    dot: "bg-primary",
    pulse: true,
  },
  warning: {
    wrap: "border-amber-500/35 bg-amber-500/12 text-amber-500",
    dot: "bg-amber-500",
    pulse: true,
  },
  error: {
    wrap: "border-destructive/35 bg-destructive/12 text-destructive",
    dot: "bg-destructive",
    pulse: false,
  },
  idle: {
    wrap: "border-border/60 bg-foreground/[0.04] text-muted-foreground",
    dot: "bg-muted-foreground/65",
    pulse: false,
  },
};

export interface SpectrumStatusBadgeProps {
  tone?: SpectrumStatusTone;
  label: React.ReactNode;
  className?: string;
}

export const SpectrumStatusBadge: React.FC<SpectrumStatusBadgeProps> = ({
  tone = "online",
  label,
  className,
}) => {
  const cfg = STATUS_TONE_CLASSES[tone];
  return (
    <span
      className={cn(
        "openz-badge inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[10.5px] font-medium tracking-tight",
        cfg.wrap,
        className,
      )}
    >
      <span className="relative inline-flex size-1.5 shrink-0 items-center justify-center">
        {cfg.pulse && (
          <span
            className={cn(
              "absolute size-2.5 animate-ping rounded-full opacity-45",
              cfg.dot,
            )}
          />
        )}
        <span className={cn("relative size-1.5 rounded-full", cfg.dot)} />
      </span>
      <span className="truncate">{label}</span>
    </span>
  );
};

/* -------------------------------------------------------------------------- */
/* 8. Tweenly AI Thinking & Assistant-UI Loading State (`@tweenly/ai-thinking`,*/
/*    `@kokonutui/ai-loading`, `assistant-ui/elements/loading-state`)         */
/* -------------------------------------------------------------------------- */

export const TweenlyAiThinkingWave: React.FC<{
  size?: number;
  className?: string;
}> = ({ size = 16, className }) => {
  return (
    <span
      style={{ height: size }}
      className={cn(
        "inline-flex shrink-0 items-center gap-[3px] text-primary",
        className,
      )}
    >
      {[0, 1, 2, 3].map((i) => (
        <motion.span
          key={i}
          style={{
            width: Math.max(2.5, Math.round(size * 0.16)),
            height: Math.max(2.5, Math.round(size * 0.16)),
          }}
          animate={{
            y: [0, -Math.max(3, Math.round(size * 0.24)), 0],
            scale: [0.85, 1.2, 0.85],
            opacity: [0.45, 1, 0.45],
          }}
          transition={{
            duration: 0.9,
            delay: i * 0.12,
            repeat: Infinity,
            ease: "easeInOut",
          }}
          className="rounded-full bg-current"
        />
      ))}
    </span>
  );
};

export type LoadingStateStyleChoice =
  | "assistant-shimmer"
  | "kokonut-ai-loading"
  | "tweenly-thinking";

export interface LoadingStateStyleDescriptor {
  id: LoadingStateStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const LOADING_STATE_STYLE_OPTIONS: ReadonlyArray<LoadingStateStyleDescriptor> =
  [
    {
      id: "assistant-shimmer",
      title: "Assistant-UI Structured Loading State (Default)",
      description:
        "assistant-ui/elements/loading-state — Clean shimmer status row with subtle skeleton preview bars while awaiting first tokens.",
      isDefault: true,
    },
    {
      id: "kokonut-ai-loading",
      title: "KokonutUI Cycling AI Task Loader",
      description:
        "@kokonutui/ai-loading — Smoothly slides live status phases vertically beside animated equalizer bars.",
    },
    {
      id: "tweenly-thinking",
      title: "Tweenly Fluid Wave Thinking",
      description:
        "@tweenly/ai-thinking — Harmonic 4-dot spring wave capsule with breathing status typography.",
    },
  ];

const KOKONUT_LOADING_PHASES = [
  "Analyzing prompt context…",
  "Scoping workspace memory…",
  "Synthesizing response…",
] as const;

export const AssistantLoadingStateElement: React.FC<{
  variant?: LoadingStateStyleChoice;
  label?: string;
  className?: string;
}> = ({ variant = "assistant-shimmer", label, className }) => {
  const [phaseIdx, setPhaseIdx] = React.useState(0);

  React.useEffect(() => {
    if (variant !== "kokonut-ai-loading") return undefined;
    const timer = window.setInterval(() => {
      setPhaseIdx((i) => (i + 1) % KOKONUT_LOADING_PHASES.length);
    }, 1900);
    return () => window.clearInterval(timer);
  }, [variant]);

  if (variant === "tweenly-thinking") {
    return (
      <div
        className={cn(
          "inline-flex select-none items-center gap-2.5 rounded-full border border-border/60 bg-foreground/[0.035] px-3 py-1.5 text-xs text-foreground/90",
          className,
        )}
      >
        <TweenlyAiThinkingWave size={15} />
        <span className="font-medium">{label || "Thinking…"}</span>
      </div>
    );
  }

  if (variant === "kokonut-ai-loading") {
    const activeText = label || KOKONUT_LOADING_PHASES[phaseIdx];
    return (
      <div
        className={cn(
          "inline-flex select-none items-center gap-2.5 rounded-xl border border-border/60 bg-card/80 px-3 py-1.5 text-xs shadow-2xs",
          className,
        )}
      >
        <span className="inline-flex h-4 items-center gap-[2px] text-primary">
          {[0.5, 1, 0.65, 0.9].map((h, i) => (
            <motion.span
              key={i}
              animate={{
                height: [`${Math.round(h * 35)}%`, `${Math.round(h * 100)}%`, `${Math.round(h * 35)}%`],
              }}
              transition={{
                duration: 0.9,
                delay: i * 0.1,
                repeat: Infinity,
                ease: "easeInOut",
              }}
              className="w-[2.5px] rounded-full bg-current"
            />
          ))}
        </span>
        <div className="relative h-4 overflow-hidden">
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={activeText}
              initial={{ opacity: 0, y: 7 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -7 }}
              transition={{ duration: 0.2, ease: "easeOut" }}
              className="block leading-4 font-medium text-foreground/90"
            >
              {activeText}
            </motion.span>
          </AnimatePresence>
        </div>
      </div>
    );
  }

  // Default: `assistant-shimmer` (`assistant-ui/elements/loading-state`)
  return (
    <div
      className={cn("flex w-full max-w-md select-none flex-col gap-2 py-1", className)}
    >
      <div className="inline-flex items-center gap-2 text-xs font-medium text-primary">
        <Sparkles className="size-3.5 animate-pulse" />
        <span className="animate-pulse text-foreground/90">
          {label || "Generating response…"}
        </span>
      </div>
      <div className="space-y-1.5">
        <div className="h-2.5 w-4/5 animate-pulse rounded-full bg-foreground/[0.07]" />
        <div className="h-2.5 w-3/5 animate-pulse rounded-full bg-foreground/[0.045]" />
      </div>
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 9. Assistant-UI Memory Chips (`assistant-ui/elements/memory-chips`)        */
/* -------------------------------------------------------------------------- */

export type MemoryChipsStyleChoice =
  | "assistant-chips"
  | "uiarc-pills"
  | "minimal-tags";

export interface MemoryChipsStyleDescriptor {
  id: MemoryChipsStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const MEMORY_CHIPS_STYLE_OPTIONS: ReadonlyArray<MemoryChipsStyleDescriptor> =
  [
    {
      id: "assistant-chips",
      title: "Assistant-UI Memory Chips (Default)",
      description:
        "assistant-ui/elements/memory-chips — Interactive memory recall & saved-fact capsules with action verb tags.",
      isDefault: true,
    },
    {
      id: "uiarc-pills",
      title: "UIArc Frosted Memory Pills",
      description:
        "@uiarc/badge — Specular frosted glass pills highlighting recalled knowledge entities and sources.",
    },
    {
      id: "minimal-tags",
      title: "Minimal Monospace Memory Tags",
      description:
        "Compact inline monospace memory markers with zero heavy borders.",
    },
  ];

export interface TurnMemoryChipItem {
  id: string;
  action: "saved" | "recalled" | "source";
  label: string;
  detail?: string;
}

const MEMORY_TOOL_NAMES = new Set([
  "smart_memory_store",
  "workflow_memory",
  "knowledge_source",
  "recall_memory",
  "working_memory",
  "create_entities",
  "add_observations",
]);

function readStrField(
  args: Record<string, unknown> | string | undefined,
  keys: ReadonlyArray<string>,
): string | undefined {
  const obj =
    typeof args === "object" && args !== null && !Array.isArray(args)
      ? args
      : undefined;
  if (!obj) return undefined;
  for (const k of keys) {
    const v = obj[k];
    if (typeof v === "string" && v.trim()) return v.trim();
  }
  return undefined;
}

export function extractTurnMemoryChips(
  tools: ReadonlyArray<ToolExecution>,
  notices?: ReadonlyArray<ActivityNotice>,
): TurnMemoryChipItem[] {
  const chips: TurnMemoryChipItem[] = [];
  const seen = new Set<string>();

  for (const tool of tools) {
    const lower = tool.name.toLowerCase();
    if (!MEMORY_TOOL_NAMES.has(lower) || tool.status === "error") continue;
    const actionVerb =
      readStrField(tool.args, ["action", "operation"])?.toLowerCase() ?? "";
    const isRecall =
      lower.includes("recall") ||
      actionVerb === "search" ||
      actionVerb === "query" ||
      actionVerb === "get" ||
      actionVerb === "list";
    const kind: TurnMemoryChipItem["action"] =
      lower === "knowledge_source"
        ? "source"
        : isRecall
          ? "recalled"
          : "saved";
    const subject =
      readStrField(tool.args, [
        "topic",
        "title",
        "key",
        "query",
        "fact",
        "content",
        "url",
      ]) ?? tool.name;
    const trimmedSubject =
      subject.length > 44 ? `${subject.slice(0, 41)}...` : subject;
    const dedupe = `${kind}:${trimmedSubject}`;
    if (seen.has(dedupe)) continue;
    seen.add(dedupe);
    chips.push({
      id: `mem-${tool.id}`,
      action: kind,
      label: trimmedSubject,
      detail: tool.name,
    });
  }

  for (const notice of notices ?? []) {
    if (notice.kind !== "memory" && notice.kind !== "source") continue;
    const kind: TurnMemoryChipItem["action"] =
      notice.kind === "source" ? "source" : "saved";
    const label = (notice.detail || notice.title).trim();
    const shortLabel = label.length > 48 ? `${label.slice(0, 45)}...` : label;
    const dedupe = `${kind}:${shortLabel}`;
    if (!shortLabel || seen.has(dedupe)) continue;
    seen.add(dedupe);
    chips.push({
      id: `mem-notice-${notice.id}`,
      action: kind,
      label: shortLabel,
      detail: notice.title,
    });
  }

  return chips.slice(0, 6);
}

export const MemoryChipsElement: React.FC<{
  chips: ReadonlyArray<TurnMemoryChipItem>;
  variant?: MemoryChipsStyleChoice;
  onInspectMemory?: () => void;
  className?: string;
}> = ({
  chips,
  variant = "assistant-chips",
  onInspectMemory,
  className,
}) => {
  if (chips.length === 0) return null;

  return (
    <div
      className={cn(
        "my-1.5 flex flex-wrap items-center gap-1.5 not-prose",
        className,
      )}
    >
      {chips.map((chip) => {
        const ChipIcon = chip.action === "source" ? Bookmark : Brain;
        const verb =
          chip.action === "recalled"
            ? "Recalled"
            : chip.action === "source"
              ? "Source"
              : "Saved";
        const interactive = Boolean(onInspectMemory);
        const Comp = interactive ? "button" : "span";

        return (
          <Comp
            key={chip.id}
            type={interactive ? "button" : undefined}
            onClick={interactive ? onInspectMemory : undefined}
            className={cn(
              "inline-flex max-w-full items-center gap-1.5 transition-colors",
              variant === "assistant-chips" &&
                "rounded-full border border-border/65 bg-card/80 px-2.5 py-0.5 text-[11px] text-foreground/85 shadow-2xs",
              variant === "uiarc-pills" &&
                "rounded-lg border border-primary/25 bg-primary/[0.07] px-2 py-0.5 text-[11px] text-foreground/90 backdrop-blur-md",
              variant === "minimal-tags" &&
                "rounded bg-foreground/[0.045] px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground",
              interactive && "cursor-pointer hover:border-primary/45 hover:text-foreground",
            )}
          >
            <ChipIcon className="size-3 shrink-0 text-primary" />
            <span className="font-semibold text-primary/90">{verb}</span>
            <span className="truncate text-muted-foreground">· {chip.label}</span>
          </Comp>
        );
      })}
    </div>
  );
};

/* -------------------------------------------------------------------------- */
/* 10. Notification Center Styles (`@uiarc/notification-center` &             */
/*     `@tweenly/notification-feed`)                                          */
/* -------------------------------------------------------------------------- */

export type NotificationCenterStyleChoice =
  | "uiarc-center"
  | "tweenly-feed"
  | "beui-minimal";

export interface NotificationCenterStyleDescriptor {
  id: NotificationCenterStyleChoice;
  title: string;
  description: string;
  isDefault?: boolean;
}

export const NOTIFICATION_CENTER_STYLE_OPTIONS: ReadonlyArray<NotificationCenterStyleDescriptor> =
  [
    {
      id: "uiarc-center",
      title: "UIArc Tabbed Notification Center (Default)",
      description:
        "@uiarc/notification-center — Segmented All / Alerts / Skills tabs, unread indicators, and frosted action cards.",
      isDefault: true,
    },
    {
      id: "tweenly-feed",
      title: "Tweenly Staggered Notification Feed",
      description:
        "@tweenly/notification-feed — Connected chronological activity rail with staggered spring item entrance.",
    },
    {
      id: "beui-minimal",
      title: "BEUI Minimal Popover List",
      description:
        "Compact top-right notification list with quiet card borders.",
    },
  ];
