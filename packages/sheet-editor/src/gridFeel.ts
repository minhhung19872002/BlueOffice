/**
 * Small pure helpers for the feel of the BlueSheet grid (spreadsheet conventions): wheel
 * scrolling by three rows a notch, the column-width / row-height tip while a header boundary
 * is dragged, and the in-cell editor that uses the cell's own (bundled) font so the text does
 * not jump between viewing and editing, grows to the right while typing and puts the caret
 * where the cell was double-clicked.
 */
import type { FontSource } from '@blueoffice/platform';
import type { GridGeometry } from './gridView';

/** Rows scrolled by one wheel notch (the common spreadsheet default). */
export const ROWS_PER_NOTCH = 3;

export interface WheelLike {
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  ctrlKey?: boolean;
}

/**
 * Rows to scroll for a wheel event, or `null` to let the browser scroll smoothly (touchpads
 * send many small pixel deltas; a mouse wheel sends whole notches of about 100 px or lines).
 */
export function wheelRows(e: WheelLike): number | null {
  if (e.ctrlKey || e.deltaY === 0 || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return null;
  if (e.deltaMode === 1) return Math.round(e.deltaY) || Math.sign(e.deltaY);
  if (e.deltaMode !== 0 || Math.abs(e.deltaY) < 50) return null;
  const notches = Math.max(1, Math.round(Math.abs(e.deltaY) / 100));
  return Math.sign(e.deltaY) * notches * ROWS_PER_NOTCH;
}

/**
 * Vertical scroll (points) that moves the top row of the scrollable pane by `rows` rows and
 * lines the new top row up with the pane's top edge.
 */
export function wheelScrollPt(g: GridGeometry, rows: number): number {
  const pane = g.rows[g.rows.length - 1];
  if (!pane || pane.segs.length === 0) return 0;
  const top = pane.segs.findIndex((s) => s.end > pane.start + 1e-3);
  if (top < 0) return 0;
  const seg = pane.segs[top]!;
  if (rows > 0) {
    const target = pane.segs[top + rows];
    if (target) return target.start - pane.start;
    // Past the drawn rows: continue with the last row's height.
    const last = pane.segs[pane.segs.length - 1]!;
    const extra = top + rows - (pane.segs.length - 1);
    return last.start - pane.start + extra * (last.end - last.start);
  }
  // Up: the rows above are not drawn; whole rows of the top row's height, aligned.
  const h = seg.end - seg.start || 15;
  const partial = pane.start - seg.start;
  const n = -rows;
  return partial > 1e-3 ? -(partial + (n - 1) * h) : -n * h;
}

/** Column width in characters of the default font (7 px digits plus 5 px padding at 100 %). */
export function widthChars(px: number): number {
  if (px <= 0) return 0;
  if (px < 12) return Math.round((px / 12) * 100) / 100;
  return Math.trunc(((px - 5) / 7) * 100 + 0.5) / 100;
}

/** Numbers of the size tip: column width in characters + pixels, row height in points + pixels. */
export function resizeTip(rows: boolean, sizePt: number): { value: number; px: number } {
  const px = Math.max(0, Math.round((sizePt * 4) / 3));
  return rows ? { value: Math.round(sizePt * 100) / 100, px } : { value: widthChars(px), px };
}

/** The bundled font that draws a cell font (same mapping as the engine's substitutes). */
export function cellFontFor(name: string | undefined): string {
  const n = (name ?? '').toLowerCase();
  if (/calibri|carlito/.test(n)) return 'Carlito';
  if (/cambria|caladea/.test(n)) return 'Caladea';
  if (/times|tinos|georgia|serif/.test(n) && !/sans/.test(n)) return 'Tinos';
  return 'LiberationSans';
}

/** CSS font-family of the in-cell editor for a bundled family. */
export const CELL_FONT_FAMILY = (family: string) =>
  `'BlueSheet ${family}', ${family === 'LiberationSans' ? "'Liberation Sans', Arial" : family}, sans-serif`;

/** Vertical metrics of a font as the engine uses them (font units). */
export interface FontMetrics {
  upem: number;
  ascender: number;
  /** Negative, below the baseline. */
  descender: number;
  lineGap: number;
}

/**
 * Reads the vertical metrics of a TrueType / OpenType font like the engine's font manager:
 * `OS/2` typographic values when its USE_TYPO_METRICS flag (fsSelection bit 7) is set,
 * otherwise `hhea` (OpenType spec, tables `head`, `hhea`, `OS/2`).
 */
export function fontMetrics(bytes: Uint8Array): FontMetrics | null {
  if (bytes.length < 12) return null;
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const n = v.getUint16(4);
  const tables = new Map<string, number>();
  for (let i = 0; i < n; i++) {
    const at = 12 + i * 16;
    if (at + 16 > bytes.length) return null;
    const tag = String.fromCharCode(bytes[at]!, bytes[at + 1]!, bytes[at + 2]!, bytes[at + 3]!);
    tables.set(tag, v.getUint32(at + 8));
  }
  const head = tables.get('head');
  const hhea = tables.get('hhea');
  if (head === undefined || hhea === undefined || hhea + 10 > bytes.length) return null;
  const upem = v.getUint16(head + 18);
  let m: FontMetrics = {
    upem,
    ascender: v.getInt16(hhea + 4),
    descender: v.getInt16(hhea + 6),
    lineGap: v.getInt16(hhea + 8),
  };
  const os2 = tables.get('OS/2');
  if (os2 !== undefined && os2 + 74 <= bytes.length && (v.getUint16(os2 + 62) & 0x80) !== 0) {
    m = {
      upem,
      ascender: v.getInt16(os2 + 68),
      descender: v.getInt16(os2 + 70),
      lineGap: v.getInt16(os2 + 72),
    };
  }
  return upem > 0 ? m : null;
}

/**
 * Where the engine puts the baseline of a single line of bottom-aligned cell text: one
 * padding half (1 px at 100 %) above the bottom grid line, less the descent and line gap.
 * Pixels from the cell top; `zoom` scales the padding.
 */
export function bottomBaseline(
  cellH: number,
  sizePx: number,
  m: FontMetrics,
  zoom: number,
): number {
  const below = ((-m.descender + Math.max(0, m.lineGap)) * sizePx) / m.upem;
  return cellH - zoom - below;
}

const loaded = new Map<string, Promise<FontMetrics | null>>();

/**
 * Registers the bundled faces of `family` for the in-cell editor (once; via the platform);
 * resolves to the regular face's metrics (`null` when fonts cannot be registered).
 */
export function loadCellFonts(
  fonts: FontSource | undefined,
  family: string,
): Promise<FontMetrics | null> {
  if (!fonts || typeof FontFace === 'undefined' || typeof document === 'undefined')
    return Promise.resolve(null);
  const have = loaded.get(family);
  if (have) return have;
  const faces: [string, string, string][] = [
    ['Regular', 'normal', 'normal'],
    ['Bold', 'bold', 'normal'],
    ['Italic', 'normal', 'italic'],
    ['BoldItalic', 'bold', 'italic'],
  ];
  const p = Promise.all(
    faces.map(async ([suffix, weight, style]) => {
      const bytes = await fonts.load(`${family}-${suffix}.ttf`);
      const face = new FontFace(`BlueSheet ${family}`, bytes as BufferSource, { weight, style });
      await face.load();
      document.fonts.add(face);
      return suffix === 'Regular' ? fontMetrics(bytes) : null;
    }),
  ).then(
    (ms) => ms[0] ?? null,
    () => null,
  );
  loaded.set(family, p);
  return p;
}

/** Caret index in `text` nearest to `x` (from the text start), given a prefix measurer. */
export function caretAt(text: string, x: number, measure: (s: string) => number): number {
  if (x <= 0) return 0;
  let prev = 0;
  for (let i = 1; i <= text.length; i++) {
    const w = measure(text.slice(0, i));
    if (w >= x) return x - prev < w - x ? i - 1 : i;
    prev = w;
  }
  return text.length;
}

/** Width of the in-cell editor: the cell, grown to fit the text, at most `max`. */
export function editorWidth(textW: number, cellW: number, pad: number, max: number): number {
  return Math.max(cellW, Math.min(max, Math.ceil(textW + 2 * pad + 4)));
}
