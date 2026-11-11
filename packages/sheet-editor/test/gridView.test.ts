import { describe, expect, it } from 'vitest';
import type { DisplayCommand, PageDisplayList, Rgba } from '@blueoffice/platform';
import {
  ENGINE_CHROME,
  SHEET_CHROME,
  cellNear,
  chromePage,
  expandMerges,
  geometryFromPage,
  hitLocal,
  rangeBox,
  rangeBoxes,
  segSize,
} from '../src/gridView';
import { colName } from '../src/grid';
import {
  accentDark,
  headerSelected,
  headerWhole,
  overlayCommands,
  parseColor,
} from '../src/gridOverlay';
import {
  bottomBaseline,
  caretAt,
  cellFontFor,
  editorWidth,
  fontMetrics,
  resizeTip,
  wheelRows,
  wheelScrollPt,
  widthChars,
} from '../src/gridFeel';

// A view drawn the way `bo_sheet::render` draws it: 30 × 15 pt headers, 48 pt columns,
// 15 pt rows, a hairline (0.75 pt) separator at the far edge of every header cell and a
// centred label run (two glyph advances of 5 pt per letter).
const LW = 0.75;
function label(text: string, cx: number, cy: number): DisplayCommand {
  const adv = 5;
  return {
    op: 'glyphs',
    face: 8,
    size: 9,
    x: cx - (text.length * adv) / 2,
    y: cy + 3,
    color: ENGINE_CHROME.headerText,
    text,
    glyphs: [...text].map((_, i) => [10 + i, i * adv, 0, i]),
  };
}
const line = (x1: number, y1: number, x2: number, y2: number, color: Rgba): DisplayCommand => ({
  op: 'line',
  x1,
  y1,
  x2,
  y2,
  width: LW,
  color,
});

interface ViewSpec {
  width: number;
  height: number;
  /** Column widths by index (default 48), `0` = hidden. */
  colW?: Record<number, number>;
  rowH?: Record<number, number>;
  scrollX?: number;
  scrollY?: number;
  frozenCols?: number;
  frozenRows?: number;
}

/** Builds the header part of an engine display list (cells are irrelevant here). */
function view(v: ViewSpec): PageDisplayList {
  const cw = (c: number) => v.colW?.[c] ?? 48;
  const rh = (r: number) => v.rowH?.[r] ?? 15;
  const hw = 30;
  const hh = 15;
  const cmds: DisplayCommand[] = [
    { op: 'fillRect', x: 0, y: 0, w: v.width, h: v.height, color: [255, 255, 255, 255] },
    // A gridline and a chart's text inside a transform: both ignored by the reader.
    line(100, 15, 100, v.height, ENGINE_CHROME.gridline),
    { op: 'pushTransform', m: [1, 0, 0, 1, 50, 50] },
    label('Z', 5, 5),
    { op: 'popTransform' },
  ];
  const axis = (rows: boolean) => {
    const size = rows ? rh : cw;
    const frozen = (rows ? v.frozenRows : v.frozenCols) ?? 0;
    const start = rows ? hh : hw;
    const end = rows ? v.height : v.width;
    const scroll = (rows ? v.scrollY : v.scrollX) ?? 0;
    let pos = start;
    for (let i = 0; i < frozen; i++) {
      const s = size(i);
      if (s > 0) {
        const c = pos + s / 2;
        if (rows) {
          cmds.push(label(String(i + 1), hw / 2, c));
          cmds.push(line(0, pos + s - LW / 2, hw, pos + s - LW / 2, ENGINE_CHROME.headerLine));
        } else {
          cmds.push(label(colName(i), c, hh / 2));
          cmds.push(line(pos + s - LW / 2, 0, pos + s - LW / 2, hh, ENGINE_CHROME.headerLine));
        }
      }
      pos += s;
    }
    // Scrollable part: sheet position of the first unfrozen index + scroll.
    let sheet = 0;
    let i = frozen;
    const paneStart = pos;
    let x = paneStart - scroll;
    while (x < end) {
      const s = size(i);
      if (s > 0 && x + s > paneStart) {
        const c = x + s / 2;
        if (rows) {
          cmds.push(label(String(i + 1), hw / 2, c));
          cmds.push(line(0, x + s - LW / 2, hw, x + s - LW / 2, ENGINE_CHROME.headerLine));
        } else {
          cmds.push(label(colName(i), c, hh / 2));
          cmds.push(line(x + s - LW / 2, 0, x + s - LW / 2, hh, ENGINE_CHROME.headerLine));
        }
      }
      x += s;
      sheet += s;
      i++;
    }
    return sheet;
  };
  axis(false);
  axis(true);
  cmds.push({ op: 'fillRect', x: 0, y: 0, w: hw, h: hh, color: ENGINE_CHROME.headerBg });
  cmds.push(line(hw - LW / 2, 0, hw - LW / 2, hh, ENGINE_CHROME.headerLine));
  cmds.push(line(0, hh - LW / 2, hw, hh - LW / 2, ENGINE_CHROME.headerLine));
  return { width: v.width, height: v.height, commands: cmds };
}

