/**
 * Algorithmic Color & Dual-Mode Theme Engine (Zero-Regex).
 *
 * Computes WCAG 2.1 relative luminance, contrast ratios, HSL/RGB conversions,
 * contrast-enforced Light & Dark palettes for every OpenZ theme family,
 * syntax/diff semantic variables, and Golden-Angle categorical palettes.
 */

export type ThemeAppearanceMode = 'dark' | 'light';

export interface ResolvedThemePalette {
  id: string;
  name: string;
  mode: ThemeAppearanceMode;
  primaryHex: string;
  primaryForegroundHex: string;
  accentHex: string;
  backgroundHex: string;
  cardHex: string;
  sidebarHex: string;
  foregroundHex: string;
  mutedHex: string;
  mutedForegroundHex: string;
  borderHex: string;
  successHex: string;
  successForegroundHex: string;
  warningHex: string;
  warningForegroundHex: string;
  infoHex: string;
  infoForegroundHex: string;
  destructiveHex: string;
  destructiveForegroundHex: string;
  syntaxKeywordHex: string;
  syntaxStringHex: string;
  syntaxNumberHex: string;
  syntaxFunctionHex: string;
  syntaxCommentHex: string;
  syntaxTypeHex: string;
  diffAddHex: string;
  diffDelHex: string;
  chartPalette: [string, string, string, string, string];
}

export interface DualModeThemeFamily {
  id: string;
  name: string;
  description: string;
  dark: ResolvedThemePalette;
  light: ResolvedThemePalette;
}

interface SurfaceOverride {
  backgroundHex: string;
  cardHex: string;
  sidebarHex: string;
  foregroundHex: string;
  mutedHex: string;
  mutedForegroundHex: string;
  borderHex: string;
}

interface ThemeFamilySeed {
  id: string;
  name: string;
  description: string;
  surfaceHue: number;
  surfaceSatDark: number;
  surfaceSatLight: number;
  primaryDarkHex: string;
  primaryLightHex?: string;
  accentDarkHex: string;
  accentLightHex?: string;
  darkSurface?: Partial<SurfaceOverride>;
  lightSurface?: Partial<SurfaceOverride>;
}

const GOLDEN_ANGLE_DEG = 137.507764;

function clamp(value: number, min: number, max: number): number {
  if (value < min) return min;
  if (value > max) return max;
  return value;
}

function wrapHue(deg: number): number {
  const mod = deg % 360;
  return mod < 0 ? mod + 360 : mod;
}

function hexNibble(code: number): number {
  if (code >= 48 && code <= 57) return code - 48;
  if (code >= 65 && code <= 70) return code - 55;
  if (code >= 97 && code <= 102) return code - 87;
  return 0;
}

function byteToHex(value: number): string {
  const clamped = Math.round(clamp(value, 0, 255));
  const hex = clamped.toString(16);
  return hex.length === 1 ? '0' + hex : hex;
}

export function parseHexRgb(hex: string): [number, number, number] {
  const raw = hex.trim().startsWith('#') ? hex.trim().slice(1) : hex.trim();
  if (raw.length === 3 || raw.length === 4) {
    const r = hexNibble(raw.charCodeAt(0)) * 17;
    const g = hexNibble(raw.charCodeAt(1)) * 17;
    const b = hexNibble(raw.charCodeAt(2)) * 17;
    return [r, g, b];
  }
  if (raw.length >= 6) {
    const r = hexNibble(raw.charCodeAt(0)) * 16 + hexNibble(raw.charCodeAt(1));
    const g = hexNibble(raw.charCodeAt(2)) * 16 + hexNibble(raw.charCodeAt(3));
    const b = hexNibble(raw.charCodeAt(4)) * 16 + hexNibble(raw.charCodeAt(5));
    return [r, g, b];
  }
  return [0, 0, 0];
}

export function rgbToHex(r: number, g: number, b: number): string {
  return '#' + byteToHex(r) + byteToHex(g) + byteToHex(b);
}

