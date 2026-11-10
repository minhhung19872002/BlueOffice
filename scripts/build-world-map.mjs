#!/usr/bin/env node
// Builds the bundled world boundary set of map charts (CORE-CHART-006) from Natural Earth
// (public domain, https://www.naturalearthdata.com/about/terms-of-use/).
//
//   node scripts/build-world-map.mjs <ne_50m_admin_0_countries.zip> [out-dir]
//
// Input: the official Natural Earth download
// https://naciscdn.org/naturalearth/50m/cultural/ne_50m_admin_0_countries.zip (shapefile, UTF-8 DBF).
// Output: `<out-dir>/countries.bin` (default `assets/maps/world`), read by
// `crates/bo-sheet/src/geo/world.rs`:
//
//   "BOGEO1\0\0"                       magic (8 bytes)
//   u32 LE                             feature count
//   per feature:
//     u8                               string count, then each: u16 LE byte length + UTF-8
//                                      [ISO 3166-1 alpha-2, alpha-3, English name, Vietnamese name,
//                                       other names (long, sovereign, formal) …]
//     u16 LE                           ring count
//     per ring: u32 LE point count, then per point zig-zag LEB128 varints of the deltas of
//               round(lon × 100), round(lat × 100) from the previous point (first point: from 0, 0)
//
// Derivation: the polygons' rings (outer and holes, file order) are simplified with
// Douglas–Peucker at TOLERANCE degrees and quantised to 0.01°; rings left with fewer than 4 points
// or smaller than MIN_AREA square degrees are dropped, except that every country keeps its largest
// ring. Codes are the `ISO_A2_EH` / `ISO_A3_EH` fields (`-99` → empty), names `NAME_EN`, `NAME_VI`,
// `NAME`, `NAME_LONG`, `ADMIN`, `FORMAL_EN` (duplicates removed). Nothing in the input is executed.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const input = process.argv[2];
if (!input) {
  console.error('usage: node scripts/build-world-map.mjs <ne_50m_admin_0_countries.zip> [out-dir]');
  process.exit(2);
}
const outDir = resolve(process.argv[3] ?? join(root, 'assets', 'maps', 'world'));

const TOLERANCE = 0.04;
const MIN_AREA = 0.02;