describe('grid geometry from the drawn view', () => {
  const g = geometryFromPage(view({ width: 400, height: 200 }), { rows: 0, cols: 0 })!;

  it('reads the headers and every visible column and row', () => {
    expect(g).not.toBeNull();
    expect([g.headerW, g.headerH]).toEqual([30, 15]);
    expect(g.cols).toHaveLength(1);
    const cols = g.cols[0]!.segs;
    // (400 − 30) / 48 → 8 columns, the last one partly visible.
    expect(cols.map((s) => s.index)).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
    expect(cols[0]).toMatchObject({ start: 30, end: 78 });
    expect(cols[2]).toMatchObject({ start: 126, end: 174 });
    expect(g.rows[0]!.segs[3]).toMatchObject({ index: 3, start: 60, end: 75 });
    expect(segSize(g, false, 4)).toBe(48);
    expect(segSize(g, true, 4)).toBe(15);
  });

  it('hit-tests like the engine (cells, headers, edges within 3 pt, corner)', () => {
    expect(hitLocal(g, 10, 5)).toEqual({ kind: 'corner' });
    expect(hitLocal(g, 31, 16)).toEqual({ kind: 'cell', row: 0, col: 0 });
    expect(hitLocal(g, 130, 70)).toEqual({ kind: 'cell', row: 3, col: 2 });
    expect(hitLocal(g, 100, 5)).toEqual({ kind: 'columnHeader', col: 1 });
    expect(hitLocal(g, 76, 5)).toEqual({ kind: 'columnEdge', col: 0 });
    // Just right of the boundary: the column on the left is resized (as the engine does).
    expect(hitLocal(g, 80, 5)).toEqual({ kind: 'columnEdge', col: 0 });
    expect(hitLocal(g, 10, 52)).toEqual({ kind: 'rowHeader', row: 2 });
    expect(hitLocal(g, 10, 59)).toEqual({ kind: 'rowEdge', row: 2 });
    expect(hitLocal(g, 500, 50)).toEqual({ kind: 'none' });
  });

  it('resolves merged cells to their anchor and finds AutoFilter buttons', () => {
    expect(hitLocal(g, 130, 70, { merges: [[2, 1, 4, 3]] })).toEqual({
      kind: 'cell',
      row: 2,
      col: 1,
    });
    // Header row 0 of a filter over A1:C9: the button is the right 12 pt (+2) of the cell.
    const o = { autoFilter: [0, 0, 8, 2] as [number, number, number, number] };
    expect(hitLocal(g, 70, 20, o)).toEqual({ kind: 'filterButton', row: 0, col: 0 });
    expect(hitLocal(g, 40, 20, o)).toEqual({ kind: 'cell', row: 0, col: 0 });
  });

  it('places ranges, also partly off-screen, and grows them over merges', () => {
    expect(rangeBox(g, { r0: 1, c0: 1, r1: 2, c1: 2 })).toEqual({ x: 78, y: 30, w: 96, h: 30 });
    // A whole column runs past the bottom: the box goes beyond the pane (clipped on screen).
    const col = rangeBox(g, { r0: 0, c0: 3, r1: 999, c1: 3 })!;
    expect(col.y).toBe(15);
    expect(col.y + col.h).toBeGreaterThan(200);
    expect(expandMerges({ r0: 3, c0: 0, r1: 3, c1: 0 }, [[2, 0, 4, 1]])).toEqual({
      r0: 2,
      c0: 0,
      r1: 4,
      c1: 1,
    });
    // Clamped drag: past the right / bottom edge the last visible cell.
    expect(cellNear(g, 1000, 1000)).toEqual({ row: 12, col: 7 });
  });

  it('handles scrolling (a first column partly scrolled away), hidden columns and frozen panes', () => {
    const s = geometryFromPage(view({ width: 400, height: 200, scrollX: 20, colW: { 2: 0 } }), {
      rows: 0,
      cols: 0,
    })!;
    const cols = s.cols[0]!.segs;
    expect(cols[0]!.index).toBe(0);
    // Column A runs from 10 to 58: its left edge is behind the row headers.
    expect(cols[0]!.end).toBe(58);
    expect(cols[0]!.start).toBeCloseTo(10, 0);
    // C is hidden: B is followed by D.
    expect(cols.map((c) => c.index).slice(0, 3)).toEqual([0, 1, 3]);
    expect(hitLocal(s, 31, 20)).toEqual({ kind: 'cell', row: 0, col: 0 });
    // The left edge of D resizes B (the visible one before it, C is hidden).
    expect(hitLocal(s, 107, 5, { hidden: { rows: [], cols: [[2, 2]] } })).toEqual({
      kind: 'columnEdge',
      col: 1,
    });
    // A range B:C is the whole of B when C is hidden.
    expect(rangeBox(s, { r0: 0, c0: 1, r1: 0, c1: 2 }, { rows: [], cols: [[2, 2]] })).toMatchObject(
      {
        x: 58,
        w: 48,
      },
    );

    const f = geometryFromPage(
      view({ width: 400, height: 200, frozenCols: 1, frozenRows: 2, scrollY: 150 }),
      { rows: 2, cols: 1 },
    )!;
    expect(f.cols).toHaveLength(2);
    expect(f.rows).toHaveLength(2);
    expect(f.rows[1]!.start).toBe(45);
    // Rows 1–2 stay; the scrollable rows start at row 13 (index 12) after 150 pt.
    expect(f.rows[1]!.segs[0]!.index).toBe(12);
    expect(hitLocal(f, 40, 20)).toEqual({ kind: 'cell', row: 0, col: 0 });
    expect(hitLocal(f, 100, 50)).toEqual({ kind: 'cell', row: 12, col: 1 });
    // A range across the frozen line is drawn in each pane it shows in.
    const boxes = rangeBoxes(f, { r0: 1, c0: 0, r1: 13, c1: 1 });
    expect(boxes).toHaveLength(4);
  });

  it('gives up (engine fallback) when the view has no headers', () => {
    const p: PageDisplayList = { width: 100, height: 100, commands: [] };
    expect(geometryFromPage(p, { rows: 0, cols: 0 })).toBeNull();
  });
});

