/**
 * Local grid geometry of the BlueSheet view, read from the display list the engine drew
 * (`SheetHost.render`): where every visible column and row is on screen. With it a click is
 * hit-tested, and the selection drawn, in the same frame as the mouse-down — no engine round
 * trip on the critical path (the engine answer reconciles afterwards). Also the "chrome" pass
 * that gives the engine's grid lines and headers the neutral spreadsheet look and aligns
 * them to device pixels. Pure functions, unit-tested; units are points like the engine.
 */
import type { DisplayCommand, PageDisplayList, Rgba } from '@blueoffice/platform';
import type { CellPos, HiddenSpans, Range } from './grid';

/** One visible column (or row): its index and its screen span `[start, end)`. */
export interface Seg {
  index: number;
  start: number;
  end: number;
  /** The header label run the engine drew for it (redrawn highlighted when selected). */
  label: Extract<DisplayCommand, { op: 'glyphs' }>;
}

/** The columns (or rows) of one pane: frozen or scrollable. */
export interface AxisPane {
  /** Screen span of the pane. */
  start: number;
  end: number;
  /** Indices the pane can show (`[min, max]`). */
  min: number;
  max: number;
  segs: Seg[];
}

export interface GridGeometry {
  width: number;
  height: number;
  headerW: number;
  headerH: number;
  /** Frozen columns pane (when any), then the scrollable one. */
  cols: AxisPane[];
  rows: AxisPane[];
}

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** The engine's view chrome colours (`bo_sheet::render::RenderOptions::default`). */
export const ENGINE_CHROME = {
  gridline: [0xda, 0xdc, 0xe0, 255] as Rgba,
  headerBg: [0xf3, 0xf4, 0xf6, 255] as Rgba,
  headerText: [0x3c, 0x40, 0x43, 255] as Rgba,
  headerLine: [0xc0, 0xc4, 0xc8, 255] as Rgba,
  frozenLine: [0x9a, 0xa0, 0xa6, 255] as Rgba,
};

/**
 * The common spreadsheet chrome: neutral light-grey grid lines, a light-grey header band with
 * dark-grey labels and a slightly darker edge towards the cells, a mid-grey frozen-pane line.
 */
export const SHEET_CHROME = {
  gridline: [0xd4, 0xd4, 0xd4, 255] as Rgba,
  headerBg: [0xf3, 0xf3, 0xf3, 255] as Rgba,
  headerText: [0x44, 0x44, 0x44, 255] as Rgba,
  headerLine: [0xd4, 0xd4, 0xd4, 255] as Rgba,
  /** Edge between the headers and the cells. */
  headerEdge: [0xb4, 0xb4, 0xb4, 255] as Rgba,
  frozenLine: [0x9a, 0x9a, 0x9a, 255] as Rgba,
};

/** One CSS pixel in points (the engine's hairline width). */
export const PT_PER_PX = 0.75;
const EPS = 1e-3;
/** Where off-screen edges of a range are put (beyond any pane, so its border is clipped). */
const OFF = 10_000;

const same = (a: Rgba, b: Rgba) => a[0] === b[0] && a[1] === b[1] && a[2] === b[2] && a[3] === b[3];

type GlyphRun = Extract<DisplayCommand, { op: 'glyphs' }>;
type LineCmd = Extract<DisplayCommand, { op: 'line' }>;

/** Approximate horizontal centre of a label run (the last glyph's advance is estimated). */
function runCentreX(r: GlyphRun): number {
  const n = r.glyphs.length;
  if (n === 0) return r.x;
  const last = r.glyphs[n - 1]![1];
  const adv = n > 1 ? last / (n - 1) : r.size * 0.55;
  return r.x + (last + adv) / 2;
}

/** Approximate vertical centre of a label run from its baseline. */
const runCentreY = (r: GlyphRun) => r.y - r.size * 0.3;

const COL_LABEL = /^[A-Z]{1,3}$/;
const ROW_LABEL = /^[0-9]{1,7}$/;

const colIndex = (s: string) => {
  let n = 0;
  for (const ch of s) n = n * 26 + (ch.charCodeAt(0) - 64);
  return n - 1;
};

/**
 * Builds the panes of one axis from the header labels and separator positions: each visible
 * column has exactly one label and one separator at its far edge (hidden ones have neither),
 * so labels and edges pair up in order inside every pane.
 */
