// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { PrintOptions, PrintPreview } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { jobOptions, PrintPanel, type PrintApp } from '../src/print/PrintPanel';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

/** A host answering with `sheets` sheets per job and recording the options. */
function fakeHost(sheets = 3) {
  const previews: PrintOptions[] = [];
  const pdfs: PrintOptions[] = [];
  const host = {
    printPreview: vi.fn(async (o: PrintOptions, index: number): Promise<PrintPreview> => {
      previews.push(o);
      if (o.pages === 'range' && o.range === 'x') throw new Error('bad page range');
      return {
        sheets,
        copies: o.copies ?? 1,
        index: Math.min(index, sheets - 1),
        page: { width: 612, height: 792, commands: [] },
      };
    }),
    printPdf: vi.fn(async (o: PrintOptions) => {
      pdfs.push(o);
      return new Uint8Array([37, 80, 68, 70]);
    }),
    getGlyphOutlines: vi.fn(async () => []),
  };
  return { host, previews, pdfs };
}

function panel(app: PrintApp, h = fakeHost(), initial: PrintOptions = {}) {
  const onPrint = vi.fn(async () => {});
  const onClose = vi.fn();
  wrap(
    <PrintPanel
      app={app}
      host={h.host}
      initial={initial}
      onPrint={onPrint}
      onClose={onClose}
      fail={(e) => {
        throw e;
      }}
    />,
  );
  return { onPrint, onClose };
}

const last = <T,>(v: T[]): T => v[v.length - 1] as T;

describe('jobOptions', () => {
  it('clamps copies and drops what the app does not use', () => {
    const o = jobOptions(
      {
        copies: 0,
        pages: 'all',
        range: '1-2',
        doc: { markup: false },
        sheet: { scope: 'workbook' },
      },
      'doc',
      7,
    );
    expect(o.copies).toBe(1);
    expect(o.range).toBeUndefined();
    expect(o.doc).toEqual({ markup: false });
    expect(o.sheet).toBeUndefined();
    expect(o.job).toBe(7);
    expect(jobOptions({ copies: 5000 }, 'sheet', 1).copies).toBe(999);
    expect(jobOptions({ copies: 2.7 }, 'slide', 1).copies).toBe(2);
  });
});

describe('PrintPanel', () => {
  it('previews the job and pages through its sheets', async () => {
    const h = fakeHost(3);
    panel('doc', h);
    await waitFor(() =>
      expect(screen.getByTestId('print-preview-count').textContent).toBe('Sheet 1 of 3 · 1 copies'),
    );
    fireEvent.click(screen.getByTestId('print-next'));
    await waitFor(() => expect(h.host.printPreview).toHaveBeenLastCalledWith(expect.anything(), 1));
    expect(screen.getByTestId('print-preview-count').textContent).toContain('Sheet 2 of 3');
  });

  it('sends pages per sheet, range and copies to preview and print', async () => {
    const h = fakeHost(2);
    const { onPrint, onClose } = panel('doc', h, { current: 4 });
    fireEvent.change(screen.getByTestId('print-per-sheet'), { target: { value: '4' } });
    fireEvent.change(screen.getByTestId('print-pages'), { target: { value: 'range' } });
    fireEvent.change(screen.getByTestId('print-range'), { target: { value: '1-3, 5' } });
    fireEvent.change(screen.getByTestId('print-copies'), { target: { value: '2' } });
    fireEvent.click(screen.getByTestId('print-markup'));
    await waitFor(() =>
      expect(last(h.previews)).toMatchObject({
        perSheet: 4,
        pages: 'range',
        range: '1-3, 5',
        copies: 2,
        current: 4,
        doc: { markup: false },
      }),
    );
    // The order choice appears once there are several pages per sheet.
    expect(screen.getByTestId('print-order')).toBeTruthy();
    // Every preview of one opening carries the same job number.
    expect(new Set(h.previews.map((p) => p.job)).size).toBe(1);
    fireEvent.click(screen.getByTestId('print-go'));
    await waitFor(() => expect(onPrint).toHaveBeenCalled());
    expect(last(h.pdfs)).toMatchObject({ perSheet: 4, range: '1-3, 5', copies: 2 });
    expect(onClose).toHaveBeenCalled();
  });

  it('shows a bad range and disables printing', async () => {
    const h = fakeHost();
    panel('slide', h);
    fireEvent.change(screen.getByTestId('print-pages'), { target: { value: 'range' } });
    fireEvent.change(screen.getByTestId('print-range'), { target: { value: 'x' } });
    await waitFor(() => expect(screen.getByTestId('print-range-error')).toBeTruthy());
    expect((screen.getByTestId('print-go') as HTMLButtonElement).disabled).toBe(true);
  });

  it('slide layouts and handouts', async () => {
    const h = fakeHost();
    panel('slide', h);
    expect(screen.queryByTestId('print-handout-per-page')).toBeNull();
    fireEvent.change(screen.getByTestId('print-slide-layout'), { target: { value: 'handouts' } });
    fireEvent.change(screen.getByTestId('print-handout-per-page'), { target: { value: '3' } });
    fireEvent.click(screen.getByTestId('print-hidden'));
    fireEvent.click(screen.getByTestId('print-frame'));
    fireEvent.change(screen.getByTestId('print-color'), { target: { value: 'grayscale' } });
    await waitFor(() =>
      expect(last(h.previews)).toMatchObject({
        slide: { layout: 'handouts', handoutPerPage: 3, hidden: true },
        frame: true,
        color: 'grayscale',
      }),
    );
    expect(last(h.previews).doc).toBeUndefined();
  });

  it('sheet scope, gridlines, titles and paper', async () => {
    const h = fakeHost();
    panel('sheet', h, { sheet: { scope: 'active', sheets: [1], selection: 'B2:C3' } });
    fireEvent.change(screen.getByTestId('print-sheet-scope'), { target: { value: 'selection' } });
    fireEvent.change(screen.getByTestId('print-gridlines'), { target: { value: 'on' } });
    fireEvent.change(screen.getByTestId('print-headings'), { target: { value: 'off' } });
    fireEvent.change(screen.getByTestId('print-title-rows'), { target: { value: '1:2' } });
    fireEvent.click(screen.getByTestId('print-ignore-area'));
    fireEvent.change(screen.getByTestId('print-paper'), { target: { value: 'letter' } });
    await waitFor(() =>
      expect(last(h.previews)).toMatchObject({
        paper: 'letter',
        sheet: {
          scope: 'selection',
          sheets: [1],
          selection: 'B2:C3',
          gridlines: true,
          headings: false,
          titleRows: '1:2',
          ignorePrintArea: true,
        },
      }),
    );
    // "Keep the page size" removes the paper again.
    fireEvent.change(screen.getByTestId('print-paper'), { target: { value: 'none' } });
    await waitFor(() => expect(last(h.previews).paper).toBeUndefined());
  });
});
