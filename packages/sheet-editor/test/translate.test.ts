import { FakeTranslationProvider, TranslationError } from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { asTextInput, translatableInput, translateCells } from '../src/translate';

/** A fake sheet: inputs by `row:col`; inserting columns shifts the cells right of them. */
function fakeHost(cells: Record<string, string>) {
  let data = new Map(Object.entries(cells));
  const calls: string[] = [];
  const host = {
    cell: async (_s: number, row: number, col: number) => {
      const input = data.get(`${row}:${col}`) ?? '';
      return { address: '', input, display: input.startsWith("'") ? input.slice(1) : input };
    },
    setInputs: async (_s: number, writes: [number, number, string][]) => {
      calls.push(`set:${writes.length}`);
      for (const [r, c, v] of writes) data.set(`${r}:${c}`, v);
    },
    structural: async (_s: number, rows: boolean, at: number, count: number, remove: boolean) => {
      calls.push(`structural:${rows}:${at}:${count}:${remove}`);
      const next = new Map<string, string>();
      for (const [k, v] of data) {
        const [r, c] = k.split(':').map(Number) as [number, number];
        next.set(`${r}:${!rows && c >= at ? c + count : c}`, v);
      }
      data = next;
    },
  };
  return { host, calls, get: (r: number, c: number) => data.get(`${r}:${c}`) };
}

describe('BlueSheet translation', () => {
  it('only translates text cells', () => {
    expect(translatableInput('Xin chào')).toBe(true);
    expect(translatableInput('=A1&"x"')).toBe(false);
    expect(translatableInput('12.5')).toBe(false);
    expect(translatableInput('TRUE')).toBe(false);
    expect(translatableInput('#N/A')).toBe(false);
    expect(translatableInput('  ')).toBe(false);
    expect(asTextInput('=sum')).toBe("'=sum");
    expect(asTextInput('- item')).toBe("'- item");
    expect(asTextInput('ok')).toBe('ok');
  });

  it('translates into new columns right of the selection', async () => {
    const { host, calls, get } = fakeHost({
      '0:0': 'Tên',
      '0:1': 'Giá',
      '0:2': 'Ghi chú cũ',
      '1:0': 'Bàn làm việc',
      '1:1': '1200',
      '2:1': '=B2*2',
    });
    const fake = new FakeTranslationProvider();
    const n = await translateCells(
      host,
      0,
      { r0: 0, c0: 0, r1: 2, c1: 1 },
      fake,
      'vi',
      'en',
      'newColumn',
    );
    expect(n).toBe(3);
    // Two columns inserted at C; the old C moved to E.
    expect(calls).toEqual(['structural:false:2:2:false', 'set:3']);
    expect(get(0, 2)).toBe('[en] Tên');
    expect(get(0, 3)).toBe('[en] Giá');
    expect(get(1, 2)).toBe('[en] Bàn làm việc');
    expect(get(0, 4)).toBe('Ghi chú cũ');
    // Originals untouched; numbers and formulas not sent.
    expect(get(0, 0)).toBe('Tên');
    expect(fake.calls.flatMap((c) => c.texts)).toEqual(['Tên', 'Giá', 'Bàn làm việc']);
  });

  it('translates in place, skipping text already in the target language', async () => {
    const { host, get } = fakeHost({ '0:0': 'Xin chào các bạn', '1:0': 'The quick brown fox' });
    const n = await translateCells(
      host,
      0,
      { r0: 0, c0: 0, r1: 1048575, c1: 0 },
      new FakeTranslationProvider(),
      'auto',
      'en',
      'inPlace',
    );
    expect(n).toBe(1);
    expect(get(0, 0)).toBe('[en] Xin chào các bạn');
    expect(get(1, 0)).toBe('The quick brown fox');
  });

  it('leaves the sheet as it was when cancelled or failing', async () => {
    const { host, calls } = fakeHost({ '0:0': 'Xin chào các bạn' });
    const fake = new FakeTranslationProvider();
    const ac = new AbortController();
    fake.gate = new Promise(() => {});
    const p = translateCells(
      host,
      0,
      { r0: 0, c0: 0, r1: 0, c1: 0 },
      fake,
      'vi',
      'en',
      'newColumn',
      {
        signal: ac.signal,
      },
    );
    ac.abort();
    await expect(p).rejects.toBeInstanceOf(TranslationError);
    fake.gate = null;
    fake.failWith = 'upstream';
    await expect(
      translateCells(host, 0, { r0: 0, c0: 0, r1: 0, c1: 0 }, fake, 'vi', 'en', 'inPlace'),
    ).rejects.toMatchObject({ code: 'upstream' });
    expect(calls).toEqual([]);
  });
});
