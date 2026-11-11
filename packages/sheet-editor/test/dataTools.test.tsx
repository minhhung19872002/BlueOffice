// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SheetCommand, SheetHost } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ConsolidateDialog,
  RemoveDuplicatesDialog,
  SubtotalDialog,
  TextToColumnsDialog,
  parseBreaks,
  parseCellAt,
} from '../src/dataTools';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const LIST = { r0: 0, c0: 0, r1: 6, c1: 2 };
const HEADS = ['Khu vực', 'Tháng', 'Doanh thu'];

function makeHost(exec: (cmd: SheetCommand) => Promise<unknown>) {
  return {
    exec: vi.fn(async (cmd: SheetCommand) => (cmd.cmd === 'region' ? LIST : exec(cmd))),
    cell: vi.fn(async (_s: number, _r: number, c: number) => ({
      address: '',
      input: HEADS[c] ?? '',
      display: HEADS[c] ?? '',
    })),
  } as unknown as SheetHost & { exec: ReturnType<typeof vi.fn> };
}

function common(host: SheetHost) {
  return { host, sheet: 0, onClose: vi.fn(), onDone: vi.fn(), fail: vi.fn() };
}

describe('helpers', () => {
  it('parses cells and break positions', () => {
    expect(parseCellAt('$C$4', ['Sheet1'], 0)).toEqual({ sheet: 0, row: 3, col: 2 });
    expect(parseCellAt("'Bảng 2'!B1", ['Sheet1', 'Bảng 2'], 0)).toEqual({
      sheet: 1,
      row: 0,
      col: 1,
    });
    expect(parseCellAt('Nowhere!A1', ['Sheet1'], 0)).toBeUndefined();
    expect(parseCellAt('', ['Sheet1'], 0)).toBeUndefined();
    expect(parseBreaks('4, 12;20')).toEqual([4, 12, 20]);
    expect(parseBreaks('4, x')).toBeNull();
    expect(parseBreaks('0')).toBeNull();
  });
});

describe('TextToColumnsDialog', () => {
  it('previews the split and sends delimiters, formats and destination', async () => {
    const host = makeHost(async (cmd) =>
      cmd.cmd === 'textToColumns' && cmd.preview
        ? { rows: [['An', 'Hà Nội, VN', '15/03/2024']], columns: 3 }
        : { rows: 1, columns: 2 },
    );
    const c = common(host);
    wrap(
      <TextToColumnsDialog
        {...c}
        sheets={['Sheet1', 'Data']}
        range={{ r0: 0, c0: 0, r1: 4, c1: 0 }}
      />,
    );
    // Step 1 of 3: how the text is divided (separators by default), raw preview.
    expect(screen.getByTestId('split-back').hasAttribute('disabled')).toBe(true);
    expect((screen.getByTestId('split-delimited') as HTMLInputElement).checked).toBe(true);
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith({
        cmd: 'textToColumns',
        sheet: 0,
        range: { r0: 0, c0: 0, r1: 4, c1: 0 },
        split: { mode: 'delimited', delimiters: [], consecutive: false, qualifier: null },
        preview: true,
      }),
    );
    fireEvent.click(screen.getByTestId('split-next'));
    // Step 2: Tab is ticked by default.
    expect((screen.getByTestId('split-tab') as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByTestId('split-tab'));
    fireEvent.click(screen.getByTestId('split-comma'));
    await waitFor(() =>
      expect(screen.getByTestId('split-preview').textContent).toContain('Hà Nội, VN'),
    );
    expect(host.exec).toHaveBeenLastCalledWith({
      cmd: 'textToColumns',
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 4, c1: 0 },
      split: { mode: 'delimited', delimiters: [','], consecutive: false, qualifier: '"' },
      preview: true,
    });
    // Step 3: pick a column in the preview, then its format.
    fireEvent.click(screen.getByTestId('split-next'));
    expect(screen.getByTestId('split-next').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('split-col-0').textContent).toBe('Automatic');
    fireEvent.click(screen.getByTestId('split-col-1'));
    fireEvent.click(screen.getByTestId('split-fmt-skip'));
    expect(screen.getByTestId('split-col-1').textContent).toBe('Leave out');
    fireEvent.click(screen.getByTestId('split-col-2'));
    fireEvent.click(screen.getByTestId('split-fmt-date'));
    expect(screen.getByTestId('split-col-2').textContent).toBe('Date DMY');
    fireEvent.change(screen.getByTestId('split-dest'), { target: { value: 'Data!C2' } });
    fireEvent.click(screen.getByTestId('split-ok'));
    await waitFor(() => expect(c.onClose).toHaveBeenCalled());
    expect(host.exec).toHaveBeenCalledWith({
      cmd: 'textToColumns',
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 4, c1: 0 },
      split: { mode: 'delimited', delimiters: [','], consecutive: false, qualifier: '"' },
      formats: ['general', 'skip', { date: 'dmy' }],
      dest: { sheet: 1, row: 1, col: 2 },
    });
    expect(c.onDone).toHaveBeenCalled();
  });

  it('checks fixed-width breaks and shows engine errors', async () => {
    const host = makeHost(async (cmd) => {
      if (cmd.cmd === 'textToColumns' && !cmd.preview) throw new Error('textToColumns: oneColumn');
      return { rows: [], columns: 0 };
    });
    const c = common(host);
    wrap(<TextToColumnsDialog {...c} sheets={['Sheet1']} range={{ r0: 0, c0: 0, r1: 4, c1: 1 }} />);
    fireEvent.click(screen.getByTestId('split-fixed'));
    fireEvent.click(screen.getByTestId('split-next'));
    fireEvent.change(screen.getByTestId('split-breaks'), { target: { value: '4, x' } });
    // Next refuses bad positions as Finish does.
    fireEvent.click(screen.getByTestId('split-next'));
    expect(screen.getByTestId('split-error').textContent).toBe(
      'Enter break positions as whole numbers, e.g. 4, 12.',
    );
    fireEvent.click(screen.getByTestId('split-ok'));
    expect(screen.getByTestId('split-error').textContent).toBe(
      'Enter break positions as whole numbers, e.g. 4, 12.',
    );
    fireEvent.change(screen.getByTestId('split-breaks'), { target: { value: '4' } });
    fireEvent.click(screen.getByTestId('split-ok'));
    await waitFor(() =>
      expect(screen.getByTestId('split-error').textContent).toBe(
        'Select cells in a single column.',
      ),
    );
    expect(c.fail).not.toHaveBeenCalled();
  });
});

