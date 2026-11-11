/**
 * Helpers of the Home tab formatting commands of BlueSheet: the font size steps, the
 * increase / decrease decimal rewrite of a number format code (ECMA-376 Part 1, §18.8.31
 * numFmt), the category shown by the number format box, and the formatting keyboard
 * shortcuts the spreadsheet convention binds to physical keys.
 */

import type { BorderSpec } from '@blueoffice/platform';

/**
 * The borders menu of the Home tab, in the spreadsheet's order, and the border specs each
 * applies to the selection's edges (§18.8.4 border; `thick` boxes use the medium line).
 */
export const BORDER_MENU: Record<string, BorderSpec[]> = {
  bottom: [{ preset: 'bottom', style: 'thin' }],
  top: [{ preset: 'top', style: 'thin' }],
  left: [{ preset: 'left', style: 'thin' }],
  right: [{ preset: 'right', style: 'thin' }],
  none: [{ preset: 'none' }],
  all: [{ preset: 'all', style: 'thin' }],
  outline: [{ preset: 'outline', style: 'thin' }],
  thickOutline: [{ preset: 'outline', style: 'medium' }],
  bottomDouble: [{ preset: 'bottom', style: 'double' }],
  thickBottom: [{ preset: 'bottom', style: 'medium' }],
  topBottom: [
    { preset: 'top', style: 'thin' },
    { preset: 'bottom', style: 'thin' },
  ],
  inside: [{ preset: 'inside', style: 'thin' }],
};

/** The comma style: two decimals, thousands, a dash for zero, aligned like accounting. */
export const COMMA_STYLE = '_-* #,##0.00_-;-* #,##0.00_-;_-* "-"??_-;_-@_-';

/** Sizes offered by the font size box (points); any other value can be typed. */
export const SHEET_FONT_SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72];

/** Font names offered by the font box (any other name can be typed). */
export const SHEET_FONTS = [
  'Calibri',
  'Cambria',
  'Arial',
  'Times New Roman',
  'Tahoma',
  'Verdana',
  'Segoe UI',
  'Courier New',
  'Liberation Sans',
  'Noto Sans',
  'Carlito',
  'Caladea',
  'Tinos',
];

/** Largest font size of a cell (§18.8.38 sz). */
const MAX_SIZE = 409;

/**
 * Increase / decrease font size: the next size of the list up or down; above 72 in steps of
 * 10, below 8 one point at a time (1 – 409).
 */
export function stepFontSize(size: number, dir: 1 | -1): number {
  if (dir > 0) {
    const next = SHEET_FONT_SIZES.find((s) => s > size);
    return Math.min(MAX_SIZE, next ?? (Math.floor(size / 10) + 1) * 10);
  }
  const prev = [...SHEET_FONT_SIZES].reverse().find((s) => s < size);
  if (prev !== undefined) return prev;
  if (size > 72) return 72;
  return Math.max(1, Math.ceil(size) - 1);
}

/** Splits a format code into sections at `;` outside quotes, brackets and escapes. */
function sections(code: string): string[] {
  const out: string[] = [];
  let cur = '';
  let quote = false;
  let bracket = false;
  for (let i = 0; i < code.length; i++) {
    const ch = code[i]!;
    if (quote) {
      if (ch === '"') quote = false;
      cur += ch;
      continue;
    }
    if (bracket) {
      if (ch === ']') bracket = false;
      cur += ch;
      continue;
    }
    if (ch === '"') quote = true;
    else if (ch === '[') bracket = true;
    else if (ch === '\\' || ch === '_' || ch === '*') {
      cur += ch + (code[i + 1] ?? '');
      i++;
      continue;
    } else if (ch === ';') {
      out.push(cur);
      cur = '';
      continue;
    }
    cur += ch;
  }
  out.push(cur);
  return out;
}

/** Positions of the digit placeholders, decimal point and exponent of one section. */
function scan(section: string): { digits: number[]; dot: number; exp: number } {
  const digits: number[] = [];
  let dot = -1;
  let exp = section.length;
  let quote = false;
  let bracket = false;
  for (let i = 0; i < section.length; i++) {
    const ch = section[i]!;
    if (quote) {
      if (ch === '"') quote = false;
      continue;
    }
    if (bracket) {
      if (ch === ']') bracket = false;
      continue;
    }
    if (ch === '"') quote = true;
    else if (ch === '[') bracket = true;
    else if (ch === '\\' || ch === '_' || ch === '*') i++;
    else if (
      (ch === 'E' || ch === 'e') &&
      /[+-]/.test(section[i + 1] ?? '') &&
      exp === section.length
    )
      exp = i;
    else if (i < exp && (ch === '0' || ch === '#' || ch === '?')) digits.push(i);
    else if (ch === '.' && i < exp && dot < 0) dot = i;
  }
  return { digits, dot, exp };
}

function stepSection(section: string, dir: 1 | -1): string {
  if (/^\s*general\s*$/i.test(section)) return section;
  const { digits, dot } = scan(section);
  if (!digits.length) return section;
  // Fractions keep their digits; the dash section of an accounting code (`"-"??`) is
  // padding standing for the decimals: one `?` more or less.
  if (digits.some((d) => section[d + 1] === '/' || section[d - 1] === '/')) return section;
  if (dot < 0 && digits.every((d) => section[d] === '?')) {
    const last = digits[digits.length - 1]!;
    if (dir > 0) return section.slice(0, last + 1) + '?' + section.slice(last + 1);
    return digits.length > 1 ? section.slice(0, last) + section.slice(last + 1) : section;
  }
  const after = dot >= 0 ? digits.filter((d) => d > dot) : [];
  if (dir > 0) {
    if (dot >= 0) {
      const at = (after.length ? after[after.length - 1]! : dot) + 1;
      return section.slice(0, at) + '0' + section.slice(at);
    }
    const at = digits[digits.length - 1]! + 1;
    return section.slice(0, at) + '.0' + section.slice(at);
  }
  if (dot < 0) return section;
  if (after.length > 1) {
    const at = after[after.length - 1]!;
    return section.slice(0, at) + section.slice(at + 1);
  }
  // The last decimal goes with the decimal point.
  const cut = after.length ? [dot, after[0]!] : [dot];
  let out = '';
  for (let i = 0; i < section.length; i++) if (!cut.includes(i)) out += section[i];
  return out;
}

