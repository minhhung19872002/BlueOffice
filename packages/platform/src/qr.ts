/**
 * QR code encoder (ISO/IEC 18004:2015), written for the phone pairing link of the slide show
 * (SLIDE-REMOTE-001): byte mode (UTF-8), error correction level M, versions 1–10 (up to 213
 * bytes — a pairing URL is well under 100). No third-party code.
 *
 * Pipeline (clause numbers of ISO/IEC 18004:2015):
 * 1. data bits: mode indicator, character count, bytes, terminator, pad codewords (§7.4);
 * 2. Reed–Solomon error correction per block over GF(2⁸) with the polynomial
 *    x⁸+x⁴+x³+x²+1, blocks interleaved (§7.5, §7.6);
 * 3. function patterns: finders, separators, timing, alignment, dark module (§6.3);
 * 4. codewords placed two columns at a time from the lower right (§7.7.3);
 * 5. the mask with the lowest penalty (§7.8.3), format and version information (§7.9, §7.10).
 */

/** Error correction levels and their format information bits (§7.9.1, Table 12). */
export type QrLevel = 'L' | 'M' | 'Q' | 'H';
const LEVEL_BITS: Record<QrLevel, number> = { L: 0b01, M: 0b00, Q: 0b11, H: 0b10 };

/**
 * Level M error correction of versions 1–10 (§7.5.1, Table 9): EC codewords per block and the
 * blocks as [count, data codewords each].
 */
const LEVEL_M: readonly { ec: number; blocks: readonly (readonly [number, number])[] }[] = [
  { ec: 10, blocks: [[1, 16]] },
  { ec: 16, blocks: [[1, 28]] },
  { ec: 26, blocks: [[1, 44]] },
  { ec: 18, blocks: [[2, 32]] },
  { ec: 24, blocks: [[2, 43]] },
  { ec: 16, blocks: [[4, 27]] },
  { ec: 18, blocks: [[4, 31]] },
  {
    ec: 22,
    blocks: [
      [2, 38],
      [2, 39],
    ],
  },
  {
    ec: 22,
    blocks: [
      [3, 36],
      [2, 37],
    ],
  },
  {
    ec: 26,
    blocks: [
      [4, 43],
      [1, 44],
    ],
  },
];

/** Centre rows / columns of the alignment patterns of versions 2–10 (Annex E, Table E.1). */
const ALIGNMENT: readonly (readonly number[])[] = [
  [],
  [6, 18],
  [6, 22],
  [6, 26],
  [6, 30],
  [6, 34],
  [6, 22, 38],
  [6, 24, 42],
  [6, 26, 46],
  [6, 28, 50],
];

export const QR_MAX_VERSION = LEVEL_M.length;

// ------------------------------------------------------------------------------ GF(2⁸)

const EXP = new Uint8Array(512);
const LOG = new Uint8Array(256);
{
  let x = 1;
  for (let i = 0; i < 255; i++) {
    EXP[i] = x;
    LOG[x] = i;
    x <<= 1;
    // §7.5.2: the field's primitive polynomial x⁸+x⁴+x³+x²+1 (0x11D).
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) EXP[i] = EXP[i - 255]!;
}

function mul(a: number, b: number): number {
  return a === 0 || b === 0 ? 0 : EXP[LOG[a]! + LOG[b]!]!;
}

/** Coefficients (highest degree first, leading 1 left out) of Π (x − αⁱ), i < `degree`. */
function generator(degree: number): number[] {
  let g = [1];
  for (let i = 0; i < degree; i++) {
    const next = new Array<number>(g.length + 1).fill(0);
    for (let j = 0; j < g.length; j++) {
      next[j] = next[j]! ^ g[j]!;
      next[j + 1] = next[j + 1]! ^ mul(g[j]!, EXP[i]!);
    }
    g = next;
  }
  return g.slice(1);
}

/** Error correction codewords of `data`: the remainder of data(x)·xⁿ divided by the generator. */
export function rsRemainder(data: readonly number[], ecLen: number): number[] {
  const g = generator(ecLen);
  const r = new Array<number>(ecLen).fill(0);
  for (const d of data) {
    const factor = d ^ r[0]!;
    r.shift();
    r.push(0);
    for (let i = 0; i < ecLen; i++) r[i] = r[i]! ^ mul(g[i]!, factor);
  }
  return r;
}