// ------------------------------------------------------------------ zip (PKWARE APPNOTE §4.3)
function unzip(buf) {
  let eocd = -1;
  for (let i = buf.length - 22; i >= Math.max(0, buf.length - 65_557); i--)
    if (buf.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  if (eocd < 0) throw new Error('not a zip file');
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  const files = new Map();
  for (let k = 0; k < count; k++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new Error('bad central directory');
    const method = buf.readUInt16LE(p + 10);
    const csize = buf.readUInt32LE(p + 20);
    const nlen = buf.readUInt16LE(p + 28);
    const xlen = buf.readUInt16LE(p + 30);
    const clen = buf.readUInt16LE(p + 32);
    const local = buf.readUInt32LE(p + 42);
    const name = buf.subarray(p + 46, p + 46 + nlen).toString('utf8');
    const lnlen = buf.readUInt16LE(local + 26);
    const lxlen = buf.readUInt16LE(local + 28);
    const start = local + 30 + lnlen + lxlen;
    const raw = buf.subarray(start, start + csize);
    files.set(name, method === 8 ? inflateRawSync(raw) : Buffer.from(raw));
    p += 46 + nlen + xlen + clen;
  }
  return files;
}

// ------------------------------------------------------------------ dBASE table
function readDbf(buf) {
  const n = buf.readUInt32LE(4);
  const headerLen = buf.readUInt16LE(8);
  const recLen = buf.readUInt16LE(10);
  const fields = [];
  for (let p = 32; buf[p] !== 0x0d && p < headerLen; p += 32) {
    const name = buf
      .subarray(p, p + 11)
      .toString('latin1')
      .replace(/\0.*$/, '');
    fields.push({ name, len: buf[p + 16] });
  }
  const rows = [];
  for (let i = 0; i < n; i++) {
    let p = headerLen + i * recLen + 1;
    const row = {};
    for (const f of fields) {
      row[f.name] = buf
        .subarray(p, p + f.len)
        .toString('utf8')
        .replace(/\0+$/, '')
        .trim();
      p += f.len;
    }
    rows.push(row);
  }
  return rows;
}

// ------------------------------------------------------------------ shapefile polygons
function readShp(buf) {
  const shapes = [];
  let p = 100;
  while (p + 8 <= buf.length) {
    const words = buf.readInt32BE(p + 4);
    const c = p + 8;
    const type = buf.readInt32LE(c);
    const rings = [];
    if (type === 5) {
      const parts = buf.readInt32LE(c + 36);
      const points = buf.readInt32LE(c + 40);
      const starts = [];
      for (let k = 0; k < parts; k++) starts.push(buf.readInt32LE(c + 44 + 4 * k));
      const base = c + 44 + 4 * parts;
      for (let k = 0; k < parts; k++) {
        const end = k + 1 < parts ? starts[k + 1] : points;
        const ring = [];
        for (let j = starts[k]; j < end; j++)
          ring.push([buf.readDoubleLE(base + 16 * j), buf.readDoubleLE(base + 16 * j + 8)]);
        rings.push(ring);
      }
    }
    shapes.push(rings);
    p = c + words * 2;
  }
  return shapes;
}

// ------------------------------------------------------------------ geometry
function simplify(pts, tol) {
  if (pts.length <= 4) return pts;
  const keep = new Uint8Array(pts.length);
  keep[0] = keep[pts.length - 1] = 1;
  const stack = [[0, pts.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop();
    const [ax, ay] = pts[a];
    const [bx, by] = pts[b];
    const dx = bx - ax;
    const dy = by - ay;
    const len = Math.hypot(dx, dy);
    let best = -1;
    let dmax = 0;
    for (let i = a + 1; i < b; i++) {
      const [px, py] = pts[i];
      const d =
        len === 0
          ? Math.hypot(px - ax, py - ay)
          : Math.abs(dy * px - dx * py + bx * ay - by * ax) / len;
      if (d > dmax) {
        dmax = d;
        best = i;
      }
    }
    if (best >= 0 && dmax > tol) {
      keep[best] = 1;
      stack.push([a, best], [best, b]);
    }
  }
  return pts.filter((_, i) => keep[i]);
}

const area = (ring) => {
  let s = 0;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++)
    s += (ring[j][0] - ring[i][0]) * (ring[j][1] + ring[i][1]);
  return Math.abs(s / 2);
};

// ------------------------------------------------------------------ encoding
const bytes = [];
const u8 = (v) => bytes.push(v & 0xff);
const u16 = (v) => {
  u8(v);
  u8(v >> 8);
};
const u32 = (v) => {
  u16(v & 0xffff);
  u16(v >>> 16);
};
const varint = (v) => {
  let z = v >= 0 ? v * 2 : -v * 2 - 1;
  while (z >= 0x80) {
    u8((z & 0x7f) | 0x80);
    z = Math.floor(z / 128);
  }
  u8(z);
};
const str = (s) => {
  const b = Buffer.from(s, 'utf8');
  u16(b.length);
  for (const x of b) u8(x);
};

const files = unzip(readFileSync(resolve(input)));
const find = (ext) => [...files.keys()].find((k) => k.toLowerCase().endsWith(ext));
const shpName = find('.shp');
const dbfName = find('.dbf');
if (!shpName || !dbfName) throw new Error('the zip holds no .shp / .dbf');
const version = (files.get(find('.version.txt')) ?? Buffer.from('?')).toString('utf8').trim();
const rows = readDbf(files.get(dbfName));
const shapes = readShp(files.get(shpName));
if (rows.length !== shapes.length) throw new Error('.shp and .dbf record counts differ');

for (const b of Buffer.from('BOGEO1\0\0', 'latin1')) u8(b);
u32(rows.length);
let points = 0;
for (let i = 0; i < rows.length; i++) {
  const r = rows[i];
  const code = (v) => (!v || v === '-99' ? '' : v);
  const names = [];
  for (const f of ['NAME_EN', 'NAME_VI', 'NAME', 'NAME_LONG', 'ADMIN', 'FORMAL_EN']) {
    const v = r[f];
    if (v && !names.includes(v)) names.push(v);
  }
  const strings = [code(r.ISO_A2_EH ?? r.ISO_A2), code(r.ISO_A3_EH ?? r.ISO_A3), ...names].slice(
    0,
    255,
  );
  u8(strings.length);
  strings.forEach(str);
  let rings = shapes[i]
    .map((ring) =>
      simplify(ring, TOLERANCE).map(([x, y]) => [Math.round(x * 100), Math.round(y * 100)]),
    )
    .map((ring) =>
      ring.filter((p, k) => k === 0 || p[0] !== ring[k - 1][0] || p[1] !== ring[k - 1][1]),
    );
  const largest = rings.reduce((m, ring, k) => (area(ring) > area(rings[m] ?? []) ? k : m), 0);
  rings = rings.filter(
    (ring, k) => k === largest || (ring.length >= 4 && area(ring) / 10_000 >= MIN_AREA),
  );
  u16(rings.length);
  for (const ring of rings) {
    u32(ring.length);
    let px = 0;
    let py = 0;
    for (const [x, y] of ring) {
      varint(x - px);
      varint(y - py);
      px = x;
      py = y;
    }
    points += ring.length;
  }
}
mkdirSync(outDir, { recursive: true });
writeFileSync(join(outDir, 'countries.bin'), Buffer.from(bytes));
console.log(
  `Natural Earth ${version}: ${rows.length} countries, ${points} points, ${bytes.length} bytes → ${join(outDir, 'countries.bin')}`,
);
