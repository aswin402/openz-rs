import { isRecord } from './format';

export type CavemanLevelSlug =
  | 'lite'
  | 'full'
  | 'ultra'
  | 'symbolic_lite'
  | 'symbolic_full'
  | 'symbolic_ultra';

export type CavemanScriptFamily = 'latin_controlled' | 'symbolic_notation';

export interface CavemanLevelOption {
  slug: CavemanLevelSlug;
  label: string;
  shortBadge: string;
  targetSavingsPct: number;
  scriptFamily: CavemanScriptFamily;
  summary: string;
  example: string;
}

export interface CavemanStatsSnapshot {
  sessionTurns: number;
  sessionTokensSaved: number;
  lifetimeTurns: number;
  lifetimeOutputTokensSaved: number;
  lifetimeCompressedFiles: number;
  lifetimeInputTokensSaved: number;
  lifetimeTotalTokensSaved: number;
  compactLifetimeBadge: string;
  compactSessionBadge: string;
  byLevelSaved: Record<string, number>;
}

export const CAVEMAN_LEVEL_OPTIONS: readonly CavemanLevelOption[] = [
  {
    slug: 'lite',
    label: 'Lite — Crisp Professional',
    shortBadge: 'Lite',
    targetSavingsPct: 31,
    scriptFamily: 'latin_controlled',
    summary:
      'Drops pleasantries, hedging, and filler while preserving complete grammatical sentences and articles.',
    example: 'Your component re-renders because options creates a new reference each render.',
  },
  {
    slug: 'full',
    label: 'Full — Smart Caveman',
    shortBadge: 'Full',
    targetSavingsPct: 57,
    scriptFamily: 'latin_controlled',
    summary:
      'Default smart caveman: drops articles and filler, uses tight active fragments, keeps 100% technical precision.',
    example: 'Bug in auth middleware: token expiry check inverted. Flip < to >.',
  },
  {
    slug: 'ultra',
    label: 'Ultra — Telegraphic Signal',
    shortBadge: 'Ultra',
    targetSavingsPct: 70,
    scriptFamily: 'latin_controlled',
    summary:
      'Maximum English compression: telegraphic signal tokens, standard engineering shorthand, zero connective prose.',
    example: 'DB pool timeout: max_connections too low -> raise 10 to 32 in db.toml.',
  },
  {
    slug: 'symbolic_lite',
    label: 'Symbolic Lite — Structured Shorthand',
    shortBadge: 'Sym·Lite',
    targetSavingsPct: 62,
    scriptFamily: 'symbolic_notation',
    summary:
      'Compact English shorthand using causal arrows and engineering abbreviations while keeping all code, paths, and flags verbatim.',
    example: 'Component re-renders each frame -> options allocates new ref; wrap in useMemo.',
  },
  {
    slug: 'symbolic_full',
    label: 'Symbolic Full — Causal Notation',
    shortBadge: 'Sym·Full',
    targetSavingsPct: 72,
    scriptFamily: 'symbolic_notation',
    summary:
      'Dense [CAUSE] -> [FIX] English causal notation paired with byte-exact ASCII code, identifiers, and commands.',
    example: '[CAUSE] auth middleware exp check inverted -> [FIX] flip < to >.',
  },
  {
    slug: 'symbolic_ultra',
    label: 'Symbolic Ultra — Atomic Delta',
    shortBadge: 'Sym·Ultra',
    targetSavingsPct: 76,
    scriptFamily: 'symbolic_notation',
    summary:
      'Ultra-compact 3–6 word English target: issue -> patch deltas paired with exact ASCII symbols for peak token density.',
    example: 'db.toml max_connections low -> set 32.',
  },
];

const CAVEMAN_ALIAS_MAP: Readonly<Record<string, CavemanLevelSlug>> = {
  lite: 'lite',
  light: 'lite',
  mild: 'lite',
  soft: 'lite',
  clean: 'lite',
  professional: 'lite',
  full: 'full',
  default: 'full',
  standard: 'full',
  classic: 'full',
  caveman: 'full',
  normal_caveman: 'full',
  ultra: 'ultra',
  ultracave: 'ultra',
  max: 'ultra',
  extreme: 'ultra',
  telegraphic: 'ultra',
  symbolic_lite: 'symbolic_lite',
  shorthand: 'symbolic_lite',
  symbolic: 'symbolic_full',
  symbolic_full: 'symbolic_full',
  notation: 'symbolic_full',
  megacave: 'symbolic_full',
  symbolic_ultra: 'symbolic_ultra',
  megacave_ultra: 'symbolic_ultra',
  atomic: 'symbolic_ultra',
};