// ------------------------------------------------------------------------------ BCH codes

/** Remainder of `value`·2^`bits` divided by `poly` (degree `bits`). */
function bch(value: number, poly: number, bits: number): number {
  let r = value << bits;
  for (let i = 31 - Math.clz32(r); i >= bits; i--) {
    if ((r >> i) & 1) r ^= poly << (i - bits);
  }
  return r;
}

/** 15 format information bits of `level` and `mask` (§7.9.1: BCH(15,5), XOR 101010000010010). */
export function formatBits(level: QrLevel, mask: number): number {
  const data = (LEVEL_BITS[level] << 3) | mask;
  return ((data << 10) | bch(data, 0b10100110111, 10)) ^ 0b101010000010010;
}

/** 18 version information bits (§7.10: BCH(18,6) with the generator 1111100100101). */
export function versionBits(version: number): number {
  return (version << 12) | bch(version, 0b1111100100101, 12);
}

// ------------------------------------------------------------------------------ data

/** Data codewords of level M `version`. */
function dataCodewords(version: number): number {
  return LEVEL_M[version - 1]!.blocks.reduce((s, [n, k]) => s + n * k, 0);
}

/** Bits of the character count indicator in byte mode (§7.4.1, Table 3). */
function countBits(version: number): number {
  return version <= 9 ? 8 : 16;
}

/** Smallest version that holds `bytes` bytes, or `null`. */
export function qrVersionFor(bytes: number): number | null {
  for (let v = 1; v <= QR_MAX_VERSION; v++) {
    if (4 + countBits(v) + 8 * bytes <= dataCodewords(v) * 8) return v;
  }
  return null;
}

/** Data codewords: byte mode segment, terminator, padding (§7.4.9, §7.4.10). */
function encodeData(bytes: Uint8Array, version: number): number[] {
  const capacity = dataCodewords(version) * 8;
  const bits: number[] = [];
  const put = (value: number, n: number) => {
    for (let i = n - 1; i >= 0; i--) bits.push((value >>> i) & 1);
  };
  // Mode indicator 0100: 8-bit byte (§7.4.2, Table 2).
  put(0b0100, 4);
  put(bytes.length, countBits(version));
  for (const b of bytes) put(b, 8);
  put(0, Math.min(4, capacity - bits.length));
  while (bits.length % 8) bits.push(0);
  const out: number[] = [];
  for (let i = 0; i < bits.length; i += 8) {
    let b = 0;
    for (let j = 0; j < 8; j++) b = (b << 1) | bits[i + j]!;
    out.push(b);
  }
  // Pad codewords 11101100 and 00010001 alternately.
  for (let pad = 0xec; out.length < capacity / 8; pad ^= 0xec ^ 0x11) out.push(pad);
  return out;
}

/** Blocks with their error correction, interleaved (§7.6). */
function interleave(data: number[], version: number): number[] {
  const spec = LEVEL_M[version - 1]!;
  const blocks: number[][] = [];
  let at = 0;
  for (const [n, k] of spec.blocks) {
    for (let i = 0; i < n; i++) {
      blocks.push(data.slice(at, at + k));
      at += k;
    }
  }
  const ec = blocks.map((b) => rsRemainder(b, spec.ec));
  const out: number[] = [];
  const longest = Math.max(...blocks.map((b) => b.length));
  for (let i = 0; i < longest; i++) for (const b of blocks) if (i < b.length) out.push(b[i]!);
  for (let i = 0; i < spec.ec; i++) for (const e of ec) out.push(e[i]!);
  return out;
}

// ------------------------------------------------------------------------------ matrix

/** A QR symbol: `size` × `size` modules, `true` = dark. */
export interface QrCode {
  version: number;
  size: number;
  mask: number;
  modules: boolean[][];
}

class Grid {
  readonly dark: boolean[][];
  readonly fixed: boolean[][];
  constructor(readonly size: number) {
    this.dark = Array.from({ length: size }, () => new Array<boolean>(size).fill(false));
    this.fixed = Array.from({ length: size }, () => new Array<boolean>(size).fill(false));
  }
  set(row: number, col: number, dark: boolean): void {
    if (row < 0 || col < 0 || row >= this.size || col >= this.size) return;
    this.dark[row]![col] = dark;
    this.fixed[row]![col] = true;
  }
}

