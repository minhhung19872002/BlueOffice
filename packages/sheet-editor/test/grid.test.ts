import { describe, expect, it } from 'vitest';
import {
  a1,
  advance,
  colName,
  fillTarget,
  move,
  visibleIndex,
  parseA1,
  parseNumber,
  parseTsv,
  pasteCells,
  rangeA1,
  rangeOf,
  single,
  span,
  toTsv,
} from '../src/grid';

describe('grid', () => {
  it('names columns and cells', () => {
    expect([0, 25, 26, 51, 701, 702, 16383].map(colName)).toEqual([
      'A',
      'Z',
      'AA',
      'AZ',
      'ZZ',
      'AAA',
      'XFD',
    ]);
    expect(a1({ row: 9, col: 2 })).toBe('C10');
    expect(rangeA1({ r0: 0, c0: 0, r1: 2, c1: 1 })).toBe('A1:B3');
  });

  it('parses Name Box input', () => {
    expect(parseA1('$b$7')).toEqual(single({ row: 6, col: 1 }));
    expect(parseA1('A1:C3')).toEqual(span({ row: 0, col: 0 }, { row: 2, col: 2 }));
    expect(parseA1('XFE1')).toBeNull();
    expect(parseA1('A0')).toBeNull();
    expect(parseA1('hello')).toBeNull();
  });

  it('moves and extends with clamping', () => {
    const s = single({ row: 0, col: 0 });
    expect(move(s, 'up', false)).toEqual(s);
    const ext = move(move(s, 'down', true), 'right', true);
    expect(rangeOf(ext)).toEqual({ r0: 0, c0: 0, r1: 1, c1: 1 });
    expect(move(ext, 'down', false).active).toEqual({ row: 1, col: 0 });
  });

  it('Enter cycles inside a range', () => {
    let cur = span({ row: 0, col: 0 }, { row: 1, col: 1 });
    const order = [cur.active];
    for (let i = 0; i < 4; i++) {
      cur = advance(cur, 'down');
      order.push(cur.active);
    }
    expect(order).toEqual([
      { row: 0, col: 0 },
      { row: 1, col: 0 },
      { row: 0, col: 1 },
      { row: 1, col: 1 },
      { row: 0, col: 0 },
    ]);
    expect(rangeOf(cur)).toEqual({ r0: 0, c0: 0, r1: 1, c1: 1 });
    expect(advance(single({ row: 0, col: 0 }), 'right').active).toEqual({ row: 0, col: 1 });
  });

  it('round-trips TSV with quoting', () => {
    const rows = [
      ['Họ tên', 'Ghi chú'],
      ['Nguyễn "A"', 'dòng 1\ndòng 2'],
      ['', 'x\ty'],
    ];
    expect(parseTsv(toTsv(rows))).toEqual(rows);
    expect(parseTsv('1\t2\r\n3\t4\r\n')).toEqual([
      ['1', '2'],
      ['3', '4'],
    ]);
    expect(pasteCells({ row: 5, col: 1 }, [['a', 'b']])).toEqual([
      [5, 1, 'a'],
      [5, 2, 'b'],
    ]);
  });

  it('parses vi-VN and en-US numbers', () => {
    expect(parseNumber('1.234,5', ',')).toBe(1234.5);
    expect(parseNumber('12,5%', ',')).toBeCloseTo(0.125);
    expect(parseNumber('1.000.000 ₫', ',')).toBe(1_000_000);
    expect(parseNumber('1,234.5', '.')).toBe(1234.5);
    expect(parseNumber('abc', ',')).toBeNull();
    expect(parseNumber('', ',')).toBeNull();
  });
});

describe('hidden rows and AutoFill', () => {
  it('skips hidden rows and columns when moving', () => {
    const hidden = { rows: [[2, 4]] as [number, number][], cols: [[1, 1]] as [number, number][] };
    expect(move(single({ row: 1, col: 0 }), 'down', false, 1, hidden).active).toEqual({
      row: 5,
      col: 0,
    });
    expect(move(single({ row: 5, col: 0 }), 'up', false, 1, hidden).active).toEqual({
      row: 1,
      col: 0,
    });
    expect(move(single({ row: 0, col: 0 }), 'right', false, 1, hidden).active).toEqual({
      row: 0,
      col: 2,
    });
    expect(visibleIndex(0, -1, [[0, 0]], 10)).toBe(0);
  });

  it('extends the fill target along the dominant direction', () => {
    const src = { r0: 0, c0: 0, r1: 1, c1: 0 };
    expect(fillTarget(src, { row: 5, col: 1 })).toEqual({ r0: 0, c0: 0, r1: 5, c1: 0 });
    expect(fillTarget(src, { row: 1, col: 4 })).toEqual({ r0: 0, c0: 0, r1: 1, c1: 4 });
    expect(fillTarget(src, { row: 1, col: 0 })).toBeNull();
  });
});
