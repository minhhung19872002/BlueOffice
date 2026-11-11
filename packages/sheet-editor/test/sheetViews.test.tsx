// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  BreakPreview,
  CustomViewInfo,
  SheetCommand,
  SheetHit,
  SheetHost,
  SheetView,
} from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  boundaryAt,
  CustomViewsDialog,
  PageBreakOverlay,
  PageLayoutView,
  sheetViewTabGroups,
  TempViewsDialog,
} from '../src/sheetViews';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (ui: ReactNode) => render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);

/** Grid of 50 pt columns and 20 pt rows below 20 pt headers, not scrolled. */
const HW = 30;
const HH = 20;
const rect = (r0: number, c0: number, r1: number, c1: number) => ({
  x: HW + c0 * 50,
  y: HH + r0 * 20,
  w: (c1 - c0 + 1) * 50,
  h: (r1 - r0 + 1) * 20,
});

const preview: BreakPreview = {
  firstPage: 1,
  areas: [
    {
      r0: 0,
      c0: 0,
      r1: 9,
      c1: 3,
      scale: 1,
      cols: [
        [0, 1],
        [2, 3],
      ],
      rows: [
        [0, 4],
        [5, 9],
      ],
      pages: [
        { col: 0, row: 0, number: 1 },
        { col: 0, row: 1, number: 2 },
        { col: 1, row: 0, number: 3 },
        { col: 1, row: 1, number: 4 },
      ],
      rowBreaks: [{ at: 5, manual: true }],
      colBreaks: [{ at: 2, manual: false }],
    },
  ],
};

function fakeHost(over: Partial<SheetHost> = {}) {
  const exec = vi.fn(async (cmd: SheetCommand) => {
    if (cmd.cmd === 'pageBreakPreview') return preview;
    return null;
  });
  const host = {
    exec,
    rangeRect: vi.fn(async (_v: SheetView, r0: number, c0: number, r1: number, c1: number) =>
      rect(r0, c0, r1, c1),
    ),
    hitTest: vi.fn(async (_v: SheetView, x: number, y: number): Promise<SheetHit> => ({
      kind: 'cell',
      row: Math.floor((y - HH) / 20),
      col: Math.floor((x - HW) / 50),
    })),
    ...over,
  } as unknown as SheetHost;
  return { host, exec };
}

const view: SheetView = { sheet: 0, scrollX: 0, scrollY: 0, width: 400, height: 300 };

describe('page layout view', () => {
  it('draws the layout pages and puts the cell editor over the active cell', async () => {
    // Every page counts as scrolled into view.
    vi.stubGlobal('IntersectionObserver', undefined);
    const exec = vi.fn(async (cmd: SheetCommand) => {
      if (cmd.cmd === 'pageLayout')
        return {
          pages: [
            {
              width: 595,
              height: 842,
              margins: [50.4, 50.4, 54, 54, 21.6, 21.6],
              cells: { c0: 0, c1: 9, r0: 0, r1: 35, scale: 1, x: 50.4, y: 54 },
            },
          ],
        };
      if (cmd.cmd === 'pageCellRect') return { x: 114.4, y: 94, w: 64, h: 20 };
      if (cmd.cmd === 'pageHit') return { row: 2, col: 2 };
      return { pages: 1, page: null };
    });
    const host = { exec } as unknown as SheetHost;
    const onCellDouble = vi.fn();
    const props = {
      host,
      sheet: 0,
      revision: 1,
      zoom: 1,
      onCell: vi.fn(),
      onCellDouble,
      onHeaderFooter: vi.fn(),
      active: { row: 2, col: 2 },
      fail: vi.fn(),
    };
    const { rerender } = wrap(<PageLayoutView {...props} />);
    await waitFor(() => expect(screen.getByTestId('page-layout-active-0')).toBeTruthy());
    // The pages are drawn as the layout view shows them (full-page grid).
    await waitFor(() =>
      expect(exec).toHaveBeenCalledWith({ cmd: 'printPreview', sheet: 0, page: 0, layout: true }),
    );
    fireEvent.doubleClick(screen.getByTestId('page-layout-page-0'), { clientX: 10, clientY: 10 });
    await waitFor(() => expect(onCellDouble).toHaveBeenCalledWith(2, 2));
    // While the cell is edited, the editor replaces the outline over the cell.
    rerender(
      <I18nextProvider i18n={i18n}>
        <PageLayoutView {...props} editor={() => <textarea data-testid="ed" />} />
      </I18nextProvider>,
    );
    expect(screen.getByTestId('ed')).toBeTruthy();
    expect(screen.queryByTestId('page-layout-active-0')).toBeNull();
    const box = screen.getByTestId('page-layout-editor-0');
    // 114.4 pt × 4/3 px per pt.
    expect(parseFloat(box.style.left)).toBeCloseTo((114.4 * 4) / 3, 3);
    vi.unstubAllGlobals();
  });
});