describe('RemoveDuplicatesDialog', () => {
  it('lists the columns by header and reports the counts', async () => {
    const host = makeHost(async () => ({ removed: 2, remaining: 4 }));
    const c = common(host);
    const onResult = vi.fn();
    wrap(
      <RemoveDuplicatesDialog {...c} range={{ r0: 2, c0: 1, r1: 2, c1: 1 }} onResult={onResult} />,
    );
    await waitFor(() => expect(screen.getByText('Doanh thu')).toBeTruthy());
    fireEvent.click(screen.getByTestId('dedupe-col-2'));
    fireEvent.click(screen.getByTestId('dedupe-ok'));
    await waitFor(() => expect(onResult).toHaveBeenCalledWith({ removed: 2, remaining: 4 }));
    expect(host.exec).toHaveBeenCalledWith({
      cmd: 'removeDuplicates',
      sheet: 0,
      range: LIST,
      columns: [0, 1],
      header: true,
    });
    // Without a header row the columns are named by letter.
    cleanup();
    wrap(<RemoveDuplicatesDialog {...c} range={LIST} onResult={onResult} />);
    await waitFor(() => expect(screen.getByText('Khu vực')).toBeTruthy());
    fireEvent.click(screen.getByTestId('dedupe-header'));
    expect(screen.getByText('Column A')).toBeTruthy();
    fireEvent.click(screen.getByText('No columns'));
    fireEvent.click(screen.getByTestId('dedupe-ok'));
    expect(screen.getByTestId('dedupe-error').textContent).toBe('Choose at least one column.');
  });
});

