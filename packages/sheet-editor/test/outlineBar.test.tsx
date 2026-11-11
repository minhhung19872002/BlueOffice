// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SheetOutlineInfo } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { GridGeometry, Seg } from '../src/gridView';
import { groupMarks, OutlineBar, outlineBarSize, visibleSpans } from '../src/outlineBar';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (ui: ReactNode) => render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);

const label = { op: 'glyphs' } as unknown as Seg['label'];
/** 30 pt row headers, 20 pt column headers; rows 20 pt from y = 20, columns 50 pt from x = 30. */
function geometry(rows: number[], cols: number[]): GridGeometry {
  return {
    width: 600,
    height: 400,
    headerW: 30,
    headerH: 20,
    rows: [
      {
        start: 20,
        end: 400,
        min: 0,
        max: 1000,
        segs: rows.map((index, i) => ({ index, start: 20 + i * 20, end: 40 + i * 20, label })),
      },
    ],
    cols: [
      {
        start: 30,
        end: 600,
        min: 0,
        max: 100,
        segs: cols.map((index, i) => ({ index, start: 30 + i * 50, end: 80 + i * 50, label })),
      },
    ],
  };
}

/** Subtotals of a list: detail rows 1-2, 4-6, 8 (level 2), subtotals 3, 7, 9, total 10. */
const subtotals: SheetOutlineInfo = {
  rowLevels: 2,
  colLevels: 0,
  rows: [
    { level: 1, first: 1, last: 9, summary: 10, collapsed: false },
    { level: 2, first: 1, last: 2, summary: 3, collapsed: false },
    { level: 2, first: 4, last: 6, summary: 7, collapsed: true },
    { level: 2, first: 8, last: 8, summary: 9, collapsed: false },
  ],
  cols: [],
};

describe('outline bar', () => {
  it('is one 24 px lane per level plus one', () => {
    expect(outlineBarSize(null)).toEqual({ left: 0, top: 0 });
    expect(outlineBarSize(subtotals)).toEqual({ left: 72, top: 0 });
    expect(outlineBarSize({ ...subtotals, colLevels: 1 })).toEqual({ left: 72, top: 48 });
  });

  it('places brackets over the shown detail and buttons on the summary rows', () => {
    // Rows 4-6 hidden: the visible rows are 0-3 and 7-10, at scale 1.
    const g = geometry([0, 1, 2, 3, 7, 8, 9, 10], [0, 1, 2]);
    const spans = visibleSpans(g, true, 1);
    expect(spans.get(7)).toEqual({ start: 100, end: 120 });
    const marks = groupMarks(subtotals.rows, spans);
    // Outer group: bracket from row 1 (40) to row 9 (160), button at row 10's middle.
    expect(marks[0]).toMatchObject({ line: { start: 40, end: 160 }, button: 170 });
    // Collapsed group 4-6: no bracket, + at row 7.
    expect(marks[2]).toMatchObject({ line: null, button: 110 });
    // Group 1-2: bracket 40-80, button in the middle of row 3 (80-100); scale 2 doubles.
    expect(marks[1]).toMatchObject({ line: { start: 40, end: 80 }, button: 90 });
    expect(groupMarks(subtotals.rows, visibleSpans(g, true, 2))[1]).toMatchObject({
      line: { start: 80, end: 160 },
      button: 180,
    });
    // Nothing on screen: no mark.
    expect(groupMarks(subtotals.rows, visibleSpans(geometry([20, 21], []), true, 1))).toEqual([]);
  });

  it('runs level buttons and +/− buttons', () => {
    const onLevel = vi.fn();
    const onToggle = vi.fn();
    wrap(
      <OutlineBar
        outline={subtotals}
        geometry={geometry([0, 1, 2, 3, 7, 8, 9, 10], [0, 1, 2])}
        scale={1}
        width={600}
        height={400}
        onLevel={onLevel}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByRole('toolbar', { name: 'Row outline' })).toBeTruthy();
    expect(screen.queryByTestId('outline-bar-cols')).toBeNull();
    // Levels 1, 2, 3 (3 = everything).
    fireEvent.click(screen.getByRole('button', { name: 'Show row level 2' }));
    expect(onLevel).toHaveBeenCalledWith(true, 2);
    expect(screen.getByRole('button', { name: 'Show row level 3' })).toBeTruthy();
    // − hides rows 1-2 (summary row 3 is "row 4" on screen), + shows rows 4-6.
    const hide = screen.getByRole('button', { name: 'Hide detail of row 4' });
    expect(hide.textContent).toBe('−');
    expect(hide.getAttribute('aria-expanded')).toBe('true');
    fireEvent.click(hide);
    expect(onToggle).toHaveBeenLastCalledWith(true, subtotals.rows[1]);
    const show = screen.getByRole('button', { name: 'Show detail of row 8' });
    expect(show.textContent).toBe('+');
    fireEvent.click(show);
    expect(onToggle).toHaveBeenLastCalledWith(true, subtotals.rows[2]);
    // The row-level buttons sit in the column-header band, lane by lane (24 px).
    const l1 = screen.getByTestId('outline-row-level-1');
    const l2 = screen.getByTestId('outline-row-level-2');
    expect(parseFloat(l2.style.left) - parseFloat(l1.style.left)).toBe(24);
    expect(parseFloat(l1.style.top)).toBe(2);
  });

  it('labels column groups with letters', () => {
    const cols: SheetOutlineInfo = {
      rowLevels: 0,
      colLevels: 1,
      rows: [],
      cols: [{ level: 1, first: 1, last: 2, summary: 3, collapsed: false }],
    };
    wrap(
      <OutlineBar
        outline={cols}
        geometry={geometry([0], [0, 1, 2, 3])}
        scale={1}
        width={600}
        height={400}
        onLevel={() => {}}
        onToggle={() => {}}
      />,
    );
    expect(screen.getByRole('button', { name: 'Hide detail of column D' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Show column level 2' })).toBeTruthy();
  });
});
