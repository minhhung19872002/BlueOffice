// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  QuickAnalysisInfo,
  RecommendChartsResult,
  SheetCommand,
  SheetRange,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  QuickAnalysis,
  RecommendedChartsDialog,
  dataBlock,
  formattingCommand,
  sparklineCommand,
  totalsCommand,
  totalsGroupOff,
  totalsPreview,
  type TotalLabels,
} from '../src/quickAnalysis';

afterEach(cleanup);

const i18n = createI18n('en-US');

// A1:C4 — header row, label column A, numbers in B2:C4.
const sel: SheetRange = { r0: 0, c0: 0, r1: 3, c1: 2 };
const info: QuickAnalysisInfo = {
  shape: {
    header: true,
    rows: 3,
    labelCols: 1,
    labels: 'text',
    series: 2,
    positive: true,
    labelLen: 6,
  },
  header: true,
  labelCols: 1,
  numericCols: [1, 2],
  numericRows: [1, 2, 3],
  belowFree: true,
  rightFree: true,
  region: sel,
};
const labels: TotalLabels = {
  sum: 'Total',
  average: 'Average',
  count: 'Count',
  percent: '% of total',
  running: 'Running total',
};

describe('Quick Analysis commands', () => {
  it('finds the numbers of the selection', () => {
    expect(dataBlock(sel, info)).toEqual({ r0: 1, c0: 1, r1: 3, c1: 2 });
    expect(dataBlock(sel, null)).toEqual(sel);
  });

  it('builds the conditional formats', () => {
    const b = dataBlock(sel, info);
    expect(formattingCommand('dataBar', 0, b)).toMatchObject({
      cmd: 'addCf',
      range: b,
      rule: { kind: 'dataBar' },
    });
    expect(formattingCommand('greaterThan', 0, b)).toMatchObject({
      rule: { kind: 'cellIs', operator: 'greaterThan', formula1: 'AVERAGE($B$2:$C$4)' },
    });
    expect(formattingCommand('top10', 0, b)).toMatchObject({
      rule: { kind: 'top10', rank: 10, percent: true },
    });
    expect(formattingCommand('clear', 0, b)).toEqual({ cmd: 'clearCf', sheet: 0, range: b });
  });

  it('writes totals below or right of the data as one batch', () => {
    expect(totalsCommand('sum', 'rows', 0, sel, info, labels)).toEqual({
      cmd: 'batch',
      cmds: [
        {
          cmd: 'setInputs',
          sheet: 0,
          cells: [
            [4, 1, '=SUM(B2:B4)'],
            [4, 2, '=SUM(C2:C4)'],
            [4, 0, 'Total'],
          ],
        },
        { cmd: 'format', sheet: 0, range: { r0: 4, c0: 0, r1: 4, c1: 2 }, patch: { bold: true } },
      ],
    });
    const cols = totalsCommand('average', 'cols', 0, sel, info, labels);
    expect(cols).toMatchObject({
      cmds: [
        {
          cells: [
            [1, 3, '=AVERAGE(B2:C2)'],
            [2, 3, '=AVERAGE(B3:C3)'],
            [3, 3, '=AVERAGE(B4:C4)'],
            [0, 3, 'Average'],
          ],
        },
        {},
      ],
    });
    const pct = totalsCommand('percent', 'rows', 0, sel, info, labels);
    expect(pct).toMatchObject({
      cmds: [
        {
          cells: [
            [4, 1, '=SUM(B2:B4)/SUM($B$2:$C$4)'],
            [4, 2, '=SUM(C2:C4)/SUM($B$2:$C$4)'],
            [4, 0, '% of total'],
          ],
        },
        {},
        { cmd: 'format', range: { r0: 4, c0: 1, r1: 4, c1: 2 }, patch: { numFmt: '0.0%' } },
      ],
    });
    expect(totalsCommand('running', 'cols', 0, sel, info, labels)).toMatchObject({
      cmds: [
        {
          cells: [
            [1, 3, '=SUM($B$2:C2)'],
            [2, 3, '=SUM($B$2:C3)'],
            [3, 3, '=SUM($B$2:C4)'],
            [0, 3, 'Running total'],
          ],
        },
        {},
      ],
    });
    expect(totalsCommand('sum', 'rows', 0, sel, { ...info, numericCols: [] }, labels)).toBeNull();
  });

  it('puts sparklines right of the rows', () => {
    expect(sparklineCommand('stacked', 0, sel, info)).toEqual({
      cmd: 'sparklineInsert',
      sheet: 0,
      type: 'stacked',
      data: 'B2:C4',
      location: { r0: 1, c0: 3, r1: 3, c1: 3 },
    });
  });
});

