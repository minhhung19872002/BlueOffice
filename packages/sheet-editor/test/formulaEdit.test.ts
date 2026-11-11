import { describe, expect, it } from 'vitest';
import {
  REF_COLORS,
  argumentHint,
  callAt,
  canPoint,
  formulaRefs,
  functionMatches,
  functionPrefix,
  mirrorParts,
  refText,
} from '../src/formulaEdit';

describe('references of a formula being typed', () => {
  it('finds cells, ranges and sheet references with colours by first appearance', () => {
    const refs = formulaRefs("=SUM(A1:B2)+$C$3*'Bán hàng'!D4+a1:b2+Sheet2!E5");
    expect(refs.map((r) => [r.sheet, r.range])).toEqual([
      [undefined, { r0: 0, c0: 0, r1: 1, c1: 1 }],
      [undefined, { r0: 2, c0: 2, r1: 2, c1: 2 }],
      ['Bán hàng', { r0: 3, c0: 3, r1: 3, c1: 3 }],
      [undefined, { r0: 0, c0: 0, r1: 1, c1: 1 }],
      ['Sheet2', { r0: 4, c0: 4, r1: 4, c1: 4 }],
    ]);
    const colours = refs.map((r) => r.color);
    // The same range twice keeps its colour; the others get the next ones.
    expect(colours).toEqual([
      REF_COLORS[0],
      REF_COLORS[1],
      REF_COLORS[2],
      REF_COLORS[0],
      REF_COLORS[3],
    ]);
  });

  it('ignores text in quotes, function names and plain text', () => {
    expect(formulaRefs('=LOG10(2)&"A1"')).toEqual([]);
    expect(formulaRefs('A1+B2')).toEqual([]);
    expect(formulaRefs('=TOTAL1+B2').map((r) => r.range)).toEqual([{ r0: 1, c0: 1, r1: 1, c1: 1 }]);
  });

  it('splits the text at its references for the coloured mirror', () => {
    const text = '=A1+B2';
    expect(mirrorParts(text, formulaRefs(text))).toEqual([
      { text: '=' },
      { text: 'A1', color: REF_COLORS[0] },
      { text: '+' },
      { text: 'B2', color: REF_COLORS[1] },
    ]);
  });
});

describe('point mode', () => {
  it('can pick after =, operators, ( and separators, not after a value', () => {
    expect(canPoint('=', 1)).toBe(true);
    expect(canPoint('=SUM(', 5)).toBe(true);
    expect(canPoint('=SUM(A1,', 8)).toBe(true);
    expect(canPoint('=A1+ ', 5)).toBe(true);
    expect(canPoint('=A1', 3)).toBe(false);
    expect(canPoint('=SUM(A1)', 8)).toBe(false);
    expect(canPoint('="a+', 4)).toBe(false);
    expect(canPoint('abc', 3)).toBe(false);
  });

  it('writes picked ranges, with the sheet name quoted when needed', () => {
    expect(refText({ r0: 0, c0: 0, r1: 0, c1: 0 })).toBe('A1');
    expect(refText({ r0: 1, c0: 1, r1: 4, c1: 2 })).toBe('B2:C5');
    expect(refText({ r0: 1, c0: 1, r1: 1, c1: 1 }, 'Sheet2')).toBe('Sheet2!B2');
    expect(refText({ r0: 1, c0: 1, r1: 1, c1: 1 }, 'Bán hàng')).toBe("'Bán hàng'!B2");
    expect(refText({ r0: 0, c0: 0, r1: 0, c1: 0 }, "Q1'24")).toBe("'Q1''24'!A1");
    expect(refText({ r0: 0, c0: 0, r1: 0, c1: 0 }, 'AB12')).toBe("'AB12'!A1");
  });
});

describe('function names and argument hint', () => {
  it('finds the name being typed', () => {
    expect(functionPrefix('=VLO', 4)).toEqual({ from: 1, prefix: 'VLO' });
    expect(functionPrefix('=A1+su', 6)).toEqual({ from: 4, prefix: 'su' });
    expect(functionPrefix('=A1', 3)).toBeNull();
    expect(functionPrefix('="VLO', 5)).toBeNull();
    expect(functionPrefix('VLO', 3)).toBeNull();
    expect(functionMatches(['COUNT', 'COUNTA', 'COUNTIF', 'COUNTIFS', 'SUM'], 'counti')).toEqual([
      'COUNTIF',
      'COUNTIFS',
    ]);
  });

  it('knows the call and the argument at the caret', () => {
    expect(callAt('=IF(A1>0,', 9)).toEqual({ name: 'IF', arg: 1 });
    expect(callAt('=IF(SUM(A1,B1', 13)).toEqual({ name: 'SUM', arg: 1 });
    expect(callAt('=IF(SUM(A1,B1),', 15)).toEqual({ name: 'IF', arg: 1 });
    expect(callAt('=IF(A1,"x,y",', 13)).toEqual({ name: 'IF', arg: 2 });
    expect(callAt('=A1', 3)).toBeNull();
    const h = argumentHint('SUM', 5)!;
    // Past the listed arguments of a repeating list, the repeating one stays current.
    expect(h.args.filter((a) => a.current).map((a) => a.text)).toEqual(['[number2]']);
    expect(argumentHint('VLOOKUP', 1)!.args.find((a) => a.current)!.text).toBe('table');
    expect(argumentHint('NOSUCH', 0)).toBeNull();
  });
});
