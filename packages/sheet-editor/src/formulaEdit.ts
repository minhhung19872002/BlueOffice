/**
 * Helpers for typing a formula in a cell (pure, unit-tested): the cell references of the
 * text with their highlight colours, whether an arrow key / click picks a reference at the
 * caret ("point" mode), the function name being typed (completion list) and the function
 * call around the caret (argument hint). References follow ECMA-376 Part 1, §18.17.2.3
 * (A1 references, `$` absolute parts, `Sheet!A1`, `'My sheet'!A1`).
 */

import { colName, type Range } from './grid';

/** Highlight colours of the references, in order of first appearance (cycled). */
export const REF_COLORS = [
  '#1f6fd1',
  '#d1342f',
  '#7a3db8',
  '#13804a',
  '#b85c00',
  '#c2187a',
  '#0b7a8a',
] as const;

export interface FormulaRef {
  /** Offsets of the reference in the text (sheet prefix included). */
  start: number;
  end: number;
  /** Sheet name of a `Sheet!A1` reference (unquoted), else undefined (the formula's sheet). */
  sheet?: string;
  range: Range;
  color: string;
}

/** Text with string literals blanked so references inside quotes are ignored. */
export function maskStrings(text: string): string {
  let out = '';
  let quoted = false;
  for (const ch of text) {
    if (ch === '"') {
      quoted = !quoted;
      out += ch;
    } else out += quoted ? ' ' : ch;
  }
  return out;
}

const SHEET = String.raw`(?:'(?:[^']|'')+'|[A-Za-z_À-￿][\w.À-￿]*)!`;
const CELL = String.raw`\$?[A-Za-z]{1,3}\$?\d{1,7}`;
const REF_RE = new RegExp(`(${SHEET})?(${CELL})(?::(${CELL}))?`, 'g');
const PART = /^\$?([A-Za-z]{1,3})\$?(\d{1,7})$/;

function cellOf(text: string): { row: number; col: number } | null {
  const m = PART.exec(text);
  if (!m) return null;
  let col = 0;
  for (const ch of m[1]!.toUpperCase()) col = col * 26 + (ch.charCodeAt(0) - 64);
  const row = Number(m[2]) - 1;
  if (row < 0 || row > 1_048_575 || col - 1 > 16_383) return null;
  return { row, col: col - 1 };
}