function axisPanes(
  labels: { index: number; centre: number; run: GlyphRun }[],
  edges: number[],
  headerEnd: number,
  frozen: number,
  end: number,
  calibrate = false,
): AxisPane[] | null {
  labels.sort((a, b) => a.index - b.index);
  const sorted = [...new Set(edges.map((e) => Math.round(e * 1000) / 1000))].sort((a, b) => a - b);
  const frozenLabels = labels.filter((l) => l.index < frozen);
  const mainLabels = labels.filter((l) => l.index >= frozen);
  const panes: AxisPane[] = [];
  let mainStart = headerEnd;
  const build = (ls: typeof labels, start: number, paneEnd: number, min: number, max: number) => {
    const es = sorted.filter((e) => e > start + EPS);
    if (es.length < ls.length) return null;
    // The labels sit at the same offset from their cell's centre (rows: same font, same
    // height); a fully visible one tells it, for the first, possibly cut, cell.
    let bias = 0;
    if (calibrate && ls.length > 2) bias = ls[1]!.centre - (es[0]! + es[1]!) / 2;
    const segs: Seg[] = ls.map((l, i) => {
      const right = es[i]!;
      let left = i === 0 ? start : es[i - 1]!;
      if (i === 0) {
        // The first column may be scrolled partly out of the pane: its label is still centred
        // in the whole column, which tells where the hidden left edge is.
        const trueLeft = 2 * (l.centre - bias) - right;
        if (trueLeft < start - 1) left = trueLeft;
      }
      return { index: l.index, start: left, end: right, label: l.run };
    });
    return { start, end: paneEnd, min, max, segs };
  };
  if (frozen > 0 && frozenLabels.length > 0) {
    const es = sorted.filter((e) => e > headerEnd + EPS);
    const last = es[frozenLabels.length - 1];
    if (last === undefined) return null;
    mainStart = last;
    const p = build(frozenLabels, headerEnd, mainStart, 0, frozen - 1);
    if (!p) return null;
    panes.push(p);
  }
  const m = build(mainLabels, mainStart, end, frozen, Number.MAX_SAFE_INTEGER);
  if (!m) return null;
  panes.push(m);
  return panes;
}

/**
 * Reads the grid geometry from a rendered sheet view. `null` when the view has no headers
 * (the positions cannot be read then; callers fall back to the engine's hit test).
 */
export function geometryFromPage(
  page: PageDisplayList,
  frozen: { rows: number; cols: number },
): GridGeometry | null {
  let depth = 0;
  let corner: Box | null = null;
  const runs: GlyphRun[] = [];
  const lines: LineCmd[] = [];
  for (const c of page.commands) {
    if (c.op === 'pushTransform') depth++;
    else if (c.op === 'popTransform') depth = Math.max(0, depth - 1);
    if (depth > 0) continue;
    if (
      c.op === 'fillRect' &&
      Math.abs(c.x) < EPS &&
      Math.abs(c.y) < EPS &&
      c.w < page.width - EPS &&
      c.h < page.height - EPS &&
      same(c.color, ENGINE_CHROME.headerBg)
    )
      corner = { x: 0, y: 0, w: c.w, h: c.h };
    else if (c.op === 'glyphs' && same(c.color, ENGINE_CHROME.headerText)) runs.push(c);
    else if (c.op === 'line' && same(c.color, ENGINE_CHROME.headerLine)) lines.push(c);
  }
  if (!corner) return null;
  const { w: hw, h: hh } = corner as Box;
  const colEdges: number[] = [];
  const rowEdges: number[] = [];
  for (const l of lines) {
    if (Math.abs(l.x1 - l.x2) < EPS && Math.abs(l.y1) < EPS && Math.abs(l.y2 - hh) < EPS)
      colEdges.push(l.x1 + l.width / 2);
    else if (Math.abs(l.y1 - l.y2) < EPS && Math.abs(l.x1) < EPS && Math.abs(l.x2 - hw) < EPS)
      rowEdges.push(l.y1 + l.width / 2);
  }
  const colLabels: { index: number; centre: number; run: GlyphRun }[] = [];
  const rowLabels: { index: number; centre: number; run: GlyphRun }[] = [];
  for (const r of runs) {
    if (r.y < hh + EPS && COL_LABEL.test(r.text))
      colLabels.push({ index: colIndex(r.text), centre: runCentreX(r), run: r });
    else if (r.x < hw && r.y > hh && ROW_LABEL.test(r.text))
      rowLabels.push({ index: Number(r.text) - 1, centre: runCentreY(r), run: r });
  }
  const cols = axisPanes(colLabels, colEdges, hw, frozen.cols, page.width);
  const rows = axisPanes(rowLabels, rowEdges, hh, frozen.rows, page.height, true);
  if (!cols || !rows) return null;
  return { width: page.width, height: page.height, headerW: hw, headerH: hh, cols, rows };
}

