// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Platform, SheetHost, WorkbookInfo } from '@blueoffice/platform';
import { BlueSheetEditor } from '../src';

afterEach(cleanup);

function fakeHost() {
  const cells = new Map<string, string>();
  let wb: WorkbookInfo = {
    name: 'Book.xlsx',
    sheets: [
      { name: 'Sheet1', lastRow: null, lastCol: null, frozenRows: 0, frozenCols: 0, hidden: false },
    ],
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
});