describe('page break preview', () => {
  it('rounds a dragged break to the nearest row / column boundary', () => {
    const cell = { x: 30, y: 120, w: 50, h: 20 };
    // Row 5 spans 120–140 pt: above its middle → before row 5, below → before row 6.
    expect(boundaryAt(true, 5, cell, 0, 125)).toBe(5);
    expect(boundaryAt(true, 5, cell, 0, 135)).toBe(6);
    expect(boundaryAt(false, 0, cell, 60, 0)).toBe(1);
    // Outside the cells: the break goes away.
    expect(boundaryAt(true, null, null, 0, 0)).toBe(0);
  });

  it('labels the pages and draws manual breaks solid, automatic ones dashed', async () => {
    const { host } = fakeHost();
    wrap(
      <PageBreakOverlay
        host={host}
        view={view}
        revision={0}
        scale={1}
        width={400}
        height={300}
        frozen={{ rows: 0, cols: 0 }}
        onMove={() => {}}
      />,
    );
    await screen.findByTestId('page-break-preview');
    for (const n of [1, 2, 3, 4]) {
      expect(screen.getByTestId(`page-label-${n}`).textContent).toBe(`Page ${n}`);
    }
    const row = screen.getByTestId('page-break-row-5');
    const col = screen.getByTestId('page-break-col-2');
    expect(row.getAttribute('data-manual')).toBe('true');
    expect(col.getAttribute('data-manual')).toBe('false');
    // The row break sits at the top of row 6 (20 + 5 × 20 = 120 pt).
    expect(row.getAttribute('y1')).toBe('120');
    // Page 3 is centred on columns C:D (30 + 100 + 50 = 180) and rows 1–5 (20 + 50 = 70).
    const p3 = screen.getByTestId('page-label-3');
    expect([p3.getAttribute('x'), p3.getAttribute('y')]).toEqual(['180', '70']);
  });

  it('dragging a break asks the engine to move it', async () => {
    const { host } = fakeHost();
    const onMove = vi.fn();
    wrap(
      <PageBreakOverlay
        host={host}
        view={view}
        revision={0}
        scale={1}
        width={400}
        height={300}
        frozen={{ rows: 0, cols: 0 }}
        onMove={onMove}
      />,
    );
    const handle = await screen.findByTestId('page-break-handle-row-5');
    // Drop on the lower half of row 8 (20 + 8 × 20 = 180 … 200): before row 9 (index 9).
    act(() => {
      fireEvent.pointerDown(handle, { button: 0, clientX: 100, clientY: 120 });
    });
    act(() => {
      window.dispatchEvent(new MouseEvent('pointerup', { clientX: 100, clientY: 195 }));
    });
    await waitFor(() =>
      expect(onMove).toHaveBeenCalledWith({ rows: true, area: 0, from: 5, to: 9 }),
    );
  });
});

describe('custom views dialog', () => {
  it('saves, shows and deletes views through the engine', async () => {
    let list: CustomViewInfo[] = [];
    const host = {
      exec: vi.fn(async (cmd: SheetCommand) => (cmd.cmd === 'customViews' ? list : null)),
    } as unknown as SheetHost;
    const exec = vi.fn(async (cmd: SheetCommand): Promise<unknown> => {
      if (cmd.cmd === 'addCustomView') {
        list = [
          {
            name: cmd.name,
            guid: '{G}',
            includePrintSettings: true,
            includeHiddenRowCol: false,
          },
        ];
        return { guid: '{G}' };
      }
      if (cmd.cmd === 'applyCustomView')
        return { sheet: 0, zoom: 75, active: [3, 2], topLeft: null };
      if (cmd.cmd === 'deleteCustomView') {
        list = [];
        return { ok: true };
      }
      return null;
    });
    const onApplied = vi.fn();
    const onClose = vi.fn();
    wrap(
      <CustomViewsDialog
        host={host}
        exec={exec as never}
        sheet={0}
        current={() => ({ zoom: 0.75, active: [3, 2], sqref: 'C4:D5' })}
        onApplied={onApplied}
        onClose={onClose}
      />,
    );
    expect(await screen.findByText('No saved views yet.')).toBeTruthy();
    // An empty name is refused.
    fireEvent.click(screen.getByTestId('custom-view-add'));
    expect(screen.getByRole('alert').textContent).toBe('Enter a name.');
    fireEvent.change(screen.getByTestId('custom-view-name'), { target: { value: ' Print ' } });
    fireEvent.click(screen.getByTestId('custom-view-hidden'));
    fireEvent.click(screen.getByTestId('custom-view-add'));
    await screen.findByTestId('custom-view-item-0');
    expect(exec).toHaveBeenCalledWith({
      cmd: 'addCustomView',
      name: 'Print',
      sheet: 0,
      includePrintSettings: true,
      includeHiddenRowCol: false,
      zoom: 75,
      active: [3, 2],
      sqref: 'C4:D5',
    });
    fireEvent.click(screen.getByTestId('custom-view-show-0'));
    await waitFor(() =>
      expect(onApplied).toHaveBeenCalledWith({ sheet: 0, zoom: 75, active: [3, 2], topLeft: null }),
    );
    expect(onClose).toHaveBeenCalled();
  });
});

describe('temporary views and the ribbon', () => {
  it('keeps a view under a default name and shows it', () => {
    const onKeep = vi.fn();
    const onShow = vi.fn();
    const v = {
      name: 'A',
      sheet: 1,
      zoom: 1.5,
      scroll: { x: 10, y: 20 },
      active: { row: 2, col: 3 },
    };
    wrap(
      <TempViewsDialog
        views={[v]}
        onKeep={onKeep}
        onShow={onShow}
        onDelete={() => {}}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId('temp-view-keep'));
    expect(onKeep).toHaveBeenCalledWith('View 2');
    fireEvent.click(screen.getByTestId('temp-view-show-0'));
    expect(onShow).toHaveBeenCalledWith(v);
  });

  it('adds the view commands to the View tab', () => {
    const t = i18n.t.bind(i18n) as never;
    const ids = sheetViewTabGroups(t, (id, label) => ({ id, label }) as never).flatMap((g) =>
      g.items.map((i) => (i as { id: string }).id),
    );
    expect(ids).toEqual([
      'view.normal',
      'view.pageBreakPreview',
      'view.pageLayout',
      'view.customViews',
      'view.showFormulas',
      'view.outlineSymbols',
      'view.tempViews',
    ]);
  });
});
