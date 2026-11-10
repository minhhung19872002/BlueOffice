/**
 * BlueScript object API against the real WASM engine (CORE-SCRIPT-001): BlueDoc,
 * BlueSheet and BlueSlide scripts change the document through engine commands,
 * and a whole run is one undo step.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmEngine, WasmSheet } from '@blueoffice/engine-wasm';
import type {
  EditCommand,
  EditResult,
  SheetCommand,
  SlideCommand,
  SlideEditResult,
} from '@blueoffice/platform';
import { docScriptHost } from '../src/hosts/doc';
import { sheetScriptHost } from '../src/hosts/sheet';
import { slideScriptHost } from '../src/hosts/slide';
import { inlineRunner } from '../src/runner';
import type { ScriptHost } from '../src/host';

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

async function script(host: ScriptHost, code: string) {
  const logs: string[] = [];
  const r = await inlineRunner().run(code, host, { log: (_l, t) => logs.push(t) }).done;
  if (!r.ok) throw new Error(`${r.error?.kind}: ${r.error?.message}`);
  return { value: r.value, logs };
}

describe('BlueDoc API', () => {
  const setup = () => {
    const e = engine();
    e.newDocument('script.docx');
    const run = async (cmd: EditCommand) => parse<EditResult>(e.command(JSON.stringify(cmd)));
    return { e, run, host: docScriptHost({ run }) };
  };

  it('writes, reads, formats, finds and replaces', async () => {
    const { run, host } = setup();
    const { value } = await script(
      host,
      `
      const first = await doc.appendParagraph('Báo cáo quý', 'Heading1');
      await doc.appendParagraph('Doanh thu tăng 10%. Doanh thu ổn định.');
      const paras = await doc.paragraphs;
      await paras[1].format({ bold: true, size: 14, color: '#C00000' });
      const style = await paras[0].getStyle();
      const hits = await doc.find('Doanh thu');
      const n = await doc.replace('Doanh thu', 'Lợi nhuận');
      return { first, count: paras.length, style, hits, n, text: await doc.text };
    `,
    );
    const v = value as {
      first: number;
      count: number;
      style: string;
      hits: { paragraph: number; start: number; end: number }[];
      n: number;
      text: string;
    };
    expect(v.first).toBe(0);
    expect(v.count).toBe(2);
    expect(v.style).toBe('Heading1');
    // Offsets are string indexes (not UTF-8 bytes).
    expect(v.hits).toEqual([
      { paragraph: 1, start: 0, endParagraph: 1, end: 9 },
      { paragraph: 1, start: 20, endParagraph: 1, end: 29 },
    ]);
    expect(v.n).toBe(2);
    expect(v.text).toBe('Báo cáo quý\nLợi nhuận tăng 10%. Lợi nhuận ổn định.');
    // Paragraph 1 is bold, 14 pt, red.
    const f = await run({
      cmd: 'setSelection',
      selection: { anchor: { p: 1, o: 2 }, focus: { p: 1, o: 2 } },
    });
    expect(f.format.bold).toBe(true);
    expect(f.format.sizeHalfPoints).toBe(28);
    expect(f.format.color?.toUpperCase()).toBe('C00000');
  });

  it('a run is one undo step', async () => {
    const { run, host } = setup();
    await script(
      host,
      `await doc.appendParagraph('Một'); await doc.appendParagraph('Hai'); await doc.appendParagraph('Ba');`,
    );
    const text = async () =>
      ((await run({ cmd: 'speechText' })).speech?.paras ?? []).map((p) => p.text).join('|');
    expect(await text()).toBe('Một|Hai|Ba');
    await run({ cmd: 'undo' });
    expect(await text()).toBe('');
    await run({ cmd: 'redo' });
    expect(await text()).toBe('Một|Hai|Ba');
  });

  it('sets paragraph text, alignment, tables and deletes paragraphs', async () => {
    const { run, host } = setup();
    await script(
      host,
      `
      await doc.appendParagraph('a');
      await doc.appendParagraph('b');
      await doc.appendParagraph('c');
      const [a, b] = await doc.paragraphs;
      await a.setText('Tiêu đề');
      await a.setFormat({ align: 'center' });
      await b.delete();
      `,
    );
    const paras = (await run({ cmd: 'speechText' })).speech?.paras ?? [];
    expect(paras.map((p) => p.text)).toEqual(['Tiêu đề', 'c']);
    const f = await run({
      cmd: 'setSelection',
      selection: { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } },
    });
    expect(f.format.align).toBe('Center');
    await script(
      host,
      `await doc.select(1, 1); await doc.insertTable(2, 2, [['x', 'y'], ['1', '2']]);`,
    );
    const after = ((await run({ cmd: 'speechText' })).speech?.paras ?? []).map((p) => p.text);
    expect(after).toEqual(expect.arrayContaining(['x', 'y', '1', '2']));
  });

  it('rejects unknown methods and bad arguments inside the script', async () => {
    const { host } = setup();
    const { value } = await script(
      host,
      `
      const out = [];
      try { await doc.select(99); } catch (e) { out.push(e.message); }
      try { await doc.format({ bold: 'yes' }); } catch (e) { out.push(e.message); }
      return out;
      `,
    );
    expect(value).toEqual(['no paragraph 99', 'format.bold: expected a boolean']);
    await expect(host.call('doc.evil', [])).rejects.toThrow(/unknown API method/);
  });
});

describe('BlueSheet API', () => {
  const setup = () => {
    const e = new WasmSheet();
    e.setLocale('en-US');
    e.newWorkbook('script.xlsx');
    const host = {
      info: async () => parse<never>(e.info()),
      cell: async (s: number, r: number, c: number) => parse<never>(e.cell(s, r, c)),
      exec: async <T>(cmd: SheetCommand) => parse<T>(e.exec(JSON.stringify(cmd))),
      addSheet: async (n?: string) => e.addSheet(n),
      renameSheet: async (s: number, n: string) => e.renameSheet(s, n),
    };
    return { e, host: sheetScriptHost({ host, activeSheet: () => 0, decimal: '.' }) };
  };

  it('values, formulas, formats and charts', async () => {
    const { e, host } = setup();
    const { value } = await script(
      host,
      `
      const r = sheet.range('A1:B3');
      await r.setValues([['Tháng', 'Doanh thu'], ['1', 120], ['2', 80.5]]);
      await sheet.range('B4').setFormulas([['=SUM(B2:B3)']]);
      await sheet.range('A1:B1').format({ bold: true, fillColor: '#FFFF00' });
      const chart = await sheet.addChart('column', 'A1:B3');
      const s2 = await workbook.addSheet('Tổng hợp');
      await s2.range('A1').setValue(true);
      return {
        values: await sheet.range('A1:B4').values,
        formulas: await sheet.range('B4').formulas,
        chart,
        sheets: (await workbook.sheets).map((s) => s.name),
        other: await s2.cell(0, 0).values,
      };
      `,
    );
    expect(value).toEqual({
      values: [
        ['Tháng', 'Doanh thu'],
        [1, 120],
        [2, 80.5],
        [null, 200.5],
      ],
      formulas: [['=SUM(B2:B3)']],
      chart: 0,
      sheets: ['Sheet1', 'Tổng hợp'],
      other: [[true]],
    });
    const fmt = parse<{ bold: boolean; fillColor: string | null }>(
      e.exec(JSON.stringify({ cmd: 'formatState', sheet: 0, row: 0, col: 1 })),
    );
    expect(fmt.bold).toBe(true);
    expect(fmt.fillColor?.toUpperCase()).toContain('FFFF00');
  });

  it('a run is one undo step', async () => {
    const { e, host } = setup();
    await script(
      host,
      `await sheet.range('A1').setValue(1); await sheet.range('A2').setValue(2); await sheet.range('A1:A2').format({ italic: true });`,
    );
    expect(parse<{ input: string }>(e.cell(0, 1, 0)).input).toBe('2');
    expect(e.undo()).toBe(true);
    expect(parse<{ input: string }>(e.cell(0, 0, 0)).input).toBe('');
    expect(parse<{ input: string }>(e.cell(0, 1, 0)).input).toBe('');
    expect(e.undo()).toBe(false);
    expect(e.redo()).toBe(true);
    expect(parse<{ input: string }>(e.cell(0, 1, 0)).input).toBe('2');
    expect(
      parse<{ italic: boolean }>(
        e.exec(JSON.stringify({ cmd: 'formatState', sheet: 0, row: 0, col: 0 })),
      ).italic,
    ).toBe(true);
  });

  it('refuses bad addresses', async () => {
    const { host } = setup();
    const { value } = await script(
      host,
      `try { await sheet.range('ZZZZ1').values; } catch (e) { return e.message; }`,
    );
    expect(value).toBe('bad range address "ZZZZ1"');
  });
});

describe('BlueSlide API', () => {
  const setup = () => {
    const e = engine();
    e.open(
      readFileSync(join(root, 'tests', 'corpus', 'slide', 'slide-edit-arrange.pptx')),
      'deck.pptx',
    );
    const run = async (cmd: SlideCommand) =>
      parse<SlideEditResult>(e.slideCommand(JSON.stringify(cmd)));
    return { e, run, host: slideScriptHost({ run }) };
  };

  it('slides, shapes, text and notes', async () => {
    const { run, host } = setup();
    const { value } = await script(
      host,
      `
      const before = (await presentation.info).slideCount;
      const s = await presentation.addSlide(0);
      const box = await s.addTextBox('Xin chào BlueSlide', { x: 50, y: 60, w: 300, h: 50 });
      const rect = await s.addShape('ellipse', { x: 400, y: 100, w: 100, h: 100 }, 'Tròn');
      await rect.setFill('#00B050');
      await box.move(80, 90);
      await s.setNotes('Ghi chú của script');
      const shapes = await s.shapes;
      return {
        before,
        after: (await presentation.info).slideCount,
        index: s.index,
        texts: shapes.map((x) => x.text).filter(Boolean),
        moved: shapes.find((x) => x.id === box.id).rect,
        notes: await s.notes,
      };
      `,
    );
    const v = value as {
      before: number;
      after: number;
      index: number;
      texts: string[];
      moved: { x: number; y: number };
      notes: string;
    };
    expect(v.after).toBe(v.before + 1);
    expect(v.index).toBe(1);
    expect(v.texts).toEqual(expect.arrayContaining(['Xin chào BlueSlide', 'Tròn']));
    expect(v.moved.x).toBeCloseTo(80, 0);
    expect(v.moved.y).toBeCloseTo(90, 0);
    expect(v.notes).toBe('Ghi chú của script');
    // One undo step removes everything the script did.
    const u = await run({ cmd: 'undo' });
    expect(u.info.pageCount).toBe(v.before);
    expect(u.canUndo).toBe(false);
  });
});
