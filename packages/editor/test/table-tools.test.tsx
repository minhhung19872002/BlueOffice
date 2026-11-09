// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult, TableState } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ConvertDialog,
  defaultFormula,
  FormulaDialog,
  formulaInstr,
  separatorOf,
  SortDialog,
  sortKeys,
} from '../src/TableToolDialogs';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const table = (patch: Partial<TableState> = {}): TableState => ({
  rows: 4,
  cols: 3,
  row: 1,
  col: 2,
  headerRow: false,
  shading: null,
  align: 'Left',
  widthPct: null,
  width: 0,
  multiCell: false,
  merged: false,
  direction: null,
  wrap: false,
  fixedLayout: false,
  ...patch,
});

/** A `run` that records the commands and answers with `error` (if any). */
const recorder = (error?: string) => {
  const commands: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    commands.push(c);
    return (error ? { error } : {}) as EditResult;
  });
  return { commands, run };
};

describe('table tool helpers', () => {
  it('builds formula field instructions', () => {
    expect(formulaInstr('=SUM(ABOVE)', '')).toBe('=SUM(ABOVE)');
    expect(formulaInstr(' = AVERAGE(B2:B5) ', '#,##0.00')).toBe('=AVERAGE(B2:B5) \\# "#,##0.00"');
    // Literal text in a picture uses apostrophes.
    expect(formulaInstr('SUM(LEFT)', '#,##0 "đ"')).toBe('=SUM(LEFT) \\# "#,##0 \'đ\'"');
    expect(defaultFormula(table({ row: 0, col: 2 }))).toBe('=SUM(LEFT)');
    expect(defaultFormula(table())).toBe('=SUM(ABOVE)');
  });

  it('maps separators and sort levels', () => {
    expect(separatorOf('tab', 'x')).toBe('\t');
    expect(separatorOf('comma', 'x')).toBe(',');
    expect(separatorOf('paragraph', 'x')).toBe('\n');
    expect(separatorOf('other', ';')).toBe(';');
    expect(
      sortKeys([
        { column: 1, kind: 'number', descending: true },
        { column: -1, kind: 'text', descending: false },
        { column: 0, kind: 'date', descending: false },
      ]),
    ).toEqual([
      { column: 1, kind: 'number', descending: true },
      { column: 0, kind: 'date', descending: false },
    ]);
  });
});

describe('sort dialog', () => {
  it('sends the chosen keys and header option', async () => {
    const { commands, run } = recorder();
    const onClose = vi.fn();
    wrap(<SortDialog table={table()} run={run} onClose={onClose} />);
    // The first key starts on the caret's column (the third).
    expect((screen.getByTestId('sort-column-0') as HTMLSelectElement).value).toBe('2');
    fireEvent.change(screen.getByTestId('sort-kind-0'), { target: { value: 'number' } });
    fireEvent.change(screen.getByTestId('sort-order-0'), { target: { value: 'desc' } });
    fireEvent.change(screen.getByTestId('sort-column-1'), { target: { value: '0' } });
    fireEvent.click(screen.getByTestId('sort-header'));
    fireEvent.click(screen.getByTestId('table-sort-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(commands).toEqual([
      {
        cmd: 'tableEdit',
        edit: {
          kind: 'sort',
          header: true,
          keys: [
            { column: 2, kind: 'number', descending: true },
            { column: 0, kind: 'text', descending: false },
          ],
        },
      },
    ]);
  });

  it('stays open and shows the engine error', async () => {
    const { run } = recorder('rows with vertically merged cells cannot be sorted');
    const onClose = vi.fn();
    wrap(<SortDialog table={table()} run={run} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('table-sort-ok'));
    expect(await screen.findByTestId('sort-error')).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('formula dialog', () => {
  it('inserts the formula as a field with the number format', async () => {
    const { commands, run } = recorder();
    const onClose = vi.fn();
    wrap(<FormulaDialog table={table()} run={run} onClose={onClose} />);
    expect((screen.getByTestId('formula-input') as HTMLInputElement).value).toBe('=SUM(ABOVE)');
    fireEvent.change(screen.getByTestId('formula-input'), { target: { value: '=' } });
    fireEvent.change(screen.getByTestId('formula-function'), { target: { value: 'MAX' } });
    expect((screen.getByTestId('formula-input') as HTMLInputElement).value).toBe('=MAX()');
    fireEvent.change(screen.getByTestId('formula-input'), { target: { value: '=MAX(B1:B3)' } });
    fireEvent.change(screen.getByTestId('formula-format'), { target: { value: '#,##0' } });
    fireEvent.click(screen.getByTestId('table-formula-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(commands).toEqual([{ cmd: 'insertField', instr: '=MAX(B1:B3) \\# "#,##0"' }]);
  });
});

describe('convert dialog', () => {
  it('turns paragraphs into a table with a column count', async () => {
    const { commands, run } = recorder();
    const onClose = vi.fn();
    wrap(<ConvertDialog toTable run={run} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('sep-paragraph'));
    fireEvent.change(screen.getByTestId('convert-cols'), { target: { value: '3' } });
    fireEvent.click(screen.getByTestId('table-convert-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(commands).toEqual([
      { cmd: 'tableEdit', edit: { kind: 'fromText', separator: '\n', cols: 3 } },
    ]);
  });

  it('turns a table into text split by another character', async () => {
    const { commands, run } = recorder();
    const onClose = vi.fn();
    wrap(<ConvertDialog toTable={false} run={run} onClose={onClose} />);
    expect(screen.queryByTestId('convert-cols')).toBeNull();
    fireEvent.click(screen.getByTestId('sep-other'));
    fireEvent.change(screen.getByTestId('sep-other-text'), { target: { value: ';' } });
    fireEvent.click(screen.getByTestId('table-convert-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(commands).toEqual([{ cmd: 'tableEdit', edit: { kind: 'toText', separator: ';' } }]);
  });
});
