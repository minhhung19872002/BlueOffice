/**
 * F4 while editing a formula (CORE-UI-004): the cell reference at the caret cycles
 * A1 → $A$1 → A$1 → $A1 → A1 (ECMA-376 Part 1, §18.17.2.3 cell references: `$` marks an
 * absolute row / column). A range (A1:B2) changes both ends together.
 */

const REF = /\$?[A-Za-z]{1,3}\$?\d{1,7}(?::\$?[A-Za-z]{1,3}\$?\d{1,7})?/g;
const PART = /^(\$?)([A-Za-z]{1,3})(\$?)(\d{1,7})$/;

/** The next absolute / relative state: [column absolute, row absolute]. */
function nextState(col: boolean, row: boolean): [boolean, boolean] {
  if (!col && !row) return [true, true];
  if (col && row) return [false, true];
  if (!col && row) return [true, false];
  return [false, false];
}

/** Text with string literals blanked so references inside quotes are ignored. */
function maskStrings(text: string): string {
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

export interface CycledReference {
  text: string;
  /** The changed reference (selected in the editor afterwards). */
  start: number;
  end: number;
}

/**
 * Cycle the reference touching `caret` (inside it or right after it). Null when the text is
 * not a formula or no reference is there.
 */
export function cycleReference(text: string, caret: number): CycledReference | null {
  if (!text.startsWith('=')) return null;
  const masked = maskStrings(text);
  for (const m of masked.matchAll(REF)) {
    const start = m.index;
    const end = start + m[0].length;
    if (caret < start || caret > end) continue;
    const before = masked[start - 1] ?? '';
    const after = masked[end] ?? '';
    // Part of a name (SUM1, my_A1), a function (LOG10(), or a sheet name (Sheet1!).
    if (/[A-Za-z0-9_.]/.test(before) || /[A-Za-z0-9_(!]/.test(after)) continue;
    const parts = m[0].split(':').map((p) => PART.exec(p));
    if (parts.some((p) => !p)) continue;
    const first = parts[0]!;
    const [col, row] = nextState(first[1] === '$', first[3] === '$');
    const ref = parts.map((p) => `${col ? '$' : ''}${p![2]}${row ? '$' : ''}${p![4]}`).join(':');
    return { text: text.slice(0, start) + ref + text.slice(end), start, end: start + ref.length };
  }
  return null;
}
