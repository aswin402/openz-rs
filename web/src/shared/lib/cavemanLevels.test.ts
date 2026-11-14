import { describe, expect, it } from 'bun:test';
import {
  CAVEMAN_LEVEL_OPTIONS,
  cycleCavemanLevelSlug,
  formatCavemanSavingsBadge,
  getCavemanLevelOption,
  normalizeCavemanLevel,
  normalizeCavemanStats,
} from './cavemanLevels';

describe('cavemanLevels descriptor helpers', () => {
  it('exposes all 6 canonical opencaveman intensity tiers in order', () => {
    expect(CAVEMAN_LEVEL_OPTIONS.map((opt) => opt.slug)).toEqual([
      'lite',
      'full',
      'ultra',
      'wenyan_lite',
      'wenyan_full',
      'wenyan_ultra',
    ]);
  });

  it('normalizes canonical slugs, hyphens, spaces, and upstream aliases without regex', () => {
    expect(normalizeCavemanLevel('lite')).toBe('lite');
    expect(normalizeCavemanLevel('ULTRACAVE')).toBe('ultra');
    expect(normalizeCavemanLevel('wenyan-lite')).toBe('wenyan_lite');
    expect(normalizeCavemanLevel('wenyan full')).toBe('wenyan_full');
    expect(normalizeCavemanLevel('megacave-ultra')).toBe('wenyan_ultra');
    expect(normalizeCavemanLevel('unknown_value')).toBe('full');
    expect(normalizeCavemanLevel(undefined)).toBe('full');
  });

  it('cycles forward and backward cleanly across all 6 tiers', () => {
    expect(cycleCavemanLevelSlug('full', true)).toBe('ultra');
    expect(cycleCavemanLevelSlug('wenyan_ultra', true)).toBe('lite');
    expect(cycleCavemanLevelSlug('lite', false)).toBe('wenyan_ultra');
    expect(getCavemanLevelOption('wenyan_full').targetSavingsPct).toBe(70);
  });

  it('normalizes telemetry snapshots and formats non-zero badges', () => {
    const empty = normalizeCavemanStats({ lifetimeTotalTokensSaved: 0 });
    expect(empty).not.toBeNull();
    expect(formatCavemanSavingsBadge(empty)).toBe('');

    const active = normalizeCavemanStats({
      sessionTurns: 4,
      sessionTokensSaved: 420,
      lifetimeTurns: 19,
      lifetimeOutputTokensSaved: 2150,
      lifetimeInputTokensSaved: 650,
      lifetimeTotalTokensSaved: 2800,
      lifetimeCompressedFiles: 2,
      byLevelSaved: { full: 1600, ultra: 550 },
    });
    expect(active?.lifetimeTotalTokensSaved).toBe(2800);
    expect(formatCavemanSavingsBadge(active)).toBe('2.8k');
  });
});