export function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  const rn = clamp(r, 0, 255) / 255;
  const gn = clamp(g, 0, 255) / 255;
  const bn = clamp(b, 0, 255) / 255;

  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const delta = max - min;
  const l = (max + min) / 2;

  if (delta === 0) {
    return [0, 0, l * 100];
  }

  const s = delta / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (max === rn) {
    h = 60 * (((gn - bn) / delta) % 6);
  } else if (max === gn) {
    h = 60 * ((bn - rn) / delta + 2);
  } else {
    h = 60 * ((rn - gn) / delta + 4);
  }

  return [wrapHue(h), clamp(s * 100, 0, 100), clamp(l * 100, 0, 100)];
}

export function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const hn = wrapHue(h);
  const sn = clamp(s, 0, 100) / 100;
  const ln = clamp(l, 0, 100) / 100;

  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const x = c * (1 - Math.abs(((hn / 60) % 2) - 1));
  const m = ln - c / 2;

  let r1 = 0;
  let g1 = 0;
  let b1 = 0;

  if (hn < 60) {
    r1 = c;
    g1 = x;
  } else if (hn < 120) {
    r1 = x;
    g1 = c;
  } else if (hn < 180) {
    g1 = c;
    b1 = x;
  } else if (hn < 240) {
    g1 = x;
    b1 = c;
  } else if (hn < 300) {
    r1 = x;
    b1 = c;
  } else {
    r1 = c;
    b1 = x;
  }

  return [
    Math.round((r1 + m) * 255),
    Math.round((g1 + m) * 255),
    Math.round((b1 + m) * 255),
  ];
}

export function hexToHsl(hex: string): [number, number, number] {
  const [r, g, b] = parseHexRgb(hex);
  return rgbToHsl(r, g, b);
}

export function hslToHex(h: number, s: number, l: number): string {
  const [r, g, b] = hslToRgb(h, s, l);
  return rgbToHex(r, g, b);
}

export function mixHex(hexA: string, hexB: string, weightB: number): string {
  const w = clamp(weightB, 0, 1);
  const [r1, g1, b1] = parseHexRgb(hexA);
  const [r2, g2, b2] = parseHexRgb(hexB);
  return rgbToHex(
    r1 + (r2 - r1) * w,
    g1 + (g2 - g1) * w,
    b1 + (b2 - b1) * w,
  );
}

