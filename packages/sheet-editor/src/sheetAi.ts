/**
 * BlueAI in BlueSheet (CORE-AI-001): a formula suggested from a description, and a short
 * textual analysis of the selected data.
 *
 * - **Formula**: the context is the first rows of the selection only (headers + a few sample
 *   rows, `formulaContext`), shown in the pane and sent only when the user runs the feature.
 *   The answer is reduced to one formula line (`extractFormula`) and **parsed by the sheet
 *   engine** (`checkFormula`: syntax + known functions) before it is offered; an answer that
 *   does not pass is dropped. A suggestion is never written by itself: the user presses
 *   "Insert into <cell>", which is an ordinary `setInputs` (one undo step, collaboration and
 *   sheet protection apply as for typing).
 * - **Analyse**: the selection as a tab-separated table (capped) → a short text, which can only
 *   be copied.
 */
import { AiError, extractFormula, type FormulaCheck, type SheetHost } from '@blueoffice/platform';
import { AiNotice, type AiFeature, type AiMaterial } from '@blueoffice/editor';
import { colName, type Range } from './grid';

type T = (key: string, options?: Record<string, unknown>) => string;

/** Rows of the selection sent with a formula request (the first is usually the headers). */
export const AI_FORMULA_ROWS = 6;
/** Columns of the selection sent. */
export const AI_MAX_COLS = 12;
/** Rows of the selection sent for an analysis. */
export const AI_ANALYZE_ROWS = 200;

/** Where the features read from and write to, at the time they run. */
export interface SheetAiTarget {
  host: Pick<SheetHost, 'cell' | 'setInputs' | 'exec'>;
  sheet: number;
  range: Range;
  active: { row: number; col: number };
}

/**
 * Holds the target of the moment for the features, which run at event time; the editor keeps
 * one per mount and refreshes it when the selection changes (like `AiLink`).
 */
export class SheetAiTargetLink {
  private target: SheetAiTarget | null = null;

  set(target: SheetAiTarget | null): void {
    this.target = target;
  }

  get(): SheetAiTarget | null {
    return this.target;
  }
}

const a1 = (row: number, col: number) => `${colName(col)}${row + 1}`;

export function rangeA1(r: Range): string {
  const from = a1(r.r0, r.c0);
  const to = a1(r.r1, r.c1);
  return from === to ? from : `${from}:${to}`;
}

/** Display texts of the top-left `rows` × `cols` of `range`, trailing empty rows dropped. */
async function readCells(
  target: SheetAiTarget,
  rows: number,
  cols: number,
): Promise<{ row: number; cells: { col: number; text: string }[] }[]> {
  const { host, sheet, range } = target;
  const out: { row: number; cells: { col: number; text: string }[] }[] = [];
  const lastRow = Math.min(range.r1, range.r0 + rows - 1);
  const lastCol = Math.min(range.c1, range.c0 + cols - 1);
  for (let row = range.r0; row <= lastRow; row++) {
    const colsOf: number[] = [];
    for (let col = range.c0; col <= lastCol; col++) colsOf.push(col);
    const details = await Promise.all(colsOf.map((col) => host.cell(sheet, row, col)));
    out.push({
      row,
      // One line per cell: tabs and line breaks inside a cell become spaces.
      cells: details.map((d, i) => ({ col: colsOf[i]!, text: d.display.replace(/\s+/g, ' ') })),
    });
  }
  while (out.length && out[out.length - 1]!.cells.every((c) => !c.text.trim())) out.pop();
  return out;
}

/** Context of a formula request: `A1 = value` for the first rows of the selection. */
export async function formulaContext(target: SheetAiTarget): Promise<string> {
  const rows = await readCells(target, AI_FORMULA_ROWS, AI_MAX_COLS);
  const lines = rows
    .map((r) =>
      r.cells
        .filter((c) => c.text.trim())
        .map((c) => `${a1(r.row, c.col)} = ${c.text}`)
        .join('\t'),
    )
    .filter(Boolean);
  if (!lines.length) return '';
  return [`Selection: ${rangeA1(target.range)}`, ...lines].join('\n');
}

