import { describe, expect, it } from 'vitest';
import { nextTheme, resolveTheme } from '../src';

describe('theme resolution', () => {
  const os = (dark: boolean, highContrast: boolean) => ({ dark, highContrast });

  it('explicit settings ignore the OS', () => {
    expect(resolveTheme('light', os(true, true))).toBe('light');
    expect(resolveTheme('dark', os(false, true))).toBe('dark');
    expect(resolveTheme('hc', os(false, false))).toBe('hc');
  });

  it('auto follows the OS, high contrast first', () => {
    expect(resolveTheme('auto', os(false, false))).toBe('light');
    expect(resolveTheme('auto', os(true, false))).toBe('dark');
    expect(resolveTheme('auto', os(true, true))).toBe('hc');
  });

  it('the toggle cycles light → dark → high contrast → light', () => {
    expect(nextTheme('light')).toBe('dark');
    expect(nextTheme('dark')).toBe('hc');
    expect(nextTheme('hc')).toBe('light');
    expect(nextTheme('auto')).toBe('light');
  });
});
