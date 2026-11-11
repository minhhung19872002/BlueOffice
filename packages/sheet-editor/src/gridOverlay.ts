/**
 * The selection layer of the BlueSheet grid, drawn over the engine's canvas in the same frame
 * as the mouse-down (spreadsheet conventions): highlighted headers of the selected rows and
 * columns (darker band, accent label, accent line on the edge towards the cells), the range
 * shaded with the active cell left unshaded, a 2 px accent border, the fill handle at the
 * bottom-right corner with a white break in the border, other ranges of a multi-range
 * selection shaded, and the select-all triangle in the corner. Built as display-list
 * commands (points) so it is drawn by the same renderer and fonts as the grid and can be
 * unit-tested; every edge is placed on device pixels (`s` = device pixels per point).
 */
import type { DisplayCommand, PathSeg, Rgba } from '@blueoffice/platform';
import type { CellPos, HiddenSpans, Range } from './grid';
import {
  type AxisPane,
  type Box,
  type GridGeometry,
  type Merge,
  PT_PER_PX,
  expandMerges,
  rangeBoxes,
} from './gridView';

export interface OverlayState {
  /** The range with the active cell. */
  range: Range;
  active: CellPos;
  /** Other ranges of a Ctrl multi-range selection. */
  extra: Range[];
  hidden?: HiddenSpans;
  merges?: Merge[];
  /** Ranges reaching these indices count as whole columns / rows (header fully selected). */
  wholeRowsFrom: number;
  wholeColsFrom: number;
  accent: Rgba;
  /** Whether to draw the border and fill handle (hidden while a chart is selected). */
  showRange: boolean;
  /** Format painter: the border is drawn by the dashed DOM frame instead. */
  painter?: boolean;
}

/** Device pixel ratio the overlay was built for (for line and handle sizes). */
export interface OverlayScale {
  /** Device pixels per point (zoom × 4/3 × devicePixelRatio). */
  s: number;
  dpr: number;
}

const mix = (a: Rgba, b: Rgba, t: number): Rgba => [
  Math.round(a[0] + (b[0] - a[0]) * t),
  Math.round(a[1] + (b[1] - a[1]) * t),
  Math.round(a[2] + (b[2] - a[2]) * t),
  255,
];
const WHITE: Rgba = [255, 255, 255, 255];
/** Selection shading: a neutral grey over the cells (they stay readable on any fill). */
export const SHADE: Rgba = [0, 0, 0, 30];
/** Darkened accent for header labels and borders. */
export const accentDark = (a: Rgba): Rgba => mix(a, [0, 0, 0, 255], 0.18);
/** Band of a header whose row / column is part of the selection. */
export const headerSelected = (a: Rgba): Rgba => mix([0xdc, 0xdc, 0xdc, 255], a, 0.12);
/** Band of a header whose whole row / column is selected. */
export const headerWhole = (a: Rgba): Rgba => mix(WHITE, a, 0.32);

const rectPath = (x: number, y: number, w: number, h: number, ccw = false): PathSeg[] =>
  ccw
    ? [{ M: [x, y] }, { L: [x, y + h] }, { L: [x + w, y + h] }, { L: [x + w, y] }, 'Z']
    : [{ M: [x, y] }, { L: [x + w, y] }, { L: [x + w, y + h] }, { L: [x, y + h] }, 'Z'];

/** Parses `#rrggbb`, `#rgb` or `rgb(r, g, b)` (the accent CSS variable). */
export function parseColor(css: string, fallback: Rgba): Rgba {
  const t = css.trim();
  let m = /^#([0-9a-f]{6})$/i.exec(t);
  if (m) {
    const n = parseInt(m[1]!, 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255, 255];
  }
  m = /^#([0-9a-f]{3})$/i.exec(t);
  if (m) {
    const [r, g, b] = [...m[1]!].map((c) => parseInt(c + c, 16)) as [number, number, number];
    return [r, g, b, 255];
  }
  const r = /^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i.exec(t);
  if (r) return [Number(r[1]), Number(r[2]), Number(r[3]), 255];
  return fallback;
}

const covers = (r: Range, i: number, rows: boolean) =>
  rows ? i >= r.r0 && i <= r.r1 : i >= r.c0 && i <= r.c1;

