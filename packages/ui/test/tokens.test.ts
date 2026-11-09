import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  ACCENTS,
  FONT_SIZES,
  gray,
  renderTokensCss,
  STATUS,
  THEMES,
  type ThemeName,
} from '../src/tokens';

const cssPath = fileURLToPath(new URL('../src/tokens/tokens.css', import.meta.url));

/** WCAG 2.2 relative luminance of an sRGB hex colour (§ "relative luminance"). */
function luminance(hex: string): number {
  const h = hex.replace('#', '');
  const ch = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  const lin = ch.map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * lin[0]! + 0.7152 * lin[1]! + 0.0722 * lin[2]!;
}

/** WCAG contrast ratio (L1 + 0.05) / (L2 + 0.05). */
function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x! + 0.05) / (y! + 0.05);
}

const themes: ThemeName[] = ['light', 'dark', 'hc'];

describe('design tokens', () => {
  it('tokens.css is generated from tokens.ts', () => {
    const css = renderTokensCss();
    if (process.env.BO_WRITE_TOKENS === '1') writeFileSync(cssPath, css);
    expect(readFileSync(cssPath, 'utf8').replace(/\r\n/g, '\n')).toBe(css);
  });

  it('contrast helper matches the WCAG reference points', () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 5);
    expect(contrast('#777777', '#ffffff')).toBeCloseTo(4.48, 2);
  });

  it.each(themes)('%s: body and muted text reach AA (4.5:1) on every surface', (t) => {
    const s = THEMES[t];
    for (const bg of [s.surface, s['surface-2'], s.bg]) {
      expect(contrast(s.text, bg)).toBeGreaterThanOrEqual(t === 'hc' ? 7 : 4.5);
      expect(contrast(s['text-muted'], bg)).toBeGreaterThanOrEqual(4.5);
    }
    expect(contrast(s['brand-contrast'], s.brand)).toBeGreaterThanOrEqual(4.5);
  });

  it.each(themes)('%s: accent text and filled accents reach AA', (t) => {
    const s = THEMES[t];
    for (const [app, a] of Object.entries(ACCENTS[t])) {
      expect([app, contrast(a.on, a.strong)]).toEqual([app, expect.any(Number)]);
      expect(contrast(a.on, a.strong)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(a.text, s.surface)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(a.text, a.subtle)).toBeGreaterThanOrEqual(4.5);
      // Non-text identity colour (stripe, tab underline): WCAG 1.4.11 needs 3:1.
      expect(contrast(a.base, s.surface)).toBeGreaterThanOrEqual(3);
    }
  });

  it.each(themes)('%s: status text reaches AA on its background and the surface', (t) => {
    for (const st of Object.values(STATUS[t])) {
      expect(contrast(st.text, st.bg)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(st.border, THEMES[t].surface)).toBeGreaterThanOrEqual(3);
    }
  });

  it('app accents are distinct per app (indigo, teal, coral) and from the brand', () => {
    const l = ACCENTS.light;
    expect([l.doc.base, l.sheet.base, l.slide.base]).toEqual(['#4F46E5', '#0F9D8A', '#E0457B']);
    expect(new Set([l.brand.base, l.doc.base, l.sheet.base, l.slide.base]).size).toBe(4);
  });

  it('neutral scale has 12 steps that get monotonically darker in light and lighter in dark', () => {
    for (const t of ['light', 'dark'] as const) {
      const g = gray(t).map(luminance);
      expect(g).toHaveLength(12);
      for (let i = 1; i < g.length; i++) {
        if (t === 'light') expect(g[i]!).toBeLessThan(g[i - 1]!);
        else expect(g[i]!).toBeGreaterThan(g[i - 1]!);
      }
    }
  });

  it('type scale and reduced motion are emitted', () => {
    const css = renderTokensCss();
    expect(FONT_SIZES).toEqual([11, 12, 13, 14, 16, 20, 24, 32]);
    for (const s of FONT_SIZES) expect(css).toContain(`--bo-font-size-${s}: ${s}px;`);
    expect(css).toMatch(/prefers-reduced-motion: reduce[\s\S]*--bo-duration-fast: 0ms/);
    expect(css).toContain("[data-app='sheet'] {");
    expect(css).toContain("[data-density='touch'] {");
  });
});
