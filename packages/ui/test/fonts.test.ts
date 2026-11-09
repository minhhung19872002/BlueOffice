import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { FONT_UI } from '../src/tokens';

const root = resolve(import.meta.dirname, '..');

describe('UI font', () => {
  it('Be Vietnam Pro leads the stack and every declared face ships with its licence', () => {
    expect(FONT_UI.startsWith("'Be Vietnam Pro'")).toBe(true);
    const css = readFileSync(resolve(root, 'src/theme.css'), 'utf8');
    const urls = [...css.matchAll(/url\('\.\.\/fonts\/([\w-]+\.ttf)'\)/g)].map((m) => m[1]!);
    expect(urls).toEqual([
      'BeVietnamPro-Regular.ttf',
      'BeVietnamPro-Medium.ttf',
      'BeVietnamPro-SemiBold.ttf',
      'BeVietnamPro-Bold.ttf',
    ]);
    for (const u of urls) expect(existsSync(resolve(root, 'fonts', u)), u).toBe(true);
    const ofl = readFileSync(resolve(root, 'fonts/OFL-BeVietnamPro.txt'), 'utf8');
    expect(ofl).toContain('SIL Open Font License, Version 1.1');
    expect(css).toContain('font-display: swap');
  });
});
