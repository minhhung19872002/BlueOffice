// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  OpenedFile,
  Platform,
  QueryDef,
  QueryStep,
  SheetCommand,
  SheetHost,
} from '@blueoffice/platform';
import { taskPaneLabels } from '@blueoffice/editor';
import { TaskPaneHost, emptyTaskPaneLayout } from '@blueoffice/ui';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { StepForm, useSheetQueries } from '../src/queries';
import {
  baseName,
  columnStep,
  errorText,
  guessFolderKind,
  insertStep,
  moveStep,
  pickWebTable,
  refineSource,
  removeStep,
  sourceLabel,
  stepLabel,
  toBase64,
  uniqueName,
} from '../src/queryModel';

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

describe('query model', () => {
  it('names, kinds and the web table to start from', () => {
    expect(baseName('C:\\data\\Doanh số.2024.csv')).toBe('Doanh số.2024');
    expect(baseName('https://example.com/prices.html')).toBe('prices');
    expect(uniqueName('Sales', ['sales', 'Sales (2)'])).toBe('Sales (3)');
    expect(guessFolderKind(['a.csv', 'b.CSV', 'n.md'])).toBe('csv');
    expect(guessFolderKind(['a.xlsx', 'b.ods'])).toBe('workbook');
    expect(guessFolderKind([])).toBe('csv');
    expect(
      pickWebTable([
        { index: 0, caption: 'nav', header: false, rows: 1, cols: 2, preview: [] },
        { index: 1, caption: 'Giá', header: true, rows: 4, cols: 3, preview: [] },
      ]),
    ).toBe(1);
    expect(toBase64(new Uint8Array([104, 105]))).toBe('aGk=');
  });

  it('refines a source with the navigator choice', () => {
    expect(refineSource({ kind: 'json', file: 'a' }, { jsonPaths: ['data.items'] })).toEqual({
      kind: 'json',
      file: 'a',
      path: 'data.items',
    });
    expect(
      refineSource(
        { kind: 'workbook', file: 'b', item: '', itemKind: 'sheet' },
        {
          items: [
            { name: 'Sheet1', kind: 'sheet' },
            { name: 'Bảng1', kind: 'table' },
          ],
        },
      ),
    ).toEqual({ kind: 'workbook', file: 'b', item: 'Bảng1', itemKind: 'table' });
    expect(
      refineSource({ kind: 'csv', file: 'c' }, { csv: { delimiter: ';', encoding: 'utf-8' } }),
    ).toEqual({
      kind: 'csv',
      file: 'c',
      delimiter: ';',
      encoding: 'utf-8',
    });
  });

  it('edits the step list', () => {
    const steps: QueryStep[] = [{ op: 'promoteHeaders' }, { op: 'keepTop', count: 3 }];
    // After the selected step, not at the end.
    const r = insertStep(steps, 0, { op: 'removeBlankRows' });
    expect(r.index).toBe(1);
    expect(r.steps.map((s) => s.op)).toEqual(['promoteHeaders', 'removeBlankRows', 'keepTop']);
    expect(insertStep(steps, -1, { op: 'demoteHeaders' }).steps[0]!.op).toBe('demoteHeaders');
    // Type changes in a row merge (the later type wins).
    const typed = insertStep([{ op: 'changeType', columns: [{ column: 'A', type: 'text' }] }], 0, {
      op: 'changeType',
      columns: [
        { column: 'A', type: 'number' },
        { column: 'B', type: 'date' },
      ],
    });
    expect(typed.steps).toEqual([
      {
        op: 'changeType',
        columns: [
          { column: 'A', type: 'number' },
          { column: 'B', type: 'date' },
        ],
      },
    ]);
    expect(moveStep(steps, 1, -1).map((s) => s.op)).toEqual(['keepTop', 'promoteHeaders']);
    expect(moveStep(steps, 0, -1)).toBe(steps);
    expect(removeStep(steps, 0)).toEqual([{ op: 'keepTop', count: 3 }]);
  });

  it('turns column menu actions into steps and labels them', () => {
    expect(columnStep('sortDesc', 'Giá', ['Giá'])).toEqual({
      op: 'sort',
      keys: [{ column: 'Giá', descending: true }],
    });
    expect(columnStep('type:integer', 'SL', [])).toEqual({
      op: 'changeType',
      columns: [{ column: 'SL', type: 'integer' }],
    });
    expect(columnStep('removeOthers', 'B', ['A', 'B', 'C'])).toEqual({
      op: 'keepColumns',
      columns: ['B'],
    });
    expect(columnStep('unpivotOthers', 'Mã', [])).toEqual({
      op: 'unpivot',
      columns: ['Mã'],
      other: true,
    });
    expect(
      stepLabel({ op: 'filterRows', column: 'Khu vực', condition: 'notEquals', value: 'Trung' }, t),
    ).toBe('Filtered: Khu vực does not equal Trung');
    expect(stepLabel({ op: 'removeDuplicates' }, t)).toBe('Removed duplicates: all columns');
    expect(stepLabel({ op: 'keepTop', count: 5 }, t)).toBe('Kept first 5 rows');
    expect(sourceLabel({ kind: 'csv', file: 'C:/x/sales.csv' }, t)).toBe('Source: CSV sales');
    expect(errorText({ code: 'column', message: 'column: Nope', step: 2 }, t)).toBe(
      'Step 3: There is no column Nope.',
    );
  });
});