const allHidden = (a: number, b: number, spans: [number, number][]) => {
  let i = a;
  while (i <= b) {
    const s = spans.find(([x, y]) => i >= x && i <= y);
    if (!s) return false;
    i = s[1] + 1;
  }
  return true;
};

/** Screen position of the near edge of index `a` (start) in a pane. */
function edgeStart(p: AxisPane, a: number, hidden: [number, number][]): number {
  const i = p.segs.findIndex((s) => s.index >= a);
  if (i < 0) return p.end + OFF;
  const s = p.segs[i]!;
  if (s.index === a || i > 0 || allHidden(a, s.index - 1, hidden)) return s.start;
  return p.start - OFF;
}

/** Screen position of the far edge of index `b` (end) in a pane. */
function edgeEnd(p: AxisPane, b: number, hidden: [number, number][]): number {
  let i = -1;
  for (let k = p.segs.length - 1; k >= 0; k--) {
    if (p.segs[k]!.index <= b) {
      i = k;
      break;
    }
  }
  if (i < 0) return p.start - OFF;
  const s = p.segs[i]!;
  if (s.index === b || i < p.segs.length - 1 || allHidden(s.index + 1, b, hidden)) return s.end;
  return p.end + OFF;
}

/** A range drawn in one pane: its rectangle (may run past the pane) and the pane's clip. */
export interface PaneBox {
  clip: Box;
  rect: Box;
}

/** The rectangles of a range in every pane that shows part of it. */
export function rangeBoxes(g: GridGeometry, r: Range, hidden?: HiddenSpans): PaneBox[] {
  const out: PaneBox[] = [];
  const hc = hidden?.cols ?? [];
  const hr = hidden?.rows ?? [];
  for (const cp of g.cols) {
    const c0 = Math.max(r.c0, cp.min);
    const c1 = Math.min(r.c1, cp.max);
    if (c0 > c1) continue;
    for (const rp of g.rows) {
      const r0 = Math.max(r.r0, rp.min);
      const r1 = Math.min(r.r1, rp.max);
      if (r0 > r1) continue;
      const x0 = edgeStart(cp, c0, hc);
      const x1 = edgeEnd(cp, c1, hc);
      const y0 = edgeStart(rp, r0, hr);
      const y1 = edgeEnd(rp, r1, hr);
      const clip = { x: cp.start, y: rp.start, w: cp.end - cp.start, h: rp.end - rp.start };
      // Nothing of it inside the pane.
      if (x1 <= clip.x || x0 >= clip.x + clip.w || y1 <= clip.y || y0 >= clip.y + clip.h) continue;
      out.push({ clip, rect: { x: x0, y: y0, w: Math.max(0, x1 - x0), h: Math.max(0, y1 - y0) } });
    }
  }
  return out;
}

