// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  PivotInfo,
  Platform,
  SheetCommand,
  SheetHost,
  WorkbookInfo,
} from '@blueoffice/platform';
import { BlueSheetEditor } from '../src';

afterEach(cleanup);

function fakeHost() {
  const cells = new Map<string, string>();
  let wb: WorkbookInfo = {
    name: 'Book.xlsx',
    sheets: [
      {
        name: 'Sheet1',
        lastRow: null,
        lastCol: null,
        frozenRows: 0,
        frozenCols: 0,
        hidden: false,
        hiddenRows: [],
        hiddenCols: [],
        autoFilter: null,
        merges: [],
        charts: 0,
        pivots: 0,
      },
    ],
  };
  let areas: Pick<PivotInfo, 'rows' | 'cols' | 'values' | 'filters'> = {
    rows: [],
    cols: [],
    values: [],
    filters: [],
  };
  const host: SheetHost = {
    init: async () => {},
    open: async () => wb,
    newWorkbook: async () => wb,
    info: async () => wb,
    render: async (v) => ({ width: v.width, height: v.height, commands: [] }),
    hitTest: async () => ({ kind: 'cell', row: 2, col: 1 }),
    rangeRect: async (_v, r0, c0) => ({ x: 30 + c0 * 48, y: 15 + r0 * 15, w: 48, h: 15 }),
    contentSize: async () => [2000, 3000],
    cell: async (_s, row, col) => {
      const v = cells.get(`${row},${col}`) ?? '';
      return { address: `${String.fromCharCode(65 + col)}${row + 1}`, input: v, display: v };
    },
    setInputs: vi.fn(async (_s, list: [number, number, string][]) => {
      for (const [r, c, v] of list) cells.set(`${r},${c}`, v);
    }),
    structural: vi.fn(async () => {}),
    undo: vi.fn(async () => true),
    redo: vi.fn(async () => false),
    addSheet: vi.fn(async () => {
      wb = {
        ...wb,
        sheets: [...wb.sheets, { ...wb.sheets[0]!, name: 'Sheet2' }],
      };
      return 1;
    }),
    renameSheet: vi.fn(async () => {}),
    deleteSheet: vi.fn(async () => {}),
    save: vi.fn(async () => new Uint8Array([80, 75])),
    exportPdf: async () => new Uint8Array([37]),
    getGlyphOutlines: async () => [],
    exec: vi.fn(async (cmd: SheetCommand) => {
      switch (cmd.cmd) {
        case 'presets':
          return {
            numberFormats: [
              { id: 'general', code: 'General' },
              { id: 'currencyVnd', code: '#,##0 [$₫-42A]' },
            ],
            cellStyles: ['Normal', 'Good'],
          };
        case 'formatState':
          return {
            numFmt: 'General',
            fontName: 'Calibri',
            fontSize: 11,
            bold: false,
            italic: false,
            underline: '',
            strike: false,
            fontColor: '',
            fillColor: '',
            hAlign: '',
            vAlign: '',
            wrap: false,
            indent: 0,
            rotation: 0,
            borders: ['', '', '', ''],
            styleName: 'Normal',
            merged: false,
          };
        case 'validation':
          return { list: null };
        case 'copy': {
          const r = cmd.range;
          const rows: string[] = [];
          for (let row = r.r0; row <= r.r1; row++) {
            const line: string[] = [];
            for (let col = r.c0; col <= r.c1; col++) line.push(cells.get(`${row},${col}`) ?? '');
            rows.push(line.join('\t'));
          }
          return { text: rows.join('\r\n') };
        }
        case 'paste':
          return null;
        case 'names':
        case 'charts':
          return [];
        case 'pivotSource':
          return {
            range: { r0: 0, c0: 0, r1: 4, c1: 1 },
            fields: ['Vùng', 'Doanh thu'],
            records: 4,
          };
        case 'pivotCreate':
          wb = { ...wb, sheets: [{ ...wb.sheets[0]!, pivots: 1 }] };
          return { sheet: 0, index: 0 };
        case 'pivots':
          return wb.sheets[0]!.pivots
            ? [
                {
                  index: 0,
                  name: 'Pivot1',
                  location: { r0: 2, c0: 3, r1: 2, c1: 3 },
                  area: { r0: 2, c0: 3, r1: 2, c1: 3 },
                  source: 'Sheet1!A1:B5',
                  fields: [
                    {
                      name: 'Vùng',
                      items: ['Bắc', 'Nam'],
                      formula: null,
                      derived: false,
                      date: false,
                      group: null,
                    },
                    {
                      name: 'Doanh thu',
                      items: ['1', '2.5'],
                      formula: null,
                      derived: false,
                      date: false,
                      group: null,
                    },
                  ],
                  layout: 'tabular',
                  rowGrandTotals: true,
                  colGrandTotals: true,
                  fieldOptions: [],
                  fieldFilters: [],
                  style: null,
                  ...areas,
                },
              ]
            : [];
        case 'pivotUpdate':
          areas = {
            rows: cmd.rows,
            cols: cmd.cols,
            values: cmd.values.map((v) => ({
              field: v.field,
              function: v.function ?? 'sum',
              name: v.name ?? null,
              showAs: v.showAs ?? 'normal',
              baseField: v.baseField ?? 0,
              baseItem: v.baseItem ?? 0,
              numFmt: v.numFmt ?? null,
            })),
            filters: cmd.filters.map((f) => ({ field: f.field, items: f.items ?? [] })),
          };
          return { ok: true };
        default:
          return { ok: true };
      }
    }) as SheetHost['exec'],
  };
  return { host, cells };
}