/**
 * Number format after Increase decimal (`dir = 1`) / Decrease decimal (`-1`). A General
 * cell starts from the decimals it shows (`shownDecimals`): 12,5 → `0.00` / `0`. Every
 * section of a code gains or loses one decimal place (`#,##0.00` → `#,##0.000`, `0%` →
 * `0.0%`, `0.0` → `0`).
 */
export function stepDecimals(code: string, shownDecimals: number, dir: 1 | -1): string {
  if (!code.trim() || /^general$/i.test(code.trim())) {
    const n = Math.max(0, shownDecimals + dir);
    return n === 0 ? '0' : `0.${'0'.repeat(n)}`;
  }
  return sections(code)
    .map((s) => stepSection(s, dir))
    .join(';');
}

/** Decimals shown in a cell's text (`12,50` → 2 with the decimal separator `,`). */
export function shownDecimals(display: string, decimal: string): number {
  const m = new RegExp(`\\${decimal}(\\d+)`).exec(display);
  return m?.[1]?.length ?? 0;
}

/** Number format categories of the number format box (preset ids of the engine). */
export type NumberCategory =
  | 'general'
  | 'number'
  | 'currencyVnd'
  | 'accounting'
  | 'date'
  | 'dateLong'
  | 'time'
  | 'percent'
  | 'fraction'
  | 'scientific'
  | 'text'
  | 'custom';

/** Category of a format code (by its tokens, outside quotes and brackets). */
export function numberCategory(code: string): NumberCategory {
  const c = code.trim();
  if (!c || /^general$/i.test(c)) return 'general';
  if (c === '@') return 'text';
  const bare = c
    .replace(/"[^"]*"/g, '')
    .replace(/\[\$([^\]-]*)[^\]]*\]/g, '$1')
    .replace(/\[[^\]]*\]/g, '');
  if (/_-\*|\*\s/.test(c) && /[0#]/.test(bare)) return 'accounting';
  if (/[₫$€£¥]/.test(bare)) return 'currencyVnd';
  if (/%/.test(bare)) return 'percent';
  if (/[eE][+-]/.test(bare)) return 'scientific';
  if (/\?\/[?\d]/.test(bare)) return 'fraction';
  if (/[dy]/i.test(bare)) return /dddd|mmmm/i.test(bare) ? 'dateLong' : 'date';
  if (/[hs]/i.test(bare)) return 'time';
  if (/[0#]/.test(bare)) return 'number';
  return 'custom';
}

/** What a formatting shortcut does (`command` of the editor's registry). */
export interface FormatKey {
  command: string;
}

/**
 * The formatting shortcut of a key press on the grid, by physical key so that it works on
 * every keyboard layout: Ctrl+2/3/4 bold / italic / underline, Ctrl+Shift+~ ! @ # $ % ^
 * number formats, Ctrl+Shift+& / _ outline border / no border, Ctrl+9 / Ctrl+0 hide rows /
 * columns and Ctrl+Shift+( / ) unhide, Ctrl++ / Ctrl+- insert / delete cells, Ctrl+Shift+F
 * the Font tab of Format Cells, Shift+F11 a new sheet, Ctrl+PgUp / PgDn the sheets.
 */
export function formatKeyCommand(
  e: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>,
): string | null {
  const ctrl = e.ctrlKey || e.metaKey;
  if (e.altKey) return null;
  if (!ctrl) return e.shiftKey && e.key === 'F11' ? 'sheet.insertBefore' : null;
  if (e.key === 'PageUp' && !e.shiftKey) return 'sheet.previous';
  if (e.key === 'PageDown' && !e.shiftKey) return 'sheet.next';
  // Ctrl++ : the + of the main keyboard (Shift+=) or of the keypad.
  if (e.code === 'NumpadAdd' || e.key === '+' || (e.shiftKey && e.code === 'Equal'))
    return 'cells.insert';
  if (e.code === 'NumpadSubtract' || (!e.shiftKey && e.code === 'Minus')) return 'cells.delete';
  if (e.shiftKey) {
    switch (e.code) {
      case 'Backquote':
        return 'format.numGeneral';
      case 'Digit1':
        return 'format.numNumber';
      case 'Digit2':
        return 'format.numTime';
      case 'Digit3':
        return 'format.numDate';
      case 'Digit4':
        return 'format.numCurrency';
      case 'Digit6':
        return 'format.numScientific';
      case 'Digit7':
        return 'format.borderOutline';
      case 'Minus':
        return 'format.borderNone';
      case 'Digit9':
        return 'cells.unhideRows';
      case 'Digit0':
        return 'cells.unhideCols';
      case 'KeyF':
      case 'KeyP':
        return 'format.fontTab';
      default:
        return null;
    }
  }
  switch (e.code) {
    case 'Digit2':
      return 'format.bold';
    case 'Digit3':
      return 'format.italic';
    case 'Digit4':
      return 'format.underline';
    case 'Digit9':
      return 'cells.hideRows';
    case 'Digit0':
      return 'cells.hideCols';
    // Ctrl+L: create table (Ctrl+T is the registry shortcut).
    case 'KeyL':
      return 'insert.table';
    default:
      return null;
  }
}