/** Finder pattern with its separator centred on (row, col) (§6.3.3, §6.3.4). */
function finder(g: Grid, row: number, col: number): void {
  for (let dr = -4; dr <= 4; dr++) {
    for (let dc = -4; dc <= 4; dc++) {
      const d = Math.max(Math.abs(dr), Math.abs(dc));
      // 3×3 dark centre, light ring, dark ring, light separator.
      g.set(row + dr, col + dc, d !== 2 && d !== 4);
    }
  }
}

function functionPatterns(g: Grid, version: number): void {
  const n = g.size;
  // Timing patterns (§6.3.5).
  for (let i = 0; i < n; i++) {
    g.set(6, i, i % 2 === 0);
    g.set(i, 6, i % 2 === 0);
  }
  finder(g, 3, 3);
  finder(g, 3, n - 4);
  finder(g, n - 4, 3);
  // Alignment patterns (§6.3.6), except where they would overlap a finder.
  const pos = ALIGNMENT[version - 1]!;
  const last = pos.length - 1;
  pos.forEach((r, i) =>
    pos.forEach((c, j) => {
      if ((i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0)) return;
      for (let dr = -2; dr <= 2; dr++) {
        for (let dc = -2; dc <= 2; dc++) {
          g.set(r + dr, c + dc, Math.max(Math.abs(dr), Math.abs(dc)) !== 1);
        }
      }
    }),
  );
  // Format information areas reserved now, written after masking; dark module (§7.9.1).
  placeFormat(g, 0);
  if (version >= 7) placeVersion(g, version);
}

/** Format information, both copies (§7.9.1, Figure 25); bit 0 is the least significant. */
function placeFormat(g: Grid, bits: number): void {
  const n = g.size;
  const bit = (i: number) => ((bits >> i) & 1) === 1;
  for (let i = 0; i <= 5; i++) g.set(i, 8, bit(i));
  g.set(7, 8, bit(6));
  g.set(8, 8, bit(7));
  g.set(8, 7, bit(8));
  for (let i = 9; i < 15; i++) g.set(8, 14 - i, bit(i));
  for (let i = 0; i < 8; i++) g.set(8, n - 1 - i, bit(i));
  for (let i = 8; i < 15; i++) g.set(n - 15 + i, 8, bit(i));
  // The dark module beside the lower left finder.
  g.set(n - 8, 8, true);
}

/** Version information, both copies (§7.10, Figure 27). */
function placeVersion(g: Grid, version: number): void {
  const bits = versionBits(version);
  for (let i = 0; i < 18; i++) {
    const dark = ((bits >> i) & 1) === 1;
    const a = g.size - 11 + (i % 3);
    const b = Math.floor(i / 3);
    g.set(b, a, dark);
    g.set(a, b, dark);
  }
}

/** Codeword bits into the free modules, upwards and downwards in two-column strips (§7.7.3). */
function placeData(g: Grid, codewords: number[]): void {
  const n = g.size;
  let i = 0;
  const total = codewords.length * 8;
  for (let right = n - 1; right >= 1; right -= 2) {
    // The vertical timing pattern's column is skipped.
    if (right === 6) right = 5;
    const upward = ((n - 1 - right) >> 1) % 2 === 0;
    for (let k = 0; k < n; k++) {
      const row = upward ? n - 1 - k : k;
      for (let c = 0; c < 2; c++) {
        const col = right - c;
        if (g.fixed[row]![col]) continue;
        // Remainder bits are light (§7.6).
        g.dark[row]![col] = i < total && ((codewords[i >> 3]! >> (7 - (i & 7))) & 1) === 1;
        i++;
      }
    }
  }
}

/** Data mask condition (§7.8.2, Table 10): `true` where the module is inverted. */
export function maskBit(mask: number, row: number, col: number): boolean {
  switch (mask) {
    case 0:
      return (row + col) % 2 === 0;
    case 1:
      return row % 2 === 0;
    case 2:
      return col % 3 === 0;
    case 3:
      return (row + col) % 3 === 0;
    case 4:
      return (Math.floor(row / 2) + Math.floor(col / 3)) % 2 === 0;
    case 5:
      return ((row * col) % 2) + ((row * col) % 3) === 0;
    case 6:
      return (((row * col) % 2) + ((row * col) % 3)) % 2 === 0;
    default:
      return (((row + col) % 2) + ((row * col) % 3)) % 2 === 0;
  }
}

