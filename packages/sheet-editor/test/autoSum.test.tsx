// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { AutoSumPlan, SheetCommand } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  AUTO_SUM_COMMANDS,
  AUTO_SUM_FUNCTIONS,
  InsertFunctionDialog,
  autoSumCommand,
  filterFunctions,
} from '../src/autoSum';

afterEach(cleanup);

const i18n = createI18n('en-US');

describe('auto total commands', () => {
  it('has one command per function, Σ being the sum', () => {
    expect(AUTO_SUM_FUNCTIONS).toEqual(['sum', 'average', 'count', 'max', 'min']);
    expect(AUTO_SUM_COMMANDS.sum).toBe('formula.autoSum');
    expect(new Set(Object.values(AUTO_SUM_COMMANDS)).size).toBe(5);
  });

  it('writes the totals of a plan and their number formats as one batch', () => {
    // The plan of C2:C4 (12.5, 25, 37.5 shown with two decimals): the total goes into C5.
    const plan: AutoSumPlan = {
      mode: 'write',
      cells: [[4, 2, '=SUM(C2:C4)']],
      formats: [{ row: 4, col: 2, numFmt: '#,##0.00' }],
      select: { r0: 1, c0: 2, r1: 4, c1: 2 },
    };
    expect(autoSumCommand(plan, 0, 'Total')).toEqual({
      cmd: 'batch',
      cmds: [
        { cmd: 'setInputs', sheet: 0, cells: [[4, 2, '=SUM(C2:C4)']] },
        {
          cmd: 'format',
          sheet: 0,
          range: { r0: 4, c0: 2, r1: 4, c1: 2 },
          patch: { numFmt: '#,##0.00' },
        },
      ],
    });
  });

  it('shows the totals row of a table and sets the function of its columns', () => {
    const plan: AutoSumPlan = {
      mode: 'table',
      table: 0,
      show: true,
      columns: [1],
      clear: [2],
      function: 'sum',
      select: { r0: 1, c0: 1, r1: 4, c1: 1 },
    };
    expect(autoSumCommand(plan, 3, 'Total')).toEqual({
      cmd: 'batch',
      cmds: [
        { cmd: 'tableUpdate', sheet: 3, table: 0, totalsRow: true, totalsLabel: 'Total' },
        { cmd: 'tableTotal', sheet: 3, table: 0, column: 2, function: 'none' },
        { cmd: 'tableTotal', sheet: 3, table: 0, column: 1, function: 'sum' },
      ],
    });
    // The totals row is already there: only the function changes.
    expect(autoSumCommand({ ...plan, show: false, clear: [] }, 3, 'Total')).toEqual({
      cmd: 'batch',
      cmds: [{ cmd: 'tableTotal', sheet: 3, table: 0, column: 1, function: 'sum' }],
    });
  });

  it('runs nothing for a proposal or an empty plan', () => {
    expect(
      autoSumCommand(
        { mode: 'edit', row: 4, col: 2, text: '=SUM(C2:C4)', selStart: 5, selEnd: 10 },
        0,
        'Total',
      ),
    ).toBeNull();
    expect(autoSumCommand({ mode: 'none' }, 0, 'Total')).toBeNull();
  });
});

describe('More functions', () => {
  const names = ['AVERAGE', 'COUNT', 'MAX', 'SUM', 'SUMIF', 'SUBTOTAL', 'DSUM'];

  it('lists names starting with what was typed before those containing it', () => {
    expect(filterFunctions(names, '')).toEqual(names);
    expect(filterFunctions(names, 'sum')).toEqual(['SUM', 'SUMIF', 'DSUM']);
    expect(filterFunctions(names, 'xyz')).toEqual([]);
  });

  it('inserts the function picked from the engine list', async () => {
    const query = vi.fn((c: SheetCommand) =>
      Promise.resolve(c.cmd === 'functionList' ? { names } : null),
    ) as unknown as <T>(c: SheetCommand) => Promise<T | null>;
    const onInsert = vi.fn();
    const onClose = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <InsertFunctionDialog query={query} onInsert={onInsert} onClose={onClose} />
      </I18nextProvider>,
    );
    const list = await screen.findByTestId('insert-function-list');
    await waitFor(() => expect(list.querySelectorAll('[data-function]').length).toBe(7));
    const search = screen.getByTestId('insert-function-search');
    fireEvent.change(search, { target: { value: 'sub' } });
    expect([...list.querySelectorAll('[data-function]')].map((b) => b.textContent)).toEqual([
      'SUBTOTAL',
    ]);
    fireEvent.keyDown(search, { key: 'Enter' });
    expect(onInsert).toHaveBeenCalledWith('SUBTOTAL');
    expect(onClose).toHaveBeenCalled();
  });

  it('says so when nothing matches', async () => {
    const query = vi.fn(() => Promise.resolve({ names })) as unknown as <T>(
      c: SheetCommand,
    ) => Promise<T | null>;
    render(
      <I18nextProvider i18n={i18n}>
        <InsertFunctionDialog query={query} onInsert={vi.fn()} onClose={vi.fn()} />
      </I18nextProvider>,
    );
    await screen.findByText('SUMIF');
    fireEvent.change(screen.getByTestId('insert-function-search'), { target: { value: 'zz' } });
    expect(screen.getByText('No function matches.')).toBeTruthy();
    expect((screen.getByTestId('insert-function-ok') as HTMLButtonElement).disabled).toBe(true);
  });
});
