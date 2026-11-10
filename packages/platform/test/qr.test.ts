import { describe, expect, it } from 'vitest';
import {
  encodeQr,
  formatBits,
  maskBit,
  qrLayout,
  qrPath,
  qrVersionFor,
  rsRemainder,
  versionBits,
} from '../src/qr';

// Format information strings after masking, from ISO/IEC 18004:2015 Annex C (Table C.1).
const FORMAT_M = [
  '101010000010010',
  '101000100100101',
  '101111001111100',
  '101101101001011',
  '100010111111001',
  '100000011001110',
  '100111110010111',
  '100101010100000',
];

/** Level M blocks of versions 1–10 as [count, data codewords] (Table 9) and EC per block. */
const BLOCKS: Record<number, { ec: number; blocks: [number, number][] }> = {
  1: { ec: 10, blocks: [[1, 16]] },
  2: { ec: 16, blocks: [[1, 28]] },
  3: { ec: 26, blocks: [[1, 44]] },
  4: { ec: 18, blocks: [[2, 32]] },
  5: { ec: 24, blocks: [[2, 43]] },
  6: { ec: 16, blocks: [[4, 27]] },
  7: { ec: 18, blocks: [[4, 31]] },
  8: {
    ec: 22,
    blocks: [
      [2, 38],
      [2, 39],
    ],
  },
  9: {
    ec: 22,
    blocks: [
      [3, 36],
      [2, 37],
    ],
  },
  10: {
    ec: 26,
    blocks: [
      [4, 43],
      [1, 44],
    ],
  },
};

/**
 * A small reader for the tests: function modules worked out from the symbol size and the
 * alignment centres, format bits read from the upper left copy, then the codewords collected
 * along the placement path, unmasked, de-interleaved and parsed as one byte-mode segment.
 */
function read(m: boolean[][], align: number[]): { mask: number; text: string; ec: number[][] } {
  const n = m.length;
  const version = (n - 17) / 4;
  const fn = Array.from({ length: n }, () => new Array<boolean>(n).fill(false));
  const mark = (r0: number, c0: number, h: number, w: number) => {
    for (let r = r0; r < r0 + h; r++)
      for (let c = c0; c < c0 + w; c++) if (r >= 0 && c >= 0 && r < n && c < n) fn[r]![c] = true;
  };
  mark(0, 0, 9, 9);
  mark(0, n - 8, 9, 8);
  mark(n - 8, 0, 8, 9);
  mark(6, 0, 1, n);
  mark(0, 6, n, 1);
  // Alignment patterns except those over a finder pattern.
  const inFinder = (r: number, c: number) =>
    (r < 9 && (c < 9 || c >= n - 8)) || (r >= n - 8 && c < 9);
  for (const r of align) for (const c of align) if (!inFinder(r, c)) mark(r - 2, c - 2, 5, 5);
  if (version >= 7) {
    mark(0, n - 11, 6, 3);
    mark(n - 11, 0, 3, 6);
  }
  // Format bits 14..9 along row 8, then 8, 7 and 6..0 up column 8 (Figure 25).
  const cells: [number, number][] = [
    [8, 0],
    [8, 1],
    [8, 2],
    [8, 3],
    [8, 4],
    [8, 5],
    [8, 7],
    [8, 8],
    [7, 8],
    [5, 8],
    [4, 8],
    [3, 8],
    [2, 8],
    [1, 8],
    [0, 8],
  ];
  const format = cells.map(([r, c]) => (m[r]![c] ? '1' : '0')).join('');
  const mask = FORMAT_M.indexOf(format);
  if (mask < 0) throw new Error(`format ${format} is not level M`);
  const bits: number[] = [];
  let up = true;
  for (let right = n - 1; right > 0; right -= 2) {
    if (right === 6) right--;
    for (let k = 0; k < n; k++) {
      const r = up ? n - 1 - k : k;
      for (const c of [right, right - 1]) {
        if (fn[r]![c]) continue;
        bits.push((m[r]![c] ? 1 : 0) ^ (maskBit(mask, r, c) ? 1 : 0));
      }
    }
    up = !up;
  }
  const spec = BLOCKS[version]!;
  const sizes = spec.blocks.flatMap(([count, k]) => new Array<number>(count).fill(k));
  const total = sizes.reduce((s, k) => s + k, 0) + sizes.length * spec.ec;
  const words: number[] = [];
  for (let i = 0; i < total; i++) {
    let b = 0;
    for (let j = 0; j < 8; j++) b = (b << 1) | bits[i * 8 + j]!;
    words.push(b);
  }
  const data: number[][] = sizes.map(() => []);
  let at = 0;
  for (let i = 0; i < Math.max(...sizes); i++)
    sizes.forEach((k, b) => {
      if (i < k) data[b]!.push(words[at++]!);
    });
  const ec: number[][] = sizes.map(() => []);
  for (let i = 0; i < spec.ec; i++) sizes.forEach((_, b) => ec[b]!.push(words[at++]!));
  const flat = data.flat();
  const bitAt = (i: number) => (flat[i >> 3]! >> (7 - (i & 7))) & 1;
  const take = (from: number, len: number) => {
    let v = 0;
    for (let i = 0; i < len; i++) v = (v << 1) | bitAt(from + i);
    return v;
  };
  expect(take(0, 4)).toBe(0b0100);
  const countLen = version <= 9 ? 8 : 16;
  const len = take(4, countLen);
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) bytes[i] = take(4 + countLen + i * 8, 8);
  // Each block's error correction is the remainder of its data.
  data.forEach((d, b) => expect(ec[b]).toEqual(rsRemainder(d, spec.ec)));
  return { mask, text: new TextDecoder().decode(bytes), ec };
}

