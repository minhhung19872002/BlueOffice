// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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
        protected: false,
        tabColor: null,
      },
    ],
    encrypted: false,
    macros: false,
    structureLocked: false,
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
          return { range: { r0: cmd.row, c0: cmd.col, r1: cmd.row, c1: cmd.col } };
        case 'flashFill':
          return cmd.col === 0 ? { found: false, filled: 0 } : { found: true, filled: 2 };
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
  it('a slow start-up workbook does not replace a file opened meanwhile', async () => {
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
    let finishNew: (() => void) | undefined;
    host.newWorkbook = () =>
      new Promise((resolve) => {
        finishNew = () => resolve(base);
      });
    host.open = async () => ({ ...base, name: 'Báo cáo.xlsx', macros: true });
    const { container } = render(<BlueSheetEditor platform={p} language="vi-VN" />);
    const root = container.querySelector('[data-app="sheet"]')!;
    expect(root.getAttribute('aria-busy')).toBe('true');
    act(() => openHandler?.({ name: 'Báo cáo.xlsx', bytes: new Uint8Array([80, 75]) }));
    await screen.findByTestId('macro-warning');
    await waitFor(() => expect(root.getAttribute('aria-busy')).toBe('false'));
    const seq = root.getAttribute('data-load-seq');
    // The blank workbook requested at start-up arrives last: it is not shown.
    await act(async () => finishNew?.());
    expect(root.getAttribute('data-load-seq')).toBe(seq);
    expect(screen.getByTestId('macro-warning')).toBeTruthy();
    expect(p.window.setTitle).toHaveBeenLastCalledWith(expect.stringContaining('Báo cáo.xlsx'));
  });

  it('shows threaded comments, replies, resolves and adds a thread', async () => {
    const { host } = fakeHost();
    const base = host.exec;
    const sent: SheetCommand[] = [];
    host.exec = (async (cmd: SheetCommand) => {
      sent.push(cmd);
      if (cmd.cmd === 'comments')
        return [
          {
            id: '{T1}',
            row: 1,
            col: 1,
            cell: 'B2',
            done: false,
            comments: [
              { id: '{T1}', author: 'Nguyễn Văn An', date: '', text: 'Kiểm tra số liệu' },
              { id: '{R1}', author: 'Trần Bình', date: '', text: 'Đã sửa' },
            ],
          },
        ];
      return base(cmd);
    }) as SheetHost['exec'];
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chèn' }));
    fireEvent.click(await screen.findByText('Bình luận'));
    const thread = await screen.findByTestId('comment-thread');
    expect(thread.textContent).toContain('Kiểm tra số liệu');
    expect(thread.textContent).toContain('Trần Bình');
    // The active cell (A1) has no thread: a new one can be started there.
    const box = screen.getByLabelText('Nhập bình luận (Ctrl+Enter để gửi)');
    fireEvent.change(box, { target: { value: 'Ô đầu' } });
    fireEvent.click(screen.getByRole('button', { name: 'Bình luận mới' }));
    await waitFor(() =>
      expect(sent).toContainEqual(
        expect.objectContaining({ cmd: 'addComment', row: 0, col: 0, text: 'Ô đầu' }),
      ),
    );
    const reply = screen.getByLabelText('Trả lời (Ctrl+Enter để gửi)');
    fireEvent.change(reply, { target: { value: 'Đồng ý' } });
    fireEvent.keyDown(reply, { key: 'Enter', ctrlKey: true });
    await waitFor(() =>
      expect(sent).toContainEqual(
        expect.objectContaining({ cmd: 'addComment', parent: '{T1}', text: 'Đồng ý' }),
      ),
    );
    fireEvent.click(screen.getByText('Đánh dấu đã xử lý'));
    await waitFor(() =>
      expect(sent).toContainEqual({ cmd: 'resolveComment', sheet: 0, id: '{T1}', done: true }),
    );
  });

  it('tells about TCVN3/VNI text converted on open and undoes it', async () => {
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
    host.open = async () => ({ ...base, legacyConverted: 3 });
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    act(() => openHandler?.({ name: 'Cũ.xlsx', bytes: new Uint8Array([80, 75]) }));
    const bar = await screen.findByTestId('legacy-notice');
    expect(bar.textContent).toContain('3 ô');
    fireEvent.click(screen.getByText('Hoàn tác chuyển đổi'));
    await waitFor(() => expect(host.undo).toHaveBeenCalled());
    expect(screen.queryByTestId('legacy-notice')).toBeNull();
  });

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
    const exec = host.exec;
    const asked: string[] = [];
    host.exec = (async (cmd: SheetCommand) => {
      if (cmd.cmd !== 'vbaProject') return exec(cmd);
      asked.push(cmd.cmd);
      return {
        name: 'VBAProject',
        codePage: 1252,
        codePageSupported: true,
        modules: [{ name: 'Module1', kind: 'standard', source: 'Sub A()\nEnd Sub\n' }],
      };
    }) as SheetHost['exec'];
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    expect(screen.queryByTestId('macro-warning')).toBeNull();
    act(() => openHandler?.({ name: 'Có macro.xlsm', bytes: new Uint8Array([80, 75]) }));
    const bar = await screen.findByTestId('macro-warning');
    expect(bar.textContent).toContain('macros');
    // Nothing is read from the project until the user asks to see it.
    expect(asked).toEqual([]);

    // "View macros" opens the read-only viewer with the engine's answer (CORE-VBA-002).
    fireEvent.click(within(bar).getByTestId('macro-warning-view'));
    const viewer = await screen.findByTestId('macro-viewer');
    const code = (await within(viewer).findByTestId('macro-viewer-code')) as HTMLTextAreaElement;
    expect(code.value).toBe('Sub A()\nEnd Sub\n');
    expect(code.readOnly).toBe(true);
    expect(asked).toEqual(['vbaProject']);
    // Converting sends the module to the BlueScript pane as a draft that is not run.
    fireEvent.click(within(viewer).getByTestId('macro-viewer-convert'));
    await waitFor(() => expect(screen.queryByTestId('macro-viewer')).toBeNull());
    const script = (await screen.findByTestId('script-code')) as HTMLTextAreaElement;
    expect(script.value).toContain('async function A()');
    expect(screen.getByTestId('script-console').textContent).toContain('It was not run.');

    fireEvent.click(within(bar).getByTestId('macro-warning-dismiss'));
    await waitFor(() => expect(screen.queryByTestId('macro-warning')).toBeNull());
  });

  it('shows the Table Design contextual tab while the active cell is in a table (CORE-UI-001)', async () => {
    const { host } = fakeHost();
    const exec = host.exec;
    host.exec = (async (cmd: SheetCommand) =>
      cmd.cmd === 'tables'
        ? {
            tables: [
              {
                index: 0,
                id: 1,
                name: 'Bang1',
                range: { r0: 0, c0: 0, r1: 2, c1: 1 },
                headerRow: true,
                totalsRow: false,
                filterButton: true,
                style: null,
                bandedRows: true,
                bandedColumns: false,
                firstColumn: false,
                lastColumn: false,
                columns: [],
              },
            ],
            lastAuto: null,
          }
        : exec(cmd)) as SheetHost['exec'];
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const ribbon = within(screen.getByTestId('ribbon'));
    const active = () =>
      ribbon.getAllByRole('tab').find((t) => t.getAttribute('aria-selected') === 'true')
        ?.textContent;
    // A1 is in the table: the tab shows under its header (cell context: Home stays active).
    await waitFor(() => expect(ribbon.getByRole('tab', { name: 'Thiết kế bảng' })).toBeTruthy());
    expect(document.querySelector('[data-context-group="table"]')!.textContent).toContain(
      'Công cụ bảng',
    );
    expect(active()).toBe('Trang đầu');
    fireEvent.mouseDown(ribbon.getByRole('tab', { name: 'Thiết kế bảng' }));
    expect(active()).toBe('Thiết kế bảng');
    // Leave the table (A4): the tab goes, back to Home.
    const input = screen.getByLabelText('Ô đang nhập');
    for (let i = 0; i < 3; i++) fireEvent.keyDown(input, { key: 'ArrowDown' });
    await waitFor(() => expect(ribbon.queryByRole('tab', { name: 'Thiết kế bảng' })).toBeNull());
    expect(active()).toBe('Trang đầu');
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

  it('inserts symbols into the active cell from the picker', async () => {
    const { host, cells } = fakeHost();
    cells.set('0,0', 'Giá ');
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chèn' }));
    fireEvent.click(await waitFor(() => document.querySelector('[data-command="insert.symbol"]')!));
    const search = await screen.findByTestId('symbol-search');
    fireEvent.change(search, { target: { value: 'U+20AB' } });
    fireEvent.keyDown(search, { key: 'Enter' });
    const input = screen.getByLabelText('Ô đang nhập') as HTMLInputElement;
    await waitFor(() => expect(input.value).toBe('Giá ₫'));
    fireEvent.keyDown(screen.getByTestId('symbol-panel'), { key: 'Escape' });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(cells.get('0,0')).toBe('Giá ₫'));
  });

  it('opens the icon library from the Insert tab', { timeout: 40_000 }, async () => {
    const { host } = fakeHost();
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chèn' }));
    fireEvent.click(await waitFor(() => document.querySelector('[data-command="insert.icon"]')!));
    const panel = await screen.findByRole('dialog', { name: 'Chèn biểu tượng' });
    fireEvent.change(await screen.findByTestId('icon-search'), {
      target: { value: 'bieu do cot' },
    });
    expect(
      await screen.findByRole('gridcell', { name: 'Biểu đồ cột' }, { timeout: 20_000 }),
    ).toBeTruthy();
    fireEvent.keyDown(panel, { key: 'Escape' });
    expect(screen.queryByTestId('icon-panel')).toBeNull();
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

  it('saves the current sheet as SYLK, DIF and formatted text from File › Save as (CORE-FMT-004)', async () => {
    const { host } = fakeHost();
    const { p, saved } = platform(host);
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    for (const ext of ['slk', 'dif', 'prn']) {
      fireEvent.click(screen.getByTestId('file-tab'));
      fireEvent.click(await screen.findByRole('button', { name: 'Save as' }));
      const entry = `[data-format=".${ext}"]`;
      await waitFor(() => expect(document.querySelector(entry)).not.toBeNull());
      fireEvent.click(document.querySelector(entry)!);
      // The engine writes the format for the sheet on screen; the file takes its extension.
      await waitFor(() => expect(host.save).toHaveBeenLastCalledWith(ext, 0));
      await waitFor(() => expect(saved.at(-1)).toMatch(new RegExp(`\\.${ext}$`)));
      await screen.findByText(/\(the current sheet only\)/);
    }
    expect(saved).toHaveLength(3);
    // A document format, not offered for sheets.
    fireEvent.click(screen.getByTestId('file-tab'));
    fireEvent.click(await screen.findByRole('button', { name: 'Save as' }));
    await waitFor(() => expect(document.querySelector('[data-format=".slk"]')).not.toBeNull());
    expect(document.querySelector('[data-format=".epub"]')).toBeNull();
  }, 30_000);

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

  it('runs Flash Fill from Ctrl+E and the Data tab', async () => {
    const { host } = fakeHost();
    const { p } = platform(host);
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const input = screen.getByLabelText('Cell editor');
    // A1 has no pattern in the fake host: the status bar says so.
    fireEvent.keyDown(input, { key: 'e', ctrlKey: true });
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith({ cmd: 'flashFill', sheet: 0, row: 0, col: 0 }),
    );
    await screen.findByText(/Flash Fill found no pattern/);
    // B1 from the ribbon.
    const nameBox = screen.getByLabelText('Name box');
    fireEvent.change(nameBox, { target: { value: 'B1' } });
    fireEvent.keyDown(nameBox, { key: 'Enter' });
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Data' }));
    fireEvent.click(await screen.findByText('Flash Fill'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith({ cmd: 'flashFill', sheet: 0, row: 0, col: 1 }),
    );
    await screen.findByText('Flash Fill filled 2 cells.');
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

  it('offers paste options after pasting copied cells (undo + paste again)', async () => {
    const { host, cells } = fakeHost();
    const { p, clip } = platform(host);
    cells.set('0,0', '7');
    render(<BlueSheetEditor platform={p} language="vi-VN" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    const input = screen.getByLabelText('Ô đang nhập');
    fireEvent.keyDown(input, { key: 'c', ctrlKey: true });
    await waitFor(() => expect(clip.text).toBe('7'));
    fireEvent.keyDown(input, { key: 'v', ctrlKey: true });
    fireEvent.click(await screen.findByTestId('paste-options'));
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Chỉ giá trị/ }));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({ cmd: 'paste', mode: 'values', row: 0, col: 0 }),
      ),
    );
    expect(host.undo).toHaveBeenCalledTimes(1);
    // Still offered for the same paste, with the new mode checked.
    fireEvent.click(await screen.findByTestId('paste-options'));
    expect(
      screen.getByRole('menuitemradio', { name: /Chỉ giá trị/ }).getAttribute('aria-checked'),
    ).toBe('true');
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

describe('BlueSheet status bar (CORE-UI-007)', { timeout: 30_000 }, () => {
  async function open() {
    const { host } = fakeHost();
    const info = await host.info();
    const used = { ...info, sheets: [{ ...info.sheets[0]!, lastRow: 9, lastCol: 3 }] };
    host.info = async () => used;
    host.newWorkbook = async () => used;
    const base = host.exec;
    const stats: SheetCommand[] = [];
    host.exec = (async (cmd: SheetCommand) => {
      if (cmd.cmd === 'rangeStats') {
        stats.push(cmd);
        return { sum: 120, count: 5, numbers: 4, average: 30, min: 10, max: 50 };
      }
      return base(cmd);
    }) as SheetHost['exec'];
    const { p } = platform(host);
    localStorage.clear();
    render(<BlueSheetEditor platform={p} language="en-US" />);
    await screen.findByRole('tab', { name: 'Sheet1' });
    return { host, stats };
  }
  const select = (ref: string) => {
    const nameBox = screen.getByLabelText('Name box');
    fireEvent.change(nameBox, { target: { value: ref } });
    fireEvent.keyDown(nameBox, { key: 'Enter' });
  };

  it('shows the statistics the engine computed for the selection', async () => {
    const { stats } = await open();
    // One cell: no statistics.
    expect(screen.queryByTestId('status-sum')).toBeNull();
    select('A1:B3');
    await waitFor(() => expect(screen.getByTestId('status-sum').textContent).toBe('Sum: 120'));
    expect(screen.getByTestId('status-average').textContent).toBe('Average: 30');
    expect(screen.getByTestId('status-count').textContent).toBe('Count: 5');
    expect(stats.at(-1)).toEqual({
      cmd: 'rangeStats',
      sheet: 0,
      ranges: [{ r0: 0, c0: 0, r1: 2, c1: 1 }],
    });
    // Minimum, maximum and numerical count are off until the user turns them on.
    expect(screen.queryByTestId('status-min')).toBeNull();
    fireEvent.contextMenu(screen.getByTestId('status-bar'));
    expect(screen.getByTestId('status-item-sum').textContent).toContain('120');
    fireEvent.click(screen.getByTestId('status-item-min'));
    fireEvent.click(screen.getByTestId('status-item-max'));
    fireEvent.click(screen.getByTestId('status-item-numericalCount'));
    fireEvent.click(screen.getByTestId('status-item-average'));
    expect(screen.getByTestId('status-min').textContent).toBe('Min: 10');
    expect(screen.getByTestId('status-max').textContent).toBe('Max: 50');
    expect(screen.getByTestId('status-numerical-count').textContent).toBe('Numerical count: 4');
    expect(screen.queryByTestId('status-average')).toBeNull();
    await waitFor(() =>
      expect(JSON.parse(localStorage.getItem('blueoffice.statusbar.sheet') ?? '{}')).toEqual({
        shown: ['min', 'max', 'numericalCount'],
        hidden: ['average'],
      }),
    );
  });

  it('asks the engine only for the used part of a large selection', async () => {
    const { stats } = await open();
    select('B1:C5000');
    await waitFor(() => expect(screen.getByTestId('status-sum')).toBeTruthy());
    expect(stats.at(-1)).toMatchObject({ ranges: [{ r0: 0, c0: 1, r1: 9, c1: 2 }] });
  });

  it('shows the cell mode: Ready, Enter while typing, Edit after F2', async () => {
    await open();
    expect(screen.getByTestId('sheet-status').textContent).toBe('Ready');
    const input = screen.getByLabelText('Cell editor');
    fireEvent.keyDown(input, { key: 'F2' });
    await waitFor(() => expect(screen.getByTestId('sheet-status').textContent).toBe('Edit'));
    expect(screen.getByTestId('sheet-status').dataset.mode).toBe('edit');
    fireEvent.keyDown(input, { key: 'Escape' });
    await waitFor(() => expect(screen.getByTestId('sheet-status').textContent).toBe('Ready'));
    fireEvent.input(input, { target: { value: '12' } });
    await waitFor(() => expect(screen.getByTestId('sheet-status').textContent).toBe('Enter'));
    // Zoom and view buttons are on the right.
    expect(screen.getByTestId('status-zoom')).toBeTruthy();
    expect(screen.getByRole('slider', { name: 'Zoom' })).toBeTruthy();
    expect(screen.getByRole('radiogroup', { name: 'Views' })).toBeTruthy();
  });
});