function platform(host: SheetHost) {
  const saved: string[] = [];
  const clip = { text: '' };
  const p = {
    kind: 'web',
    files: {
      openFile: async () => null,
      saveFile: async (_b: Uint8Array, n: string) => {
        saved.push(n);
        return n;
      },
    },
    clipboard: {
      readText: async () => clip.text,
      writeText: async (t: string) => {
        clip.text = t;
      },
      writeHtml: async () => {},
    },
    print: { printPdf: async () => {} },
    window: { setTitle: vi.fn(), openNew: async () => {} },
    shell: {
      recentFiles: async () => [],
      addRecentFile: async () => {},
      onOpenRequest: () => () => {},
    },
    sheet: host,
  } as unknown as Platform;
  return { p, saved, clip };
}

// happy-dom has no canvas 2D context / ResizeObserver; the editor tolerates both.
globalThis.ResizeObserver ??= class {
  observe() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

describe('BlueSheetEditor', () => {
  it('warns about macros in an opened .xlsm and dismisses the bar', async () => {
    const { host } = fakeHost();
    const { p } = platform(host);
    let openHandler: ((f: { name: string; bytes: Uint8Array }) => void) | undefined;
    (p.shell as { onOpenRequest: unknown }).onOpenRequest = (
      h: (f: { name: string; bytes: Uint8Array }) => void,
    ) => {
      openHandler = h;
      return () => {};
    };
    const base = await host.info();
    host.open = async () => ({ ...base, name: 'Có macro.xlsm', macros: true });
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    expect(screen.queryByTestId('macro-warning')).toBeNull();
    act(() => openHandler?.({ name: 'Có macro.xlsm', bytes: new Uint8Array([80, 75]) }));
    const bar = await screen.findByTestId('macro-warning');
    expect(bar.textContent).toContain('macros');
    fireEvent.click(bar.querySelector('button')!);
    await waitFor(() => expect(screen.queryByTestId('macro-warning')).toBeNull());
  });

  it('types Vietnamese text into the active cell and commits on Enter', async () => {
    const { host, cells } = fakeHost();
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const input = screen.getByLabelText('Ô đang nhập') as HTMLInputElement;
    fireEvent.input(input, { target: { value: 'Hà Nội' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(cells.get('0,0')).toBe('Hà Nội'));
    // Enter moved to A2.
    await waitFor(() =>
      expect((screen.getByLabelText('Hộp tên') as HTMLInputElement).value).toBe('A2'),
    );
  });

  it('IME composition does not commit early', async () => {
    const { host } = fakeHost();
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const input = screen.getByLabelText('Ô đang nhập') as HTMLInputElement;
    fireEvent.compositionStart(input);
    fireEvent.keyDown(input, { key: 'Enter', isComposing: true });
    expect(host.setInputs).not.toHaveBeenCalled();
    fireEvent.compositionEnd(input, { target: { value: 'Việt' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(host.setInputs).toHaveBeenCalledWith(0, [[0, 0, 'Việt']]));
  });

  it('runs ribbon commands and shortcuts', async () => {
    const { host } = fakeHost();
    const { p, saved } = platform(host);
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    fireEvent.click(screen.getByText('Insert row'));
    await waitFor(() => expect(host.structural).toHaveBeenCalledWith(0, true, 0, 1, false));
    const input = screen.getByLabelText('Cell editor');
    fireEvent.keyDown(input, { key: 'z', ctrlKey: true });
    await waitFor(() => expect(host.undo).toHaveBeenCalled());
    fireEvent.keyDown(input, { key: 's', ctrlKey: true });
    await waitFor(() => expect(saved[0]).toMatch(/\.xlsx$/));
    fireEvent.click(screen.getByLabelText('Add sheet'));
    await screen.findByRole('tab', { name: 'Sheet2' });
  });

  it('formats cells from the ribbon, shortcuts and the Format Cells dialog', async () => {
    const { host } = fakeHost();
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const input = screen.getByLabelText('Cell editor');
    fireEvent.keyDown(input, { key: 'b', ctrlKey: true });
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith({
        cmd: 'format',
        sheet: 0,
        range: { r0: 0, c0: 0, r1: 0, c1: 0 },
        patch: { bold: true },
      }),
    );
    fireEvent.click(screen.getByText('Merge & center'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({ cmd: 'merge', center: true }),
      ),
    );
    fireEvent.keyDown(input, { key: '1', ctrlKey: true });
    await screen.findByTestId('format-cells');
    fireEvent.click(screen.getByRole('tab', { name: 'Fill' }));
    fireEvent.change(screen.getByTestId('fill-color-input'), { target: { value: '#ffee00' } });
    fireEvent.click(screen.getByTestId('format-cells-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({ cmd: 'format', patch: { fillColor: 'FFEE00' } }),
      ),
    );
    // Fill down and freeze panes are registry commands too.
    fireEvent.keyDown(input, { key: 'd', ctrlKey: true });
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({ cmd: 'fillDir', dir: 'down' }),
      ),
    );
    fireEvent.keyDown(input, { key: 'L', ctrlKey: true, shiftKey: true });
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(expect.objectContaining({ cmd: 'toggleFilter' })),
    );
  });

  it('opens find, go to and sort dialogs', async () => {
    const { host } = fakeHost();
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const input = screen.getByLabelText('Ô đang nhập');
    fireEvent.keyDown(input, { key: 'f', ctrlKey: true });
    await screen.findByTestId('find-dialog');
    fireEvent.keyDown(screen.getByTestId('find-query'), { key: 'Escape' });
    fireEvent.keyDown(input, { key: 'F5' });
    await screen.findByTestId('goto-dialog');
  });

  it('copies and pastes as TSV', async () => {
    const { host, cells } = fakeHost();
    const { p, clip } = platform(host);
    cells.set('0,0', '1,5');
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const input = screen.getByLabelText('Ô đang nhập');
    fireEvent.keyDown(input, { key: 'c', ctrlKey: true });
    await waitFor(() => expect(clip.text).toBe('1,5'));
    clip.text = 'a\tb\r\nc\td';
    fireEvent.keyDown(input, { key: 'v', ctrlKey: true });
    await waitFor(() =>
      expect(host.setInputs).toHaveBeenCalledWith(0, [
        [0, 0, 'a'],
        [0, 1, 'b'],
        [1, 0, 'c'],
        [1, 1, 'd'],
      ]),
    );
  });

  it('creates a pivot table and edits its areas from the field list', async () => {
    const { host } = fakeHost();
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Insert' }), { button: 0 });
    fireEvent.click(screen.getByText('Pivot table'));
    await screen.findByTestId('pivot-dialog');
    await waitFor(() =>
      expect((screen.getByTestId('pivot-source') as HTMLInputElement).value).toBe('A1:B5'),
    );
    fireEvent.click(await screen.findByTestId('pivot-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: 'pivotCreate',
          range: { r0: 0, c0: 0, r1: 4, c1: 1 },
          dest: null,
          labels: expect.objectContaining({ grandTotal: 'Grand Total' }),
        }),
      ),
    );
    // The selection moves into the new pivot table: its field list opens.
    await screen.findByTestId('pivot-panel');
    fireEvent.click(await screen.findByTestId('pivot-field-0'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({ cmd: 'pivotUpdate', index: 0, rows: [0], values: [] }),
      ),
    );
    fireEvent.click(await screen.findByTestId('pivot-field-1'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: 'pivotUpdate',
          rows: [0],
          values: [{ field: 1, function: 'sum', name: 'Sum of Doanh thu' }],
        }),
      ),
    );
    fireEvent.change(await screen.findByTestId('pivot-fn-0'), { target: { value: 'average' } });
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: 'pivotUpdate',
          values: [
            expect.objectContaining({
              field: 1,
              function: 'average',
              name: 'Average of Doanh thu',
            }),
          ],
        }),
      ),
    );
    fireEvent.click(screen.getByTestId('pivot-refresh'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(expect.objectContaining({ cmd: 'pivotRefresh' })),
    );
    // Value field settings: show values as % of the grand total, number format.
    fireEvent.click(await screen.findByTestId('pivot-settings-values-0'));
    await screen.findByTestId('pivot-value-dialog');
    fireEvent.change(screen.getByTestId('pivot-show-as'), { target: { value: 'percentOfTotal' } });
    fireEvent.change(screen.getByTestId('pivot-num-fmt'), { target: { value: '0.00%' } });
    fireEvent.click(screen.getByTestId('pivot-value-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: 'pivotUpdate',
          values: [
            expect.objectContaining({ field: 1, showAs: 'percentOfTotal', numFmt: '0.00%' }),
          ],
        }),
      ),
    );
    // Row field settings: no subtotals, sort descending, group by months.
    fireEvent.click(await screen.findByTestId('pivot-settings-rows-0'));
    await screen.findByTestId('pivot-field-dialog');
    fireEvent.click(screen.getByTestId('pivot-subtotal-none'));
    fireEvent.change(screen.getByTestId('pivot-sort'), { target: { value: 'descending' } });
    fireEvent.change(screen.getByTestId('pivot-group-by'), { target: { value: 'dates' } });
    fireEvent.click(screen.getByTestId('pivot-field-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: 'pivotUpdate',
          fieldOptions: [
            expect.objectContaining({ field: 0, subtotal: false, sort: 'descending' }),
          ],
        }),
      ),
    );
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({ cmd: 'pivotGroup', field: 0, by: 'dates', months: true }),
      ),
    );
    // Report options: outline layout, no grand total column.
    fireEvent.click(await screen.findByTestId('pivot-options'));
    await screen.findByTestId('pivot-options-dialog');
    fireEvent.click(screen.getByTestId('pivot-layout-outline'));
    fireEvent.click(screen.getByTestId('pivot-row-grand-totals'));
    fireEvent.click(screen.getByTestId('pivot-options-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({ cmd: 'pivotUpdate', layout: 'outline', rowGrandTotals: false }),
      ),
    );
    // Calculated field.
    fireEvent.click(await screen.findByTestId('pivot-calc'));
    await screen.findByTestId('pivot-calc-dialog');
    fireEvent.change(screen.getByTestId('pivot-calc-name'), { target: { value: 'Tax' } });
    fireEvent.change(screen.getByTestId('pivot-calc-formula'), {
      target: { value: "='Doanh thu'*0.1" },
    });
    fireEvent.click(screen.getByTestId('pivot-calc-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: 'pivotCalcField',
          name: 'Tax',
          formula: "='Doanh thu'*0.1",
        }),
      ),
    );
  });
});
