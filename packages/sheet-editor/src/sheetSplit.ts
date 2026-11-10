/**
 * Split panes of a worksheet (View › Window › Split, CORE-UI-007): the grid is divided at
 * the active cell into two panes of the same sheet that scroll (and zoom) on their own.
 * The split is a view of this window only — unlike frozen panes (§18.3.1.66 pane) it is not
 * written to the file. Split and Freeze Panes exclude each other: splitting a frozen sheet
 * unfreezes it, and Freeze Panes on a split sheet turns the split into frozen panes at the
 * cell the split was made at.
 */
import { SPLIT_DEFAULT, clampSplit } from '@blueoffice/platform';

export interface SheetSplit {
  /** `horizontal`: the second pane is below; `vertical`: it is to the right. */
  orientation: 'horizontal' | 'vertical';
  /** The first pane's share of the split axis. */
  ratio: number;
  /** The cell the split was made at (top-left cell of the second pane). */
  cell: { row: number; col: number };
  /** Scroll position of the second pane, in points. */
  scroll: { x: number; y: number };
  /** Zoom factor of the second pane. */
  zoom: number;
}

export interface SplitRequest {
  /** The active cell. */
  cell: { row: number; col: number };
  /** Its rectangle in the main pane (points from the pane's top-left corner); `null` when it is scrolled out. */
  rect: { x: number; y: number; w: number; h: number } | null;
  /** Size of the row / column headers (points). */
  headers: { x: number; y: number };
  /** Main pane: size and scroll position in points, zoom factor. */
  view: { width: number; height: number; scrollX: number; scrollY: number };
  zoom: number;
}

/**
 * Where Split puts the bar: above the active cell, or — when that cell is in the first
 * visible row — to its left; a cell in the top-left corner (or out of view) splits the
 * grid in the middle. The second pane starts at the active cell.
 */
export function splitAtCell(q: SplitRequest): SheetSplit {
  const { rect, headers, view } = q;
  const base = { cell: q.cell, zoom: q.zoom };
  const visible =
    !!rect && rect.x < view.width && rect.y < view.height && rect.x >= 0 && rect.y >= 0;
  if (visible && rect.y > headers.y + 0.5) {
    return {
      ...base,
      orientation: 'horizontal',
      ratio: clampSplit(rect.y / view.height),
      scroll: { x: view.scrollX, y: view.scrollY + rect.y - headers.y },
    };
  }
  if (visible && rect.x > headers.x + 0.5) {
    return {
      ...base,
      orientation: 'vertical',
      ratio: clampSplit(rect.x / view.width),
      scroll: { x: view.scrollX + rect.x - headers.x, y: view.scrollY },
    };
  }
  return {
    ...base,
    orientation: 'horizontal',
    ratio: SPLIT_DEFAULT,
    scroll: { x: view.scrollX, y: view.scrollY },
  };
}

/** Freeze Panes on a split sheet: the rows above / columns left of the split cell stay. */
export function freezeFromSplit(s: SheetSplit): { rows: number; cols: number } {
  return s.orientation === 'horizontal'
    ? { rows: s.cell.row, cols: 0 }
    : { rows: 0, cols: s.cell.col };
}

/** Splitting needs the frozen panes gone first. */
export function splitNeedsUnfreeze(frozen: { rows: number; cols: number }): boolean {
  return frozen.rows + frozen.cols > 0;
}

/** Scrolling or zooming the second pane leaves the main pane alone. */
export function updateSplit(
  s: SheetSplit,
  change: Partial<Pick<SheetSplit, 'ratio' | 'scroll' | 'zoom'>>,
): SheetSplit {
  return {
    ...s,
    ...change,
    ...(change.ratio !== undefined ? { ratio: clampSplit(change.ratio) } : {}),
  };
}
