/**
 * Pure grid logic of the BlueSheet editor: cell addresses, selection and
 * keyboard navigation, clipboard (TSV) encoding. No DOM, fully unit-tested.
 */

/** Grid limits (ECMA-376 Part 1, §18.3.1.73 / §18.3.1.13). */
export const MAX_ROW = 1_048_575;
export const MAX_COL = 16_383;

export interface CellPos {
  row: number;
  col: number;
}

/**
 * Anchor = where the selection started, focus = its moving end; together they
 * span the range. `active` is the cell being edited (inside the range).
 */
export interface Selection {
  anchor: CellPos;
  focus: CellPos;
  active: CellPos;
}

export interface Range {
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

export const clampPos = (p: CellPos): CellPos => ({
  row: Math.min(MAX_ROW, Math.max(0, p.row)),
  col: Math.min(MAX_COL, Math.max(0, p.col)),
});

export const single = (p: CellPos): Selection => ({ anchor: p, focus: p, active: p });

export const span = (anchor: CellPos, focus: CellPos): Selection => ({
  anchor,
  focus,
  active: anchor,
});

export function rangeOf(s: Selection): Range {
  return {
    r0: Math.min(s.anchor.row, s.focus.row),
    c0: Math.min(s.anchor.col, s.focus.col),
    r1: Math.max(s.anchor.row, s.focus.row),
    c1: Math.max(s.anchor.col, s.focus.col),
  };
}

/** `0` → `A`, `25` → `Z`, `26` → `AA`. */
export function colName(col: number): string {
  let n = col + 1;
  let s = '';
  while (n > 0) {
    const m = (n - 1) % 26;
    s = String.fromCharCode(65 + m) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}

export const a1 = (p: CellPos): string => `${colName(p.col)}${p.row + 1}`;

export function rangeA1(r: Range): string {
  const a = a1({ row: r.r0, col: r.c0 });
  return r.r0 === r.r1 && r.c0 === r.c1 ? a : `${a}:${a1({ row: r.r1, col: r.c1 })}`;
}

/** Parses `B7`, `$B$7`, `b7:d9` (Name Box input). */
export function parseA1(text: string): Selection | null {
  const parts = text.trim().replace(/\$/g, '').toUpperCase().split(':');
  if (parts.length < 1 || parts.length > 2) return null;
  const cells: CellPos[] = [];
  for (const p of parts) {
    const m = /^([A-Z]{1,3})([0-9]{1,7})$/.exec(p);
    if (!m) return null;
    let col = 0;
    for (const ch of m[1]!) col = col * 26 + (ch.charCodeAt(0) - 64);
    const row = Number(m[2]) - 1;
    if (row < 0 || row > MAX_ROW || col - 1 > MAX_COL) return null;
    cells.push({ row, col: col - 1 });
  }
  return span(cells[0]!, cells[1] ?? cells[0]!);
}

export type Direction = 'up' | 'down' | 'left' | 'right';

const DELTA: Record<Direction, CellPos> = {
  up: { row: -1, col: 0 },
  down: { row: 1, col: 0 },
  left: { row: 0, col: -1 },
  right: { row: 0, col: 1 },
};

/** Hidden spans `[first, last]` of rows and columns (filtered / hidden). */
export interface HiddenSpans {
  rows: [number, number][];
  cols: [number, number][];
}

const inSpan = (i: number, spans: [number, number][]) => spans.find(([a, b]) => i >= a && i <= b);

/** First visible index from `i` going in direction `sign` (stays put when none). */
export function visibleIndex(
  i: number,
  sign: number,
  spans: [number, number][],
  max: number,
): number {
  let j = i;
  for (let guard = 0; guard < spans.length + 1; guard++) {
    const s = inSpan(j, spans);
    if (!s) return j;
    j = sign >= 0 ? s[1] + 1 : s[0] - 1;
    if (j < 0 || j > max) return i;
  }
  return j;
}

/** Arrow-key movement; `extend` (Shift) keeps the anchor. Hidden rows/columns are skipped. */
export function move(
  s: Selection,
  dir: Direction,
  extend: boolean,
  step = 1,
  hidden?: HiddenSpans,
): Selection {
  const d = DELTA[dir];
  const from = extend ? s.focus : s.active;
  let focus = clampPos({ row: from.row + d.row * step, col: from.col + d.col * step });
  if (hidden) {
    focus = {
      row: d.row ? visibleIndex(focus.row, d.row, hidden.rows, MAX_ROW) : focus.row,
      col: d.col ? visibleIndex(focus.col, d.col, hidden.cols, MAX_COL) : focus.col,
    };
  }
  return extend ? { anchor: s.anchor, focus, active: s.active } : single(focus);
}

/**
 * Target of an AutoFill drag from `src` to the cell under the mouse: the
 * source extended along the dominant direction (down/up/right/left).
 */
export function fillTarget(src: Range, to: CellPos): Range | null {
  const down = to.row - src.r1;
  const up = src.r0 - to.row;
  const right = to.col - src.c1;
  const left = src.c0 - to.col;
  const best = Math.max(down, up, right, left);
  if (best <= 0) return null;
  if (best === down) return { ...src, r1: to.row };
  if (best === up) return { ...src, r0: to.row };
  if (best === right) return { ...src, c1: to.col };
  return { ...src, c0: to.col };
}

/**
 * Enter / Tab inside a multi-cell selection cycle through it without
 * collapsing it (like common spreadsheet apps); otherwise they just move.
 */
export function advance(s: Selection, dir: 'down' | 'right' | 'up' | 'left'): Selection {
  const r = rangeOf(s);
  if (r.r0 === r.r1 && r.c0 === r.c1) return move(s, dir, false);
  let { row, col } = s.active;
  if (dir === 'down' || dir === 'up') {
    row += dir === 'down' ? 1 : -1;
    if (row > r.r1) {
      row = r.r0;
      col = col >= r.c1 ? r.c0 : col + 1;
    } else if (row < r.r0) {
      row = r.r1;
      col = col <= r.c0 ? r.c1 : col - 1;
    }
  } else {
    col += dir === 'right' ? 1 : -1;
    if (col > r.c1) {
      col = r.c0;
      row = row >= r.r1 ? r.r0 : row + 1;
    } else if (col < r.c0) {
      col = r.c1;
      row = row <= r.r0 ? r.r1 : row - 1;
    }
  }
  return { anchor: s.anchor, focus: s.focus, active: { row, col } };
}

export const cellsOf = (r: Range): number => (r.r1 - r.r0 + 1) * (r.c1 - r.c0 + 1);

/** Tab-separated text, quoting fields with tabs/newlines/quotes (clipboard). */
export function toTsv(rows: string[][]): string {
  const q = (v: string) => (/[\t\n\r"]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v);
  return rows.map((r) => r.map(q).join('\t')).join('\r\n');
}

/** Parses clipboard text (TSV with optional quoting) into rows of fields. */
export function parseTsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';
  let i = 0;
  let quoted = false;
  const t = text.replace(/\r\n?/g, '\n');
  while (i < t.length) {
    const ch = t[i]!;
    if (quoted) {
      if (ch === '"' && t[i + 1] === '"') {
        field += '"';
        i += 2;
        continue;
      }
      if (ch === '"') quoted = false;
      else field += ch;
      i++;
      continue;
    }
    if (ch === '"' && field === '') quoted = true;
    else if (ch === '\t') {
      row.push(field);
      field = '';
    } else if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else field += ch;
    i++;
  }
  if (field !== '' || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

/** Inputs for pasting `rows` with the top-left at `at`. */
export function pasteCells(at: CellPos, rows: string[][]): [number, number, string][] {
  const out: [number, number, string][] = [];
  rows.forEach((r, dr) =>
    r.forEach((v, dc) => {
      const row = at.row + dr;
      const col = at.col + dc;
      if (row <= MAX_ROW && col <= MAX_COL) out.push([row, col, v]);
    }),
  );
  return out;
}

/** Parses a number typed or displayed in vi-VN (`1.234,5`) or en-US (`1,234.5`) form. */
export function parseNumber(text: string, decimal: ',' | '.'): number | null {
  let t = text.trim().replace(/\s|₫|\$/g, '');
  if (!t) return null;
  let scale = 1;
  if (t.endsWith('%')) {
    scale = 0.01;
    t = t.slice(0, -1);
  }
  const group = decimal === ',' ? '.' : ',';
  t = t.split(group).join('');
  if (decimal === ',') t = t.replace(',', '.');
  if (!/^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/.test(t)) return null;
  const n = Number(t) * scale;
  return Number.isFinite(n) ? n : null;
}
