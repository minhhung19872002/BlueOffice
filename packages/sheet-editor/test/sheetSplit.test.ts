import { describe, expect, it } from 'vitest';
import {
  freezeFromSplit,
  splitAtCell,
  splitNeedsUnfreeze,
  updateSplit,
  type SplitRequest,
} from '../src/sheetSplit';

const view = { width: 800, height: 400, scrollX: 0, scrollY: 0 };
const headers = { x: 30, y: 15 };
const request = (over: Partial<SplitRequest>): SplitRequest => ({
  cell: { row: 5, col: 2 },
  rect: { x: 158, y: 115, w: 64, h: 20 },
  headers,
  view,
  zoom: 1,
  ...over,
});

describe('sheet split panes', () => {
  it('splits above the active cell; the second pane starts at that cell', () => {
    const s = splitAtCell(request({}));
    expect(s.orientation).toBe('horizontal');
    expect(s.ratio).toBeCloseTo(115 / 400, 5);
    // Rows above the cell (100 pt below the header) stay in the first pane.
    expect(s.scroll).toEqual({ x: 0, y: 100 });
    expect(s.cell).toEqual({ row: 5, col: 2 });
    expect(s.zoom).toBe(1);
  });

  it('a scrolled view adds its own scroll position', () => {
    const s = splitAtCell(
      request({
        view: { ...view, scrollX: 40, scrollY: 600 },
        rect: { x: 158, y: 215, w: 64, h: 20 },
      }),
    );
    expect(s.scroll).toEqual({ x: 40, y: 800 });
  });

  it('a cell in the first visible row splits to its left', () => {
    const s = splitAtCell(
      request({ cell: { row: 0, col: 3 }, rect: { x: 222, y: 15, w: 64, h: 20 } }),
    );
    expect(s.orientation).toBe('vertical');
    expect(s.ratio).toBeCloseTo(222 / 800, 5);
    expect(s.scroll).toEqual({ x: 192, y: 0 });
  });

  it('the top-left cell or a cell out of view splits in the middle', () => {
    const corner = splitAtCell(
      request({ cell: { row: 0, col: 0 }, rect: { x: 30, y: 15, w: 64, h: 20 } }),
    );
    expect(corner).toMatchObject({ orientation: 'horizontal', ratio: 0.5, scroll: { x: 0, y: 0 } });
    expect(splitAtCell(request({ rect: null })).ratio).toBe(0.5);
    expect(splitAtCell(request({ rect: { x: 158, y: 4000, w: 64, h: 20 } })).ratio).toBe(0.5);
  });

  it('keeps both panes usable when the cell is near an edge', () => {
    const s = splitAtCell(request({ rect: { x: 158, y: 390, w: 64, h: 20 } }));
    expect(s.ratio).toBe(0.85);
    expect(splitAtCell(request({ rect: { x: 158, y: 20, w: 64, h: 20 } })).ratio).toBe(0.15);
  });

  it('the second pane scrolls and zooms on its own', () => {
    const s = splitAtCell(request({}));
    const moved = updateSplit(s, { scroll: { x: 0, y: 5000 }, zoom: 1.5 });
    expect(moved.scroll.y).toBe(5000);
    expect(moved.zoom).toBe(1.5);
    expect(moved.ratio).toBe(s.ratio);
    expect(s.scroll.y).toBe(100);
    expect(updateSplit(s, { ratio: 2 }).ratio).toBe(0.85);
  });

  it('freeze and split exclude each other', () => {
    expect(splitNeedsUnfreeze({ rows: 0, cols: 0 })).toBe(false);
    expect(splitNeedsUnfreeze({ rows: 1, cols: 0 })).toBe(true);
    expect(splitNeedsUnfreeze({ rows: 0, cols: 2 })).toBe(true);
    // Freeze Panes on a split sheet freezes what the first pane showed.
    expect(freezeFromSplit(splitAtCell(request({})))).toEqual({ rows: 5, cols: 0 });
    expect(
      freezeFromSplit(
        splitAtCell(request({ cell: { row: 0, col: 3 }, rect: { x: 222, y: 15, w: 64, h: 20 } })),
      ),
    ).toEqual({ rows: 0, cols: 3 });
  });
});
