/**
 * Selection statistics of the BlueSheet status bar (CORE-UI-007): the real WASM engine
 * answers `rangeStats` with the numbers a person would work out by hand.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmSheet } from '@blueoffice/engine-wasm';
import type { RangeStats, SheetRange } from '../src';

let stats: (...ranges: SheetRange[]) => RangeStats;

beforeAll(() => {
  const root = join(__dirname, '..', '..', '..');
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
  // A1:A5 = 10, 20, 30.5, "text", =SUM(A1:A3); B1:B2 = -4, 0; C1 = TRUE
  const cells: [number, number, string][] = [
    [0, 0, '10'],
    [1, 0, '20'],
    [2, 0, '30.5'],
    [3, 0, 'text'],
    [4, 0, '=SUM(A1:A3)'],
    [0, 1, '-4'],
    [1, 1, '0'],
    [0, 2, 'TRUE'],
  ];
  const e = new WasmSheet();
  e.setLocale('en-US');
  e.newWorkbook('stats.xlsx');
  e.setInputs(0, JSON.stringify(cells));
  stats = (...ranges) =>
    JSON.parse(e.exec(JSON.stringify({ cmd: 'rangeStats', sheet: 0, ranges }))) as RangeStats;
});

const rng = (r0: number, c0: number, r1: number, c1: number): SheetRange => ({ r0, c0, r1, c1 });

describe('selection statistics (rangeStats)', () => {
  it('average, count, numerical count, min, max and sum of a range', () => {
    const s = stats(rng(0, 0, 4, 0));
    expect(s.sum).toBeCloseTo(121, 10);
    expect(s.count).toBe(5);
    expect(s.numbers).toBe(4);
    expect(s.average).toBeCloseTo(30.25, 10);
    expect(s.min).toBe(10);
    expect(s.max).toBeCloseTo(60.5, 10);
  });

  it('negative numbers and zero count as numbers; text and booleans only as cells', () => {
    const s = stats(rng(0, 0, 4, 2));
    expect(s.count).toBe(8);
    expect(s.numbers).toBe(6);
    expect(s.sum).toBeCloseTo(117, 10);
    expect(s.min).toBe(-4);
    expect(s.max).toBeCloseTo(60.5, 10);
    expect(s.average).toBeCloseTo(19.5, 10);
  });

  it('cells in overlapping ranges count once', () => {
    const s = stats(rng(0, 0, 1, 1), rng(1, 0, 2, 0), rng(0, 1, 0, 1));
    expect(s.numbers).toBe(5);
    expect(s.sum).toBeCloseTo(56.5, 10);
    expect(s.min).toBe(-4);
    expect(s.max).toBe(30.5);
  });

  it('no numbers: no average, minimum or maximum', () => {
    const s = stats(rng(3, 0, 3, 0), rng(20, 20, 30, 30));
    expect(s).toMatchObject({ sum: 0, count: 1, numbers: 0, average: null, min: null, max: null });
  });

  it('a large selection is answered from the used cells', () => {
    const t0 = Date.now();
    const s = stats(rng(0, 0, 99_999, 255));
    expect(s.count).toBe(8);
    expect(s.sum).toBeCloseTo(117, 10);
    expect(Date.now() - t0).toBeLessThan(5_000);
  });
});
