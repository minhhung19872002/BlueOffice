import type { FormatMark } from '@blueoffice/platform';

/** Colour of formatting marks (never printed: drawn by the view only). */
export const MARK_COLOR = '#3b6fd8';

/** The canvas calls formatting marks need (mockable in tests). */
export interface MarkCanvas {
  save(): void;
  restore(): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  stroke(): void;
  fillRect(x: number, y: number, w: number, h: number): void;
  fillText(text: string, x: number, y: number): void;
  fillStyle: unknown;
  strokeStyle: unknown;
  lineWidth: number;
  font: string;
  setLineDash?(segments: number[]): void;
}

/** Symbol drawn for a mark kind (the paragraph and line-break marks are glyphs). */
export function markSymbol(kind: FormatMark['kind']): string {
  switch (kind) {
    case 'paragraph':
      return '¶';
    case 'space':
      return '·';
    case 'noBreakSpace':
      return '°';
    case 'tab':
      return '→';
    case 'lineBreak':
      return '↵';
    case 'anchor':
      return '⚓';
    case 'cellEnd':
    case 'rowEnd':
      return '¤';
    case 'pageBreak':
    case 'columnBreak':
      return '';
  }
}

/**
 * Draw formatting marks over a rendered page: ¶ at paragraph ends, · for spaces,
 * → for tabs, ↵ for line breaks, ¤ at the ends of table cells and rows, a dotted
 * rule for page/column breaks and an anchor beside floating objects. `scale` is canvas pixels per point.
 */
export function drawFormattingMarks(ctx: MarkCanvas, marks: FormatMark[], scale: number): void {
  ctx.save();
  ctx.setTransform(scale, 0, 0, scale, 0, 0);
  ctx.fillStyle = MARK_COLOR;
  ctx.strokeStyle = MARK_COLOR;
  ctx.lineWidth = 0.6;
  for (const m of marks) {
    const size = Math.max(4, m.size);
    if (m.kind === 'space') {
      // A centred dot (drawn, so it stays crisp at every zoom).
      const d = Math.max(0.8, size * 0.09);
      ctx.fillRect(m.x + m.w / 2 - d / 2, m.baseline - size * 0.3 - d / 2, d, d);
      continue;
    }
    if (m.kind === 'pageBreak' || m.kind === 'columnBreak') {
      ctx.setLineDash?.([1.5, 1.5]);
      ctx.beginPath();
      const y = m.baseline - size * 0.3;
      ctx.moveTo(m.x, y);
      ctx.lineTo(m.x + Math.max(m.w, size * 6), y);
      ctx.stroke();
      ctx.setLineDash?.([]);
      continue;
    }
    ctx.font = `${size * (m.kind === 'anchor' ? 0.8 : 1)}px sans-serif`;
    const sym = markSymbol(m.kind);
    const x = m.kind === 'tab' ? m.x + Math.max(0, m.w / 2 - size * 0.4) : m.x + 0.5;
    ctx.fillText(sym, m.kind === 'anchor' ? m.x - size * 0.9 : x, m.baseline);
  }
  ctx.restore();
}
