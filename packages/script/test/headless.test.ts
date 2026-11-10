/**
 * Headless BlueScript (CORE-SCRIPT-004): the bundle the native runtime embeds is
 * built from these sources, and the same script gives the same result in the
 * browser runner (QuickJS WASM + WASM engine) and in `bo-cli script run`
 * (QuickJS native + native engine).
 */
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmEngine, WasmSheet } from '@blueoffice/engine-wasm';
import type { EditCommand, SheetCommand, SlideCommand } from '@blueoffice/platform';
import type { ScriptHost } from '../src/host';
import { docScriptHost } from '../src/hosts/doc';
import { sheetScriptHost } from '../src/hosts/sheet';
import { slideScriptHost } from '../src/hosts/slide';
import { inlineRunner } from '../src/runner';
// @ts-expect-error -- plain ESM build script without type declarations
import { BUNDLE, bundle } from '../scripts/build-headless.mjs';

const root = join(__dirname, '..', '..', '..');
const exe = process.platform === 'win32' ? 'bo-cli.exe' : 'bo-cli';
const targetDir = process.env.CARGO_TARGET_DIR ?? join(root, 'target');

/** The most recently built bo-cli (as the platform parity test). */
function cli(): string {
  const built = ['release', 'debug']
    .map((p) => join(targetDir, p, exe))
    .filter((f) => existsSync(f))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (built[0]) return built[0];
  execFileSync('cargo', ['build', '-q', '-p', 'bo-cli'], { cwd: root, stdio: 'inherit' });
  return join(targetDir, 'debug', exe);
}

const parse = <T>(s: string) => JSON.parse(s) as T;
const fonts = () => {
  const dir = join(root, 'assets', 'fonts');
  const m = parse<{ fonts: string[] }>(readFileSync(join(dir, 'manifest.json'), 'utf8'));
  return m.fonts.slice(0, 4).map((f) => readFileSync(join(dir, f)));
};

beforeAll(() => {
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
});

interface Outcome {
  ok: boolean;
  value: unknown;
  logs: string[];
  calls: number;
  error?: string;
}

async function browser(host: ScriptHost, code: string): Promise<Outcome> {
  const logs: string[] = [];
  const r = await inlineRunner().run(code, host, { log: (_l, t) => logs.push(t) }).done;
  return {
    ok: r.ok,
    value: r.value ?? null,
    logs,
    calls: r.calls,
    ...(r.error ? { error: r.error.message } : {}),
  };
}

function native(file: string, code: string): Outcome {
  const dir = mkdtempSync(join(tmpdir(), 'bo-script-'));
  const script = join(dir, 's.js');
  writeFileSync(script, code);
  const p = spawnSync(cli(), ['script', 'run', file, '--script', script, '--json'], {
    encoding: 'utf8',
  });
  const r = parse<{
    ok: boolean;
    value?: unknown;
    logs: { text: string }[];
    calls: number;
    error?: { message: string };
  }>(p.stdout);
  return {
    ok: r.ok,
    value: r.value ?? null,
    logs: r.logs.map((l) => l.text),
    calls: r.calls,
    ...(r.error ? { error: r.error.message } : {}),
  };
}

const work = mkdtempSync(join(tmpdir(), 'bo-script-parity-'));

describe('headless bundle', () => {
  it('is built from the current sources', async () => {
    const fresh = (await bundle()) as string;
    expect(readFileSync(BUNDLE as string, 'utf8').replace(/\r\n/g, '\n')).toBe(fresh);
  });
});