const charts: RecommendChartsResult = {
  shape: info.shape,
  range: sel,
  charts: [
    {
      kind: 'column',
      score: 86,
      reason: 'compare',
      svg: '<svg xmlns="http://www.w3.org/2000/svg"/>',
    },
    { kind: 'stackedColumn', score: 66, reason: 'composition', svg: '<svg/>' },
  ],
};

function engine() {
  const exec = vi.fn<(c: SheetCommand) => Promise<unknown>>(() => Promise.resolve({}));
  const query = vi.fn<(c: SheetCommand) => Promise<unknown>>((c) =>
    Promise.resolve(c.cmd === 'quickAnalysis' ? info : c.cmd === 'recommendCharts' ? charts : null),
  );
  return {
    exec: exec as unknown as <T>(c: SheetCommand) => Promise<T | null>,
    query: query as unknown as <T>(c: SheetCommand) => Promise<T | null>,
    execMock: exec,
    queryMock: query,
  };
}

describe('QuickAnalysis', () => {
  it('opens the tabs and applies formatting, totals and a recommended chart', async () => {
    const e = engine();
    const insertChart = vi.fn();
    const runCommand = vi.fn();
    const onNotice = vi.fn();
    let open = false;
    const ui = () => (
      <I18nextProvider i18n={i18n}>
        <QuickAnalysis
          sheet={0}
          range={sel}
          anchor={{ x: 100, y: 80 }}
          exec={e.exec}
          query={e.query}
          insertChart={insertChart}
          runCommand={runCommand}
          onNotice={onNotice}
          open={open}
          onOpenChange={(o) => {
            open = o;
            rerender(ui());
          }}
        />
      </I18nextProvider>
    );
    const { rerender } = render(ui());
    // A choice closes the panel; the button opens it again.
    const reopen = async () => {
      await waitFor(() => expect(screen.queryByTestId('quick-analysis-panel')).toBeNull());
      fireEvent.click(screen.getByTestId('quick-analysis'));
    };
    const button = screen.getByTestId('quick-analysis');
    expect(button.getAttribute('aria-label')).toBe('Quick analysis');
    fireEvent.click(button);
    expect(await screen.findByTestId('quick-analysis-panel')).toBeTruthy();
    await waitFor(() =>
      expect(e.queryMock).toHaveBeenCalledWith({ cmd: 'quickAnalysis', sheet: 0, range: sel }),
    );
    fireEvent.click(screen.getByTestId('qa-fmt-dataBar'));
    expect(e.execMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ cmd: 'addCf', range: { r0: 1, c0: 1, r1: 3, c1: 2 } }),
    );

    await reopen();
    fireEvent.click(await screen.findByTestId('qa-tab-totals'));
    fireEvent.click(screen.getByTestId('qa-total-rows-sum'));
    expect(e.execMock).toHaveBeenLastCalledWith(expect.objectContaining({ cmd: 'batch' }));

    await reopen();
    fireEvent.click(await screen.findByTestId('qa-tab-charts'));
    const chart = await screen.findByTestId('recommended-chart-column');
    expect(chart.textContent).toBe('Column');
    fireEvent.mouseEnter(chart);
    expect(screen.getByText('Compares the values of a few categories.')).toBeTruthy();
    fireEvent.click(chart);
    expect(insertChart).toHaveBeenCalledWith('column');

    await reopen();
    fireEvent.click(await screen.findByTestId('qa-tab-tables'));
    fireEvent.click(screen.getByTestId('qa-pivot'));
    expect(runCommand).toHaveBeenCalledWith('insert.pivot');
  });

  it('names where the totals go and previews the formula on hover', async () => {
    const e = engine();
    render(
      <I18nextProvider i18n={i18n}>
        <QuickAnalysis
          sheet={0}
          range={sel}
          anchor={{ x: 0, y: 0 }}
          exec={e.exec}
          query={e.query}
          insertChart={vi.fn()}
          runCommand={vi.fn()}
          onNotice={vi.fn()}
          open
          onOpenChange={vi.fn()}
        />
      </I18nextProvider>,
    );
    await waitFor(() => expect(e.queryMock).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId('qa-tab-totals'));
    // The row below the data comes first, then the column on the right.
    const groups = [screen.getByTestId('qa-totals-rows'), screen.getByTestId('qa-totals-cols')];
    expect(groups.map((g) => g.getAttribute('aria-label'))).toEqual([
      'Add a row of totals below the data',
      'Add a column of totals right of the data',
    ]);
    expect(
      groups[0]!.compareDocumentPosition(groups[1]!) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    const below = screen.getByTestId('qa-total-rows-sum');
    const right = screen.getByTestId('qa-total-cols-sum');
    expect(below.getAttribute('aria-label')).toBe('Sum (adds a row below)');
    expect(right.getAttribute('aria-label')).toBe('Sum (adds a column on the right)');
    // The icons show which strip is filled.
    expect(below.querySelector('svg')?.getAttribute('data-strip')).toBe('bottom');
    expect(right.querySelector('svg')?.getAttribute('data-strip')).toBe('right');
    // Hover: the first formula and its cell (B5 sums B2:B4; D2 sums B2:C2).
    await waitFor(() => {
      fireEvent.mouseEnter(below);
      expect(screen.getByTestId('qa-hint').textContent).toBe(
        'Sum (adds a row below) — B5: =SUM(B2:B4)',
      );
    });
    fireEvent.mouseLeave(below);
    fireEvent.mouseEnter(right);
    expect(screen.getByTestId('qa-hint').textContent).toBe(
      'Sum (adds a column on the right) — D2: =SUM(B2:C2)',
    );
  });

  it('turns the column group off for a single column and the row group for a single row', async () => {
    // C2:C4 — the selection of the report: a total right of it would repeat the values.
    const col: SheetRange = { r0: 1, c0: 2, r1: 3, c1: 2 };
    expect(totalsGroupOff('cols', col)).toBe('oneColumn');
    expect(totalsGroupOff('rows', col)).toBeNull();
    const row: SheetRange = { r0: 0, c0: 1, r1: 0, c1: 3 };
    expect(totalsGroupOff('rows', row)).toBe('oneRow');
    expect(totalsGroupOff('cols', row)).toBeNull();
    expect(totalsGroupOff('rows', sel)).toBeNull();
    expect(totalsGroupOff('cols', sel)).toBeNull();

    const e = engine();
    const colInfo: QuickAnalysisInfo = {
      ...info,
      shape: null,
      header: false,
      labelCols: 0,
      numericCols: [2],
      numericRows: [1, 2, 3],
      region: col,
    };
    e.queryMock.mockImplementation((c) =>
      Promise.resolve(c.cmd === 'quickAnalysis' ? colInfo : null),
    );
    render(
      <I18nextProvider i18n={i18n}>
        <QuickAnalysis
          sheet={0}
          range={col}
          anchor={{ x: 0, y: 0 }}
          exec={e.exec}
          query={e.query}
          insertChart={vi.fn()}
          runCommand={vi.fn()}
          onNotice={vi.fn()}
          open
          onOpenChange={vi.fn()}
        />
      </I18nextProvider>,
    );
    await waitFor(() => expect(e.queryMock).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId('qa-tab-totals'));
    const off = screen.getByTestId('qa-total-cols-sum');
    expect(off.getAttribute('aria-disabled')).toBe('true');
    expect(off.getAttribute('title')).toContain('single column');
    fireEvent.click(off);
    expect(e.execMock).not.toHaveBeenCalled();
    // The row below works: C5 = SUM(C2:C4).
    const on = screen.getByTestId('qa-total-rows-sum');
    expect(on.getAttribute('aria-disabled')).toBeNull();
    await waitFor(() => {
      fireEvent.click(on);
      expect(e.execMock).toHaveBeenCalledTimes(1);
    });
    const sent = e.execMock.mock.calls[0]![0];
    expect(totalsPreview(sent)).toBe('C5: =SUM(C2:C4)');
  });

  it('refuses totals when the next cells are taken', async () => {
    const e = engine();
    e.queryMock.mockImplementation((c) =>
      Promise.resolve(c.cmd === 'quickAnalysis' ? { ...info, belowFree: false } : null),
    );
    const onNotice = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <QuickAnalysis
          sheet={0}
          range={sel}
          anchor={{ x: 0, y: 0 }}
          exec={e.exec}
          query={e.query}
          insertChart={vi.fn()}
          runCommand={vi.fn()}
          onNotice={onNotice}
          open
          onOpenChange={vi.fn()}
        />
      </I18nextProvider>,
    );
    await waitFor(() => expect(e.queryMock).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId('qa-tab-totals'));
    await waitFor(() => {
      fireEvent.click(screen.getByTestId('qa-total-rows-sum'));
      expect(onNotice).toHaveBeenCalledWith(
        'The cells next to the selection are not empty; clear them first.',
      );
    });
    expect(e.execMock).not.toHaveBeenCalled();
  });

  it('the Recommended charts dialog inserts the chart picked', async () => {
    const e = engine();
    const onInsert = vi.fn();
    const onClose = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <RecommendedChartsDialog
          sheet={0}
          range={sel}
          query={e.query}
          onInsert={onInsert}
          onClose={onClose}
        />
      </I18nextProvider>,
    );
    fireEvent.click(await screen.findByTestId('recommended-chart-stackedColumn'));
    expect(onInsert).toHaveBeenCalledWith('stackedColumn');
    expect(onClose).toHaveBeenCalled();
    expect(e.queryMock).toHaveBeenCalledWith(
      expect.objectContaining({ cmd: 'recommendCharts', limit: 8 }),
    );
  });
});