export function withHexAlpha(hex: string, alpha: number): string {
  const [r, g, b] = parseHexRgb(hex);
  const a = Math.round(clamp(alpha, 0, 1) * 1000) / 1000;
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

function srgbChannelToLinear(channel255: number): number {
  const c = clamp(channel255, 0, 255) / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

export function relativeLuminance(hex: string): number {
  const [r, g, b] = parseHexRgb(hex);
  const rl = srgbChannelToLinear(r);
  const gl = srgbChannelToLinear(g);
  const bl = srgbChannelToLinear(b);
  return 0.2126 * rl + 0.7152 * gl + 0.0722 * bl;
}

export function contrastRatio(hexA: string, hexB: string): number {
  const l1 = relativeLuminance(hexA);
  const l2 = relativeLuminance(hexB);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

export function pickReadableForeground(
  bgHex: string,
  darkInkHex = '#09090b',
  lightInkHex = '#ffffff',
): string {
  const darkRatio = contrastRatio(bgHex, darkInkHex);
  const lightRatio = contrastRatio(bgHex, lightInkHex);
  return lightRatio >= darkRatio ? lightInkHex : darkInkHex;
}

export function ensureContrast(
  fgHex: string,
  bgHex: string,
  minRatio = 4.2,
): string {
  if (contrastRatio(fgHex, bgHex) >= minRatio) {
    return fgHex;
  }
  const bgLum = relativeLuminance(bgHex);
  const [h, s, initialL] = hexToHsl(fgHex);
  const step = bgLum > 0.35 ? -2.5 : 2.5;
  let l = initialL;
  let candidate = fgHex;
  for (let i = 0; i < 36; i += 1) {
    l = clamp(l + step, 6, 94);
    candidate = hslToHex(h, s, l);
    if (contrastRatio(candidate, bgHex) >= minRatio) {
      return candidate;
    }
  }
  return candidate;
}

export function deriveCategoricalPalette(
  anchorHex: string,
  mode: ThemeAppearanceMode,
  count: number,
): string[] {
  const [baseHue, baseSat] = hexToHsl(anchorHex);
  const sat = clamp(Math.max(baseSat, 64), 58, 84);
  const baseLight = mode === 'dark' ? 66 : 44;
  const result: string[] = [];
  for (let i = 0; i < count; i += 1) {
    const hue = wrapHue(baseHue + i * GOLDEN_ANGLE_DEG);
    const lightnessJitter = ((i % 3) - 1) * (mode === 'dark' ? 4 : 3);
    result.push(hslToHex(hue, sat, clamp(baseLight + lightnessJitter, 28, 78)));
  }
  return result;
}

function buildSurfaceForMode(
  seed: ThemeFamilySeed,
  mode: ThemeAppearanceMode,
): SurfaceOverride {
  if (mode === 'dark') {
    const h = seed.surfaceHue;
    const s = seed.surfaceSatDark;
    const defaults: SurfaceOverride = {
      backgroundHex: hslToHex(h, s, 5),
      sidebarHex: hslToHex(h, s, 7),
      cardHex: hslToHex(h, s, 9.5),
      mutedHex: hslToHex(h, clamp(s + 2, 0, 38), 14.5),
      borderHex: hslToHex(h, clamp(s + 2, 0, 38), 19.5),
      foregroundHex: hslToHex(h, clamp(s * 0.6, 0, 28), 96),
      mutedForegroundHex: hslToHex(h, clamp(s * 0.55, 0, 24), 66),
    };
    return { ...defaults, ...seed.darkSurface };
  }

  const h = seed.surfaceHue;
  const s = seed.surfaceSatLight;
  const defaults: SurfaceOverride = {
    backgroundHex: hslToHex(h, s, 97.5),
    cardHex: '#ffffff',
    sidebarHex: hslToHex(h, clamp(s + 4, 0, 42), 94.5),
    mutedHex: hslToHex(h, clamp(s + 5, 0, 42), 91),
    borderHex: hslToHex(h, clamp(s + 6, 0, 40), 84),
    foregroundHex: hslToHex(h, clamp(s + 10, 0, 38), 10),
    mutedForegroundHex: hslToHex(h, clamp(s + 6, 0, 26), 41),
  };
  return { ...defaults, ...seed.lightSurface };
}

function deriveLightAccentFromDark(darkHex: string, bgHex: string): string {
  const [h, s, l] = hexToHsl(darkHex);
  if (s < 10) {
    return ensureContrast(hslToHex(h, s, 18), bgHex, 4.8);
  }
  const targetL = clamp(l > 54 ? 43 : l, 32, 48);
  const targetS = clamp(s < 55 ? s + 14 : s, 48, 92);
  return ensureContrast(hslToHex(h, targetS, targetL), bgHex, 4.3);
}

function buildResolvedPalette(
  seed: ThemeFamilySeed,
  mode: ThemeAppearanceMode,
): ResolvedThemePalette {
  const surface = buildSurfaceForMode(seed, mode);
  const bg = surface.backgroundHex;
  const cardBg = surface.cardHex;

  const rawPrimary =
    mode === 'dark'
      ? seed.primaryDarkHex
      : (seed.primaryLightHex ?? deriveLightAccentFromDark(seed.primaryDarkHex, cardBg));
  const primaryHex =
    mode === 'dark'
      ? ensureContrast(rawPrimary, bg, 3.4)
      : ensureContrast(rawPrimary, cardBg, 4.0);

  const rawAccent =
    mode === 'dark'
      ? seed.accentDarkHex
      : (seed.accentLightHex ?? deriveLightAccentFromDark(seed.accentDarkHex, cardBg));
  const accentHex =
    mode === 'dark'
      ? ensureContrast(rawAccent, bg, 3.2)
      : ensureContrast(rawAccent, cardBg, 3.8);

  const primaryForegroundHex = pickReadableForeground(primaryHex, '#09090b', '#ffffff');

  // Semantic status tokens tuned for perceptual clarity in both Dark and Light modes
  const successHex = ensureContrast(
    mode === 'dark' ? '#22c55e' : '#059669',
    cardBg,
    mode === 'dark' ? 4.2 : 4.2,
  );
  const warningHex = ensureContrast(
    mode === 'dark' ? '#f59e0b' : '#b45309',
    cardBg,
    mode === 'dark' ? 4.4 : 4.3,
  );
  const infoHex = ensureContrast(
    mode === 'dark' ? '#38bdf8' : '#0284c7',
    cardBg,
    mode === 'dark' ? 4.4 : 4.3,
  );
  const destructiveHex = ensureContrast(
    mode === 'dark' ? '#f43f5e' : '#dc2626',
    cardBg,
    mode === 'dark' ? 4.1 : 4.3,
  );

  // Syntax & diff tokens harmonized with active surface luminance & primary hue
  const [pHue] = hexToHsl(primaryHex);
  const kwHue = wrapHue(pHue > 200 && pHue < 310 ? pHue : 282);
  const fnHue = wrapHue(kwHue - 68);
  const strHue = 148;
  const numHue = 32;
  const typeHue = 194;

  const syntaxKeywordHex = ensureContrast(
    hslToHex(kwHue, mode === 'dark' ? 82 : 72, mode === 'dark' ? 74 : 42),
    cardBg,
    4.3,
  );
  const syntaxFunctionHex = ensureContrast(
    hslToHex(fnHue, mode === 'dark' ? 84 : 76, mode === 'dark' ? 72 : 40),
    cardBg,
    4.3,
  );
  const syntaxStringHex = ensureContrast(
    hslToHex(strHue, mode === 'dark' ? 66 : 72, mode === 'dark' ? 68 : 34),
    cardBg,
    4.3,
  );
  const syntaxNumberHex = ensureContrast(
    hslToHex(numHue, mode === 'dark' ? 88 : 82, mode === 'dark' ? 68 : 39),
    cardBg,
    4.3,
  );
  const syntaxTypeHex = ensureContrast(
    hslToHex(typeHue, mode === 'dark' ? 78 : 76, mode === 'dark' ? 68 : 37),
    cardBg,
    4.3,
  );
  const syntaxCommentHex = mixHex(surface.mutedForegroundHex, surface.cardHex, 0.12);

  const diffAddHex = successHex;
  const diffDelHex = destructiveHex;

  const cat = deriveCategoricalPalette(primaryHex, mode, 5);
  const chartPalette: [string, string, string, string, string] = [
    cat[0],
    cat[1],
    cat[2],
    cat[3],
    cat[4],
  ];

  return {
    id: seed.id,
    name: seed.name,
    mode,
    primaryHex,
    primaryForegroundHex,
    accentHex,
    backgroundHex: surface.backgroundHex,
    cardHex: surface.cardHex,
    sidebarHex: surface.sidebarHex,
    foregroundHex: surface.foregroundHex,
    mutedHex: surface.mutedHex,
    mutedForegroundHex: surface.mutedForegroundHex,
    borderHex: surface.borderHex,
    successHex,
    successForegroundHex: pickReadableForeground(successHex),
    warningHex,
    warningForegroundHex: pickReadableForeground(warningHex),
    infoHex,
    infoForegroundHex: pickReadableForeground(infoHex),
    destructiveHex,
    destructiveForegroundHex: pickReadableForeground(destructiveHex),
    syntaxKeywordHex,
    syntaxStringHex,
    syntaxNumberHex,
    syntaxFunctionHex,
    syntaxCommentHex,
    syntaxTypeHex,
    diffAddHex,
    diffDelHex,
    chartPalette,
  };
}

const THEME_FAMILY_SEEDS: ThemeFamilySeed[] = [
  {
    id: 'obsidian-ember',
    name: 'Crimson Fox (Default)',
    description: 'Signature OpenZ crimson-orange fox brand in obsidian dark & warm daylight',
    surfaceHue: 240,
    surfaceSatDark: 8,
    surfaceSatLight: 16,
    primaryDarkHex: '#ff4f17',
    primaryLightHex: '#e0410c',
    accentDarkHex: '#ea580c',
    accentLightHex: '#c2410c',
    darkSurface: {
      backgroundHex: '#09090b',
      cardHex: '#121215',
      sidebarHex: '#0d0d10',
      foregroundHex: '#fafafa',
      mutedHex: '#1c1c22',
      mutedForegroundHex: '#a1a1aa',
      borderHex: '#27272a',
    },
    lightSurface: {
      backgroundHex: '#fcfbfa',
      cardHex: '#ffffff',
      sidebarHex: '#f5f3ef',
      foregroundHex: '#18181b',
      mutedHex: '#eeebe5',
      mutedForegroundHex: '#63636b',
      borderHex: '#e2ded7',
    },
  },
  {
    id: 'amber-gold',
    name: 'Obsidian Amber',
    description: 'Warm golden amber accents paired with deep zinc dark & sunlit parchment light',
    surfaceHue: 36,
    surfaceSatDark: 10,
    surfaceSatLight: 24,
    primaryDarkHex: '#f59e0b',
    primaryLightHex: '#b45309',
    accentDarkHex: '#ea580c',
    accentLightHex: '#c2410c',
    darkSurface: {
      backgroundHex: '#09090b',
      cardHex: '#121215',
      sidebarHex: '#0d0d10',
      foregroundHex: '#fafafa',
      mutedHex: '#1c1c22',
      mutedForegroundHex: '#a1a1aa',
      borderHex: '#27272a',
    },
  },
  {
    id: 'midnight-zinc',
    name: 'Zinc Monochrome',
    description: 'Pure architectural zinc contrast in both pitch dark and crisp studio white',
    surfaceHue: 240,
    surfaceSatDark: 5,
    surfaceSatLight: 5,
    primaryDarkHex: '#e4e4e7',
    primaryLightHex: '#18181b',
    accentDarkHex: '#a1a1aa',
    accentLightHex: '#3f3f46',
    darkSurface: {
      backgroundHex: '#09090b',
      cardHex: '#121214',
      sidebarHex: '#0c0c0e',
      foregroundHex: '#fafafa',
      mutedHex: '#1c1c1f',
      mutedForegroundHex: '#a1a1aa',
      borderHex: '#27272a',
    },
    lightSurface: {
      backgroundHex: '#fafafa',
      cardHex: '#ffffff',
      sidebarHex: '#f4f4f5',
      foregroundHex: '#09090b',
      mutedHex: '#e4e4e7',
      mutedForegroundHex: '#52525b',
      borderHex: '#d4d4d8',
    },
  },
  {
    id: 'tokyo-night',
    name: 'Tokyo Storm & Day',
    description: 'Electric indigo-violet Tokyo Night dark paired with Tokyo Day slate light',
    surfaceHue: 232,
    surfaceSatDark: 28,
    surfaceSatLight: 26,
    primaryDarkHex: '#7aa2f7',
    primaryLightHex: '#2e5cb8',
    accentDarkHex: '#bb9af7',
    accentLightHex: '#6d44b8',
    darkSurface: {
      backgroundHex: '#16161e',
      cardHex: '#1f2335',
      sidebarHex: '#1a1b26',
      foregroundHex: '#c0caf5',
      mutedHex: '#292e42',
      mutedForegroundHex: '#8992b8',
      borderHex: '#2f3549',
    },
    lightSurface: {
      backgroundHex: '#eef0f7',
      cardHex: '#ffffff',
      sidebarHex: '#e4e7f2',
      foregroundHex: '#1f2335',
      mutedHex: '#d8dceb',
      mutedForegroundHex: '#565f89',
      borderHex: '#c7cde2',
    },
  },
  {
    id: 'catppuccin-mocha',
    name: 'Catppuccin Mocha & Latte',
    description: 'Soothing pastel lavender & sapphire across Mocha dark and Latte light',
    surfaceHue: 240,
    surfaceSatDark: 23,
    surfaceSatLight: 24,
    primaryDarkHex: '#cba6f7',
    primaryLightHex: '#7c3aed',
    accentDarkHex: '#89b4fa',
    accentLightHex: '#1e66f5',
    darkSurface: {
      backgroundHex: '#1e1e2e',
      cardHex: '#262639',
      sidebarHex: '#181825',
      foregroundHex: '#cdd6f4',
      mutedHex: '#313244',
      mutedForegroundHex: '#a6adc8',
      borderHex: '#3b3d54',
    },
    lightSurface: {
      backgroundHex: '#eff1f5',
      cardHex: '#ffffff',
      sidebarHex: '#e6e9ef',
      foregroundHex: '#232634',
      mutedHex: '#dce0e8',
      mutedForegroundHex: '#5c5f77',
      borderHex: '#ccd0da',
    },
  },
  {
    id: 'emerald-forest',
    name: 'Emerald Forest',
    description: 'Botanical emerald canopy in deep pine dark and fresh meadow light',
    surfaceHue: 156,
    surfaceSatDark: 34,
    surfaceSatLight: 22,
    primaryDarkHex: '#10b981',
    primaryLightHex: '#047857',
    accentDarkHex: '#34d399',
    accentLightHex: '#059669',
    darkSurface: {
      backgroundHex: '#080f0c',
      cardHex: '#101c17',
      sidebarHex: '#0b1410',
      foregroundHex: '#ecfdf5',
      mutedHex: '#162921',
      mutedForegroundHex: '#86efac',
      borderHex: '#1e3a2f',
    },
  },
  {
    id: 'nordic-frost',
    name: 'Nordic Polar & Snow',
    description: 'Arctic cyan-glacier accents across Polar Night dark and Snow Storm light',
    surfaceHue: 220,
    surfaceSatDark: 17,
    surfaceSatLight: 24,
    primaryDarkHex: '#88c0d0',
    primaryLightHex: '#0369a1',
    accentDarkHex: '#81a1c1',
    accentLightHex: '#2563eb',
    darkSurface: {
      backgroundHex: '#242933',
      cardHex: '#2e3440',
      sidebarHex: '#20252e',
      foregroundHex: '#eceff4',
      mutedHex: '#3b4252',
      mutedForegroundHex: '#9aa5b8',
      borderHex: '#434c5e',
    },
    lightSurface: {
      backgroundHex: '#f3f6fa',
      cardHex: '#ffffff',
      sidebarHex: '#e8eef5',
      foregroundHex: '#1e2430',
      mutedHex: '#dde5f0',
      mutedForegroundHex: '#4c566a',
      borderHex: '#cbd5e1',
    },
  },
  {
    id: 'rose-pine',
    name: 'Rosé Pine & Dawn',
    description: 'Velvet blush & warm gold in Soho Rosé Pine dark and Rosé Pine Dawn light',
    surfaceHue: 248,
    surfaceSatDark: 24,
    surfaceSatLight: 28,
    primaryDarkHex: '#ebbcba',
    primaryLightHex: '#b44c58',
    accentDarkHex: '#f6c177',
    accentLightHex: '#b86e14',
    darkSurface: {
      backgroundHex: '#191724',
      cardHex: '#1f1d2e',
      sidebarHex: '#16141f',
      foregroundHex: '#e0def4',
      mutedHex: '#26233a',
      mutedForegroundHex: '#908caa',
      borderHex: '#332f4c',
    },
    lightSurface: {
      backgroundHex: '#faf4ed',
      cardHex: '#fffaf3',
      sidebarHex: '#f2e9e1',
      foregroundHex: '#28243d',
      mutedHex: '#ebe0d6',
      mutedForegroundHex: '#6e6a86',
      borderHex: '#dfdad9',
    },
  },
  {
    id: 'dracula-pro',
    name: 'Dracula & Alucard',
    description: 'Vibrant orchid & magenta in classic Dracula dark and crisp Alucard light',
    surfaceHue: 232,
    surfaceSatDark: 16,
    surfaceSatLight: 20,
    primaryDarkHex: '#bd93f9',
    primaryLightHex: '#6c3ce9',
    accentDarkHex: '#ff79c6',
    accentLightHex: '#c02678',
    darkSurface: {
      backgroundHex: '#191a21',
      cardHex: '#222430',
      sidebarHex: '#15161c',
      foregroundHex: '#f8f8f2',
      mutedHex: '#2c2f3f',
      mutedForegroundHex: '#939bc2',
      borderHex: '#383c52',
    },
  },
  {
    id: 'alabaster-light',
    name: 'Alabaster Terracotta',
    description: 'Editorial warm stone paper light & charred terracotta dark',
    surfaceHue: 36,
    surfaceSatDark: 12,
    surfaceSatLight: 22,
    primaryDarkHex: '#fb923c',
    primaryLightHex: '#c2410c',
    accentDarkHex: '#ff5722',
    accentLightHex: '#ea580c',
    lightSurface: {
      backgroundHex: '#faf9f6',
      cardHex: '#ffffff',
      sidebarHex: '#f3f1ec',
      foregroundHex: '#18181b',
      mutedHex: '#edeae3',
      mutedForegroundHex: '#5f636e',
      borderHex: '#e2ded5',
    },
  },
  {
    id: 'frost-daylight',
    name: 'Cobalt Sapphire',
    description: 'Precision engineering cobalt blue in crisp daylight and deep navy night',
    surfaceHue: 215,
    surfaceSatDark: 28,
    surfaceSatLight: 32,
    primaryDarkHex: '#60a5fa',
    primaryLightHex: '#1d4ed8',
    accentDarkHex: '#38bdf8',
    accentLightHex: '#0284c7',
    lightSurface: {
      backgroundHex: '#f8fafc',
      cardHex: '#ffffff',
      sidebarHex: '#f1f5f9',
      foregroundHex: '#0f172a',
      mutedHex: '#e2e8f0',
      mutedForegroundHex: '#475569',
      borderHex: '#cbd5e1',
    },
  },
  {
    id: 'matcha-latte',
    name: 'Matcha Sage',
    description: 'Calming organic Kyoto matcha green in serene daylight and deep moss night',
    surfaceHue: 138,
    surfaceSatDark: 22,
    surfaceSatLight: 18,
    primaryDarkHex: '#4ade80',
    primaryLightHex: '#047857',
    accentDarkHex: '#34d399',
    accentLightHex: '#059669',
    lightSurface: {
      backgroundHex: '#f6f8f5',
      cardHex: '#ffffff',
      sidebarHex: '#eef2ec',
      foregroundHex: '#14241b',
      mutedHex: '#e2eae0',
      mutedForegroundHex: '#4b6355',
      borderHex: '#cfdcd0',
    },
  },
  {
    id: 'sakura-snow',
    name: 'Sakura Rose',
    description: 'Refined crimson-blossom rose in petal snow light and midnight plum dark',
    surfaceHue: 344,
    surfaceSatDark: 22,
    surfaceSatLight: 30,
    primaryDarkHex: '#fb7185',
    primaryLightHex: '#be123c',
    accentDarkHex: '#f472b6',
    accentLightHex: '#db2777',
    lightSurface: {
      backgroundHex: '#fffafa',
      cardHex: '#ffffff',
      sidebarHex: '#fdf2f4',
      foregroundHex: '#1f1216',
      mutedHex: '#fce7eb',
      mutedForegroundHex: '#6b4c55',
      borderHex: '#f3d1d8',
    },
  },
];

export const THEME_FAMILIES: DualModeThemeFamily[] = THEME_FAMILY_SEEDS.map((seed) => ({
  id: seed.id,
  name: seed.name,
  description: seed.description,
  dark: buildResolvedPalette(seed, 'dark'),
  light: buildResolvedPalette(seed, 'light'),
}));

export function resolveThemeFamily(themeId: string): DualModeThemeFamily {
  return THEME_FAMILIES.find((family) => family.id === themeId) ?? THEME_FAMILIES[0];
}

export function resolveDualModePalette(
  themeId: string,
  mode: ThemeAppearanceMode,
): ResolvedThemePalette {
  const family = resolveThemeFamily(themeId);
  return mode === 'light' ? family.light : family.dark;
}
