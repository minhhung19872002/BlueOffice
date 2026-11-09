/**
 * Header / footer text helpers for the page setup dialog: the `&L` / `&C` /
 * `&R` section codes of ECMA-376 Part 1, §18.3.1.39 (the code table given
 * with `evenFooter`). Text before any section code belongs to the centre
 * section (the same reading as `bo_sheet::headerfooter`).
 */
import { colName } from './grid';

export interface HfSections {
  left: string;
  center: string;
  right: string;
}

/** Splits header / footer text into its three sections (codes other than &L &C &R stay as typed). */
export function splitSections(text: string): HfSections {
  const out: HfSections = { left: '', center: '', right: '' };
  let sec: keyof HfSections = 'center';
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (ch === '&' && i + 1 < text.length) {
      const code = text[i + 1];
      if (code === 'L' || code === 'C' || code === 'R') {
        sec = code === 'L' ? 'left' : code === 'C' ? 'center' : 'right';
        i += 2;
        continue;
      }
      // Keep any other two-character code (including `&&`) intact.
      out[sec] += ch + code;
      i += 2;
      continue;
    }
    out[sec] += ch;
    i += 1;
  }
  return out;
}

/** Joins sections back into header / footer text (empty sections are left out). */
export function joinSections(s: HfSections): string {
  return (
    (s.left ? `&L${s.left}` : '') +
    (s.center ? `&C${s.center}` : '') +
    (s.right ? `&R${s.right}` : '')
  );
}

/** Codes offered by the insert buttons (i18n key suffix → code). */
export const HF_FIELDS: ReadonlyArray<readonly [string, string]> = [
  ['page', '&P'],
  ['pages', '&N'],
  ['date', '&D'],
  ['time', '&T'],
  ['path', '&Z&F'],
  ['file', '&F'],
  ['sheet', '&A'],
  ['bold', '&B'],
  ['italic', '&I'],
  ['underline', '&U'],
];

/** Inserts `code` into `text` at `[start, end)`; returns the new text and caret. */
export function insertCode(
  text: string,
  code: string,
  start: number,
  end: number,
): { text: string; caret: number } {
  const a = Math.max(0, Math.min(start, text.length));
  const b = Math.max(a, Math.min(end, text.length));
  return { text: text.slice(0, a) + code + text.slice(b), caret: a + code.length };
}

/** `"1:3"` → `[0, 2]` (0-based rows); `null` when empty or invalid. */
export function parseRowSpan(text: string): [number, number] | null {
  const m = /^\s*\$?(\d+)\s*(?::\s*\$?(\d+))?\s*$/.exec(text);
  if (!m) return null;
  const a = Number(m[1]) - 1;
  const b = Number(m[2] ?? m[1]) - 1;
  if (a < 0 || b < 0) return null;
  return [Math.min(a, b), Math.max(a, b)];
}

function colIndex(letters: string): number {
  let n = 0;
  for (const ch of letters.toUpperCase()) n = n * 26 + (ch.charCodeAt(0) - 64);
  return n - 1;
}

/** `"A:B"` → `[0, 1]` (0-based columns); `null` when empty or invalid. */
export function parseColSpan(text: string): [number, number] | null {
  const m = /^\s*\$?([A-Za-z]{1,3})\s*(?::\s*\$?([A-Za-z]{1,3}))?\s*$/.exec(text);
  if (!m) return null;
  const a = colIndex(m[1]!);
  const b = colIndex(m[2] ?? m[1]!);
  return [Math.min(a, b), Math.max(a, b)];
}

export function rowSpanText(s: [number, number] | null): string {
  return s ? `${s[0] + 1}:${s[1] + 1}` : '';
}

export function colSpanText(s: [number, number] | null): string {
  return s ? `${colName(s[0])}:${colName(s[1])}` : '';
}

/** Strips sheet names and `$` from a print area (`Sheet1!$A$1:$B$2,…` → `A1:B2,…`). */
export function areaText(area: string | null): string {
  if (!area) return '';
  return area
    .split(',')
    .map((p) => {
      const i = p.lastIndexOf('!');
      return (i >= 0 ? p.slice(i + 1) : p).replace(/\$/g, '');
    })
    .join(',');
}
