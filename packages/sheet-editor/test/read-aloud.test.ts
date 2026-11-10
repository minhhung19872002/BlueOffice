import { ReadAloudSession } from '@blueoffice/editor';
import { FakeSpeechProvider, type CellDetails } from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { EMPTY_ROWS_STOP, sheetReadUnits } from '../src/readAloud';

const VI = { id: 'vi', name: 'An', lang: 'vi-VN', local: true, default: true };
const EN = { id: 'en', name: 'Sam', lang: 'en-US', local: true, default: false };
const settings = { rate: 1, pitch: 1, volume: 1, voice: null, fallbackLang: 'vi-VN' };

function host(cells: Record<string, string>) {
  let calls = 0;
  return {
    get calls() {
      return calls;
    },
    cell: async (_sheet: number, row: number, col: number): Promise<CellDetails> => {
      calls++;
      const display = cells[`${row},${col}`] ?? '';
      return { address: `${row},${col}`, input: display, display };
    },
  };
}

describe('BlueSheet read aloud', () => {
  it('reads non-empty cells row by row, left to right, with their language', async () => {
    const h = host({
      '0,1': 'Doanh thu',
      '0,0': 'Tháng một',
      '1,1': '1.250.000',
      '2,0': 'The total revenue grew',
    });
    const { units, cells } = await sheetReadUnits(h, 0, { r0: 0, c0: 0, r1: 2, c1: 1 }, 'vi-VN');
    expect(units.map((u) => u.text)).toEqual([
      'Tháng một',
      'Doanh thu',
      '1.250.000',
      'The total revenue grew',
    ]);
    expect(cells).toEqual([
      { row: 0, col: 0 },
      { row: 0, col: 1 },
      { row: 1, col: 1 },
      { row: 2, col: 0 },
    ]);
    expect(units.map((u) => u.lang)).toEqual(['vi-VN', 'vi-VN', 'vi-VN', 'en']);
  });

  it('stops a whole-column selection after a run of empty rows', async () => {
    const h = host({ '0,0': 'Một' });
    const { units } = await sheetReadUnits(h, 0, { r0: 0, c0: 0, r1: 1_048_575, c1: 0 }, 'vi-VN');
    expect(units).toHaveLength(1);
    expect(h.calls).toBe(EMPTY_ROWS_STOP + 1);
  });

  it('follows each cell as it is read', async () => {
    const h = host({ '0,0': 'Một', '0,2': 'Hai', '3,1': 'Ba' });
    const { units, cells } = await sheetReadUnits(h, 0, { r0: 0, c0: 0, r1: 3, c1: 2 }, 'vi-VN');
    const p = new FakeSpeechProvider([VI, EN]);
    const followed: { row: number; col: number }[] = [];
    const s = new ReadAloudSession(p, units, settings, {
      onUnit: (i) => followed.push(cells[i]!),
    });
    await s.start();
    p.run();
    expect(p.spoken.map((u) => u.text)).toEqual(['Một', 'Hai', 'Ba']);
    expect(followed).toEqual([
      { row: 0, col: 0 },
      { row: 0, col: 2 },
      { row: 3, col: 1 },
    ]);
  });
});
