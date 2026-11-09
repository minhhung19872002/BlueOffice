import { useSyncExternalStore } from 'react';

/** UI density (docs/ui/DESIGN.md): Normal (default), Compact, Touch (targets ≥ 40px). */
export type Density = 'normal' | 'compact' | 'touch';

const KEY = 'bo.density';
const listeners = new Set<() => void>();
let current: Density = load();

function load(): Density {
  try {
    const v = globalThis.localStorage?.getItem(KEY);
    if (v === 'compact' || v === 'touch') return v;
  } catch {
    // Storage blocked (private window): fall back to the default.
  }
  return 'normal';
}

function apply(d: Density) {
  if (typeof document === 'undefined') return;
  // Tokens read [data-density] from any ancestor; the document element covers every app.
  if (d === 'normal') document.documentElement.removeAttribute('data-density');
  else document.documentElement.setAttribute('data-density', d);
}

apply(current);

export function getDensity(): Density {
  return current;
}

/** Set and remember the density for this user (per browser). */
export function setDensity(d: Density) {
  current = d;
  try {
    globalThis.localStorage?.setItem(KEY, d);
  } catch {
    // Not persisted; still applied for this session.
  }
  apply(d);
  for (const f of listeners) f();
}

/** Normal → Compact → Touch → Normal. */
export function nextDensity(d: Density): Density {
  return d === 'normal' ? 'compact' : d === 'compact' ? 'touch' : 'normal';
}

const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};

export function useDensity(): Density {
  return useSyncExternalStore(subscribe, getDensity, getDensity);
}
