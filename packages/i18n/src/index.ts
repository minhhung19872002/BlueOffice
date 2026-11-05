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
  return inst;
}

/** Number formatting per locale (vi-VN: `1.234,5`). */
export function formatNumber(n: number, lng: Language, opts?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(lng, opts).format(n);
}

export { I18nextProvider, useTranslation } from 'react-i18next';
