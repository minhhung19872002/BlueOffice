// CORE-PERF-001: no memory leak when opening and closing 100 documents in one WASM
// instance. Opens each corpus file once (warm-up), then 100 more documents of each kind
// (cycling through the corpus: BlueDoc .docx, BlueSlide .pptx through `WasmEngine`,
// BlueSheet .xlsx through `WasmSheet`),
// draws the first page / sheet of each, then closes it (a blank document / workbook
// replaces it, as the editors do) and records the size of the WebAssembly memory.
// WebAssembly memory never shrinks, so a leak shows as a memory that keeps growing;
// without one it reaches a plateau once the allocator's pool holds the largest file.
// Fails when the memory after the last of `cycles` further opens exceeds the memory
// after the warm-up pass (every file opened once) by more than `BO_LEAK_SLACK_MB` (default 8 MB).
//
// Usage:
//   pnpm build:wasm
//   node scripts/leak-wasm.mjs [cycles=100] [big.docx big.pptx …]
// Extra files (e.g. `bo-cli sample-big --pages 1000 --rich`, `bo-cli sample-big-deck`) are
// opened in one more `WasmEngine`: twice each as warm-up (the heap reserved for a big file,
// `bo-wasm/src/heap.rs`, may be taken a second time before the memory is twice its size),
// then `cycles` / 10 more opens.
import { readdirSync, readFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = process.env.BO_WASM_PKG ?? join(root, 'packages', 'engine-wasm', 'pkg');
const glue = pathToFileURL(join(pkg, 'bo_wasm.js')).href;
const wasmBytes = readFileSync(join(pkg, 'bo_wasm_bg.wasm'));
const fontsDir = join(root, 'assets', 'fonts');
const manifest = JSON.parse(readFileSync(join(fontsDir, 'manifest.json'), 'utf8'));
const fonts = manifest.fonts.map((f) => readFileSync(join(fontsDir, f)));
const MB = 1024 * 1024;
const cycles = Number(process.argv[2] ?? 100);
const slack = Number(process.env.BO_LEAK_SLACK_MB ?? 8);

function corpus(dir, ext) {
  const d = join(root, 'tests', 'corpus', dir);
  return readdirSync(d)
    .filter((f) => f.toLowerCase().endsWith(ext))
    .sort()
    .map((f) => ({ name: f, bytes: readFileSync(join(d, f)) }));
}

async function instance(tag) {
  const mod = await import(`${glue}?${tag}`);
  const exports = mod.initSync({ module: wasmBytes });
  return { mod, memory: () => exports.memory.buffer.byteLength / MB };
}

/** Open/close `cycles` documents in one instance; returns the memory samples. */
async function run(kind, files, tag, more, passes) {
  const { mod, memory } = await instance(tag);
  const sheet = kind === 'xlsx';
  const engine = sheet ? new mod.WasmSheet() : new mod.WasmEngine();
  for (const f of fonts) engine.addFont(f);
  if (sheet) engine.newWorkbook('blank.xlsx');
  else engine.newDocument('blank.docx');
  const base = memory();
  const samples = [];
  let opened = 0;
  let failed = 0;
  // Warm-up: every file `passes` times; then `more` open/close.
  const warmup = files.length * passes;
  const total = warmup + more;
  for (let i = 0; i < total; i++) {
    const f = files[i % files.length];
    try {
      if (sheet) {
        engine.open(f.bytes, f.name, undefined);
        engine.render(0, 0, 0, 800, 600);
      } else {
        const info = JSON.parse(engine.open(f.bytes, f.name, undefined));
        if (info.pageCount > 0) engine.pageJson(0);
      }
      opened++;
    } catch {
      failed++; // encrypted / unsupported samples: still exercise the error path
    }
    if (sheet) engine.newWorkbook('blank.xlsx');
    else engine.newDocument('blank.docx');
    samples.push(memory());
  }
  engine.free();
  const warm = samples[warmup - 1];
  const last = samples[samples.length - 1];
  return {
    kind,
    files: files.length,
    opened,
    failed,
    base,
    warm,
    last,
    peak: Math.max(...samples),
  };
}

const kinds = [
  ['docx', corpus('doc', '.docx'), cycles, 1],
  ['pptx', corpus('slide', '.pptx'), cycles, 1],
  ['xlsx', corpus('sheet', '.xlsx'), cycles, 1],
];
const big = process.argv.slice(3).map((f) => ({ name: basename(f), bytes: readFileSync(f) }));
if (big.length) kinds.push(['big', big, Math.max(1, Math.round(cycles / 10)), 2]);
let ok = true;
for (const [i, [kind, files, more, passes]] of kinds.entries()) {
  const r = await run(kind, files, i, more, passes);
  const grew = r.last - r.warm;
  const pass = grew <= slack;
  ok &&= pass;
  console.log(
    `${kind}: ${passes} × ${r.files} + ${more} open/close (${r.opened} opened, ${r.failed} refused) ` +
      `memory base=${r.base.toFixed(1)} MB after-warm-up=${r.warm.toFixed(1)} MB ` +
      `last=${r.last.toFixed(1)} MB peak=${r.peak.toFixed(1)} MB growth=${grew.toFixed(1)} MB ${pass ? 'OK' : 'LEAK'}`,
  );
}
process.exit(ok ? 0 : 1);