describe('chrome pass', () => {
  const page = view({ width: 400, height: 200 });
  const g = geometryFromPage(page, { rows: 0, cols: 0 });

  it('uses neutral grey colours for grid lines and headers', () => {
    const out = chromePage(page, 4 / 3, g).commands;
    const lines = out.filter((c) => c.op === 'line');
    expect(lines.some((c) => c.op === 'line' && c.color === SHEET_CHROME.gridline)).toBe(true);
    expect(lines.every((c) => c.op === 'line' && c.color !== ENGINE_CHROME.gridline)).toBe(true);
    const corner = out.find((c) => c.op === 'fillRect' && c.w === 30 && c.h === 15);
    expect(corner).toMatchObject({ color: SHEET_CHROME.headerBg });
    // The header band's edge towards the cells is darker than the separators.
    const edge = out.find(
      (c) => c.op === 'line' && c.x1 === 0 && Math.abs(c.y1 - 15) < 1 && c.x2 === 30,
    );
    expect(edge).toMatchObject({ color: SHEET_CHROME.headerEdge });
    // The chart text inside a transform is untouched.
    expect(out).toContainEqual(page.commands[3]);
  });

  for (const dpr of [1, 1.25, 1.5, 1.75]) {
    it(`puts 1 px lines on whole device pixels at ${dpr * 100} %`, () => {
      const s = (4 / 3) * dpr;
      for (const c of chromePage(page, s, g).commands) {
        if (c.op !== 'line') continue;
        const wd = c.width * s;
        expect(Math.abs(wd - Math.round(wd))).toBeLessThan(1e-6);
        const across = (c.x1 === c.x2 ? c.x1 : c.y1) * s - wd / 2;
        expect(Math.abs(across - Math.round(across))).toBeLessThan(1e-6);
      }
    });
  }
});