describe('QR code encoder (ISO/IEC 18004)', () => {
  it('computes Reed–Solomon codewords of a known block', () => {
    // "HELLO WORLD" as 1-M (alphanumeric) data codewords and their published EC codewords.
    const data = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17];
    expect(rsRemainder(data, 10)).toEqual([196, 35, 39, 119, 235, 215, 231, 226, 93, 23]);
  });

  it('produces the format and version information of the standard', () => {
    FORMAT_M.forEach((s, mask) =>
      expect(formatBits('M', mask).toString(2).padStart(15, '0')).toBe(s),
    );
    expect(formatBits('L', 0).toString(2).padStart(15, '0')).toBe('111011111000100');
    expect(formatBits('H', 0).toString(2).padStart(15, '0')).toBe('001011010001001');
    // Annex D, Table D.1: version 7 → 000111110010010100.
    expect(versionBits(7).toString(2).padStart(18, '0')).toBe('000111110010010100');
    expect(versionBits(10).toString(2).padStart(18, '0')).toBe('001010010011010011');
  });

  it('leaves exactly the room of the codewords plus the remainder bits', () => {
    // Remainder bits per version (Table 1): 0, 7 × 5, 0 × 4.
    const remainder = [0, 7, 7, 7, 7, 7, 0, 0, 0, 0];
    const raw = [26, 44, 70, 100, 134, 172, 196, 242, 292, 346];
    for (let v = 1; v <= 10; v++) {
      const l = qrLayout(v);
      expect(l.codewords).toBe(raw[v - 1]);
      expect(l.free).toBe(l.codewords * 8 + remainder[v - 1]!);
    }
  });

  it('picks the smallest version for the data', () => {
    // Byte capacity at level M (Table 7): 14, 26, 42, 62, 84, 106, 122, 152, 180, 213.
    const cap = [14, 26, 42, 62, 84, 106, 122, 152, 180, 213];
    cap.forEach((c, i) => {
      expect(qrVersionFor(c)).toBe(i + 1);
      if (i < 9) expect(qrVersionFor(c + 1)).toBe(i + 2);
    });
    expect(qrVersionFor(214)).toBeNull();
    expect(() => encodeQr('x'.repeat(214))).toThrow('qr.tooLong');
  });

  it('draws finder, timing and dark modules', () => {
    const q = encodeQr('https://office.example.vn/remote/abc');
    const n = q.size;
    expect(n).toBe(17 + 4 * q.version);
    const finderAt = (r: number, c: number) => {
      for (let i = 0; i < 7; i++) {
        expect(q.modules[r]![c + i]).toBe(true);
        expect(q.modules[r + 6]![c + i]).toBe(true);
        expect(q.modules[r + i]![c]).toBe(true);
        expect(q.modules[r + i]![c + 6]).toBe(true);
      }
      expect(q.modules[r + 1]![c + 1]).toBe(false);
      expect(q.modules[r + 3]![c + 3]).toBe(true);
    };
    finderAt(0, 0);
    finderAt(0, n - 7);
    finderAt(n - 7, 0);
    for (let i = 8; i < n - 8; i++) {
      expect(q.modules[6]![i]).toBe(i % 2 === 0);
      expect(q.modules[i]![6]).toBe(i % 2 === 0);
    }
    expect(q.modules[n - 8]![8]).toBe(true);
  });

  it('round-trips URLs through every version, mask and both format copies', () => {
    const align: Record<number, number[]> = {
      1: [],
      2: [6, 18],
      3: [6, 22],
      4: [6, 26],
      5: [6, 30],
      6: [6, 34],
      7: [6, 22, 38],
      8: [6, 24, 42],
      9: [6, 26, 46],
      10: [6, 28, 50],
    };
    const base = 'http://192.168.1.20:8081/remote/';
    for (let v = 1; v <= 10; v++) {
      const len = [14, 26, 42, 62, 84, 106, 122, 152, 180, 213][v - 1]!;
      const text = (base + 'Ab0_-'.repeat(60)).slice(0, len);
      const q = encodeQr(text);
      expect(q.version).toBe(v);
      const r = read(q.modules, align[v]!);
      expect(r.text).toBe(text);
      expect(r.mask).toBe(q.mask);
      // The second copy of the format information matches the first.
      const n = q.size;
      const second = [
        ...Array.from({ length: 7 }, (_, i) => q.modules[n - 1 - i]![8]),
        ...Array.from({ length: 8 }, (_, i) => q.modules[8]![n - 8 + i]),
      ]
        .map((d) => (d ? '1' : '0'))
        .join('');
      expect(second).toBe(FORMAT_M[q.mask]);
    }
    for (let mask = 0; mask < 8; mask++) {
      const q = encodeQr('Trình chiếu — điều khiển từ điện thoại', { mask });
      expect(read(q.modules, align[q.version]!)).toMatchObject({
        mask,
        text: 'Trình chiếu — điều khiển từ điện thoại',
      });
    }
  });

  it('renders an SVG path with a quiet zone', () => {
    const q = encodeQr('a');
    const d = qrPath(q, 4);
    expect(d.startsWith('M4 4h1v1h-1z')).toBe(true);
    expect(d.match(/M/g)?.length).toBe(q.modules.flat().filter(Boolean).length);
  });
});
