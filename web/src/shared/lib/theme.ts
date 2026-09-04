import { useEffect, useState } from 'react';

export type ThemePreference = 'light' | 'dark' | 'system';
export type ResolvedTheme = Exclude<ThemePreference, 'system'>;

export function resolveTheme(preference: ThemePreference, prefersDark = false): ResolvedTheme {
  return preference === 'system' ? (prefersDark ? 'dark' : 'light') : preference;
}

function currentSystemTheme(): ResolvedTheme {
  return resolveTheme('system', typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches);
}

export function useResolvedTheme(preference: ThemePreference): ResolvedTheme {
  const [resolvedTheme, setResolvedTheme] = useState<ResolvedTheme>(() =>
    preference === 'system' ? currentSystemTheme() : preference,
  );

  useEffect(() => {
    if (preference !== 'system') {
      setResolvedTheme(preference);
      return;
    }

    const query = window.matchMedia('(prefers-color-scheme: dark)');
    const update = () => setResolvedTheme(resolveTheme('system', query.matches));
    update();
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }, [preference]);

  return resolvedTheme;
}
