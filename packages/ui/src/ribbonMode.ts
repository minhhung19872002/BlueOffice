import { useSyncExternalStore } from 'react';

/**
 * Ribbon display preferences (CORE-UI-001, UI-RIBBON-003), shared by the three apps and
 * remembered per browser like the density (`density.ts`):
 *  - `simplified`: one row per tab, each group shows its most-used commands + "…";
 *  - `collapsed`: only the tab strip; clicking a tab shows its commands over the document
 *    (Ctrl+F1 toggles);
 *  - `autoHide`: no ribbon at all, only a thin bar at the top; clicking it or pressing Alt shows
 *    the ribbon over the document until a command runs or the user clicks elsewhere.
 */
export interface RibbonPrefs {
  simplified: boolean;
  collapsed: boolean;
  autoHide: boolean;
}

const KEY = 'bo.ribbon';
const DEFAULT: RibbonPrefs = { simplified: false, collapsed: false, autoHide: false };
const listeners = new Set<() => void>();
let current: RibbonPrefs = load();

function load(): RibbonPrefs {
  try {
    const raw = globalThis.localStorage?.getItem(KEY);
    if (!raw) return DEFAULT;
    const v: unknown = JSON.parse(raw);
    if (v && typeof v === 'object') {
      const o = v as Record<string, unknown>;
      return {
        simplified: o.simplified === true,
        collapsed: o.collapsed === true,
        autoHide: o.autoHide === true,
      };
    }
  } catch {
    // Storage blocked or bad JSON: defaults.
  }
  return DEFAULT;
}

export function getRibbonPrefs(): RibbonPrefs {
  return current;
}

/** Change and remember the ribbon preferences. */
export function setRibbonPrefs(change: Partial<RibbonPrefs>) {
  const next = { ...current, ...change };
  if (
    next.simplified === current.simplified &&
    next.collapsed === current.collapsed &&
    next.autoHide === current.autoHide
  )
    return;
  current = next;
  try {
    globalThis.localStorage?.setItem(KEY, JSON.stringify(next));
  } catch {
    // Not persisted; still applied for this session.
  }
  for (const f of listeners) f();
}

/** Ctrl+F1: collapse / expand the ribbon. */
export function toggleRibbonCollapsed() {
  setRibbonPrefs({ collapsed: !current.collapsed });
}

/** Auto-hide the whole ribbon (UI-RIBBON-003) / show it again. */
export function toggleRibbonAutoHide() {
  setRibbonPrefs({ autoHide: !current.autoHide });
}

/** Full ↔ simplified ribbon. */
export function toggleRibbonSimplified() {
  setRibbonPrefs({ simplified: !current.simplified });
}

const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};

export function useRibbonPrefs(): RibbonPrefs {
  return useSyncExternalStore(subscribe, getRibbonPrefs, getRibbonPrefs);
}