describe('selection layer', () => {
  const g = geometryFromPage(view({ width: 400, height: 200 }), { rows: 0, cols: 0 })!;
  const accent: [number, number, number, number] = [0x0b, 0x7a, 0x6b, 255];
  const base = {
    extra: [],
    wholeRowsFrom: 999,
    wholeColsFrom: 25,
    accent,
    showRange: true,
  };

  it('highlights the headers of the selected rows and columns with the accent edge line', () => {
    const cmds = overlayCommands(
      g,
      { ...base, range: { r0: 1, c0: 1, r1: 2, c1: 2 }, active: { row: 1, col: 1 } },
      { s: 4 / 3, dpr: 1 },
    );
    const bands = cmds.filter(
      (c) => c.op === 'fillRect' && c.color.join() === headerSelected(accent).join(),
    );
    // Columns B, C and rows 2, 3.
    expect(bands).toHaveLength(4);
    const labels = cmds.filter((c) => c.op === 'glyphs').map((c) => c.op === 'glyphs' && c.text);
    expect(labels.sort()).toEqual(['2', '3', 'B', 'C']);
    expect(
      cmds
        .filter((c) => c.op === 'glyphs')
        .every((c) => c.op === 'glyphs' && c.color.join() === accentDark(accent).join()),
    ).toBe(true);
    // Accent line at the bottom of column header B (2 px at 100 %: 1.5 pt).
    expect(cmds).toContainEqual(
      expect.objectContaining({ op: 'fillRect', y: 13.5, h: 1.5, color: accent }),
    );
  });

  it('marks whole columns with the stronger band', () => {
    const cmds = overlayCommands(
      g,
      { ...base, range: { r0: 0, c0: 3, r1: 999, c1: 3 }, active: { row: 0, col: 3 } },
      { s: 4 / 3, dpr: 1 },
    );
    expect(
      cmds.filter((c) => c.op === 'fillRect' && c.color.join() === headerWhole(accent).join()),
    ).toHaveLength(1);
  });

  it('shades a range with a hole at the active cell, a 2 px border and the fill handle', () => {
    const cmds = overlayCommands(
      g,
      { ...base, range: { r0: 1, c0: 1, r1: 2, c1: 2 }, active: { row: 1, col: 1 } },
      { s: 4 / 3, dpr: 1 },
    );
    const shade = cmds.find(
      (c) => c.op === 'path' && c.fill?.type === 'solid' && c.fill.color[3] < 255,
    );
    expect(shade).toBeDefined();
    // Two sub-paths: the range and the active cell (the hole).
    expect(shade!.op === 'path' && shade.segs.filter((x) => x === 'Z')).toHaveLength(2);
    // Border: 2 device px around B2:C3 (x 78..174 pt = 104..232 px), from the grid line
    // before the range to one pixel past the one after it.
    const border = cmds.filter(
      (c) => c.op === 'fillRect' && c.color === accent && c.y > 15 && c.x >= 30,
    );
    const xs = border.map((c) => (c.op === 'fillRect' ? Math.round(c.x * (4 / 3)) : 0));
    expect(Math.min(...xs)).toBe(103);
    const handle = border.find(
      (c) =>
        c.op === 'fillRect' && Math.round(c.w * (4 / 3)) === 6 && Math.round(c.h * (4 / 3)) === 6,
    );
    expect(handle).toBeDefined();
    // The white break around the handle.
    expect(
      cmds.some(
        (c) =>
          c.op === 'fillRect' &&
          c.color.join() === '255,255,255,255' &&
          Math.round(c.w * (4 / 3)) === 8,
      ),
    ).toBe(true);
  });

  it('a single cell has no shading; several ranges have no thick border or handle', () => {
    const one = overlayCommands(
      g,
      { ...base, range: { r0: 1, c0: 1, r1: 1, c1: 1 }, active: { row: 1, col: 1 } },
      { s: 4 / 3, dpr: 1 },
    );
    expect(
      one.some((c) => c.op === 'path' && c.fill?.type === 'solid' && c.fill.color[3] < 255),
    ).toBe(false);
    const multi = overlayCommands(
      g,
      {
        ...base,
        extra: [{ r0: 5, c0: 0, r1: 6, c1: 0 }],
        range: { r0: 1, c0: 1, r1: 2, c1: 2 },
        active: { row: 1, col: 1 },
      },
      { s: 4 / 3, dpr: 1 },
    );
    expect(
      multi.filter((c) => c.op === 'path' && c.fill?.type === 'solid' && c.fill.color[3] < 255),
    ).toHaveLength(2);
    expect(
      multi.some((c) => c.op === 'fillRect' && c.color === accent && c.y > 15 && c.x >= 30),
    ).toBe(false);
  });

  it('parses the accent colour', () => {
    expect(parseColor('#0b7a6b', [0, 0, 0, 255])).toEqual([11, 122, 107, 255]);
    expect(parseColor(' rgb(1, 2, 3)', [0, 0, 0, 255])).toEqual([1, 2, 3, 255]);
    expect(parseColor('#abc', [0, 0, 0, 255])).toEqual([0xaa, 0xbb, 0xcc, 255]);
    expect(parseColor('', [9, 9, 9, 255])).toEqual([9, 9, 9, 255]);
  });
});