/** Bounding box of a range on screen (unclipped), or `null` when no pane shows it. */
export function rangeBox(g: GridGeometry, r: Range, hidden?: HiddenSpans): Box | null {
  const bs = rangeBoxes(g, r, hidden);
  if (bs.length === 0) return null;
  const x0 = Math.min(...bs.map((b) => b.rect.x));
  const y0 = Math.min(...bs.map((b) => b.rect.y));
  const x1 = Math.max(...bs.map((b) => b.rect.x + b.rect.w));
  const y1 = Math.max(...bs.map((b) => b.rect.y + b.rect.h));
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

/** Merged ranges `[r0, c0, r1, c1]` as in `SheetSummary.merges`. */
export type Merge = [number, number, number, number];

/** Grows a range until it covers every merged area it touches (spreadsheet convention). */
export function expandMerges(r: Range, merges: Merge[]): Range {
  const out = { ...r };
  for (let guard = 0; guard < 8; guard++) {
    let grew = false;
    for (const [a, b, c, d] of merges) {
      if (a > out.r1 || c < out.r0 || b > out.c1 || d < out.c0) continue;
      if (a < out.r0 || b < out.c0 || c > out.r1 || d > out.c1) {
        out.r0 = Math.min(out.r0, a);
        out.c0 = Math.min(out.c0, b);
        out.r1 = Math.max(out.r1, c);
        out.c1 = Math.max(out.c1, d);
        grew = true;
      }
    }
    if (!grew) break;
  }
  return out;
}

/** What is under a point, like `SheetHost.hitTest` (charts / SmartArt are not known here). */
export type LocalHit =
  | { kind: 'cell'; row: number; col: number }
  | { kind: 'columnHeader'; col: number }
  | { kind: 'rowHeader'; row: number }
  | { kind: 'columnEdge'; col: number }
  | { kind: 'rowEdge'; row: number }
  | { kind: 'filterButton'; row: number; col: number }
  | { kind: 'corner' }
  | { kind: 'none' };

/** Header edges within this distance of a border resize (points, as the engine). */
const EDGE = 3;

function segAt(panes: AxisPane[], v: number): { pane: AxisPane; seg: Seg; i: number } | null {
  for (const pane of panes) {
    if (v < pane.start - EPS || v >= pane.end) continue;
    const i = pane.segs.findIndex((s) => v >= s.start && v < s.end);
    if (i >= 0) return { pane, seg: pane.segs[i]!, i };
  }
  return null;
}

/** The visible index before `seg` in its pane (for a hit on its near edge). */
function before(p: AxisPane, i: number, hidden: [number, number][]): number | null {
  if (i > 0) return p.segs[i - 1]!.index;
  let k = p.segs[i]!.index - 1;
  while (k >= 0 && hidden.some(([a, b]) => k >= a && k <= b)) k--;
  return k >= 0 ? k : null;
}

export interface HitOptions {
  hidden?: HiddenSpans;
  merges?: Merge[];
  /** AutoFilter range `[r0, c0, r1, c1]`: its header row has dropdown buttons. */
  autoFilter?: [number, number, number, number] | null;
}

/** Hit-tests a point (points, relative to the canvas) like the engine does. */
export function hitLocal(g: GridGeometry, x: number, y: number, o: HitOptions = {}): LocalHit {
  if (x < 0 || y < 0 || x >= g.width || y >= g.height) return { kind: 'none' };
  const inHx = x < g.headerW;
  const inHy = y < g.headerH;
  if (inHx && inHy) return { kind: 'corner' };
  if (inHy) {
    const c = segAt(g.cols, x);
    if (!c) return { kind: 'none' };
    if (Math.abs(x - c.seg.end) <= EDGE) return { kind: 'columnEdge', col: c.seg.index };
    if (Math.abs(x - c.seg.start) <= EDGE && c.seg.index > 0) {
      const p = before(c.pane, c.i, o.hidden?.cols ?? []);
      if (p !== null) return { kind: 'columnEdge', col: p };
    }
    return { kind: 'columnHeader', col: c.seg.index };
  }
  if (inHx) {
    const r = segAt(g.rows, y);
    if (!r) return { kind: 'none' };
    if (Math.abs(y - r.seg.end) <= EDGE) return { kind: 'rowEdge', row: r.seg.index };
    if (Math.abs(y - r.seg.start) <= EDGE && r.seg.index > 0) {
      const p = before(r.pane, r.i, o.hidden?.rows ?? []);
      if (p !== null) return { kind: 'rowEdge', row: p };
    }
    return { kind: 'rowHeader', row: r.seg.index };
  }
  const c = segAt(g.cols, x);
  const r = segAt(g.rows, y);
  if (!c || !r) return { kind: 'none' };
  let row = r.seg.index;
  let col = c.seg.index;
  const m = (o.merges ?? []).find(([a, b, cc, d]) => row >= a && row <= cc && col >= b && col <= d);
  if (m) {
    row = m[0];
    col = m[1];
  }
  const f = o.autoFilter;
  if (f && row === f[0] && col >= f[1] && col <= f[3]) {
    const box = rangeBox(
      g,
      m ? { r0: m[0], c0: m[1], r1: m[2], c1: m[3] } : { r0: row, c0: col, r1: row, c1: col },
      o.hidden,
    );
    if (box && x >= box.x + box.w - Math.min(12, Math.max(6, box.h - 3)) - 2)
      return { kind: 'filterButton', row, col };
  }
  return { kind: 'cell', row, col };
}

/** The cell nearest to a point, clamped into the panes (drag-selecting past the edge). */
export function cellNear(g: GridGeometry, x: number, y: number): CellPos | null {
  const clampTo = (panes: AxisPane[], v: number) => {
    const first = panes[0]!;
    const last = panes[panes.length - 1]!;
    const lo = Math.max(first.start, first.segs[0]?.start ?? first.start);
    const lastSeg = last.segs[last.segs.length - 1];
    const hi = Math.min(last.end, lastSeg?.end ?? last.end) - EPS;
    return Math.min(hi, Math.max(lo, v));
  };
  const c = segAt(g.cols, clampTo(g.cols, x));
  const r = segAt(g.rows, clampTo(g.rows, y));
  return c && r ? { row: r.seg.index, col: c.seg.index } : null;
}

/** Width of a column / height of a row in points when visible. */
export function segSize(g: GridGeometry, rows: boolean, index: number): number | null {
  for (const p of rows ? g.rows : g.cols) {
    const s = p.segs.find((x) => x.index === index);
    if (s) return s.end - s.start;
  }
  return null;
}

// ------------------------------------------------------------------- chrome pass

/**
 * Gives the engine's view chrome the neutral spreadsheet colours and aligns axis-parallel
 * hairlines and cell fills to device pixels (`s` = device pixels per point), so 1 px grid
 * lines stay sharp at 100 %, 125 %, 150 % and other scalings. Commands inside transforms
 * (charts, pictures) are left alone.
 */
export function chromePage(
  page: PageDisplayList,
  s: number,
  g: GridGeometry | null,
): PageDisplayList {
  if (!(s > 0)) return page;
  const snap = (v: number) => Math.round(v * s) / s;
  let depth = 0;
  const hw = g?.headerW ?? 0;
  const hh = g?.headerH ?? 0;
  const out: DisplayCommand[] = [];
  for (const c of page.commands) {
    if (c.op === 'pushTransform') depth++;
    else if (c.op === 'popTransform') depth = Math.max(0, depth - 1);
    if (depth > 0) {
      out.push(c);
      continue;
    }
    if (c.op === 'fillRect') {
      const x0 = snap(c.x);
      const y0 = snap(c.y);
      const inHeader = g !== null && (c.y < hh - EPS || c.x < hw - EPS);
      const color =
        inHeader && same(c.color, ENGINE_CHROME.headerBg) ? SHEET_CHROME.headerBg : c.color;
      out.push({ ...c, x: x0, y: y0, w: snap(c.x + c.w) - x0, h: snap(c.y + c.h) - y0, color });
      continue;
    }
    if (c.op === 'line') {
      let color = c.color;
      if (same(color, ENGINE_CHROME.gridline)) color = SHEET_CHROME.gridline;
      else if (same(color, ENGINE_CHROME.frozenLine)) color = SHEET_CHROME.frozenLine;
      else if (same(color, ENGINE_CHROME.headerLine) && g) {
        // The header band's outer edges (towards the cells) are a little darker.
        const edge =
          (Math.abs(c.y1 - c.y2) < EPS && Math.abs(c.y1 + c.width / 2 - hh) < EPS) ||
          (Math.abs(c.x1 - c.x2) < EPS && Math.abs(c.x1 + c.width / 2 - hw) < EPS);
        color = edge ? SHEET_CHROME.headerEdge : SHEET_CHROME.headerLine;
      }
      const vertical = Math.abs(c.x1 - c.x2) < EPS;
      const horizontal = Math.abs(c.y1 - c.y2) < EPS;
      if ((vertical || horizontal) && c.width * s <= 4) {
        const wd = Math.max(1, Math.round(c.width * s));
        const across = vertical ? c.x1 : c.y1;
        const endPx = Math.round(across * s + wd / 2);
        const mid = (endPx - wd / 2) / s;
        const width = wd / s;
        out.push(
          vertical
            ? { ...c, x1: mid, x2: mid, y1: snap(c.y1), y2: snap(c.y2), width, color }
            : { ...c, y1: mid, y2: mid, x1: snap(c.x1), x2: snap(c.x2), width, color },
        );
      } else out.push({ ...c, color });
      continue;
    }
    if (c.op === 'glyphs' && g && same(c.color, ENGINE_CHROME.headerText)) {
      out.push({ ...c, color: SHEET_CHROME.headerText });
      continue;
    }
    out.push(c);
  }
  return { ...page, commands: out };
}
