import { useEffect, useMemo, useState } from 'react';

export type ThemeSetting = 'light' | 'dark' | 'auto';
export type ResolvedTheme = 'light' | 'dark';

/** Resolve `auto` with the OS preference and keep it in sync. */
export function useResolvedTheme(setting: ThemeSetting): ResolvedTheme {
  const query = useMemo(
    () => (typeof window !== 'undefined' && window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null),
    [],
  );
  const [osDark, setOsDark] = useState(query?.matches ?? false);
  useEffect(() => {
    if (!query) return;
    const f = (e: MediaQueryListEvent) => setOsDark(e.matches);
    query.addEventListener('change', f);
    return () => query.removeEventListener('change', f);
  }, [query]);
  if (setting === 'auto') return osDark ? 'dark' : 'light';
  return setting;
}
