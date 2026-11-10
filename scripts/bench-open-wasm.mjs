// Open time and WASM linear memory of the web engine on the BlueOffice-PROMPT §5 files:
// "Mở .docx 300 trang có ảnh < 5 s (web)", "Mở .xlsx 1 triệu ô có 100.000 công thức < 5 s",
// "Bộ nhớ WASM < 1 GB cho các file trên", and CORE-PERF-001: ".docx 1 000 trang < 8 s",
// ".pptx 300 slide có video < 5 s". Runs the same bo_wasm build the browser loads
// (Node's V8, no worker / canvas: picture decoding by the browser is measured by
// tests/e2e/specs/perf.spec.ts). Each file is opened in a fresh WASM instance so that
// the reported memory belongs to that file alone.
//
// Usage:
//   pnpm build:wasm
//   bo-cli sample-big out/big-300.docx && bo-cli sample-big-sheet out/big-1m.xlsx
//   node scripts/bench-open-wasm.mjs out/big-300.docx out/big-1m.xlsx
//   bo-cli sample-big out/big-1000.docx --pages 1000 --rich && bo-cli sample-big-deck out/deck-300.pptx
//   node scripts/bench-open-wasm.mjs out/big-1000.docx out/deck-300.pptx
import { readFileSync } from 'node:fs';
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

/** A fresh module instance (a new import URL gives new module state). */
async function instance(tag) {
  const mod = await import(`${glue}?${tag}`);
  const exports = mod.initSync({ module: wasmBytes });
  return { mod, memory: () => exports.memory.buffer.byteLength / MB };
}

const files = process.argv.slice(2);
if (files.length === 0) {
  console.error('usage: node scripts/bench-open-wasm.mjs <file.docx|file.pptx|file.xlsx>…');
  process.exit(2);
}
const results = [];
for (const [i, file] of files.entries()) {
  const bytes = readFileSync(file);
  const name = basename(file);
  const { mod, memory } = await instance(i);
  const before = memory();
  if (name.toLowerCase().endsWith('.xlsx')) {
    const sheet = new mod.WasmSheet();
    for (const f of fonts) sheet.addFont(f);
    let t = performance.now();
    const cpu = process.cpuUsage();
    sheet.open(bytes, name, undefined);
    const open = performance.now() - t;
    // CPU time of this (single-threaded) open: stable when other processes load the machine.
    const openCpu = process.cpuUsage(cpu).user / 1000;
    const formula = JSON.parse(sheet.cell(0, 0, 90));
    t = performance.now();
    sheet.setInputs(0, JSON.stringify([[10, 1, '123']]));
    const edit = performance.now() - t;
    results.push({
      file: name,
      sizeMB: bytes.length / MB,
      openMs: open,
      openCpuMs: openCpu,
      editLookupCellMs: edit,
      check: `${formula.address} ${formula.input} = ${formula.display}`,
      wasmMemoryMB: memory(),
      baseMemoryMB: before,
    });
  } else {
    const engine = new mod.WasmEngine();
    for (const f of fonts) engine.addFont(f);
    let t = performance.now();
    const cpu = process.cpuUsage();
    const info = JSON.parse(engine.open(bytes, name));
    engine.pageJson(0);
    const first = performance.now() - t;
    const firstCpu = process.cpuUsage(cpu).user / 1000;
    t = performance.now();
    for (let p = 0; p < info.pageCount; p++) engine.pageJson(p);
    const all = performance.now() - t;
    results.push({
      file: name,
      sizeMB: bytes.length / MB,
      pages: info.pageCount,
      openFirstPageMs: first,
      openFirstPageCpuMs: firstCpu,
      allDisplayListsMs: all,
      wasmMemoryMB: memory(),
      baseMemoryMB: before,
    });
  }
}
for (const r of results) {
  console.log(
    Object.entries(r)
      .map(([k, v]) => `${k}=${typeof v === 'number' ? v.toFixed(1) : v}`)
      .join(' '),
  );
}
