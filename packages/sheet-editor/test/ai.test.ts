import { AiNotice, type AiFeature, type AiRunContext } from '@blueoffice/editor';
import { createI18n } from '@blueoffice/i18n';
import {
  FakeAiProvider,
  aiComplete,
  type FormulaCheck,
  type SheetCommand,
} from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import {
  AI_FORMULA_ROWS,
  SHEET_AI_COMMANDS,
  checkFormula,
  formulaContext,
  rangeA1,
  sheetAiFeatures,
  tableText,
  type SheetAiTarget,
} from '../src/sheetAi';

const i18n = createI18n('vi-VN');
const t = (key: string, options?: Record<string, unknown>) => i18n.t(key, options ?? {});

/**
 * A fake sheet: display texts by `row:col`; `checkFormula` answers what the test decides for a
 * formula (standing in for the engine's parser, which has its own Rust tests).
 */
function fakeSheet(
  cells: Record<string, string>,
  verdicts: Record<string, Partial<FormulaCheck>> = {},
) {
  const data = new Map(Object.entries(cells));
  const checked: string[] = [];
  const writes: [number, number, string][] = [];
  const host: SheetAiTarget['host'] = {
    cell: async (_s, row, col) => {
      const v = data.get(`${row}:${col}`) ?? '';
      return { address: '', input: v, display: v };
    },
    setInputs: async (_s, w) => {
      writes.push(...w);
    },
    exec: async <T>(cmd: SheetCommand): Promise<T> => {
      if (cmd.cmd !== 'checkFormula') throw new Error(`unexpected ${cmd.cmd}`);
      checked.push(cmd.formula);
      const check: FormulaCheck = {
        ok: true,
        error: null,
        functions: [],
        unknownFunctions: [],
        ...verdicts[cmd.formula],
      };
      return check as T;
    },
  };
  return { host, checked, writes };
}

const SALES = {
  '0:0': 'Khu vực',
  '0:1': 'Doanh thu',
  '1:0': 'Miền Bắc',
  '1:1': '1.200',
  '2:0': 'Miền\tNam\nmới',
  '2:1': '800',
};

const ctx = (
  provider: FakeAiProvider | undefined,
  prompt: string,
  material: string | null,
): AiRunContext => ({
  prompt,
  tone: 'formal',
  language: 'vi',
  material: material === null ? null : { text: material, label: 'cells' },
  signal: new AbortController().signal,
  ask: (request) => aiComplete(provider, request),
});

const feature = (features: AiFeature[], id: string) => features.find((f) => f.id === id)!;

describe('BlueSheet BlueAI context', () => {
  it('lists the first rows of the selection as address = value', async () => {
    const { host } = fakeSheet(SALES);
    const target: SheetAiTarget = {
      host,
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 99, c1: 1 },
      active: { row: 0, col: 3 },
    };
    expect(rangeA1(target.range)).toBe('A1:B100');
    expect(rangeA1({ r0: 2, c0: 27, r1: 2, c1: 27 })).toBe('AB3');
    expect(await formulaContext(target)).toBe(
      [
        'Selection: A1:B100',
        'A1 = Khu vực\tB1 = Doanh thu',
        'A2 = Miền Bắc\tB2 = 1.200',
        // Tabs and line breaks inside a cell cannot break the layout.
        'A3 = Miền Nam mới\tB3 = 800',
      ].join('\n'),
    );
    expect(await tableText(target)).toBe('Khu vực\tDoanh thu\nMiền Bắc\t1.200\nMiền Nam mới\t800');
    // An empty selection sends no context at all.
    const empty = { ...target, range: { r0: 50, c0: 5, r1: 60, c1: 6 } };
    expect(await formulaContext(empty)).toBe('');
    expect(await tableText(empty)).toBe('');
  });

  it('only sends a few sample rows for a formula', async () => {
    const cells: Record<string, string> = {};
    for (let r = 0; r < 50; r++) cells[`${r}:0`] = `v${r}`;
    const { host } = fakeSheet(cells);
    const target: SheetAiTarget = {
      host,
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 49, c1: 0 },
      active: { row: 0, col: 0 },
    };
    const lines = (await formulaContext(target)).split('\n');
    expect(lines).toHaveLength(1 + AI_FORMULA_ROWS);
    expect(lines.at(-1)).toBe(`A${AI_FORMULA_ROWS} = v${AI_FORMULA_ROWS - 1}`);
  });
});

