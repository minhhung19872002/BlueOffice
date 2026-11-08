import { describe, expect, it } from 'vitest';
import { createI18n, formatNumber, resources } from '../src';
import enSource from '../src/locales/en-US.json?raw';
import viSource from '../src/locales/vi-VN.json?raw';

const LANGS = ['vi-VN', 'en-US'] as const;
const sources: Record<(typeof LANGS)[number], string> = { 'vi-VN': viSource, 'en-US': enSource };

function keys(obj: object, prefix = ''): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === 'object' ? keys(v as object, `${prefix}${k}.`) : [`${prefix}${k}`],
  );
}

describe('i18n', () => {
  it('vi-VN and en-US define exactly the same keys', () => {
    const vi = keys(resources['vi-VN'].translation).sort();
    const en = keys(resources['en-US'].translation).sort();
    expect(vi).toEqual(en);
    expect(vi.length).toBeGreaterThan(40);
  });

  it('has no duplicate top-level sections (a later one would silently replace the first)', () => {
    for (const lang of LANGS) {
      const raw = JSON.stringify(resources[lang].translation);
      // Parsed objects cannot show duplicates: compare the section count with the source.
      const source = sources[lang].match(/^ {2}"[^"]+": /gm) ?? [];
      expect(source.length).toBe(Object.keys(JSON.parse(raw) as object).length);
    }
  });

  it('defaults to Vietnamese and interpolates', () => {
    const t = createI18n();
    expect(t.language).toBe('vi-VN');
    expect(t.t('status.page', { current: 2, total: 5 })).toBe('Trang 2/5');
    void t.changeLanguage('en-US');
    expect(t.t('status.page', { current: 2, total: 5 })).toBe('Page 2 of 5');
  });

  it('formats numbers the Vietnamese way', () => {
    expect(formatNumber(1234567.5, 'vi-VN')).toBe('1.234.567,5');
    expect(formatNumber(1234567.5, 'en-US')).toBe('1,234,567.5');
  });
});
