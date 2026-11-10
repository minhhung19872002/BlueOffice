/**
 * Customisable keyboard shortcuts (CORE-UI-004): the user's changes are a map
 * `command id → shortcut | null` (null = removed) applied over the shortcuts the commands are
 * registered with. Kept per user and per app in the platform's setting store
 * (`ShellIntegration.getSetting` / `setSetting`), else `localStorage`; exported / imported as
 * a small JSON file.
 */
import type { SettingStore } from '@blueoffice/ui';
import { normalizeShortcut } from './commands';

export type ShortcutApp = 'doc' | 'sheet' | 'slide';
export type ShortcutMap = Record<string, string | null>;

export const shortcutsKey = (app: ShortcutApp) => `blueoffice.shortcuts.${app}`;

/** `format` field of the exported file. */
export const SHORTCUTS_FORMAT = 'blueoffice-shortcuts';

const MODIFIERS = ['Control', 'Shift', 'Alt', 'Meta', 'OS', 'AltGraph'];
/** Keys the editors keep for themselves (typing, caret, menus, KeyTips). */
const RESERVED = new Set([
  'escape',
  'tab',
  'shift+tab',
  'enter',
  'shift+enter',
  'backspace',
  'delete',
  'f10',
  'shift+f10',
  'contextmenu',
]);

/** `Ctrl+Shift+K` form of a key press (letters / digits by physical key); null for a lone modifier. */
export function shortcutFromKey(
  e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'> &
    Partial<Pick<KeyboardEvent, 'code'>>,
): string | null {
  if (MODIFIERS.includes(e.key)) return null;
  const physical = /^(?:Digit|Key)([0-9A-Z])$/.exec(e.code ?? '')?.[1];
  let key = physical ?? (e.key.length === 1 ? e.key.toUpperCase() : e.key);
  if (key === ' ') key = 'Space';
  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push('Ctrl');
  if (e.shiftKey) mods.push('Shift');
  if (e.altKey) mods.push('Alt');
  return [...mods, key].join('+');
}

/**
 * Whether a shortcut may be assigned: a function key (any modifiers), or a key with Ctrl or
 * Alt; never the keys the editors reserve.
 */
export function validShortcut(shortcut: string): boolean {
  const n = normalizeShortcut(shortcut);
  if (!n || RESERVED.has(n)) return false;
  const parts = n.split('+');
  const key = parts[parts.length - 1] ?? '';
  if (!key) return false;
  if (/^f([1-9]|1[0-2])$/.test(key)) return true;
  return parts.includes('ctrl') || parts.includes('alt');
}

function clean(v: unknown): ShortcutMap | null {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return null;
  const out: ShortcutMap = {};
  for (const [id, s] of Object.entries(v as Record<string, unknown>)) {
    if (!/^[\w.-]+$/.test(id)) continue;
    if (s === null) out[id] = null;
    else if (typeof s === 'string' && validShortcut(s)) out[id] = s;
  }
  return out;
}

function parseStored(text: string | null): ShortcutMap {
  if (!text) return {};
  try {
    return clean(JSON.parse(text)) ?? {};
  } catch {
    return {};
  }
}

export async function loadShortcuts(store: SettingStore, app: ShortcutApp): Promise<ShortcutMap> {
  const key = shortcutsKey(app);
  try {
    if (store.getSetting) return parseStored(await store.getSetting(key));
    return parseStored(globalThis.localStorage?.getItem(key) ?? null);
  } catch {
    return {};
  }
}

export async function saveShortcuts(
  store: SettingStore,
  app: ShortcutApp,
  map: ShortcutMap,
): Promise<void> {
  const key = shortcutsKey(app);
  const text = Object.keys(map).length ? JSON.stringify(map) : null;
  if (store.setSetting) {
    await store.setSetting(key, text);
    return;
  }
  try {
    if (text) globalThis.localStorage?.setItem(key, text);
    else globalThis.localStorage?.removeItem(key);
  } catch {
    // Storage unavailable: the shortcuts last for this session.
  }
}

/** The exported file. */
export function exportShortcuts(app: ShortcutApp, map: ShortcutMap): string {
  return `${JSON.stringify({ format: SHORTCUTS_FORMAT, version: 1, app, shortcuts: map }, null, 2)}\n`;
}

/**
 * Shortcuts of an exported file (null: not such a file). A file of another app is refused
 * (`wrongApp`) since command ids differ between apps.
 */
export function importShortcuts(
  text: string,
  app: ShortcutApp,
): { ok: true; map: ShortcutMap } | { ok: false; error: 'invalid' | 'wrongApp' } {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return { ok: false, error: 'invalid' };
  }
  if (typeof v !== 'object' || v === null) return { ok: false, error: 'invalid' };
  const f = v as { format?: unknown; app?: unknown; shortcuts?: unknown };
  if (f.format !== SHORTCUTS_FORMAT) return { ok: false, error: 'invalid' };
  if (f.app !== app) return { ok: false, error: 'wrongApp' };
  const map = clean(f.shortcuts);
  return map ? { ok: true, map } : { ok: false, error: 'invalid' };
}

/**
 * The map after assigning `shortcut` to `id` (null: remove it): commands that had the same
 * effective shortcut lose it (`taken` lists them). Returns the map unchanged when equal.
 */
export function assignShortcut(
  map: ShortcutMap,
  id: string,
  shortcut: string | null,
  taken: readonly string[],
  defaults: (id: string) => string | undefined,
): ShortcutMap {
  const next: ShortcutMap = { ...map };
  const set = (cmd: string, s: string | null) => {
    const def = defaults(cmd) ?? null;
    const same =
      s === null ? def === null : def !== null && normalizeShortcut(def) === normalizeShortcut(s);
    if (same) delete next[cmd];
    else next[cmd] = s;
  };
  for (const other of taken) if (other !== id) set(other, null);
  set(id, shortcut);
  return next;
}
