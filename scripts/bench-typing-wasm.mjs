// Typing latency of the WASM engine in a 300-page BlueDoc document (BlueOffice-PROMPT §5:
// "gõ phím → hiển thị < 16 ms"), the same scenario as `cargo bench -p bo-engine --bench
// doc_typing`: one edit in the middle of the document, then the display list (JSON) of
// the page showing the caret. The inverse edit that keeps the document stable is not timed.
// Usage: pnpm build:wasm && node scripts/bench-typing-wasm.mjs
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
// BO_WASM_PKG: another wasm-bindgen output directory (e.g. a build of an older commit).
const pkg = process.env.BO_WASM_PKG ?? join(root, 'packages', 'engine-wasm', 'pkg');
const { initSync, WasmEngine } = await import(pathToFileURL(join(pkg, 'bo_wasm.js')).href);
initSync({ module: readFileSync(join(pkg, 'bo_wasm_bg.wasm')) });

const engine = new WasmEngine();
const fontsDir = join(root, 'assets', 'fonts');
const manifest = JSON.parse(readFileSync(join(fontsDir, 'manifest.json'), 'utf8'));
for (const f of manifest.fonts) engine.addFont(readFileSync(join(fontsDir, f)));

// The native benchmark document: 4300 justified paragraphs, first-line indent 567 twips,
// 6 pt after, Times New Roman 13 pt (crates/bo-engine/tests/support/big_doc.rs).
const TEXT =
  'Căn cứ Luật Tổ chức Chính phủ ngày 19 tháng 6 năm 2015; Căn cứ Luật sửa đổi, bổ sung một số điều của Luật Tổ chức Chính phủ và Luật Tổ chức chính quyền địa phương ngày 22 tháng 11 năm 2019; Theo đề nghị của Bộ trưởng Bộ Nội vụ.';
const PARAGRAPHS = 4300;
const text = Array.from({ length: PARAGRAPHS }, (_, i) => `${i}. ${TEXT}`).join('\n');
let t = performance.now();
engine.open(new TextEncoder().encode(text), 'bench.txt');
const cmd = (c) => JSON.parse(engine.command(JSON.stringify({ ...c, now: Date.now() })));
cmd({ cmd: 'selectAll' });
cmd({ cmd: 'formatParagraph', format: { kind: 'align', align: 'Justify' } });
cmd({ cmd: 'formatParagraph', format: { kind: 'indentFirstLine', twips: 567 } });
const info = cmd({ cmd: 'formatParagraph', format: { kind: 'spaceAfter', twips: 120 } }).info;
console.log(
  `benchmark document: ${info.pageCount} pages (open + format ${(performance.now() - t).toFixed(0)} ms)`,
);

const mid = PARAGRAPHS / 2;
const sel = (a, b = a) => ({
  cmd: 'setSelection',
  selection: { anchor: { p: mid, o: a }, focus: { p: mid, o: b } },
});
const bold = { cmd: 'formatText', format: { kind: 'bold' } };
// name, selection, timed command, restoring command (as in big_doc::edits / prepare).
const edits = [
  ['type_char', sel(10), { cmd: 'insertText', text: 'a' }, { cmd: 'deleteBackward' }],
  ['enter', sel(10), { cmd: 'splitParagraph' }, { cmd: 'deleteBackward' }],
  ['enter_at_paragraph_end', sel(100000), { cmd: 'splitParagraph' }, { cmd: 'deleteBackward' }],
  ['backspace', sel(7), { cmd: 'deleteBackward' }, { cmd: 'insertText', text: 'C' }],
  ['bold_toggle', sel(16, 22), bold, bold],
];
const run = (c) => {
  const r = cmd(c);
  engine.pageJson(r.caret ? r.caret.page : 0);
  return r;
};
const ITER = 30;
for (const [name, s, timed, restore] of edits) {
  cmd(s);
  for (let i = 0; i < 3; i++) {
    run(timed);
    run(restore);
  }
  const times = [];
  let dirty = 0;
  for (let i = 0; i < ITER; i++) {
    t = performance.now();
    const r = run(timed);
    times.push(performance.now() - t);
    dirty = r.dirty.length;
    run(restore);
  }
  times.sort((a, b) => a - b);
  const med = times[times.length >> 1];
  console.log(
    `${name.padEnd(24)} median ${med.toFixed(2)} ms  min ${times[0].toFixed(2)}  max ${times[times.length - 1].toFixed(2)}  (dirty pages: ${dirty})`,
  );
}
