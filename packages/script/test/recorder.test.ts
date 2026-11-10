/**
 * Macro recorder (CORE-SCRIPT-002) against the real WASM engine: the commands
 * the editors send become BlueScript source, and running that source on a
 * fresh copy of the starting document produces the same document.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmEngine, WasmSheet } from '@blueoffice/engine-wasm';
import type {
  EditCommand,
  EditResult,
  EngineHost,
  SheetCommand,
  SheetHost,
  SlideCommand,
  SlideEditResult,
} from '@blueoffice/platform';
import { docScriptHost } from '../src/hosts/doc';
import { sheetScriptHost } from '../src/hosts/sheet';
import { slideScriptHost } from '../src/hosts/slide';
import { inlineRunner } from '../src/runner';
import type { ScriptHost } from '../src/host';
import { recordEngine, recordSheetHost, ScriptRecorder } from '../src/recorder';

const root = join(__dirname, '..', '..', '..');
const parse = <T>(s: string) => JSON.parse(s) as T;

beforeAll(() => {
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
});

function engine(): WasmEngine {
  const e = new WasmEngine();
  const fontsDir = join(root, 'assets', 'fonts');
  const manifest = parse<{ fonts: string[] }>(
    readFileSync(join(fontsDir, 'manifest.json'), 'utf8'),
  );
  for (const f of manifest.fonts.slice(0, 4)) e.addFont(readFileSync(join(fontsDir, f)));
  return e;
}

/** The part of `EngineHost` the recorder and the tests use, over a `WasmEngine`. */
const engineHost = (e: WasmEngine) =>
  ({
    command: async (c: EditCommand) => parse<EditResult>(e.command(JSON.stringify(c))),
    slideCommand: async (c: SlideCommand) =>
      parse<SlideEditResult>(e.slideCommand(JSON.stringify(c))),
  }) as unknown as EngineHost;

async function replay(host: ScriptHost, code: string) {
  const r = await inlineRunner().run(code, host).done;
  if (!r.ok) throw new Error(`${r.error?.kind}: ${r.error?.message}\n${code}`);
}

describe('BlueDoc recorder', () => {
  const fresh = () => {
    const e = engine();
    e.newDocument('rec.docx');
    return e;
  };

  it('records typing, formatting and selections; the replay builds the same document', async () => {
    const a = fresh();
    const rec = new ScriptRecorder('doc');
    const ui = recordEngine(engineHost(a), rec);
    await rec.start();
    const type = async (text: string) => {
      for (const ch of text) await ui.command({ cmd: 'insertText', text: ch, now: 1 });
    };
    await type('Hello ');
    await ui.command({ cmd: 'formatText', format: { kind: 'bold' } });
    await type('Wörld');
    await ui.command({ cmd: 'formatText', format: { kind: 'bold' } });
    await ui.command({ cmd: 'splitParagraph' });
    await type('Tiêu đề x');
    await ui.command({ cmd: 'deleteBackward' });
    await ui.command({ cmd: 'deleteBackward' });
    await ui.command({ cmd: 'applyStyle', id: 'Heading1' });
    // Select "Hello" with the keyboard and make it italic, red, 14 pt.
    await ui.command({ cmd: 'move', motion: 'docStart' });
    for (let i = 0; i < 5; i++) await ui.command({ cmd: 'move', motion: 'right', extend: true });
    await ui.command({ cmd: 'formatText', format: { kind: 'italic' } });
    await ui.command({ cmd: 'formatText', format: { kind: 'color', hex: 'C00000' } });
    await ui.command({ cmd: 'formatText', format: { kind: 'size', halfPoints: 28 } });
    await ui.command({ cmd: 'formatParagraph', format: { kind: 'align', align: 'Center' } });
    // Reading and navigation are not recorded.
    await ui.command({ cmd: 'state' });
    await ui.command({ cmd: 'find', query: 'x', options: {} });
    const source = await rec.stop();

    expect(source).toContain('await doc.insertText("Hello ");');
    expect(source).toContain('await doc.format({"bold":true});');
    expect(source).toContain('await doc.insertText("Wörld");');
    expect(source).toContain('await doc.format({"bold":false});');
    // Enter and Backspace fold into the typed text.
    expect(source).toContain('await doc.insertText("\\nTiêu đề");');
    expect(source).toContain('await (await doc.paragraph(1)).setStyle("Heading1");');
    expect(source).toContain('await doc.select(0, 0, 5);');
    expect(source).toContain('await doc.format({"color":"#C00000"});');
    expect(source).toContain('await doc.format({"size":14});');
    expect(source).toContain('"formatParagraph"');
    expect(source).not.toContain('"move"');
    expect(source).not.toContain('"state"');
    expect(source.split('insertText').length - 1).toBe(3);

    const b = fresh();
    const run = async (cmd: EditCommand) => parse<EditResult>(b.command(JSON.stringify(cmd)));
    await replay(docScriptHost({ run }), source);
    expect(b.pageJson(0)).toBe(a.pageJson(0));
  });

  it('starts from the user’s caret and ignores what scripts do', async () => {
    const a = fresh();
    const ui0 = engineHost(a);
    await ui0.command({ cmd: 'insertText', text: 'abc def' });
    const rec = new ScriptRecorder('doc');
    const ui = recordEngine(ui0, rec);
    await ui.command({
      cmd: 'setSelection',
      selection: { anchor: { p: 0, o: 3 }, focus: { p: 0, o: 3 } },
    });
    await rec.start();
    await ui.command({ cmd: 'insertText', text: '!' });
    await rec.pause(() => ui.command({ cmd: 'insertText', text: '?' }));
    const source = await rec.stop();
    expect(source).toBe('await doc.select(0, 3, 3);\nawait doc.insertText("!");');
  });
});