/** The cell references of a formula (`=` first), coloured like the range borders drawn. */
export function formulaRefs(text: string): FormulaRef[] {
  if (!text.startsWith('=')) return [];
  const masked = maskStrings(text);
  const out: FormulaRef[] = [];
  const colours = new Map<string, string>();
  for (const m of masked.matchAll(REF_RE)) {
    const start = m.index;
    const end = start + m[0].length;
    const before = masked[start - 1] ?? '';
    const after = masked[end] ?? '';
    // Part of a name (SUM1, my_A1), a function call (LOG10(), or a longer token.
    if (/[A-Za-z0-9_.$]/.test(before) || /[A-Za-z0-9_(!]/.test(after)) continue;
    const a = cellOf(m[2]!);
    const b = m[3] ? cellOf(m[3]) : a;
    if (!a || !b) continue;
    const sheet = m[1]
      ? m[1]
          .slice(0, -1)
          .replace(/^'(.*)'$/, '$1')
          .replace(/''/g, "'")
      : undefined;
    const key = `${sheet ?? ''}!${m[2]!.replace(/\$/g, '').toUpperCase()}:${(m[3] ?? m[2]!).replace(/\$/g, '').toUpperCase()}`;
    let color = colours.get(key);
    if (!color) {
      color = REF_COLORS[colours.size % REF_COLORS.length]!;
      colours.set(key, color);
    }
    out.push({
      start,
      end,
      ...(sheet !== undefined ? { sheet } : {}),
      range: {
        r0: Math.min(a.row, b.row),
        c0: Math.min(a.col, b.col),
        r1: Math.max(a.row, b.row),
        c1: Math.max(a.col, b.col),
      },
      color,
    });
  }
  return out;
}

/** The text cut at its references, each piece with the colour of its reference (if any). */
export function mirrorParts(
  text: string,
  refs: readonly FormulaRef[],
): { text: string; color?: string }[] {
  const out: { text: string; color?: string }[] = [];
  let at = 0;
  for (const r of refs) {
    if (r.start > at) out.push({ text: text.slice(at, r.start) });
    out.push({ text: text.slice(r.start, r.end), color: r.color });
    at = r.end;
  }
  if (at < text.length) out.push({ text: text.slice(at) });
  return out;
}

/**
 * Whether a reference can be picked at `caret` (arrow keys / a click put one there): the
 * text is a formula and the caret follows `=`, an operator, `(`, a separator or a space —
 * the places where a reference may start.
 */
export function canPoint(text: string, caret: number): boolean {
  if (!text.startsWith('=') || caret < 1) return false;
  const masked = maskStrings(text.slice(0, caret));
  // Inside an unterminated string literal.
  if ((masked.match(/"/g)?.length ?? 0) % 2 === 1) return false;
  const prev = masked.trimEnd().slice(-1);
  return /[=(,;+\-*/^&<>]/.test(prev);
}

/** A1 text of a picked range (`sheet` given: `Sheet!A1`, quoted when needed). */
export function refText(range: Range, sheet?: string): string {
  const a = `${colName(range.c0)}${range.r0 + 1}`;
  const b = `${colName(range.c1)}${range.r1 + 1}`;
  const cell = a === b ? a : `${a}:${b}`;
  if (sheet === undefined) return cell;
  const plain = /^[A-Za-z_À-￿][\w.À-￿]*$/.test(sheet) && !/^[A-Za-z]{1,3}\d+$/.test(sheet);
  return `${plain ? sheet : `'${sheet.replace(/'/g, "''")}'`}!${cell}`;
}

/** The function name being typed before `caret` (letters after `=`, an operator or `(`). */
export function functionPrefix(
  text: string,
  caret: number,
): { from: number; prefix: string } | null {
  if (!text.startsWith('=')) return null;
  const head = maskStrings(text.slice(0, caret));
  const m = /([A-Za-z][A-Za-z0-9.]*)$/.exec(head);
  if (!m) return null;
  const from = caret - m[1]!.length;
  const before = head.slice(0, from).trimEnd().slice(-1);
  if (!/[=(,;+\-*/^&<>]/.test(before)) return null;
  // A reference being typed (A1, AB12) is not a function name.
  if (/^[A-Za-z]{1,3}\d+$/.test(m[1]!)) return null;
  return { from, prefix: m[1]! };
}

/** Function names starting with `prefix` (ignoring case), at most `limit`. */
export function functionMatches(names: readonly string[], prefix: string, limit = 12): string[] {
  const p = prefix.toUpperCase();
  return names.filter((n) => n.startsWith(p)).slice(0, limit);
}

/** The innermost function call around `caret`: its name and the index of the argument. */
export function callAt(text: string, caret: number): { name: string; arg: number } | null {
  if (!text.startsWith('=')) return null;
  const head = maskStrings(text.slice(0, caret));
  const stack: { name: string; arg: number }[] = [];
  for (let i = 0; i < head.length; i++) {
    const ch = head[i]!;
    if (ch === '(') {
      const m = /([A-Za-z][A-Za-z0-9.]*)\s*$/.exec(head.slice(0, i));
      stack.push({ name: m ? m[1]!.toUpperCase() : '', arg: 0 });
    } else if (ch === ')') stack.pop();
    else if ((ch === ',' || ch === ';') && stack.length) stack[stack.length - 1]!.arg++;
    else if (ch === '{') stack.push({ name: '', arg: 0 });
    else if (ch === '}') stack.pop();
  }
  for (let i = stack.length - 1; i >= 0; i--) if (stack[i]!.name) return stack[i]!;
  return null;
}

/**
 * Argument names of common functions for the hint shown while typing a call (BlueOffice's
 * own wording; `[x]` optional, `…` repeats).
 */
export const FUNCTION_ARGS: Readonly<Record<string, readonly string[]>> = {
  SUM: ['number1', '[number2]', '…'],
  AVERAGE: ['number1', '[number2]', '…'],
  COUNT: ['value1', '[value2]', '…'],
  COUNTA: ['value1', '[value2]', '…'],
  COUNTBLANK: ['range'],
  MIN: ['number1', '[number2]', '…'],
  MAX: ['number1', '[number2]', '…'],
  PRODUCT: ['number1', '[number2]', '…'],
  IF: ['test', 'value_if_true', '[value_if_false]'],
  IFS: ['test1', 'value1', '[test2, value2]', '…'],
  IFERROR: ['value', 'value_if_error'],
  AND: ['test1', '[test2]', '…'],
  OR: ['test1', '[test2]', '…'],
  NOT: ['test'],
  SUMIF: ['range', 'criteria', '[sum_range]'],
  SUMIFS: ['sum_range', 'criteria_range1', 'criteria1', '…'],
  COUNTIF: ['range', 'criteria'],
  COUNTIFS: ['criteria_range1', 'criteria1', '…'],
  AVERAGEIF: ['range', 'criteria', '[average_range]'],
  VLOOKUP: ['lookup_value', 'table', 'column_index', '[approximate]'],
  HLOOKUP: ['lookup_value', 'table', 'row_index', '[approximate]'],
  XLOOKUP: [
    'lookup_value',
    'lookup_array',
    'return_array',
    '[if_not_found]',
    '[match_mode]',
    '[search_mode]',
  ],
  INDEX: ['array', 'row', '[column]'],
  MATCH: ['lookup_value', 'lookup_array', '[match_type]'],
  ROUND: ['number', 'digits'],
  ROUNDUP: ['number', 'digits'],
  ROUNDDOWN: ['number', 'digits'],
  INT: ['number'],
  ABS: ['number'],
  MOD: ['number', 'divisor'],
  LEFT: ['text', '[count]'],
  RIGHT: ['text', '[count]'],
  MID: ['text', 'start', 'count'],
  LEN: ['text'],
  TRIM: ['text'],
  UPPER: ['text'],
  LOWER: ['text'],
  PROPER: ['text'],
  CONCAT: ['text1', '[text2]', '…'],
  CONCATENATE: ['text1', '[text2]', '…'],
  TEXT: ['value', 'format'],
  VALUE: ['text'],
  DATE: ['year', 'month', 'day'],
  YEAR: ['date'],
  MONTH: ['date'],
  DAY: ['date'],
  TODAY: [],
  NOW: [],
  DATEDIF: ['start_date', 'end_date', 'unit'],
  NETWORKDAYS: ['start_date', 'end_date', '[holidays]'],
  EDATE: ['start_date', 'months'],
  EOMONTH: ['start_date', 'months'],
  WEEKDAY: ['date', '[type]'],
  UNIQUE: ['array', '[by_column]', '[exactly_once]'],
  FILTER: ['array', 'include', '[if_empty]'],
  SORT: ['array', '[sort_index]', '[order]', '[by_column]'],
  SUBTOTAL: ['function_number', 'ref1', '[ref2]', '…'],
};

/** The hint parts of a call: the name and its arguments, the current one marked. */
export function argumentHint(
  name: string,
  arg: number,
): { name: string; args: { text: string; current: boolean }[] } | null {
  const args = FUNCTION_ARGS[name];
  if (!args) return null;
  const repeat = args.indexOf('…');
  // Past the listed ones, a repeating list keeps the one before `…` current.
  const current = repeat > 0 && arg >= repeat ? repeat - 1 : arg;
  return { name, args: args.map((text, i) => ({ text, current: i === current })) };
}