describe('StepForm', () => {
  it('builds a filter step and refuses an empty rename', () => {
    const onOk = vi.fn();
    wrap(
      <StepForm
        op="filterRows"
        column="Giá"
        columns={['Mã', 'Giá']}
        others={[]}
        onClose={vi.fn()}
        onOk={onOk}
      />,
    );
    fireEvent.change(screen.getByTestId('query-form-condition'), {
      target: { value: 'greaterThan' },
    });
    fireEvent.change(screen.getByTestId('query-form-value'), { target: { value: '10' } });
    fireEvent.click(screen.getByTestId('query-form-ok'));
    expect(onOk).toHaveBeenCalledWith({
      op: 'filterRows',
      column: 'Giá',
      condition: 'greaterThan',
      value: '10',
    });
    cleanup();
    const ok2 = vi.fn();
    wrap(<StepForm op="renameColumns" columns={['A']} others={[]} onClose={vi.fn()} onOk={ok2} />);
    fireEvent.click(screen.getByTestId('query-form-ok'));
    expect(ok2).not.toHaveBeenCalled();
    expect(screen.getByRole('alert').textContent).toBe('Enter the new name.');
  });
});

/** A fake engine keeping the saved queries; previews return a fixed table. */
function fakeHost() {
  const saved: QueryDef[] = [];
  const calls: SheetCommand[] = [];
  const exec = vi.fn(async (cmd: SheetCommand) => {
    calls.push(cmd);
    switch (cmd.cmd) {
      case 'querySource':
        return { ok: true };
      case 'queryInspect':
        return {
          csv: { encoding: 'utf-8', delimiter: ';' },
          suggested: [{ op: 'promoteHeaders' }],
          preview: { columns: ['Column1'], types: ['any'], rows: [], rowCount: 0 },
        };
      case 'queryPreview':
        return {
          columns: ['Mã', 'Số lượng'],
          types: ['text', 'integer'],
          rows: [
            ['A01', 10],
            ['A02', null],
          ],
          rowCount: 2,
        };
      case 'queryList':
        return {
          queries: saved.map((q) => ({
            name: q.name,
            kind: q.source.kind,
            key: 'file' in q.source ? q.source.file : null,
            access: 'file',
            steps: q.steps.length,
            table: 'Sales',
            sheet: 1,
            sheetName: 'Sales',
            rows: 2,
            available: true,
          })),
        };
      case 'querySave':
        saved.push(cmd.query);
        return { name: cmd.query.name, table: 'Sales', sheet: 1, rows: 2 };
      case 'queryRefresh':
        return { refreshed: [{ name: 'sales', rows: 3 }], errors: [] };
      case 'queryGet':
        return saved[0];
      default:
        return null;
    }
  });
  return { host: { exec } as unknown as SheetHost, calls, saved };
}

function Harness({
  host,
  platform,
  onChanged,
  goToSheet,
}: {
  host: SheetHost;
  platform: Platform;
  onChanged: () => Promise<void>;
  goToSheet: (s: number) => void;
}) {
  const q = useSheetQueries({
    host,
    platform,
    sheet: 0,
    cell: { row: 0, col: 0 },
    onChanged,
    goToSheet,
    fail: (e) => {
      throw e;
    },
  });
  return (
    <>
      <button type="button" data-testid="from-csv" onClick={() => void q.startFrom('csv')} />
      <button type="button" data-testid="refresh-all" onClick={() => void q.refresh()} />
      <TaskPaneHost
        layout={emptyTaskPaneLayout()}
        onLayout={() => {}}
        panes={q.pane ? [q.pane] : []}
        onClose={() => q.closePane()}
        labels={taskPaneLabels(t)}
        focusApp={() => {}}
      />
      {q.element}
    </>
  );
}

