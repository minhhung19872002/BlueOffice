import { describe, expect, it } from 'vitest';
import { boxHandles, drawnRect, hitShape, nextCase, snapTo } from '../src/editing';
import { fontChanges, fontChoiceOf, rulerUnit, SUBSCRIPT, SUPERSCRIPT } from '../src/officeFeel';
import { newRun, runHidden } from '../src/presenting';

const box = (x: number, y: number, w: number, h: number, rot = 0) => ({
  cx: x + w / 2,
  cy: y + h / 2,
  w,
  h,
  rot,
});

describe('hit test in the editor', () => {
  const boxes = {
    tolerance: 3,
    shapes: [
      { id: 2, rect: box(100, 100, 200, 100), boxes: [box(100, 100, 200, 100)] },
      // A group: hit through its shapes, not its whole box.
      {
        id: 9,
        rect: box(150, 150, 300, 200),
        boxes: [box(150, 150, 50, 50), box(400, 300, 50, 50)],
      },
    ],
  };
  it('takes the topmost shape whose box is under the point', () => {
    expect(hitShape(boxes, 160, 160)).toBe(9);
    expect(hitShape(boxes, 120, 120)).toBe(2);
    // Inside the group's box but between its shapes: the shape below.
    expect(hitShape(boxes, 250, 180)).toBe(2);
    expect(hitShape(boxes, 600, 500)).toBeNull();
    // The tolerance around a box counts.
    expect(hitShape(boxes, 98, 120)).toBe(2);
  });
  it('follows rotated boxes', () => {
    const r = {
      tolerance: 0,
      shapes: [{ id: 1, rect: box(0, 0, 100, 10, 90), boxes: [box(0, 0, 100, 10, 90)] }],
    };
    // Turned a quarter: the long side is vertical through the centre (50, 5).
    expect(hitShape(r, 50, 40)).toBe(1);
    expect(hitShape(r, 90, 5)).toBeNull();
  });
});

describe('handles', () => {
  it('eight sizing handles on the box and the rotation handle 18 pt above the top edge', () => {
    const h = boxHandles(box(100, 100, 200, 100));
    expect(h.map((x) => x.kind)).toEqual(['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w', 'rot']);
    expect(h[0]).toEqual({ kind: 'nw', x: 100, y: 100 });
    expect(h[4]).toEqual({ kind: 'se', x: 300, y: 200 });
    expect(h[8]).toEqual({ kind: 'rot', x: 200, y: 82 });
  });
});

describe('drawing a new object', () => {
  it('a click gives the default size at the point, a drag the dragged box', () => {
    expect(drawnRect({ x: 60, y: 300 }, null)).toEqual({ x: 60, y: 300, w: 72, h: 72 });
    expect(drawnRect({ x: 60, y: 300 }, null, { text: true })).toEqual({
      x: 60,
      y: 300,
      w: 144,
      h: 30,
    });
    // Dragged up and to the left: the box still starts at its top-left corner.
    expect(drawnRect({ x: 300, y: 300 }, { x: 200, y: 250 })).toEqual({
      x: 200,
      y: 250,
      w: 100,
      h: 50,
    });
    // Shift: a square of the larger side.
    expect(drawnRect({ x: 0, y: 0 }, { x: 120, y: 60 }, { square: true })).toEqual({
      x: 0,
      y: 0,
      w: 120,
      h: 120,
    });
  });
  it('snaps to the grid', () => {
    expect(snapTo(61, 6, 3)).toBe(60);
    expect(snapTo(64, 6, 3)).toBe(66);
    expect(snapTo(64, 6, 1)).toBe(64);
  });
});

describe('font dialog', () => {
  const start = fontChoiceOf({
    hasText: true,
    bold: true,
    italic: false,
    underline: false,
    strike: false,
    size: 44,
    color: '000000',
    highlight: null,
    font: 'Arial',
    align: 'left',
    bullets: false,
    numbering: false,
    level: 0,
    lineSpacing: null,
  });
  it('applies only what changed', () => {
    expect(fontChanges(start, start)).toEqual([]);
    expect(
      fontChanges(start, { ...start, baseline: SUPERSCRIPT, caps: 'small', spacing: 2 }),
    ).toEqual([
      { kind: 'baseline', value: 30000 },
      { kind: 'caps', value: 'small' },
      { kind: 'spacing', points: 2 },
    ]);
    expect(fontChanges(start, { ...start, bold: false, size: 24 })).toEqual([
      { kind: 'bold', value: false },
      { kind: 'size', points: 24 },
    ]);
    expect(SUBSCRIPT).toBeLessThan(0);
  });
  it('double strikethrough replaces the single one', () => {
    expect(fontChanges({ ...start, strike: true }, { ...start, doubleStrike: true })).toEqual([
      { kind: 'doubleStrike', value: true },
    ]);
    expect(fontChanges({ ...start, doubleStrike: true }, start)).toEqual([
      { kind: 'strike', value: false },
    ]);
  });
  it('rulers in centimetres for Vietnamese, inches for English', () => {
    expect(rulerUnit('vi-VN').pt).toBeCloseTo(28.35, 2);
    expect(rulerUnit('en-US').pt).toBe(72);
  });
});

describe('H in the show', () => {
  it('shows the hidden slide that follows, then goes on after it', () => {
    // Slide 1 (0-based) is hidden: the order skips it.
    const run = newRun([0, 2, 3]);
    const pos = { at: 0, slide: 0, click: 0, enter: 'jump' as const };
    const r = runHidden(run, pos, 4, () => 0);
    expect(r?.pos.slide).toBe(1);
    expect(r?.run.order).toEqual([0, 1, 2, 3]);
    // No hidden slide after the current one: nothing happens.
    expect(runHidden(run, { ...pos, at: 1, slide: 2 }, 4, () => 0)).toBeNull();
  });
});

describe('Shift+F3', () => {
  it('cycles lower case → UPPER CASE → Each Word Capitalised → lower case', () => {
    expect(nextCase('báo cáo quý')).toBe('BÁO CÁO QUÝ');
    expect(nextCase('BÁO CÁO QUÝ')).toBe('Báo Cáo Quý');
    expect(nextCase('Báo Cáo Quý')).toBe('báo cáo quý');
    // Any other mix first goes to upper case.
    expect(nextCase('Báo cáo quý')).toBe('BÁO CÁO QUÝ');
  });
});