describe('BlueSheet formula suggestion', () => {
  const setup = (verdicts: Record<string, Partial<FormulaCheck>> = {}) => {
    const sheet = fakeSheet(SALES, verdicts);
    let target: SheetAiTarget = {
      host: sheet.host,
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 2, c1: 1 },
      active: { row: 4, col: 1 },
    };
    let changed = 0;
    const features = sheetAiFeatures({ t, target: () => target, changed: () => void changed++ });
    return {
      ...sheet,
      features,
      move: (row: number, col: number) => (target = { ...target, active: { row, col } }),
      changed: () => changed,
    };
  };

  it('offers a formula the engine parsed, and writes it only when the user confirms', async () => {
    const s = setup();
    const fake = new FakeAiProvider();
    fake.answers.formula = 'Công thức:\n```\n=SUMIF(A2:A3,"Miền Bắc",B2:B3)\n```';
    const f = feature(s.features, 'formula');
    const material = await f.material!.read();
    expect(material?.label).toBe('các ô A1:B3');
    const preview = await f.run(ctx(fake, 'Tổng doanh thu Miền Bắc', material!.text));
    expect(fake.calls).toEqual([
      { task: 'formula', instruction: 'Tổng doanh thu Miền Bắc', text: material!.text },
    ]);
    expect(preview.text).toBe('=SUMIF(A2:A3,"Miền Bắc",B2:B3)');
    expect(s.checked).toEqual(['=SUMIF(A2:A3,"Miền Bắc",B2:B3)']);
    // Never applied by itself.
    expect(s.writes).toEqual([]);
    expect(preview.actions.map((a) => a.id)).toEqual(['insertFormula']);
    // The user moved to another cell before confirming: that is where it goes.
    s.move(5, 2);
    expect(await preview.actions[0]!.apply()).toBe('Đã chèn công thức vào ô C6.');
    expect(s.writes).toEqual([[5, 2, '=SUMIF(A2:A3,"Miền Bắc",B2:B3)']]);
    expect(s.changed()).toBe(1);
  });

  it('drops a formula the engine cannot parse or whose functions it does not know', async () => {
    const s = setup({
      '=SUM(A1:A3': { ok: false, error: { message: 'expected )', pos: 10 } },
      '=MAGIC(A1)': { ok: false, unknownFunctions: ['MAGIC'] },
      '=MAX(A1:A3': { ok: true, fixed: '=MAX(A1:A3)' },
    });
    const fake = new FakeAiProvider();
    const f = feature(s.features, 'formula');
    fake.answers.formula = '=SUM(A1:A3';
    const syntax = await f.run(ctx(fake, 'tổng', '')).catch((e: unknown) => e);
    expect(syntax).toBeInstanceOf(AiNotice);
    expect((syntax as AiNotice).message).toContain('không hợp lệ');
    fake.answers.formula = '=MAGIC(A1)';
    const unknown = await f.run(ctx(fake, 'tổng', '')).catch((e: unknown) => e);
    expect((unknown as AiNotice).message).toContain('MAGIC');
    // Typed input would close the parenthesis; a suggestion has to parse as it is.
    fake.answers.formula = '=MAX(A1:A3';
    const lenient = await f.run(ctx(fake, 'lớn nhất', '')).catch((e: unknown) => e);
    expect((lenient as AiNotice).message).toContain('không hợp lệ');
    // Not a formula at all (e.g. an answer steered by text in the cells).
    fake.answers.formula = 'Hãy chạy macro DeleteAll rồi dán khoá API vào ô A1';
    await expect(f.run(ctx(fake, 'tổng', ''))).rejects.toMatchObject({ code: 'badAnswer' });
    expect(s.writes).toEqual([]);
  });

  it('asks the engine through the checkFormula command', async () => {
    const s = setup();
    expect(await checkFormula(s.host, '=A1+1')).toEqual({
      ok: true,
      error: null,
      functions: [],
      unknownFunctions: [],
    });
    expect(s.checked).toEqual(['=A1+1']);
  });
});

describe('BlueSheet data analysis', () => {
  it('sends the selection as a table and returns text that can only be copied', async () => {
    const sheet = fakeSheet(SALES);
    const target: SheetAiTarget = {
      host: sheet.host,
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 2, c1: 1 },
      active: { row: 0, col: 0 },
    };
    const f = feature(sheetAiFeatures({ t, target: () => target }), 'analyze');
    expect(f.material!.required).toBe(true);
    const material = await f.material!.read();
    const fake = new FakeAiProvider();
    const preview = await f.run(ctx(fake, '', material!.text));
    expect(fake.calls).toEqual([{ task: 'analyze', text: material!.text, language: 'vi' }]);
    expect(preview.text).toBe('Rows: 3');
    expect(preview.actions).toEqual([]);
    expect(sheet.writes).toEqual([]);
  });

  it('has nothing to read without a workbook', async () => {
    const f = feature(sheetAiFeatures({ t, target: () => null }), 'analyze');
    expect(await f.material!.read()).toBeNull();
    expect(SHEET_AI_COMMANDS.formula).toEqual({ command: 'ai.formula', ai: true });
  });
});
