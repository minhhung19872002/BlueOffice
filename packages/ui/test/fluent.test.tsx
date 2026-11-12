// @vitest-environment happy-dom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import {
  BlueFluentProvider,
  blueOfficeBrand,
  blueOfficeFluentTheme,
} from '../src/BlueFluentProvider';
import { ACCENTS } from '../src/tokens/tokens';

function luminance(hex: string): number {
  const rgb = [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16) / 255);
  const linear = rgb.map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
}

describe('BlueOffice Fluent theme', () => {
  it('uses each app identity at the centre of a complete brand ramp', () => {
    for (const app of ['doc', 'sheet', 'slide'] as const) {
      const ramp = blueOfficeBrand(app);
      expect(Object.keys(ramp)).toHaveLength(16);
      expect(ramp[80].toLowerCase()).toBe(ACCENTS.light[app].strong.toLowerCase());
      expect(blueOfficeFluentTheme(app, 'light')).toBe(blueOfficeFluentTheme(app, 'light'));
      expect(blueOfficeFluentTheme(app, 'dark')).not.toBe(blueOfficeFluentTheme(app, 'light'));
      expect(blueOfficeFluentTheme(app, 'hc').colorNeutralForeground1).toBeTruthy();
      const theme = blueOfficeFluentTheme(app, 'light');
      const values = [
        luminance(theme.colorBrandBackground),
        luminance(theme.colorNeutralForegroundOnBrand),
      ].sort((a, b) => b - a);
      expect((values[0]! + 0.05) / (values[1]! + 0.05)).toBeGreaterThanOrEqual(4.5);
    }
  });
  it('keeps app, mode and accessible content on the existing root', () => {
    render(
      <BlueFluentProvider app="sheet" mode="dark" data-app="sheet" data-theme="dark">
        <button>Save</button>
      </BlueFluentProvider>,
    );
    const button = screen.getByRole('button', { name: 'Save' });
    expect(button.closest('[data-app="sheet"]')?.getAttribute('data-theme')).toBe('dark');
  });
});