describe('ConsolidateDialog', () => {
  it('collects ranges, labels, links and the destination', async () => {
    const host = makeHost(async () => ({ sheet: 1, range: { r0: 1, c0: 5, r1: 4, c1: 8 } }));
    const c = common(host);
    wrap(<ConsolidateDialog {...c} sheets={['Sheet1', 'Tổng hợp']} selection="A1:C3" />);
    fireEvent.click(screen.getByTestId('consolidate-add'));
    fireEvent.change(screen.getByTestId('consolidate-ref'), { target: { value: "'Quý 2'!A1:C3" } });
    fireEvent.click(screen.getByTestId('consolidate-add'));
    fireEvent.click(screen.getByTestId('consolidate-add'));
    expect(screen.getByTestId('consolidate-sources').querySelectorAll('li')).toHaveLength(2);
    fireEvent.change(screen.getByTestId('consolidate-fn'), { target: { value: 'average' } });
    fireEvent.click(screen.getByTestId('consolidate-top'));
    fireEvent.click(screen.getByTestId('consolidate-left'));
    fireEvent.click(screen.getByTestId('consolidate-links'));
    fireEvent.click(screen.getByTestId('consolidate-ok'));
    expect(screen.getByTestId('consolidate-error').textContent).toBe(
      'Enter a cell such as F2 or Sheet2!A1.',
    );
    fireEvent.change(screen.getByTestId('consolidate-dest'), {
      target: { value: "'Tổng hợp'!$F$2" },
    });
    fireEvent.click(screen.getByTestId('consolidate-ok'));
    await waitFor(() => expect(c.onClose).toHaveBeenCalled());
    expect(host.exec).toHaveBeenCalledWith({
      cmd: 'consolidate',
      sheet: 0,
      sources: ['A1:C3', "'Quý 2'!A1:C3"],
      function: 'average',
      topRow: true,
      leftCol: true,
      links: true,
      dest: { sheet: 1, row: 1, col: 5 },
    });
  });

  it('shows the overlap error from the engine', async () => {
    const host = makeHost(async () => {
      throw new Error('consolidate: overlap');
    });
    const c = common(host);
    wrap(<ConsolidateDialog {...c} sheets={['Sheet1']} selection="A1:C3" />);
    fireEvent.change(screen.getByTestId('consolidate-dest'), { target: { value: 'B2' } });
    fireEvent.click(screen.getByTestId('consolidate-ok'));
    await waitFor(() =>
      expect(screen.getByTestId('consolidate-error').textContent).toBe(
        'The result would cover one of the ranges.',
      ),
    );
  });
});

describe('SubtotalDialog', () => {
  it('sends the group column, function, columns and options', async () => {
    const host = makeHost(async () => ({ groups: 3, range: LIST, levels: 2 }));
    const c = common(host);
    wrap(<SubtotalDialog {...c} range={{ r0: 1, c0: 0, r1: 1, c1: 0 }} />);
    await waitFor(() => expect(screen.getAllByText('Doanh thu').length).toBe(2));
    // Default: group by the first column, subtotal of the last one.
    expect((screen.getByTestId('subtotal-col-2') as HTMLInputElement).checked).toBe(true);
    fireEvent.change(screen.getByTestId('subtotal-fn'), { target: { value: 'count' } });
    fireEvent.click(screen.getByTestId('subtotal-col-1'));
    fireEvent.click(screen.getByTestId('subtotal-breaks'));
    fireEvent.click(screen.getByTestId('subtotal-below'));
    fireEvent.click(screen.getByTestId('subtotal-ok'));
    await waitFor(() => expect(c.onClose).toHaveBeenCalled());
    expect(host.exec).toHaveBeenCalledWith({
      cmd: 'subtotal',
      sheet: 0,
      range: LIST,
      groupBy: 0,
      function: 'count',
      columns: [1, 2],
      replace: true,
      pageBreaks: true,
      summaryBelow: false,
      label: 'Total: {0}',
      grandLabel: 'Total of all',
    });
  });

  it('removes subtotals and maps engine errors', async () => {
    const host = makeHost(async (cmd) => {
      if (cmd.cmd === 'subtotal') throw new Error('subtotal: noData');
      return { range: LIST };
    });
    const c = common(host);
    wrap(<SubtotalDialog {...c} range={LIST} />);
    await waitFor(() => expect(screen.getAllByText('Khu vực').length).toBe(2));
    fireEvent.click(screen.getByTestId('subtotal-ok'));
    await waitFor(() =>
      expect(screen.getByTestId('subtotal-error').textContent).toBe(
        'The list has no rows below its header.',
      ),
    );
    fireEvent.click(screen.getByTestId('subtotal-remove'));
    await waitFor(() => expect(c.onClose).toHaveBeenCalled());
    expect(host.exec).toHaveBeenCalledWith({ cmd: 'removeSubtotals', sheet: 0, range: LIST });
  });
});
