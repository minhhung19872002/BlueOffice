// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { getDensity, nextDensity, setDensity } from '../src';
import { DENSITY } from '../src/tokens';

describe('density', () => {
  it('cycles normal → compact → touch → normal', () => {
    expect(nextDensity('normal')).toBe('compact');
    expect(nextDensity('compact')).toBe('touch');
    expect(nextDensity('touch')).toBe('normal');
  });

  it('applies to the document element and is remembered', () => {
    setDensity('touch');
    expect(document.documentElement.getAttribute('data-density')).toBe('touch');
    expect(localStorage.getItem('bo.density')).toBe('touch');
    expect(getDensity()).toBe('touch');
    setDensity('normal');
    expect(document.documentElement.hasAttribute('data-density')).toBe(false);
  });

  it('touch keeps every control a ≥ 40px target and compact is smaller than normal', () => {
    expect(DENSITY.touch.control).toBeGreaterThanOrEqual(40);
    expect(DENSITY.compact.control).toBeLessThan(DENSITY.normal.control);
    expect(DENSITY.compact.ribbonLarge).toBeLessThan(DENSITY.normal.ribbonLarge);
  });
});