/** The selection as a tab-separated table (first row = headers), capped. */
export async function tableText(target: SheetAiTarget): Promise<string> {
  const rows = await readCells(target, AI_ANALYZE_ROWS, AI_MAX_COLS);
  return rows.map((r) => r.cells.map((c) => c.text).join('\t')).join('\n');
}

/** Has the sheet engine parse `formula` (nothing is written or calculated). */
export async function checkFormula(
  host: Pick<SheetHost, 'exec'>,
  formula: string,
): Promise<FormulaCheck> {
  return host.exec<FormulaCheck>({ cmd: 'checkFormula', formula });
}

export interface SheetAiDeps {
  t: T;
  /** The workbook, sheet and selection **now** (the pane stays open while the user moves). */
  target: () => SheetAiTarget | null;
  /** Called after cells were written. */
  changed?: () => void | Promise<void>;
}

export function sheetAiFeatures({ t, target, changed }: SheetAiDeps): AiFeature[] {
  const read =
    (text: (tg: SheetAiTarget) => Promise<string>) => async (): Promise<AiMaterial | null> => {
      const tg = target();
      if (!tg) return null;
      const body = await text(tg);
      return body.trim()
        ? { text: body, label: t('ai.material.cells', { range: rangeA1(tg.range) }) }
        : null;
    };
  return [
    {
      id: 'formula',
      label: t('ai.features.formula'),
      hint: t('ai.hints.formula'),
      ai: true,
      prompt: {
        label: t('ai.prompts.formula'),
        placeholder: t('ai.prompts.formulaPlaceholder'),
        required: true,
        rows: 3,
      },
      material: { read: read(formulaContext), required: false, empty: t('ai.sheet.noContext') },
      run: async ({ prompt, material, ask }) => {
        const tg = target();
        if (!tg) throw new AiError('empty');
        const answer = await ask({ task: 'formula', instruction: prompt, text: material?.text });
        const formula = extractFormula(answer);
        const check = await checkFormula(tg.host, formula);
        if (!check.ok || check.fixed) {
          // Dropped: an unparsable formula or one calling functions the engine does not know.
          // The leniency of typed input (`;`, decimal commas, `)` added) is for people at the
          // keyboard: a suggestion must parse exactly as offered.
          throw new AiNotice(
            check.unknownFunctions.length
              ? t('ai.sheet.unknownFunctions', { names: check.unknownFunctions.join(', ') })
              : t('ai.sheet.invalidFormula'),
          );
        }
        return {
          text: formula,
          note: t('ai.sheet.checked'),
          actions: [
            {
              id: 'insertFormula',
              label: t('ai.actions.insertFormula'),
              apply: async () => {
                // Into the cell that is active when the user confirms.
                const now = target();
                if (!now) return;
                const again = await checkFormula(now.host, formula);
                if (!again.ok || again.fixed) throw new AiNotice(t('ai.sheet.invalidFormula'));
                const { row, col } = now.active;
                await now.host.setInputs(now.sheet, [[row, col, formula]]);
                await changed?.();
                return t('ai.sheet.inserted', { cell: a1(row, col) });
              },
            },
          ],
        };
      },
    },
    {
      id: 'analyze',
      label: t('ai.features.analyze'),
      hint: t('ai.hints.analyze'),
      ai: true,
      material: { read: read(tableText), required: true, empty: t('ai.sheet.selectData') },
      run: async ({ material, language, ask }) => ({
        text: await ask({ task: 'analyze', text: material?.text ?? '', language }),
        actions: [],
      }),
    },
  ];
}

/** Feature ids → commands of BlueSheet. */
export const SHEET_AI_COMMANDS = {
  formula: { command: 'ai.formula', ai: true },
  analyze: { command: 'ai.analyze', ai: true },
} as const;
