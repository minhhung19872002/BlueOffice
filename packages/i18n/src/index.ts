import i18next, { type i18n } from 'i18next';
import { initReactI18next } from 'react-i18next';
import enUS from './locales/en-US.json';
import viVN from './locales/vi-VN.json';

export type Language = 'vi-VN' | 'en-US';
export const LANGUAGES: Language[] = ['vi-VN', 'en-US'];
export const DEFAULT_LANGUAGE: Language = 'vi-VN';

export const resources = {
  'vi-VN': { translation: viVN },
  'en-US': { translation: enUS },
} as const;

/** Create an isolated i18next instance (one per embedded editor). vi-VN is the default. */
export function createI18n(lng: Language = DEFAULT_LANGUAGE): i18n {
  const inst = i18next.createInstance();
  void inst.use(initReactI18next).init({
    resources,
    lng,
    fallbackLng: DEFAULT_LANGUAGE,
    interpolation: { escapeValue: false },
    initAsync: false,
    returnNull: false,
  });
  memoizePlainLookups(inst);
  return inst;
}

/** Options a fixed `t` (react-i18next) always passes; other options change the result. */
const PLAIN_OPTIONS = new Set(['lng', 'lngs', 'ns', 'keyPrefix']);

/**
 * Typing latency: the editors render the whole ribbon after every edit, thousands of `t(key)`
 * lookups without interpolation. Those results are cached per language and namespace (the
 * cache is dropped when resources change); calls with other options go to i18next.
 */
function memoizePlainLookups(inst: i18n): void {
  const cache = new Map<string, unknown>();
  const clear = () => cache.clear();
  inst.on('languageChanged', clear);
  inst.store.on('added', clear);
  inst.store.on('removed', clear);
  const translate = inst.t.bind(inst) as unknown as (...args: unknown[]) => unknown;
  const cached = (...args: unknown[]): unknown => {
    const [key, opts] = args;
    if (args.length > 2 || typeof key !== 'string') return translate(...args);
    let lng = inst.language;
    let ns = '';
    if (opts !== undefined) {
      if (typeof opts !== 'object' || opts === null) return translate(...args);
      for (const [k, v] of Object.entries(opts)) {
        if (v === undefined) continue;
        if (!PLAIN_OPTIONS.has(k) || k === 'keyPrefix') return translate(...args);
      }
      const o = opts as { lng?: string; lngs?: readonly string[]; ns?: string | readonly string[] };
      lng = o.lng ?? o.lngs?.join(',') ?? lng;
      ns = Array.isArray(o.ns) ? o.ns.join(',') : String(o.ns ?? '');
    }
    const id = `${lng}\u0000${ns}\u0000${key}`;
    if (cache.has(id)) return cache.get(id);
    const v = translate(...args);
    if (typeof v === 'string') cache.set(id, v);
    return v;
  };
  inst.t = cached as unknown as i18n['t'];
  // `exists(key)` (command labels of the menus and the toolbar) the same way.
  const exists = inst.exists.bind(inst) as unknown as (...args: unknown[]) => boolean;
  const known = new Map<string, boolean>();
  inst.on('languageChanged', () => known.clear());
  inst.store.on('added', () => known.clear());
  inst.store.on('removed', () => known.clear());
  inst.exists = ((...args: unknown[]): boolean => {
    const [key] = args;
    if (args.length !== 1 || typeof key !== 'string') return exists(...args);
    const id = `${inst.language}\u0000${key}`;
    let v = known.get(id);
    if (v === undefined) {
      v = exists(key);
      known.set(id, v);
    }
    return v;
  }) as unknown as i18n['exists'];
}

/** Number formatting per locale (vi-VN: `1.234,5`). */
export function formatNumber(n: number, lng: Language, opts?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(lng, opts).format(n);
}

export { I18nextProvider, useTranslation } from 'react-i18next';