describe('grid feel helpers', () => {
  it('scrolls three rows a wheel notch and leaves touchpads to the browser', () => {
    expect(wheelRows({ deltaX: 0, deltaY: 100, deltaMode: 0 })).toBe(3);
    expect(wheelRows({ deltaX: 0, deltaY: -200, deltaMode: 0 })).toBe(-6);
    expect(wheelRows({ deltaX: 0, deltaY: 3, deltaMode: 1 })).toBe(3);
    expect(wheelRows({ deltaX: 0, deltaY: 4.5, deltaMode: 0 })).toBeNull();
    expect(wheelRows({ deltaX: 0, deltaY: 100, deltaMode: 0, ctrlKey: true })).toBeNull();
    const g = geometryFromPage(view({ width: 400, height: 200, scrollY: 5 }), {
      rows: 0,
      cols: 0,
    })!;
    // Row 1 is 5 pt scrolled away: down 3 rows puts row 4 at the top (40 pt further).
    expect(wheelScrollPt(g, 3)).toBeCloseTo(40);
    // Up: back to the top of row 1 (5 pt), then two whole rows.
    expect(wheelScrollPt(g, -3)).toBeCloseTo(-35);
  });

  it('describes column widths in characters and row heights in points', () => {
    // The default column: 48 pt = 64 px = 8.43 characters; the default row 15 pt = 20 px.
    expect(resizeTip(false, 48)).toEqual({ value: 8.43, px: 64 });
    expect(resizeTip(true, 15)).toEqual({ value: 15, px: 20 });
    expect(widthChars(75)).toBe(10);
    expect(widthChars(6)).toBe(0.5);
  });

  it('puts the caret at the nearest character boundary', () => {
    const m = (s: string) => s.length * 7;
    expect(caretAt('12345', 0, m)).toBe(0);
    expect(caretAt('12345', 10, m)).toBe(1);
    expect(caretAt('12345', 11, m)).toBe(2);
    expect(caretAt('12345', 99, m)).toBe(5);
    expect(editorWidth(30, 64, 2, 500)).toBe(64);
    expect(editorWidth(200, 64, 2, 500)).toBe(208);
    expect(editorWidth(900, 64, 2, 500)).toBe(500);
  });

  it('maps cell fonts to the bundled faces and reads their metrics', () => {
    expect(cellFontFor('Calibri')).toBe('Carlito');
    expect(cellFontFor('Cambria')).toBe('Caladea');
    expect(cellFontFor('Times New Roman')).toBe('Tinos');
    expect(cellFontFor('Arial')).toBe('LiberationSans');
    expect(cellFontFor(undefined)).toBe('LiberationSans');
    // A minimal sfnt with head + hhea (+ OS/2 without USE_TYPO_METRICS).
    const b = new Uint8Array(12 + 3 * 16 + 54 + 36 + 78);
    const v = new DataView(b.buffer);
    v.setUint16(4, 3);
    const tables: [string, number][] = [
      ['head', 60],
      ['hhea', 114],
      ['OS/2', 150],
    ];
    tables.forEach(([tag, off], i) => {
      for (let k = 0; k < 4; k++) b[12 + i * 16 + k] = tag.charCodeAt(k);
      v.setUint32(12 + i * 16 + 8, off);
    });
    v.setUint16(60 + 18, 2048);
    v.setInt16(114 + 4, 1536);
    v.setInt16(114 + 6, -512);
    v.setInt16(114 + 8, 452);
    v.setInt16(150 + 68, 1000);
    const m = fontMetrics(b)!;
    expect(m).toEqual({ upem: 2048, ascender: 1536, descender: -512, lineGap: 452 });
    // USE_TYPO_METRICS switches to the OS/2 values.
    v.setUint16(150 + 62, 0x80);
    expect(fontMetrics(b)!.ascender).toBe(1000);
    // 20 px row, 14.67 px text: 1 px padding, descent 3.67, gap 3.24.
    expect(bottomBaseline(20, 14.667, m, 1)).toBeCloseTo(20 - 1 - (964 * 14.667) / 2048, 3);
  });
});
