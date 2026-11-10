/**
 * Storage and the export file of the ribbon / Quick Access Toolbar customisation
 * (CORE-UI-005). Kept per user and per app like the custom shortcuts (`blueoffice.ribbon.<app>`
 * through `ShellIntegration.getSetting` / `setSetting`, else `localStorage`). The exported file
 * holds the ribbon, the toolbar and the shortcuts together.
 */
import {
  compactCustomization,
  sanitizeCustomization,
  sanitizeQat,
  type QatCustomization,
  type RibbonCustomization,
  type SettingStore,
} from '@blueoffice/ui';
import { importShortcuts, validShortcut, type ShortcutApp, type ShortcutMap } from './shortcuts';

export const ribbonKey = (app: ShortcutApp) => `blueoffice.ribbon.${app}`;

/** `format` field of the exported file. */
export const CUSTOMIZATION_FORMAT = 'blueoffice-customization';
export const CUSTOMIZATION_VERSION = 1;

export interface UiCustomization {
  ribbon: RibbonCustomization;
  qat: QatCustomization;
}

function parseStored(text: string | null): UiCustomization {
  if (!text) return { ribbon: {}, qat: {} };
  try {
    const v = JSON.parse(text) as { ribbon?: unknown; qat?: unknown } | null;
    return {
      ribbon: sanitizeCustomization(v?.ribbon).value,
      qat: sanitizeQat(v?.qat).value,
    };
  } catch {
    return { ribbon: {}, qat: {} };
  }
}

function serialize(c: UiCustomization): string | null {
  const ribbon = compactCustomization(c.ribbon);
  const qat: QatCustomization = {};
  if (c.qat.commands) qat.commands = [...c.qat.commands];
  if (c.qat.below) qat.below = true;
  const out: Partial<UiCustomization> = {};
  if (Object.keys(ribbon).length) out.ribbon = ribbon;
  if (Object.keys(qat).length) out.qat = qat;
  return Object.keys(out).length ? JSON.stringify(out) : null;
}

export async function loadCustomization(
  store: SettingStore,
  app: ShortcutApp,
): Promise<UiCustomization> {
  const key = ribbonKey(app);
  try {
    if (store.getSetting) return parseStored(await store.getSetting(key));
    return parseStored(globalThis.localStorage?.getItem(key) ?? null);
  } catch {
    return { ribbon: {}, qat: {} };
  }
}

export async function saveCustomization(
  store: SettingStore,
  app: ShortcutApp,
  c: UiCustomization,
): Promise<void> {
  const key = ribbonKey(app);
  const text = serialize(c);
  if (store.setSetting) {
    await store.setSetting(key, text);
    return;
  }
  try {
    if (text) globalThis.localStorage?.setItem(key, text);
    else globalThis.localStorage?.removeItem(key);
  } catch {
    // Storage unavailable: the customisation lasts for this session.
  }
}

/** The exported file: ribbon, Quick Access Toolbar and shortcuts of one app. */
export function exportCustomization(
  app: ShortcutApp,
  c: UiCustomization,
  shortcuts: ShortcutMap,
): string {
  const file = {
    format: CUSTOMIZATION_FORMAT,
    version: CUSTOMIZATION_VERSION,
    app,
    ribbon: compactCustomization(c.ribbon),
    qat: c.qat.commands || c.qat.below ? c.qat : {},
    shortcuts,
  };
  return `${JSON.stringify(file, null, 2)}\n`;
}

export type ImportResult =
  | {
      ok: true;
      value: UiCustomization;
      shortcuts: ShortcutMap;
      /** Command ids the app does not have (left out). */
      ignored: string[];
    }
  | { ok: false; error: 'invalid' | 'wrongApp' | 'newer' };

/**
 * Reads an exported file. Command ids `known` rejects (custom groups, toolbar, shortcuts)
 * are left out and listed in `ignored`. A shortcuts-only file (CORE-UI-004) is accepted too
 * and leaves the ribbon and toolbar as they are (`value` is `current`).
 */
export function importCustomization(
  text: string,
  app: ShortcutApp,
  known: (command: string) => boolean,
  current: UiCustomization,
): ImportResult {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return { ok: false, error: 'invalid' };
  }
  if (typeof v !== 'object' || v === null || Array.isArray(v))
    return { ok: false, error: 'invalid' };
  const f = v as Record<string, unknown>;
  if (f.format === 'blueoffice-shortcuts') {
    const r = importShortcuts(text, app);
    if (!r.ok) return r;
    const { map, ignored } = filterShortcuts(r.map, known);
    return { ok: true, value: current, shortcuts: map, ignored };
  }
  if (f.format !== CUSTOMIZATION_FORMAT) return { ok: false, error: 'invalid' };
  if (typeof f.version !== 'number' || !Number.isInteger(f.version) || f.version < 1)
    return { ok: false, error: 'invalid' };
  if (f.version > CUSTOMIZATION_VERSION) return { ok: false, error: 'newer' };
  if (f.app !== app) return { ok: false, error: 'wrongApp' };
  const ribbon = sanitizeCustomization(f.ribbon, known);
  const qat = sanitizeQat(f.qat, known);
  const raw: ShortcutMap = {};
  if (typeof f.shortcuts === 'object' && f.shortcuts !== null && !Array.isArray(f.shortcuts))
    for (const [id, s] of Object.entries(f.shortcuts as Record<string, unknown>)) {
      if (!/^[\w.-]+$/.test(id)) continue;
      if (s === null) raw[id] = null;
      else if (typeof s === 'string' && validShortcut(s)) raw[id] = s;
    }
  const sc = filterShortcuts(raw, known);
  return {
    ok: true,
    value: { ribbon: ribbon.value, qat: qat.value },
    shortcuts: sc.map,
    ignored: [...new Set([...ribbon.ignored, ...qat.ignored, ...sc.ignored])],
  };
}

function filterShortcuts(
  map: ShortcutMap,
  known: (command: string) => boolean,
): { map: ShortcutMap; ignored: string[] } {
  const out: ShortcutMap = {};
  const ignored: string[] = [];
  for (const [id, s] of Object.entries(map)) {
    if (known(id)) out[id] = s;
    else ignored.push(id);
  }
  return { map: out, ignored };
}
