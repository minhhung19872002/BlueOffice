import { describe, expect, it } from 'vitest';
import { createI18n, formatNumber, resources } from '../src';

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
