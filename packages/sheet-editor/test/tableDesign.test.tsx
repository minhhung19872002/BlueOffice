// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SheetCommand, TableInfo, TableList } from '@blueoffice/platform';
import {
  TABLE_STYLE_NAMES,
  TableAutoBar,
  TableResizeDialog,
  TableTotalsDialog,
  parseTableRange,
  registerTableCommands,
  styleLabel,
  tableAt,
  totalsCommands,
  type TableState,
} from '../src/tableDesign';

afterEach(cleanup);

const i18n = createI18n('en-US');

const table = (over: Partial<TableInfo> = {}): TableInfo => ({
  index: 0,
  id: 1,
  name: 'Sales',
  range: { r0: 0, c0: 0, r1: 4, c1: 1 },
  headerRow: true,
  totalsRow: true,
  filterButton: true,
  style: 'TableStyleMedium2',
  bandedRows: true,
  bandedColumns: false,
  firstColumn: false,
  lastColumn: false,
  columns: [
    {
      name: 'Item',
      totalsFunction: 'none',
      totalsFormula: null,
      totalsLabel: 'Total',
      calculated: null,
    },
    {
      name: 'Price',
      totalsFunction: 'sum',
      totalsFormula: null,
      totalsLabel: null,
      calculated: null,
    },
  ],
  ...over,
});

const list = (over: Partial<TableList> = {}): TableList => ({
  tables: [table()],
  autoExpand: true,
  autoCalc: true,
  lastAuto: null,
  ...over,
});

function wrap(ui: React.ReactElement) {
  return render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);
}

describe('helpers', () => {
  it('finds the table of a cell', () => {
    expect(tableAt(list(), 2, 1)?.name).toBe('Sales');
    expect(tableAt(list(), 5, 1)).toBeNull();
    expect(tableAt(null, 0, 0)).toBeNull();
  });

  it('names styles with the own palette names', () => {
    const t = i18n.t.bind(i18n);
    expect(styleLabel(t, 'TableStyleMedium2')).toBe('Medium · Blue');
    // 9 → (9 − 1) mod 7 + 1 = accent 2.
    expect(styleLabel(t, 'TableStyleLight9')).toBe('Soft · Blue');
    expect(styleLabel(t, 'TableStyleDark7')).toBe('Strong · Brick');
    expect(styleLabel(t, null)).toBe('No colors');
    expect(styleLabel(t, 'MyStyle')).toBe('MyStyle');
    expect(TABLE_STYLE_NAMES).toHaveLength(21);
    expect(TABLE_STYLE_NAMES[8]).toBe('TableStyleMedium2');
  });

  it('parses a range', () => {
    expect(parseTableRange('$A$1:$C$9')).toEqual({ r0: 0, c0: 0, r1: 8, c1: 2 });
    expect(parseTableRange('nope')).toBeNull();
  });

  it('builds totals commands for changed columns only', () => {
    const cmds = totalsCommands(0, table(), [
      { function: 'none', formula: '' },
      { function: 'custom', formula: '=SUM(Sales[Price])*2' },
    ]);
    expect(cmds).toEqual([
      {
        cmd: 'tableTotal',
        sheet: 0,
        table: 0,
        column: 1,
        function: 'custom',
        formula: 'SUM(Sales[Price])*2',
      },
    ]);
    expect(
      totalsCommands(0, table(), [
        { function: 'none', formula: '' },
        { function: 'sum', formula: '' },
      ]),
    ).toEqual([]);
  });
});

