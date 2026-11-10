/**
 * BlueSheet read aloud (CORE-LANG-003): the selected cells are read row by row (left to right
 * in each row), each cell's formatted text as one unit; the active cell follows the cell being
 * read inside the selection. Empty cells are skipped. Whole-row / whole-column selections stop
 * after a long run of empty rows or a bounded number of cells (SpreadsheetML keeps no reading
 * order of its own; this order is our choice).
 */
import type { ReadUnit } from '@blueoffice/editor';
import { detectLanguage, speechLanguage, type SheetHost } from '@blueoffice/platform';
import type { Range } from './grid';

/** Cells looked at in one run. */
export const MAX_READ_VISITED = 20_000;
/** Cells with text read in one run. */
export const MAX_READ_CELLS = 2_000;
/** Empty rows after which a selection is considered to have no more text. */
export const EMPTY_ROWS_STOP = 200;

export interface ReadCell {
  row: number;
  col: number;
}

/** Units (one per non-empty cell) and the cell of each unit, in reading order. */
export async function sheetReadUnits(
  host: Pick<SheetHost, 'cell'>,
  sheet: number,
  range: Range,
  fallbackLang: string,
): Promise<{ units: ReadUnit[]; cells: ReadCell[] }> {
  const units: ReadUnit[] = [];
  const cells: ReadCell[] = [];
  let visited = 0;
  let emptyRows = 0;
  for (let row = range.r0; row <= range.r1; row++) {
    const cols: number[] = [];
    for (let col = range.c0; col <= range.c1; col++) cols.push(col);
    visited += cols.length;
    const details = await Promise.all(cols.map((col) => host.cell(sheet, row, col)));
    let any = false;
    details.forEach((d, i) => {
      const text = d.display.trim() ? d.display : '';
      if (!text || units.length >= MAX_READ_CELLS) return;
      any = true;
      units.push({ text, lang: speechLanguage(detectLanguage(text), null, fallbackLang) });
      cells.push({ row, col: cols[i]! });
    });
    emptyRows = any ? 0 : emptyRows + 1;
    if (emptyRows >= EMPTY_ROWS_STOP || visited >= MAX_READ_VISITED) break;
    if (units.length >= MAX_READ_CELLS) break;
  }
  return { units, cells };
}
