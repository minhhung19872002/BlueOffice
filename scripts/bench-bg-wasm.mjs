// Time of the picture background removal (CORE-PIC-002) in the WASM release build the browser
// loads: a 512 × 512 picture (a red disk on a blue → green gradient) inserted on a slide, then
// `pictureBackgroundPreview` (the segmentation at the working size) and `pictureRemoveBackground`
// (segmentation + full-size mask + PNG), each run `RUNS` times. Budget: < 1.5 s at 512 px.
//
// Usage: pnpm build:wasm && node scripts/bench-bg-wasm.mjs [side]
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { deflateSync } from 'node:zlib';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = process.env.BO_WASM_PKG ?? join(root, 'packages', 'engine-wasm', 'pkg');
const mod = await import(pathToFileURL(join(pkg, 'bo_wasm.js')).href);
mod.initSync({ module: readFileSync(join(pkg, 'bo_wasm_bg.wasm')) });
const side = Number(process.argv[2] ?? 512);
const RUNS = 3;

const CRC = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (b) => {
  let c = 0xffffffff;
  for (const x of b) c = CRC[(c ^ x) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
function png(w, h) {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 2, 0, 0, 0], 8);
  const r = Math.min(w, h) / 3;
  const rows = [];
  for (let y = 0; y < h; y++) {
    const row = Buffer.alloc(1 + w * 3);
    for (let x = 0; x < w; x++) {
      const inside = (x + 0.5 - w / 2) ** 2 + (y + 0.5 - h / 2) ** 2 <= r * r;
      const t = x / w;
      const n = (x * 7 + y * 13) % 5;
      row.set(
        inside
          ? [210 + n, 45 + n, 40]
          : [25 + 15 * t + n, 70 + 120 * t + n, 200 - 130 * t].map(Math.floor),
        1 + x * 3,
      );
    }
    rows.push(row);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const deck = readFileSync(join(root, 'tests', 'corpus', 'slide', 'slide-shapes-groups.pptx'));
const engine = new mod.WasmEngine();
engine.open(deck, 'd.pptx');
engine.slideInsertPicture(png(side, side), 'disk.png');
const time = (cmd) => {
  const out = [];
  let last;
  for (let i = 0; i < RUNS; i++) {
    const t = performance.now();
    last = JSON.parse(engine.slideCommand(JSON.stringify(cmd)));
    out.push(performance.now() - t);
  }
  if (last.error) throw new Error(last.error);
  return { ms: out.map((m) => Math.round(m)), last };
};
const marks = [{ keep: true, points: [[0.5, 0.5]] }];
const preview = time({ cmd: 'pictureBackgroundPreview', marks });
const kept = preview.last.picture.background.kept / 1000;
const removeRuns = [];
for (let i = 0; i < RUNS; i++) {
  const t = performance.now();
  const r = JSON.parse(
    engine.slideCommand(JSON.stringify({ cmd: 'pictureRemoveBackground', marks })),
  );
  removeRuns.push(Math.round(performance.now() - t));
  if (r.error) throw new Error(r.error);
  engine.slideCommand(JSON.stringify({ cmd: 'undo' }));
}
console.log(
  JSON.stringify(
    { side, previewMs: preview.ms, removeMs: removeRuns, keptPercent: kept, budgetMs: 1500 },
    null,
    1,
  ),
);