describe('commands', () => {
  type Cmd = {
    run: (s: TableState) => Promise<unknown>;
    isEnabled?: (s: TableState) => boolean;
    isChecked?: (s: TableState) => boolean;
  };
  const setup = () => {
    const cmds = new Map<string, Cmd>();
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    const opened: string[] = [];
    registerTableCommands<TableState>(
      (id, run, extra) => {
        cmds.set(id, { run, ...extra });
      },
      {
        exec,
        totalLabel: () => 'Total',
        openRename: () => opened.push('rename'),
        openResize: () => opened.push('resize'),
        openTotals: () => opened.push('totals'),
        openDedupe: (r) => opened.push(`dedupe:${r.r0},${r.c0},${r.r1},${r.c1}`),
      },
    );
    return { cmds, exec, opened };
  };
  const state = (t: TableInfo | null): TableState => ({
    sheet: 2,
    hasWorkbook: true,
    table: t,
    tableAuto: { autoExpand: true, autoCalc: false },
    selection: { active: { row: 1, col: 1 } },
  });

  it('toggles style options and the totals row with its label', async () => {
    const { cmds, exec } = setup();
    const s = state(table({ totalsRow: false }));
    expect(cmds.get('tableDesign.totalsRow')?.isChecked?.(s)).toBe(false);
    await cmds.get('tableDesign.totalsRow')?.run(s);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'tableUpdate',
      sheet: 2,
      table: 0,
      totalsRow: true,
      totalsLabel: 'Total',
    });
    await cmds.get('tableDesign.bandedRows')?.run(s);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'tableUpdate',
      sheet: 2,
      table: 0,
      bandedRows: false,
    });
    await cmds.get('tableDesign.style.TableStyleDark4')?.run(s);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'tableUpdate',
      sheet: 2,
      table: 0,
      style: 'TableStyleDark4',
    });
    expect(cmds.get('tableDesign.style.TableStyleMedium2')?.isChecked?.(s)).toBe(true);
    await cmds.get('tableDesign.styleNone')?.run(s);
    expect(exec).toHaveBeenLastCalledWith({ cmd: 'tableUpdate', sheet: 2, table: 0, style: '' });
  });

  it('is disabled outside a table and runs the tools', async () => {
    const { cmds, exec, opened } = setup();
    expect(cmds.get('tableDesign.convert')?.isEnabled?.(state(null))).toBe(false);
    const s = state(table());
    await cmds.get('tableDesign.convert')?.run(s);
    expect(exec).toHaveBeenLastCalledWith({ cmd: 'tableConvert', sheet: 2, table: 0 });
    await cmds.get('tableDesign.slicer')?.run(s);
    expect(exec).toHaveBeenLastCalledWith({ cmd: 'insertSlicer', sheet: 2, row: 1, col: 1 });
    await cmds.get('tableDesign.removeDuplicates')?.run(s);
    await cmds.get('tableDesign.rename')?.run(s);
    await cmds.get('tableDesign.totals')?.run(s);
    expect(opened).toEqual(['dedupe:0,0,4,1', 'rename', 'totals']);
    // Automatic options flip the current value.
    expect(cmds.get('tableDesign.autoCalc')?.isChecked?.(s)).toBe(false);
    await cmds.get('tableDesign.autoCalc')?.run(s);
    expect(exec).toHaveBeenLastCalledWith({ cmd: 'tableOptions', autoCalc: true });
    // Totals functions need a totals row; filter buttons need a header row.
    expect(cmds.get('tableDesign.totals')?.isEnabled?.(state(table({ totalsRow: false })))).toBe(
      false,
    );
    expect(
      cmds.get('tableDesign.filterButton')?.isEnabled?.(state(table({ headerRow: false }))),
    ).toBe(false);
  });
});

describe('dialogs', () => {
  it('applies totals in one batch', async () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    const onClose = vi.fn();
    wrap(<TableTotalsDialog sheet={0} table={table()} exec={exec} onClose={onClose} />);
    fireEvent.change(screen.getByTestId('table-total-0'), { target: { value: 'count' } });
    fireEvent.change(screen.getByTestId('table-total-1'), { target: { value: 'average' } });
    fireEvent.click(screen.getByTestId('table-totals-ok'));
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(exec).toHaveBeenCalledWith({
      cmd: 'batch',
      cmds: [
        { cmd: 'tableTotal', sheet: 0, table: 0, column: 0, function: 'count' },
        { cmd: 'tableTotal', sheet: 0, table: 0, column: 1, function: 'average' },
      ],
    });
  });

  it('asks for a custom formula', () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    wrap(<TableTotalsDialog sheet={0} table={table()} exec={exec} onClose={() => {}} />);
    fireEvent.change(screen.getByTestId('table-total-1'), { target: { value: 'custom' } });
    expect(screen.getByTestId('table-total-formula-1')).toBeTruthy();
    fireEvent.click(screen.getByTestId('table-totals-ok'));
    expect(screen.getByRole('alert').textContent).toContain('formula');
    expect(exec).not.toHaveBeenCalled();
  });

  it('resizes keeping the header row', async () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    const onClose = vi.fn();
    wrap(<TableResizeDialog sheet={1} table={table()} exec={exec} onClose={onClose} />);
    const input = screen.getByTestId('table-resize-input') as HTMLInputElement;
    expect(input.value).toBe('A1:B5');
    fireEvent.change(input, { target: { value: 'A2:B9' } });
    fireEvent.click(screen.getByTestId('table-resize-ok'));
    expect(screen.getByRole('alert')).toBeTruthy();
    fireEvent.change(input, { target: { value: 'A1:C9' } });
    fireEvent.click(screen.getByTestId('table-resize-ok'));
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(exec).toHaveBeenCalledWith({
      cmd: 'tableResize',
      sheet: 1,
      table: 0,
      range: { r0: 0, c0: 0, r1: 8, c1: 2 },
    });
  });

  it('offers to undo an automatic expansion', () => {
    const run = vi.fn();
    const { rerender } = wrap(<TableAutoBar last={{ kind: 'expand', seq: 1 }} run={run} />);
    fireEvent.click(screen.getByTestId('table-auto-undo'));
    expect(run).toHaveBeenCalledWith('edit.undo');
    expect(screen.queryByTestId('table-auto-bar')).toBeNull();
    rerender(
      <I18nextProvider i18n={i18n}>
        <TableAutoBar last={{ kind: 'calc', seq: 2 }} run={run} />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('table-auto-stop'));
    expect(run).toHaveBeenLastCalledWith('tableDesign.autoCalc');
  });
});
