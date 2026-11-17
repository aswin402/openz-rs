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
      'symbolic_lite',
      'symbolic_full',
      'symbolic_ultra',
    ]);
  });

  it('normalizes canonical slugs, hyphens, spaces, and upstream aliases without regex', () => {
    expect(normalizeCavemanLevel('lite')).toBe('lite');
    expect(normalizeCavemanLevel('ULTRACAVE')).toBe('ultra');
    expect(normalizeCavemanLevel('symbolic-lite')).toBe('symbolic_lite');
    expect(normalizeCavemanLevel('symbolic full')).toBe('symbolic_full');
    expect(normalizeCavemanLevel('megacave-ultra')).toBe('symbolic_ultra');
    expect(normalizeCavemanLevel('unknown_value')).toBe('full');
    expect(normalizeCavemanLevel(undefined)).toBe('full');
  });

  it('cycles forward and backward cleanly across all 6 tiers', () => {
    expect(cycleCavemanLevelSlug('full', true)).toBe('ultra');
    expect(cycleCavemanLevelSlug('symbolic_ultra', true)).toBe('lite');
    expect(cycleCavemanLevelSlug('lite', false)).toBe('symbolic_ultra');
    expect(getCavemanLevelOption('symbolic_full').targetSavingsPct).toBe(72);
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