function sanitizeLevelToken(raw: string): string {
  return raw.trim().toLowerCase().split('-').join('_').split(' ').join('_');
}

export function normalizeCavemanLevel(raw: unknown): CavemanLevelSlug {
  if (typeof raw !== 'string') return 'full';
  const token = sanitizeLevelToken(raw);
  return CAVEMAN_ALIAS_MAP[token] ?? 'full';
}

export function getCavemanLevelOption(raw: unknown): CavemanLevelOption {
  const slug = normalizeCavemanLevel(raw);
  return (
    CAVEMAN_LEVEL_OPTIONS.find((option) => option.slug === slug) ??
    CAVEMAN_LEVEL_OPTIONS[1]
  );
}

export function cycleCavemanLevelSlug(
  current: unknown,
  forward = true,
): CavemanLevelSlug {
  const slug = normalizeCavemanLevel(current);
  const idx = CAVEMAN_LEVEL_OPTIONS.findIndex((option) => option.slug === slug);
  const safeIdx = idx >= 0 ? idx : 1;
  const len = CAVEMAN_LEVEL_OPTIONS.length;
  const nextIdx = forward ? (safeIdx + 1) % len : (safeIdx + len - 1) % len;
  return CAVEMAN_LEVEL_OPTIONS[nextIdx].slug;
}

function finiteNonNegative(val: unknown): number {
  return typeof val === 'number' && Number.isFinite(val) && val >= 0 ? val : 0;
}

export function formatCompactTokenCount(tokens: number): string {
  const safe = Math.max(0, Math.floor(tokens));
  if (safe >= 1_000_000) return `${(safe / 1_000_000).toFixed(1)}M`;
  if (safe >= 1_000) return `${(safe / 1_000).toFixed(1)}k`;
  return String(safe);
}

export function normalizeCavemanStats(raw: unknown): CavemanStatsSnapshot | null {
  if (!isRecord(raw)) return null;
  const lifetimeOut = finiteNonNegative(
    raw.lifetimeOutputTokensSaved ?? raw.lifetime_output_tokens_saved,
  );
  const lifetimeIn = finiteNonNegative(
    raw.lifetimeInputTokensSaved ?? raw.lifetime_input_tokens_saved,
  );
  const totalLifetime =
    finiteNonNegative(raw.lifetimeTotalTokensSaved ?? raw.lifetime_total_tokens_saved) ||
    lifetimeOut + lifetimeIn;
  const sessionSaved = finiteNonNegative(
    raw.sessionTokensSaved ?? raw.session_tokens_saved,
  );

  const byLevelSaved: Record<string, number> = {};
  const rawMap = raw.byLevelSaved ?? raw.by_level_saved;
  if (isRecord(rawMap)) {
    for (const [key, value] of Object.entries(rawMap)) {
      byLevelSaved[key] = finiteNonNegative(value);
    }
  }

  return {
    sessionTurns: finiteNonNegative(raw.sessionTurns ?? raw.session_turns),
    sessionTokensSaved: sessionSaved,
    lifetimeTurns: finiteNonNegative(raw.lifetimeTurns ?? raw.lifetime_turns),
    lifetimeOutputTokensSaved: lifetimeOut,
    lifetimeCompressedFiles: finiteNonNegative(
      raw.lifetimeCompressedFiles ?? raw.lifetime_compressed_files,
    ),
    lifetimeInputTokensSaved: lifetimeIn,
    lifetimeTotalTokensSaved: totalLifetime,
    compactLifetimeBadge:
      typeof raw.compactLifetimeBadge === 'string' && raw.compactLifetimeBadge.trim()
        ? raw.compactLifetimeBadge
        : formatCompactTokenCount(totalLifetime),
    compactSessionBadge:
      typeof raw.compactSessionBadge === 'string' && raw.compactSessionBadge.trim()
        ? raw.compactSessionBadge
        : formatCompactTokenCount(sessionSaved),
    byLevelSaved,
  };
}

export function formatCavemanSavingsBadge(
  stats: CavemanStatsSnapshot | null | undefined,
): string {
  if (!stats || stats.lifetimeTotalTokensSaved <= 0) return '';
  return stats.compactLifetimeBadge || formatCompactTokenCount(stats.lifetimeTotalTokensSaved);
}
