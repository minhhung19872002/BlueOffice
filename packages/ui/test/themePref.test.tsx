// @vitest-environment happy-dom
import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import {
  THEME_SETTINGS,
  getThemePref,
  resolveTheme,
  setThemePref,
  useThemePref,
  type ThemeSetting,
} from '../src';

afterEach(() => {
  cleanup();
  setThemePref(null);
});

function Probe({ fallback }: { fallback: ThemeSetting }) {
  return <span data-testid="theme">{useThemePref(fallback)}</span>;
}

describe('theme preference (File › Options)', () => {
  it('nothing picked: the editor setting applies', () => {
    expect(getThemePref()).toBeNull();
    render(<Probe fallback="light" />);
    expect(screen.getByTestId('theme').textContent).toBe('light');
  });

  it('a picked theme wins, is remembered and can be forgotten', () => {
    render(<Probe fallback="light" />);
    act(() => setThemePref('hc'));
    expect(screen.getByTestId('theme').textContent).toBe('hc');
    expect(localStorage.getItem('bo.theme')).toBe('hc');
    act(() => setThemePref(null));
    expect(screen.getByTestId('theme').textContent).toBe('light');
    expect(localStorage.getItem('bo.theme')).toBeNull();
  });

  it('offers system, light, dark and high contrast', () => {
    expect([...THEME_SETTINGS]).toEqual(['auto', 'light', 'dark', 'hc']);
  });

  it('"system" follows forced colours / more contrast before dark mode', () => {
    expect(resolveTheme('auto', { dark: true, highContrast: true })).toBe('hc');
    expect(resolveTheme('auto', { dark: false, highContrast: true })).toBe('hc');
    // The manual choice is kept whatever the system says.
    expect(resolveTheme('hc', { dark: false, highContrast: false })).toBe('hc');
    expect(resolveTheme('light', { dark: true, highContrast: true })).toBe('light');
  });
});
