import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  ACCENTS,
  CHROME,
  CHROME_FORCED,
  PAPER,
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

/** `fg` at `alpha` over `bg`, as an sRGB hex colour. */
function blend(fg: string, bg: string, alpha: number): string {
  const ch = (h: string, i: number) => parseInt(h.replace('#', '').slice(i, i + 2), 16);
  return `#${[0, 2, 4]
    .map((i) =>
      Math.round(ch(fg, i) * alpha + ch(bg, i) * (1 - alpha))
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`;
}

describe('high contrast and canvas chrome (CORE-UI-007)', () => {
  it.each(themes)('%s: focus ring is at least 2px and 3:1 against every surface', (t) => {
    const s = THEMES[t];
    expect(parseFloat(CHROME[t]['focus-width'])).toBeGreaterThanOrEqual(2);
    for (const bg of [s.surface, s['surface-2'], s.bg])
      expect(contrast(s.focus, bg)).toBeGreaterThanOrEqual(3);
  });

  it.each(themes)('%s: caret, cursor, handles and comment markers show on the paper', (t) => {
    const c = CHROME[t];
    const min = t === 'hc' ? 7 : 3;
    for (const role of ['caret', 'cursor', 'handle-border', 'comment'] as const)
      expect([role, contrast(c[role], PAPER) >= min]).toEqual([role, true]);
    // Text stays readable under the selection tint.
    const tinted = blend(c.selection, PAPER, Number(c['selection-alpha']));
    expect(contrast('#000000', tinted)).toBeGreaterThanOrEqual(4.5);
    // (In high contrast the selection is told apart by its outline in the cursor colour.)
  });

  it('high contrast: text 7:1, borders 3:1, two-toned marks', () => {
    const s = THEMES.hc;
    const c = CHROME.hc;
    expect(contrast(s.text, s.surface)).toBeGreaterThanOrEqual(7);
    expect(contrast(s.border, s.surface)).toBeGreaterThanOrEqual(3);
    expect(contrast(s.focus, s.surface)).toBeGreaterThanOrEqual(7);
    // The halo is the opposite tone of the mark it surrounds.
    expect(contrast(c.caret, c.halo)).toBeGreaterThanOrEqual(7);
    expect(contrast(c.handle, c['handle-border'])).toBeGreaterThanOrEqual(7);
    for (const st of Object.values(STATUS.hc))
      expect(contrast(st.text, st.bg)).toBeGreaterThanOrEqual(7);
    for (const a of Object.values(ACCENTS.hc))
      expect(contrast(a.on, a.strong)).toBeGreaterThanOrEqual(7);
  });

  it('forced colours use the system palette only and are emitted in tokens.css', () => {
    const system = /^(Canvas|CanvasText|Highlight|HighlightText|LinkText|ButtonText|ButtonFace)$/;
    for (const [k, v] of Object.entries(CHROME_FORCED))
      if (k !== 'selection-alpha' && k !== 'focus-width')
        expect([k, system.test(v)]).toEqual([k, true]);
    expect(parseFloat(CHROME_FORCED['focus-width'])).toBeGreaterThanOrEqual(2);
    const css = renderTokensCss();
    expect(css).toMatch(/forced-colors: active[\s\S]*--bo-chrome-cursor: Highlight;/);
    expect(css).toMatch(/\[data-theme='hc'\] \{[\s\S]*--bo-chrome-focus-width: 3px;/);
  });

  it('theme.css draws focus rings and the document caret from the tokens', () => {
    const theme = readFileSync(fileURLToPath(new URL('../src/theme.css', import.meta.url)), 'utf8');
    expect(theme).toContain('outline: var(--bo-chrome-focus-width, 2px) solid var(--bo-focus);');
    expect(theme).toMatch(/forced-colors: active[\s\S]*outline: 3px solid Highlight;/);
    expect(theme).toMatch(/\.bo-caret \{[\s\S]*forced-color-adjust: none;/);
  });
});

describe('motion tokens', () => {
  it('stay within 120–200 ms', async () => {
    const { MOTION } = await import('../src/tokens');
    for (const v of Object.values(MOTION)) {
      expect(v).toBeGreaterThanOrEqual(120);
      expect(v).toBeLessThanOrEqual(200);
    }
  });
});