describe('Get & Transform flow', () => {
  it('imports a CSV, adds a step from a column menu, loads, lists and refreshes', async () => {
    const { host, calls } = fakeHost();
    const file: OpenedFile = {
      name: 'sales.csv',
      bytes: new TextEncoder().encode('Mã;SL\nA01;10\n'),
    };
    const openFile = vi.fn(async () => file);
    const platform = { files: { openFile, saveFile: vi.fn() } } as unknown as Platform;
    const onChanged = vi.fn(async () => {});
    const goToSheet = vi.fn();
    wrap(<Harness host={host} platform={platform} onChanged={onChanged} goToSheet={goToSheet} />);
    fireEvent.click(screen.getByTestId('from-csv'));
    const editor = await screen.findByTestId('query-editor');
    expect(openFile).toHaveBeenCalledWith(['.csv', '.txt', '.tsv']);
    expect(calls[0]).toMatchObject({ cmd: 'querySource', key: 'sales.csv', access: 'file' });
    expect((calls[0] as { files: { data: string }[] }).files[0]!.data).toBe(toBase64(file.bytes));
    expect((screen.getByTestId('query-name') as HTMLInputElement).value).toBe('sales');
    // The suggested step is listed; the preview shows the engine's table.
    expect(within(editor).getByTestId('query-step-0').textContent).toContain(
      'First row as headers',
    );
    await waitFor(() =>
      expect(screen.getByTestId('query-col-1').textContent).toContain('Số lượng'),
    );
    // Column header menu → sort descending: a new step after the selected one.
    fireEvent.click(screen.getByTestId('query-col-1'));
    fireEvent.click(screen.getByTestId('query-colmenu-sortDesc'));
    expect(within(editor).getByTestId('query-step-1').textContent).toContain('Số lượng ↓');
    // Add a step from the menu: keep top rows (with its form).
    fireEvent.click(screen.getByTestId('query-add-step'));
    fireEvent.click(screen.getByTestId('query-add-keepTop'));
    fireEvent.change(screen.getByTestId('query-form-count'), { target: { value: '5' } });
    fireEvent.click(screen.getByTestId('query-form-ok'));
    expect(within(editor).getByTestId('query-step-2').textContent).toContain('Kept first 5 rows');
    // Move it up, then delete the sort.
    fireEvent.click(screen.getByTestId('query-step-up-2'));
    expect(within(editor).getByTestId('query-step-1').textContent).toContain('Kept first 5 rows');
    fireEvent.click(screen.getByTestId('query-step-delete-2'));
    expect(within(editor).queryByTestId('query-step-2')).toBeNull();
    // Close & Load on a new sheet.
    await act(async () => {
      fireEvent.click(screen.getByTestId('query-load'));
    });
    const save = calls.find((c) => c.cmd === 'querySave') as Extract<
      SheetCommand,
      { cmd: 'querySave' }
    >;
    expect(save.load).toBe('newSheet');
    expect(save.query.source).toEqual({
      kind: 'csv',
      file: 'sales.csv',
      delimiter: ';',
      encoding: 'utf-8',
    });
    expect(save.query.steps).toEqual([{ op: 'promoteHeaders' }, { op: 'keepTop', count: 5 }]);
    await waitFor(() => expect(screen.queryByTestId('query-editor')).toBeNull());
    expect(goToSheet).toHaveBeenCalledWith(1);
    expect(onChanged).toHaveBeenCalled();
    // The Queries pane lists it.
    const pane = await screen.findByTestId('queries-pane');
    expect(within(pane).getByTestId('query-item-0').textContent).toContain('sales');
    expect(within(pane).getByTestId('query-info-0').textContent).toContain('2 rows');

    // Refresh: the browser cannot read the file again → pick it again (or use the copy).
    const edited: OpenedFile = {
      name: 'sales.csv',
      bytes: new TextEncoder().encode('Mã;SL\nA01;11\n'),
    };
    openFile.mockResolvedValueOnce(edited);
    fireEvent.click(screen.getByTestId('query-refresh-0'));
    const reload = await screen.findByTestId('query-reload');
    expect(within(reload).getByText('Using the copy read before')).toBeTruthy();
    fireEvent.click(screen.getByTestId('query-reload-pick-0'));
    await waitFor(() => expect(within(reload).getByText('Picked again')).toBeTruthy());
    const sources = calls.filter((c) => c.cmd === 'querySource');
    expect((sources[1] as { files: { data: string }[] }).files[0]!.data).toBe(
      toBase64(edited.bytes),
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('query-reload-ok'));
    });
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === 'queryRefresh' && c.names?.[0] === 'sales')).toBe(true),
    );
  });

  it('reads desktop paths again without asking and reports refresh errors', async () => {
    const { host, saved, calls } = fakeHost();
    saved.push({ name: 'P', source: { kind: 'csv', file: 'C:\\data\\p.csv' }, steps: [] });
    const openPath = vi.fn(async (path: string) => ({
      name: 'p.csv',
      path,
      bytes: new Uint8Array([1]),
    }));
    const platform = {
      files: { openFile: vi.fn(), saveFile: vi.fn(), openPath },
    } as unknown as Platform;
    const exec = host.exec as unknown as ReturnType<typeof vi.fn>;
    const base = exec.getMockImplementation() as (cmd: SheetCommand) => Promise<unknown>;
    exec.mockImplementation(async (cmd: SheetCommand) =>
      cmd.cmd === 'queryRefresh'
        ? {
            refreshed: [],
            errors: [{ name: 'P', step: 1, code: 'column', message: 'column: X', missing: null }],
          }
        : base(cmd),
    );
    wrap(
      <Harness host={host} platform={platform} onChanged={async () => {}} goToSheet={vi.fn()} />,
    );
    fireEvent.click(screen.getByTestId('refresh-all'));
    const msg = await screen.findByTestId('query-message');
    expect(openPath).toHaveBeenCalledWith('C:\\data\\p.csv');
    expect(calls.find((c) => c.cmd === 'querySource')).toMatchObject({ key: 'C:\\data\\p.csv' });
    expect(msg.textContent).toContain('P: Step 2: There is no column X.');
    expect(screen.queryByTestId('query-reload')).toBeNull();
  });
});