function applyMask(g: Grid, mask: number): void {
  for (let r = 0; r < g.size; r++) {
    for (let c = 0; c < g.size; c++) {
      if (!g.fixed[r]![c] && maskBit(mask, r, c)) g.dark[r]![c] = !g.dark[r]![c];
    }
  }
}

/** Penalty of a masked symbol (§7.8.3.1, Table 11). */
export function penalty(m: boolean[][]): number {
  const n = m.length;
  let score = 0;
  const line = (get: (i: number) => boolean) => {
    // N1: runs of five or more modules of one colour.
    let run = 1;
    for (let i = 1; i <= n; i++) {
      if (i < n && get(i) === get(i - 1)) run++;
      else {
        if (run >= 5) score += 3 + (run - 5);
        run = 1;
      }
    }
    // N3: 1:1:3:1:1 finder-like runs with four light modules on one side.
    const pattern = [true, false, true, true, true, false, true];
    for (let i = 0; i + 7 <= n; i++) {
      if (!pattern.every((p, k) => get(i + k) === p)) continue;
      const lightBefore = i >= 4 && [1, 2, 3, 4].every((k) => !get(i - k));
      const lightAfter = i + 11 <= n && [7, 8, 9, 10].every((k) => !get(i + k));
      if (lightBefore || lightAfter) score += 40;
    }
  };
  for (let r = 0; r < n; r++) line((i) => m[r]![i]!);
  for (let c = 0; c < n; c++) line((i) => m[i]![c]!);
  // N2: 2×2 blocks of one colour.
  for (let r = 0; r + 1 < n; r++) {
    for (let c = 0; c + 1 < n; c++) {
      const v = m[r]![c];
      if (m[r]![c + 1] === v && m[r + 1]![c] === v && m[r + 1]![c + 1] === v) score += 3;
    }
  }
  // N4: proportion of dark modules away from 50 %, per 5 %.
  const dark = m.reduce((s, row) => s + row.filter(Boolean).length, 0);
  score += Math.floor(Math.abs((dark * 100) / (n * n) - 50) / 5) * 10;
  return score;
}

/** Modules left for codewords in `version` and the codewords (data + error correction) placed there. */
export function qrLayout(version: number): { free: number; codewords: number } {
  const g = new Grid(17 + 4 * version);
  functionPatterns(g, version);
  const free = g.fixed.reduce((s, row) => s + row.filter((f) => !f).length, 0);
  const spec = LEVEL_M[version - 1]!;
  const blocks = spec.blocks.reduce((s, [n]) => s + n, 0);
  return { free, codewords: dataCodewords(version) + blocks * spec.ec };
}

/** The QR code of `text` (UTF-8, byte mode, level M); throws `qr.tooLong` past version 10. */
export function encodeQr(text: string, opts: { mask?: number } = {}): QrCode {
  const bytes = new TextEncoder().encode(text);
  const version = qrVersionFor(bytes.length);
  if (version === null) throw new Error('qr.tooLong');
  const codewords = interleave(encodeData(bytes, version), version);
  const size = 17 + 4 * version;
  let best: { mask: number; grid: Grid; score: number } | null = null;
  for (let mask = 0; mask < 8; mask++) {
    if (opts.mask !== undefined && mask !== opts.mask) continue;
    const g = new Grid(size);
    functionPatterns(g, version);
    placeData(g, codewords);
    applyMask(g, mask);
    placeFormat(g, formatBits('M', mask));
    const score = penalty(g.dark);
    if (!best || score < best.score) best = { mask, grid: g, score };
  }
  const chosen = best!;
  return { version, size, mask: chosen.mask, modules: chosen.grid.dark };
}

/** SVG path data of the dark modules, one unit per module, offset by the quiet zone `quiet`. */
export function qrPath(code: QrCode, quiet = 4): string {
  let d = '';
  code.modules.forEach((row, r) =>
    row.forEach((dark, c) => {
      if (dark) d += `M${c + quiet} ${r + quiet}h1v1h-1z`;
    }),
  );
  return d;
}
