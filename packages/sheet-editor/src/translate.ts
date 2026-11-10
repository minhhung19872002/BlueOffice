/**
 * BlueSheet translation (CORE-LANG-001): the text cells of the selection are translated — not
 * numbers, formulas or empty cells — either in place (one undo step) or into new columns
 * inserted right of the selection (each translated cell lands in the matching column of the
 * inserted block, so nothing existing is overwritten). The cells are read and translated first;
 * the sheet changes only once the translation succeeded (cancelling leaves it as it was).
 * Whole-row / whole-column selections stop after a long run of empty rows, as read aloud does.
 */
import {
  translateTexts,
  type SheetHost,
  type TranslateTextsOptions,
  type TranslationProvider,
} from '@blueoffice/platform';
import type { Range } from './grid';
import { EMPTY_ROWS_STOP, MAX_READ_VISITED } from './readAloud';

/** Most cells translated in one run. */
export const MAX_TRANSLATE_CELLS = 5_000;

export type CellTranslateMode = 'newColumn' | 'inPlace';

/** Is `input` a text to translate (not a formula, number, logical or error)? */
export function translatableInput(input: string): boolean {
  if (!input.trim() || input.startsWith('=')) return false;
  if (!/\p{L}/u.test(input)) return false;
  if (/^(TRUE|FALSE|#[A-Z/0!?]+)$/i.test(input.trim())) return false;
  return true;
}

/** Cell input that stays text (a leading `=`, `+`, `-` or `@` would start a formula). */
export function asTextInput(text: string): string {
  return /^[=+\-@]/.test(text) ? `'${text}` : text;
}

/** Text cells of `range` in reading order: `[row, col, text]` (a leading `'` dropped). */
export async function translatableCells(
  host: Pick<SheetHost, 'cell'>,
  sheet: number,
  range: Range,
): Promise<[number, number, string][]> {
  const out: [number, number, string][] = [];
  let visited = 0;
  let emptyRows = 0;
  for (let row = range.r0; row <= range.r1; row++) {
    const cols: number[] = [];
    for (let col = range.c0; col <= range.c1; col++) cols.push(col);
    visited += cols.length;
    const details = await Promise.all(cols.map((col) => host.cell(sheet, row, col)));
    let any = false;
    details.forEach((d, i) => {
      if (d.input.trim()) any = true;
      if (out.length >= MAX_TRANSLATE_CELLS || !translatableInput(d.input)) return;
      out.push([row, cols[i]!, d.input.startsWith("'") ? d.input.slice(1) : d.input]);
    });
    emptyRows = any ? 0 : emptyRows + 1;
    if (emptyRows >= EMPTY_ROWS_STOP || visited >= MAX_READ_VISITED) break;
    if (out.length >= MAX_TRANSLATE_CELLS) break;
  }
  return out;
}

/** Translates the text cells of `range`; returns how many cells were written. */
export async function translateCells(
  host: Pick<SheetHost, 'cell' | 'setInputs' | 'structural'>,
  sheet: number,
  range: Range,
  provider: TranslationProvider,
  from: string,
  to: string,
  mode: CellTranslateMode,
  options: TranslateTextsOptions = {},
): Promise<number> {
  const cells = await translatableCells(host, sheet, range);
  if (!cells.length) return 0;
  const out = await translateTexts(
    provider,
    cells.map((c) => c[2]),
    from,
    to,
    options,
  );
  if (options.signal?.aborted) return 0;
  // The columns that hold text (a whole-row selection does not insert 16 384 columns).
  const lastCol = Math.max(...cells.map((c) => c[1]));
  const at = Math.min(range.c1, lastCol) + 1;
  const width = Math.min(range.c1, lastCol) - range.c0 + 1;
  if (mode === 'newColumn') await host.structural(sheet, false, at, width, false);
  const writes = cells.flatMap(([row, col, text], i): [number, number, string][] =>
    mode === 'inPlace' && out[i] === text
      ? []
      : [[row, mode === 'newColumn' ? at + (col - range.c0) : col, asTextInput(out[i]!)]],
  );
  if (writes.length) await host.setInputs(sheet, writes);
  return writes.length;
}
