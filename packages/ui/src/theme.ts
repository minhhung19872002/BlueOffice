import { useEffect, useMemo, useState } from 'react';

/** User choice: light, dark, high contrast, or follow the system (`auto`). */
export type ThemeSetting = 'light' | 'dark' | 'hc' | 'auto';
export type ResolvedTheme = 'light' | 'dark' | 'hc';

/** OS signals `auto` follows. */
export interface SystemThemeSignals {
  dark: boolean;
  /** `forced-colors: active` (Windows high contrast) or `prefers-contrast: more`. */
  highContrast: boolean;
}

/** Pure resolution of a setting against the OS signals (high contrast wins over dark). */
export function resolveTheme(setting: ThemeSetting, os: SystemThemeSignals): ResolvedTheme {
  if (setting !== 'auto') return setting;
  if (os.highContrast) return 'hc';
  return os.dark ? 'dark' : 'light';
}

function useMedia(query: string): boolean {
  const mql = useMemo(
    () => (typeof window !== 'undefined' && window.matchMedia ? window.matchMedia(query) : null),
    [query],
  );
  const [matches, setMatches] = useState(mql?.matches ?? false);
  useEffect(() => {
    if (!mql) return;
    const f = (e: MediaQueryListEvent) => setMatches(e.matches);
    mql.addEventListener('change', f);
    return () => mql.removeEventListener('change', f);
  }, [mql]);
  return matches;
}

/** Resolve `auto` with the OS preferences and keep it in sync. */
export function useResolvedTheme(setting: ThemeSetting): ResolvedTheme {
  const dark = useMedia('(prefers-color-scheme: dark)');
  const forced = useMedia('(forced-colors: active)');
  const more = useMedia('(prefers-contrast: more)');
  return resolveTheme(setting, { dark, highContrast: forced || more });
}

/** The next theme for the "Theme" toggle: light → dark → high contrast → light. */
export function nextTheme(current: ThemeSetting): ThemeSetting {
  return current === 'light' ? 'dark' : current === 'dark' ? 'hc' : 'light';
}