describe('BlueSheet recorder', () => {
  const fresh = () => {
    const e = new WasmSheet();
    e.setLocale('en-US');
    e.newWorkbook('rec.xlsx');
    const host = {
      info: async () => parse<never>(e.info()),
      cell: async (s: number, r: number, c: number) => parse<never>(e.cell(s, r, c)),
      exec: async <T>(cmd: SheetCommand) => parse<T>(e.exec(JSON.stringify(cmd))),
      setInputs: async (s: number, cells: [number, number, string][]) =>
        e.setInputs(s, JSON.stringify(cells)),
      addSheet: async (n?: string) => e.addSheet(n),
      renameSheet: async (s: number, n: string) => e.renameSheet(s, n),
    } as unknown as SheetHost;
    return { e, host };
  };
  const dump = async (h: SheetHost) => {
    const out: string[] = [];
    for (let r = 0; r < 4; r++) {
      for (let c = 0; c < 3; c++) out.push(JSON.stringify(await h.cell(0, r, c)));
    }
    return out;
  };

  it('records cell input and range formats as range calls', async () => {
    const a = fresh();
    const rec = new ScriptRecorder('sheet');
    const ui = recordSheetHost(a.host, rec);
    await rec.start();
    await ui.setInputs(0, [[0, 0, 'Tên']]);
    await ui.setInputs(0, [[0, 1, '12.5']]);
    await ui.setInputs(0, [
      [1, 0, 'a'],
      [1, 1, '2'],
      [2, 0, 'b'],
      [2, 1, '=B1*2'],
    ]);
    await ui.exec({
      cmd: 'format',
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 0, c1: 1 },
      patch: { bold: true, fillColor: 'FFFF00' },
    });
    await ui.exec({ cmd: 'merge', sheet: 0, range: { r0: 3, c0: 0, r1: 3, c1: 2 }, center: true });
    await ui.exec({ cmd: 'charts', sheet: 0 });
    const source = await rec.stop();
    expect(source).toContain('await (await workbook.sheet(0)).range("A1").setValues([["Tên"]]);');
    expect(source).toContain('range("A2:B3").setValues([["a","2"],["b","=B1*2"]]);');
    expect(source).toContain('range("A1:B1").format({"bold":true,"fillColor":"#FFFF00"});');
    expect(source).toContain('range("A4:C4").merge(true);');
    expect(source).not.toContain('charts');

    const b = fresh();
    await replay(sheetScriptHost({ host: b.host, activeSheet: () => 0, decimal: '.' }), source);
    expect(await dump(b.host)).toEqual(await dump(a.host));
  });
});

describe('BlueSlide recorder', () => {
  const fresh = () => {
    const e = engine();
    e.open(
      readFileSync(join(root, 'tests', 'corpus', 'slide', 'slide-edit-arrange.pptx')),
      'deck.pptx',
    );
    return e;
  };

  it('records shape text, fill and moves on the selected shape', async () => {
    const a = fresh();
    const rec = new ScriptRecorder('slide');
    const ui = recordEngine(engineHost(a), rec);
    await rec.start();
    const all = await ui.slideCommand({ cmd: 'selectAll' });
    const [first, second] = all.selection;
    expect(first && second).toBeTruthy();
    await ui.slideCommand({ cmd: 'select', ids: [first!.id] });
    await ui.slideCommand({ cmd: 'setFill', color: '00B050' });
    await ui.slideCommand({ cmd: 'transform', transform: { kind: 'move', dx: 20, dy: 10 } });
    await ui.slideCommand({ cmd: 'textSelect', id: first!.id, start: 0, end: 0 });
    await ui.slideCommand({ cmd: 'setText', id: first!.id, text: 'H', merge: true });
    await ui.slideCommand({ cmd: 'setText', id: first!.id, text: 'Hi', merge: true });
    await ui.slideCommand({ cmd: 'textExit' });
    await ui.slideCommand({ cmd: 'select', ids: [second!.id] });
    await ui.slideCommand({ cmd: 'setLine', color: 'C00000', width: 2 });
    await ui.slideCommand({ cmd: 'goTo', slide: 0 });
    await ui.slideCommand({ cmd: 'addSlide', after: 0 });
    const source = await rec.stop();
    expect(source).toContain(`find((s) => s.id === ${first!.id})?.setFill("#00B050");`);
    expect(source).toContain(`find((s) => s.id === ${first!.id})?.move(`);
    expect(source).toContain(`find((s) => s.id === ${first!.id})?.setText("Hi");`);
    expect(source).toContain(`find((s) => s.id === ${second!.id})?.setLine("#C00000", 2);`);
    expect(source).toContain('await presentation.command({"cmd":"addSlide","after":0});');
    expect(source.match(/setText/g)?.length).toBe(1);

    const b = fresh();
    const run = async (cmd: SlideCommand) =>
      parse<SlideEditResult>(b.slideCommand(JSON.stringify(cmd)));
    await replay(slideScriptHost({ run }), source);
    expect(b.slideCount()).toBe(a.slideCount());
    for (let i = 0; i < a.slideCount(); i++) expect(b.slideJson(i)).toBe(a.slideJson(i));
  });
});
