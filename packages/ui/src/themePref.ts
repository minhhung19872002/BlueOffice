import { useSyncExternalStore } from 'react';
import type { ThemeSetting } from './theme';

/**
 * The theme the user picked in File › Options (CORE-UI-007), remembered per browser like the
 * density and shared by the three apps. `null`: nothing picked — the editor's own setting
 * applies (normally `auto`, which follows `forced-colors` / `prefers-contrast` / dark mode).
 */
const KEY = 'bo.theme';
export const THEME_SETTINGS: readonly ThemeSetting[] = ['auto', 'light', 'dark', 'hc'];
const listeners = new Set<() => void>();
let current: ThemeSetting | null = load();

function load(): ThemeSetting | null {
  try {
    const v = globalThis.localStorage?.getItem(KEY);
    if (v && (THEME_SETTINGS as readonly string[]).includes(v)) return v as ThemeSetting;
  } catch {
    // Storage blocked (private window): nothing remembered.
  }
  return null;
}

export function getThemePref(): ThemeSetting | null {
  return current;
}

/** Set and remember the theme (`null` forgets the choice). */
export function setThemePref(t: ThemeSetting | null) {
  current = t;
  try {
    if (t) globalThis.localStorage?.setItem(KEY, t);
    else globalThis.localStorage?.removeItem(KEY);
  } catch {
    // Not persisted; still applied for this session.
  }
  for (const f of listeners) f();
}

const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};

/** The user's theme, else `fallback` (the editor's `theme` prop). */
export function useThemePref(fallback: ThemeSetting): ThemeSetting {
  return useSyncExternalStore(subscribe, getThemePref, getThemePref) ?? fallback;
}