describe('browser runner vs bo-cli script run', () => {
  const DOC = `
    await doc.appendParagraph('Báo cáo tuần', 'Heading1');
    await doc.appendParagraph('Doanh thu 100. Doanh thu ổn.');
    const n = await doc.replace('Doanh thu', 'Lợi nhuận');
    const p = await doc.paragraphs;
    await p[1].format({ bold: true });
    console.log('đoạn', p.length, 'thay', n);
    const errors = [];
    try { await doc.select(42); } catch (e) { errors.push(e.message); }
    try { await doc.format({ size: 'x' }); } catch (e) { errors.push(e.message); }
    return { n, text: await doc.text, style: await p[0].getStyle(), hits: await doc.find('ổn'), errors };
  `;

  it('BlueDoc', async () => {
    const e = new WasmEngine();
    for (const f of fonts()) e.addFont(f);
    e.newDocument('parity.docx');
    const file = join(work, 'parity.docx');
    writeFileSync(file, e.saveDocx());
    const run = async (cmd: EditCommand) => parse<never>(e.command(JSON.stringify(cmd)));
    const web = await browser(docScriptHost({ run }), DOC);
    const cliOut = native(file, DOC);
    expect(web.ok).toBe(true);
    expect(cliOut).toEqual(web);
  });

  const SHEET = `
    await sheet.range('A1:C2').setValues([[1, 2.5, 'x'], [true, null, 'Tổng']]);
    await sheet.range('D1').setFormulas([['=A1+B1']]);
    await sheet.range('A1:D1').format({ bold: true, fillColor: '#FFEEAA' });
    const s = await workbook.addSheet('Dữ liệu');
    await s.range('B2').setValue(7);
    console.log((await workbook.sheets).map((x) => x.name).join('|'));
    let bad = '';
    try { await sheet.range('ZZZZ1').values; } catch (e) { bad = e.message; }
    return { v: await sheet.range('A1:D2').values, f: await sheet.range('D1').formulas,
             s2: await s.range('A1:B2').values, bad };
  `;

  it('BlueSheet', async () => {
    const e = new WasmSheet();
    e.newWorkbook('parity.xlsx');
    const file = join(work, 'parity.xlsx');
    writeFileSync(file, e.save('xlsx', 0));
    const host = {
      info: async () => parse<never>(e.info()),
      cell: async (s: number, r: number, c: number) => parse<never>(e.cell(s, r, c)),
      exec: async <T>(cmd: SheetCommand) => parse<T>(e.exec(JSON.stringify(cmd))),
      addSheet: async (n?: string) => e.addSheet(n),
      renameSheet: async (s: number, n: string) => e.renameSheet(s, n),
    };
    // Both engines use their default input locale (vi-VN: decimal comma).
    const web = await browser(sheetScriptHost({ host, activeSheet: () => 0, decimal: ',' }), SHEET);
    const cliOut = native(file, SHEET);
    expect(web.ok).toBe(true);
    expect(cliOut).toEqual(web);
  });

  const SLIDE = `
    const s = presentation.slide(0);
    const box = await s.addTextBox('Xin chào', { x: 40, y: 50, w: 200, h: 40 });
    await box.format({ bold: true, size: 20 });
    await box.move(60, 70);
    await s.setNotes('Ghi chú');
    const shapes = await s.shapes;
    const moved = shapes.find((x) => x.id === box.id);
    return { count: (await presentation.info).slideCount, notes: await s.notes,
             rect: moved && moved.rect, names: shapes.map((x) => x.kind) };
  `;

  it('BlueSlide', async () => {
    const file = join(root, 'tests', 'corpus', 'slide', 'slide-edit-arrange.pptx');
    const e = new WasmEngine();
    for (const f of fonts()) e.addFont(f);
    e.open(readFileSync(file), 'slide-edit-arrange.pptx');
    const run = async (cmd: SlideCommand) => parse<never>(e.slideCommand(JSON.stringify(cmd)));
    const web = await browser(slideScriptHost({ run }), SLIDE);
    const cliOut = native(file, SLIDE);
    expect(web.ok).toBe(true);
    expect(cliOut).toEqual(web);
  });

  it('failures look the same', async () => {
    const e = new WasmEngine();
    for (const f of fonts()) e.addFont(f);
    e.newDocument('fail.docx');
    const file = join(work, 'fail.docx');
    writeFileSync(file, e.saveDocx());
    const code = `await doc.appendParagraph('a'); await doc.command({ cmd: 'nope' });`;
    const run = async (cmd: EditCommand) => parse<never>(e.command(JSON.stringify(cmd)));
    const web = await browser(docScriptHost({ run }), code);
    const cliOut = native(file, code);
    expect(web.ok).toBe(false);
    expect(cliOut).toEqual(web);
  });
});
