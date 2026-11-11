// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CellDetails, CellFormatState, SheetHit, SheetView } from '@blueoffice/platform';
import { SheetA11y, cellAnnouncement, cellsInView } from '../src/SheetA11y';
import { colName } from '../src/grid';

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = (k: string, o?: Record<string, unknown>) => String(i18n.t(k, o));

const FMT: CellFormatState = {
  numFmt: '0.00',
  fontName: 'Arial',
  fontSize: 11,
  bold: true,
  italic: false,
  underline: 'none',
  strike: false,
  fontColor: '',
  fillColor: '',
  hAlign: '',
  vAlign: '',
  wrap: false,
  indent: 0,
  rotation: 0,
  borders: ['', '', '', ''],
  styleName: 'Normal',
  merged: false,
  locked: true,
  formulaHidden: false,
};

/** A sheet of 40 × 20 pt cells after a 30 × 20 pt header; cell (r, c) shows "rN cM". */
function fakeHost() {
  const hitTest = async (_v: SheetView, x: number, y: number): Promise<SheetHit> => {
    if (x < 30 && y < 20) return { kind: 'corner' };
    if (y < 20) return { kind: 'columnHeader', col: Math.floor((x - 30) / 40) };
    if (x < 30) return { kind: 'rowHeader', row: Math.floor((y - 20) / 20) };
    return { kind: 'cell', row: Math.floor((y - 20) / 20), col: Math.floor((x - 30) / 40) };
  };
  const cell = async (_s: number, row: number, col: number): Promise<CellDetails> => ({
    address: `${colName(col)}${row + 1}`,
    input: row === 0 && col === 0 ? '=SUM(B1:C1)' : `r${row} c${col}`,
    display: row === 0 && col === 0 ? '12.00' : `r${row} c${col}`,
  });
  return { hitTest, cell };
}

const VIEW: SheetView = { sheet: 0, scrollX: 0, scrollY: 0, width: 190, height: 100 };

describe('BlueSheet accessibility', () => {
  it('says the address, value, formula and formatting of a cell', () => {
    expect(
      cellAnnouncement({ address: 'A1', input: '=SUM(B1:C1)', display: '12.00' }, FMT, t),
    ).toBe('A1: 12.00, formula =SUM(B1:C1), bold, format 0.00');
    expect(cellAnnouncement({ address: 'B2', input: '', display: '' }, null, t)).toBe('B2: empty');
  });

  it('finds the cells in view', async () => {
    // 190 pt: columns 0..3 (30 + 4 × 40); 100 pt: rows 0..3.
    expect(await cellsInView(fakeHost(), VIEW)).toEqual({ r0: 0, c0: 0, r1: 3, c1: 3 });
  });

  it('mirrors the visible cells as a grid and announces the active cell', async () => {
    render(
      <I18nextProvider i18n={i18n}>
        <SheetA11y
          host={fakeHost()}
          sheet={0}
          view={VIEW}
          active={{ row: 0, col: 0 }}
          fmt={FMT}
          revision={0}
          label="Grid"
          rows={100}
          cols={10}
        />
      </I18nextProvider>,
    );
    const grid = screen.getByRole('grid', { name: 'Grid' });
    expect(grid.getAttribute('aria-rowcount')).toBe('100');
    await waitFor(() => expect(grid.querySelectorAll('[role="row"]')).toHaveLength(5));
    const headers = [...grid.querySelectorAll('[role="columnheader"]')].map((h) => h.textContent);
    expect(headers).toEqual(['', 'A', 'B', 'C', 'D']);
    const rowHeaders = [...grid.querySelectorAll('[role="rowheader"]')].map((h) => h.textContent);
    expect(rowHeaders).toEqual(['1', '2', '3', '4']);
    const b2 = grid.querySelector('[data-cell="B2"]')!;
    expect(b2.textContent).toBe('r1 c1');
    expect(b2.getAttribute('aria-colindex')).toBe('3');
    expect(b2.parentElement?.getAttribute('aria-rowindex')).toBe('3');
    expect(grid.querySelector('[aria-selected="true"]')?.getAttribute('data-cell')).toBe('A1');
    const live = screen.getByTestId('sheet-a11y-live');
    expect(live.getAttribute('aria-live')).toBe('polite');
    await waitFor(() =>
      expect(live.textContent).toBe('A1: 12.00, formula =SUM(B1:C1), bold, format 0.00'),
    );
    // Hidden from sight, not from assistive technology.
    expect(grid.style.display).not.toBe('none');
  });
});
