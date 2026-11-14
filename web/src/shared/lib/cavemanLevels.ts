import { isRecord } from './format';

export type CavemanLevelSlug =
  | 'lite'
  | 'full'
  | 'ultra'
  | 'wenyan_lite'
  | 'wenyan_full'
  | 'wenyan_ultra';

export type CavemanScriptFamily = 'latin_controlled' | 'classical_wenyan';

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
    slug: 'wenyan_lite',
    label: 'Wenyan Lite — Semi-Classical',
    shortBadge: '文·Lite',
    targetSavingsPct: 62,
    scriptFamily: 'classical_wenyan',
    summary:
      'Concise semi-classical Chinese prose while keeping all code, paths, CLI flags, and symbols verbatim.',
    example: '組件重繪因 options 每幀新建引用，宜以 useMemo 緩存之。',
  },
  {
    slug: 'wenyan_full',
    label: 'Wenyan Full — Classical 文言',
    shortBadge: '文·Full',
    targetSavingsPct: 70,
    scriptFamily: 'classical_wenyan',
    summary:
      'Classical Literary Chinese syntax for prose paired with byte-exact ASCII code, identifiers, and commands.',
    example: 'auth 鑑權誤：過期判式反置，易 < 為 > 即癒。',
  },
  {
    slug: 'wenyan_ultra',
    label: 'Wenyan Ultra — Extreme 極簡文言',
    shortBadge: '文·Ultra',
    targetSavingsPct: 76,
    scriptFamily: 'classical_wenyan',
    summary:
      'Ultra-compact 4–8 character classical clauses paired with exact ASCII symbols for peak token density.',
    example: 'db.toml max_connections 隘→改 32。',
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
  wenyan_lite: 'wenyan_lite',
  wenyan: 'wenyan_full',
  wenyan_full: 'wenyan_full',
  classical: 'wenyan_full',
  megacave: 'wenyan_full',
  wenyan_ultra: 'wenyan_ultra',
  megacave_ultra: 'wenyan_ultra',
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
