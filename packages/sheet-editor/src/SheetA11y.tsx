/**
 * Screen-reader access to the BlueSheet grid (WCAG 2.2 1.3.1, 4.1.2): the cells are pixels on a
 * canvas, so a visually hidden `role="grid"` mirrors the rows and columns in view (column and row
 * headers, `aria-rowindex` / `aria-colindex`, the active cell `aria-selected`), and a polite live
 * region announces the active cell when it moves or changes: its address, value, formula and a
 * summary of its formatting.
 */
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type { CellDetails, CellFormatState, SheetHost, SheetView } from '@blueoffice/platform';
import { colName, type CellPos } from './grid';

/** Visually hidden but in the accessibility tree (not `display: none`). */
const HIDDEN: CSSProperties = {
  position: 'absolute',
  left: 0,
  top: 0,
  width: 1,
  height: 1,
  margin: -1,
  padding: 0,
  border: 0,
  overflow: 'hidden',
  clipPath: 'inset(50%)',
  whiteSpace: 'nowrap',
  pointerEvents: 'none',
};

/** Most rows and columns mirrored (the grid in view is smaller). */
export const MAX_ROWS = 60;
export const MAX_COLS = 30;

type Translate = (key: string, opts?: Record<string, unknown>) => string;

/** What is said about the active cell: address, value, formula, formatting summary. */
export function cellAnnouncement(d: CellDetails, f: CellFormatState | null, t: Translate): string {
  const parts = [
    t('a11y.cell', {
      address: d.address,
      value: d.display.trim() ? d.display : t('a11y.emptyCell'),
    }),
  ];
  if (d.input.startsWith('=')) parts.push(t('a11y.formula', { formula: d.input }));
  if (f) {
    if (f.bold) parts.push(t('a11y.bold'));
    if (f.italic) parts.push(t('a11y.italic'));
    if (f.underline && f.underline !== 'none') parts.push(t('a11y.underline'));
    if (f.strike) parts.push(t('a11y.strike'));
    if (f.numFmt && f.numFmt !== 'General')
      parts.push(t('a11y.numberFormat', { format: f.numFmt }));
    if (f.merged) parts.push(t('a11y.merged'));
  }
  return parts.join(', ');
}

/** Rows and columns of the cells in `view` (probing the grid from the top-left corner). */
export async function cellsInView(
  host: Pick<SheetHost, 'hitTest'>,
  view: SheetView,
): Promise<{ r0: number; c0: number; r1: number; c1: number } | null> {
  const cell = async (x: number, y: number) => {
    const h = await host.hitTest(view, x, y);
    return 'row' in h && 'col' in h ? { row: h.row, col: h.col } : null;
  };
  let tl: { row: number; col: number } | null = null;
  for (let k = 1; k <= 12 && !tl; k++) tl = await cell(k * 8, k * 4);
  const br =
    (await cell(view.width - 2, view.height - 2)) ??
    (await cell(view.width - 24, view.height - 24));
  if (!tl) return null;
  const end = br ?? tl;
  return {
    r0: tl.row,
    c0: tl.col,
    r1: Math.min(Math.max(end.row, tl.row), tl.row + MAX_ROWS - 1),
    c1: Math.min(Math.max(end.col, tl.col), tl.col + MAX_COLS - 1),
  };
}

export interface SheetA11yProps {
  host: Pick<SheetHost, 'hitTest' | 'cell'> | null;
  sheet: number;
  view: SheetView;
  active: CellPos;
  /** Formatting of the active cell (ribbon state). */
  fmt: CellFormatState | null;
  /** Changes when the workbook changes (cells read again). */
  revision: number;
  /** Name of the grid. */
  label: string;
  /** Rows and columns of the sheet (`aria-rowcount` / `aria-colcount`). */
  rows?: number;
  cols?: number;
}

/** The hidden grid and the live region of the active cell. */
export function SheetA11y({
  host,
  sheet,
  view,
  active,
  fmt,
  revision,
  label,
  rows,
  cols,
}: SheetA11yProps) {
  const { t } = useTranslation();
  const [cells, setCells] = useState<{
    range: { r0: number; c0: number; r1: number; c1: number };
    text: string[][];
  } | null>(null);
  const [said, setSaid] = useState('');
  const seq = useRef(0);

  // The cells in view (after a short pause while scrolling or typing).
  useEffect(() => {
    if (!host) return;
    const n = ++seq.current;
    const timer = setTimeout(() => {
      void (async () => {
        const range = await cellsInView(host, view);
        if (!range || n !== seq.current) return;
        const text: string[][] = [];
        for (let r = range.r0; r <= range.r1; r++) {
          const row = await Promise.all(
            Array.from({ length: range.c1 - range.c0 + 1 }, (_, i) =>
              host.cell(sheet, r, range.c0 + i).then((d) => d.display),
            ),
          );
          text.push(row);
        }
        if (n === seq.current) setCells({ range, text });
      })().catch(() => {});
    }, 120);
    return () => clearTimeout(timer);
  }, [host, sheet, view, revision]);

  // The active cell, said when it moves or its formatting / content changes.
  useEffect(() => {
    if (!host) return;
    let cancelled = false;
    void host
      .cell(sheet, active.row, active.col)
      .then((d) => {
        if (!cancelled) setSaid(cellAnnouncement(d, fmt, t));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, sheet, active.row, active.col, fmt, revision, t]);

  const r = cells?.range;
  return (
    <>
      <div
        role="grid"
        aria-label={label}
        aria-rowcount={rows ?? -1}
        aria-colcount={cols ?? -1}
        aria-readonly="true"
        data-testid="sheet-a11y-grid"
        style={HIDDEN}
      >
        {r && cells && (
          <>
            <div role="row" aria-rowindex={1}>
              <span role="columnheader" aria-colindex={1} />
              {cells.text[0]?.map((_, i) => (
                <span role="columnheader" key={i} aria-colindex={r.c0 + i + 2}>
                  {colName(r.c0 + i)}
                </span>
              ))}
            </div>
            {cells.text.map((row, ri) => {
              const rowNo = r.r0 + ri;
              return (
                <div role="row" key={rowNo} aria-rowindex={rowNo + 2}>
                  <span role="rowheader" aria-colindex={1}>
                    {rowNo + 1}
                  </span>
                  {row.map((v, ci) => {
                    const col = r.c0 + ci;
                    const isActive = rowNo === active.row && col === active.col;
                    return (
                      <span
                        role="gridcell"
                        key={col}
                        aria-colindex={col + 2}
                        aria-selected={isActive}
                        data-cell={`${colName(col)}${rowNo + 1}`}
                      >
                        {v}
                      </span>
                    );
                  })}
                </div>
              );
            })}
          </>
        )}
      </div>
      <div aria-live="polite" aria-atomic="true" data-testid="sheet-a11y-live" style={HIDDEN}>
        {said}
      </div>
    </>
  );
}
