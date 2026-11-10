/**
 * Customisable status bar (CORE-UI-007, UI-FRAME-004): which items each app can show, which
 * are on by default, and the user's choices. Only the differences from the defaults are
 * stored (`blueoffice.statusbar.<app>`, the same per-user storage as the ribbon
 * customisation: `ShellIntegration.getSetting` / `setSetting`, else `localStorage`), so an
 * item added later appears with its default.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import type { SettingStore } from './pinnedFiles';

export type StatusBarApp = 'doc' | 'sheet' | 'slide';

export interface StatusItemDef {
  id: string;
  /** Shown unless the user turns it off. */
  default: boolean;
}

const on = (id: string): StatusItemDef => ({ id, default: true });
const off = (id: string): StatusItemDef => ({ id, default: false });

/** Items in the order of the checklist (labels: i18n `statusBar.items.<id>`). */
export const STATUS_ITEMS: Record<StatusBarApp, readonly StatusItemDef[]> = {
  doc: [
    on('page'),
    on('words'),
    on('proofing'),
    on('language'),
    on('trackChanges'),
    off('capsLock'),
    off('overtype'),
    on('viewButtons'),
    on('zoomSlider'),
    on('zoom'),
  ],
  sheet: [
    on('cellMode'),
    on('macro'),
    on('filter'),
    on('average'),
    on('count'),
    off('numericalCount'),
    off('min'),
    off('max'),
    on('sum'),
    on('viewButtons'),
    on('zoomSlider'),
    on('zoom'),
  ],
  slide: [
    on('slide'),
    on('language'),
    on('notes'),
    on('comments'),
    on('viewButtons'),
    on('zoomSlider'),
    on('zoom'),
    on('fit'),
  ],
};

/** The user's differences from the defaults. */
export interface StatusBarPrefs {
  /** Default-off items turned on. */
  shown?: string[];
  /** Default-on items turned off. */
  hidden?: string[];
}

const def = (app: StatusBarApp, id: string) => STATUS_ITEMS[app].find((d) => d.id === id);

export function isStatusItemShown(app: StatusBarApp, prefs: StatusBarPrefs, id: string): boolean {
  const d = def(app, id);
  if (!d) return false;
  return d.default ? !prefs.hidden?.includes(id) : !!prefs.shown?.includes(id);
}

/** The ids shown, in checklist order. */
export function shownStatusItems(app: StatusBarApp, prefs: StatusBarPrefs): string[] {
  return STATUS_ITEMS[app].filter((d) => isStatusItemShown(app, prefs, d.id)).map((d) => d.id);
}

function compact(prefs: StatusBarPrefs): StatusBarPrefs {
  const out: StatusBarPrefs = {};
  if (prefs.shown?.length) out.shown = prefs.shown;
  if (prefs.hidden?.length) out.hidden = prefs.hidden;
  return out;
}

/** Turn one item on or off (`value` omitted: toggle). Unknown ids change nothing. */
export function setStatusItem(
  app: StatusBarApp,
  prefs: StatusBarPrefs,
  id: string,
  value?: boolean,
): StatusBarPrefs {
  const d = def(app, id);
  if (!d) return prefs;
  const next = value ?? !isStatusItemShown(app, prefs, id);
  const shown = (prefs.shown ?? []).filter((x) => x !== id);
  const hidden = (prefs.hidden ?? []).filter((x) => x !== id);
  if (next && !d.default) shown.push(id);
  if (!next && d.default) hidden.push(id);
  return compact({ shown, hidden });
}

/** Keeps what is valid of a stored value (ids of the app, each in the list it belongs to). */
export function sanitizeStatusPrefs(app: StatusBarApp, value: unknown): StatusBarPrefs {
  if (!value || typeof value !== 'object') return {};
  const v = value as { shown?: unknown; hidden?: unknown };
  const ids = (list: unknown, wantDefault: boolean) =>
    Array.isArray(list)
      ? [...new Set(list.filter((x): x is string => typeof x === 'string'))].filter(
          (id) => def(app, id)?.default === wantDefault,
        )
      : [];
  return compact({ shown: ids(v.shown, false), hidden: ids(v.hidden, true) });
}

export const statusBarKey = (app: StatusBarApp) => `blueoffice.statusbar.${app}`;

function parse(app: StatusBarApp, text: string | null): StatusBarPrefs {
  if (!text) return {};
  try {
    return sanitizeStatusPrefs(app, JSON.parse(text));
  } catch {
    return {};
  }
}

export async function loadStatusPrefs(
  store: SettingStore,
  app: StatusBarApp,
): Promise<StatusBarPrefs> {
  const key = statusBarKey(app);
  try {
    if (store.getSetting) return parse(app, await store.getSetting(key));
    return parse(app, globalThis.localStorage?.getItem(key) ?? null);
  } catch {
    return {};
  }
}

export async function saveStatusPrefs(
  store: SettingStore,
  app: StatusBarApp,
  prefs: StatusBarPrefs,
): Promise<void> {
  const key = statusBarKey(app);
  const c = compact(prefs);
  const text = Object.keys(c).length ? JSON.stringify(c) : null;
  if (store.setSetting) {
    await store.setSetting(key, text);
    return;
  }
  try {
    if (text) globalThis.localStorage?.setItem(key, text);
    else globalThis.localStorage?.removeItem(key);
  } catch {
    // Storage unavailable: the choice lasts for this session.
  }
}

export interface StatusBarCustomization {
  app: StatusBarApp;
  prefs: StatusBarPrefs;
  shown: (id: string) => boolean;
  toggle: (id: string) => void;
  reset: () => void;
}

/** The user's status-bar choices of one app, loaded once and saved on every change. */
export function useStatusBarPrefs(store: SettingStore, app: StatusBarApp): StatusBarCustomization {
  const [prefs, setPrefs] = useState<StatusBarPrefs>({});
  const prefsRef = useRef(prefs);
  const touched = useRef(false);
  const storeRef = useRef(store);
  useEffect(() => {
    storeRef.current = store;
  }, [store]);
  useEffect(() => {
    let live = true;
    void loadStatusPrefs(storeRef.current, app).then((p) => {
      if (!live || touched.current) return;
      prefsRef.current = p;
      setPrefs(p);
    });
    return () => {
      live = false;
    };
  }, [app]);
  const update = useCallback(
    (f: (p: StatusBarPrefs) => StatusBarPrefs) => {
      touched.current = true;
      const next = f(prefsRef.current);
      prefsRef.current = next;
      setPrefs(next);
      void saveStatusPrefs(storeRef.current, app, next).catch(() => {});
    },
    [app],
  );
  const shown = useCallback((id: string) => isStatusItemShown(app, prefs, id), [app, prefs]);
  const toggle = useCallback(
    (id: string) => update((p) => setStatusItem(app, p, id)),
    [app, update],
  );
  const reset = useCallback(() => update(() => ({})), [update]);
  return { app, prefs, shown, toggle, reset };
}