/** Builds the overlay commands (points). */
export function overlayCommands(
  g: GridGeometry,
  st: OverlayState,
  { s, dpr }: OverlayScale,
): DisplayCommand[] {
  const out: DisplayCommand[] = [];
  const px = (v: number) => Math.round(v * s);
  const pt = (d: number) => d / s;
  /** Device-pixel width of the grid's hairlines and of the selection border. */
  const wd = Math.max(1, Math.round(PT_PER_PX * s));
  const t = Math.max(2, Math.round(2 * dpr));
  const accent = st.accent;
  const dark = accentDark(accent);
  const areas = [...st.extra, st.range].map((r) => expandMerges(r, st.merges ?? []));
  const main = areas[areas.length - 1]!;
  const multi = st.extra.length > 0;

  // ---- headers of the selected columns and rows
  const headers = (rows: boolean) => {
    const panes: AxisPane[] = rows ? g.rows : g.cols;
    for (const p of panes) {
      const band: Box = rows
        ? { x: 0, y: p.start, w: g.headerW, h: p.end - p.start }
        : { x: p.start, y: 0, w: p.end - p.start, h: g.headerH };
      if (band.w <= 0 || band.h <= 0) continue;
      const hit = p.segs.filter((sg) => areas.some((a) => covers(a, sg.index, rows)));
      if (hit.length === 0) continue;
      out.push({ op: 'pushClip', ...band });
      for (const sg of hit) {
        const whole = areas.some(
          (a) =>
            covers(a, sg.index, rows) &&
            (rows
              ? a.c0 === 0 && a.c1 >= st.wholeColsFrom
              : a.r0 === 0 && a.r1 >= st.wholeRowsFrom),
        );
        const a0 = pt(px(sg.start));
        const a1 = pt(px(sg.end));
        const cell: Box = rows
          ? { x: 0, y: a0, w: pt(px(g.headerW) - wd), h: a1 - a0 - pt(wd) }
          : { x: a0, y: 0, w: a1 - a0 - pt(wd), h: pt(px(g.headerH) - wd) };
        out.push({
          op: 'fillRect',
          ...cell,
          color: whole ? headerWhole(accent) : headerSelected(accent),
        });
        out.push({ ...sg.label, color: dark });
        // The accent line on the header edge towards the cells.
        const line = rows
          ? { x: pt(px(g.headerW) - t), y: a0 - pt(wd), w: pt(t), h: a1 - a0 + pt(wd) }
          : { x: a0 - pt(wd), y: pt(px(g.headerH) - t), w: a1 - a0 + pt(wd), h: pt(t) };
        out.push({ op: 'fillRect', ...line, color: accent });
      }
      out.push({ op: 'popClip' });
    }
  };
  headers(false);
  headers(true);

  // ---- select-all triangle in the corner
  {
    const w = px(g.headerW) - wd;
    const h = px(g.headerH) - wd;
    const k = Math.round(Math.min(w, h) * 0.55);
    const m = Math.max(2, Math.round(3 * dpr));
    const all =
      main.r0 === 0 && main.c0 === 0 && main.r1 >= st.wholeRowsFrom && main.c1 >= st.wholeColsFrom;
    out.push({
      op: 'path',
      segs: [
        { M: [pt(w - m), pt(h - m - k)] },
        { L: [pt(w - m), pt(h - m)] },
        { L: [pt(w - m - k), pt(h - m)] },
        'Z',
      ],
      fill: { type: 'solid', color: all ? accent : [0xb4, 0xb4, 0xb4, 255] },
      stroke: null,
    });
  }

  // ---- shading, borders, fill handle (per pane, clipped to it)
  const activeArea = expandMerges(
    { r0: st.active.row, c0: st.active.col, r1: st.active.row, c1: st.active.col },
    st.merges ?? [],
  );
  const activeBoxes = rangeBoxes(g, activeArea, st.hidden);
  const single = (a: Range) =>
    a.r0 === activeArea.r0 &&
    a.c0 === activeArea.c0 &&
    a.r1 === activeArea.r1 &&
    a.c1 === activeArea.c1;
  /** Device-pixel box of a cell area: from its near grid line to its far one. */
  const dev = (b: Box) => ({ x0: px(b.x), y0: px(b.y), x1: px(b.x + b.w), y1: px(b.y + b.h) });
  for (const [i, a] of areas.entries()) {
    const isMain = i === areas.length - 1;
    if (!st.showRange && isMain) continue;
    for (const pb of rangeBoxes(g, a, st.hidden)) {
      out.push({ op: 'pushClip', ...pb.clip });
      const d = dev(pb.rect);
      const segs: PathSeg[] = [];
      if (!(isMain && !multi && single(a))) {
        segs.push(...rectPath(pt(d.x0), pt(d.y0), pt(d.x1 - d.x0), pt(d.y1 - d.y0)));
        // The active cell stays unshaded (a hole in the shading).
        const ab = activeBoxes.find((x) => x.clip.x === pb.clip.x && x.clip.y === pb.clip.y);
        if (ab && (isMain || (covers(a, st.active.row, true) && covers(a, st.active.col, false)))) {
          const q = dev(ab.rect);
          segs.push(
            ...rectPath(pt(q.x0), pt(q.y0), pt(q.x1 - q.x0 - wd), pt(q.y1 - q.y0 - wd), true),
          );
        }
        out.push({ op: 'path', segs, fill: { type: 'solid', color: SHADE }, stroke: null });
      }
      out.push({ op: 'popClip' });
    }
  }
  if (st.showRange && !st.painter) {
    const off = Math.floor((t - wd) / 2);
    if (multi) {
      // Several ranges: no thick border; a thin one marks the active cell.
      for (const ab of activeBoxes) {
        const q = dev(ab.rect);
        out.push({ op: 'pushClip', ...ab.clip });
        const x0 = q.x0 - wd;
        const y0 = q.y0 - wd;
        const x1 = q.x1;
        const y1 = q.y1;
        for (const r of [
          [x0, y0, x1 - x0, wd],
          [x0, y1 - wd, x1 - x0, wd],
          [x0, y0, wd, y1 - y0],
          [x1 - wd, y0, wd, y1 - y0],
        ] as const)
          out.push({
            op: 'fillRect',
            x: pt(r[0]),
            y: pt(r[1]),
            w: pt(r[2]),
            h: pt(r[3]),
            color: dark,
          });
        out.push({ op: 'popClip' });
      }
    } else {
      const boxes = rangeBoxes(g, main, st.hidden);
      // The handle sits on the bottom-right corner of the whole range.
      const far = boxes.reduce<{ x: number; y: number } | null>((m, b) => {
        const d = dev(b.rect);
        return !m || (d.x1 >= m.x && d.y1 >= m.y) ? { x: d.x1, y: d.y1 } : m;
      }, null);
      for (const pb of boxes) {
        const d = dev(pb.rect);
        // Outer box: the border covers the grid lines around the range and one pixel past.
        const x0 = d.x0 - wd - off;
        const y0 = d.y0 - wd - off;
        const x1 = d.x1 - wd - off + t;
        const y1 = d.y1 - wd - off + t;
        out.push({ op: 'pushClip', ...pb.clip });
        for (const r of [
          [x0, y0, x1 - x0, t],
          [x0, y1 - t, x1 - x0, t],
          [x0, y0, t, y1 - y0],
          [x1 - t, y0, t, y1 - y0],
        ] as const)
          out.push({
            op: 'fillRect',
            x: pt(r[0]),
            y: pt(r[1]),
            w: pt(r[2]),
            h: pt(r[3]),
            color: accent,
          });
        if (far && d.x1 === far.x && d.y1 === far.y) {
          const hs = Math.max(5, Math.round(6 * dpr));
          const ring = Math.max(1, Math.round(dpr));
          const cx = x1 - t / 2;
          const cy = y1 - t / 2;
          const hx = Math.round(cx - hs / 2);
          const hy = Math.round(cy - hs / 2);
          out.push({
            op: 'fillRect',
            x: pt(hx - ring),
            y: pt(hy - ring),
            w: pt(hs + 2 * ring),
            h: pt(hs + 2 * ring),
            color: WHITE,
          });
          out.push({ op: 'fillRect', x: pt(hx), y: pt(hy), w: pt(hs), h: pt(hs), color: accent });
        }
        out.push({ op: 'popClip' });
      }
    }
  }
  return out;
}

/** Whether a header index is highlighted (for tests and the accessibility layer). */
export function headerHighlighted(
  st: Pick<OverlayState, 'range' | 'extra'>,
  rows: boolean,
  i: number,
) {
  return [...st.extra, st.range].some((a) => covers(a, i, rows));
}
