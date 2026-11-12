// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SheetCommand, SheetHost } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AnalysisDialog, analysisErrorCode } from '../src/analysis';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

function setup(exec: (cmd: SheetCommand) => Promise<unknown>) {
  const host = { exec: vi.fn(exec) } as unknown as SheetHost;
  const onDone = vi.fn();
  const onClose = vi.fn();
  const onCreated = vi.fn();
  const fail = vi.fn();
  wrap(
    <AnalysisDialog
      host={host}
      sheet={0}
      sheets={['Sheet1', 'Data']}
      input="A1:B10"
      onClose={onClose}
      onDone={onDone}
      onCreated={onCreated}
      fail={fail}
    />,
  );
  return { host, onDone, onClose, onCreated, fail };
}

describe('AnalysisDialog', () => {
  it('sends descriptive statistics to a new sheet', async () => {
    const { host, onCreated, onClose } = setup(async () => ({ sheet: 2 }));
    fireEvent.click(screen.getByTestId('analysis-labels'));
    fireEvent.click(screen.getByTestId('analysis-summaryConfidence'));
    fireEvent.change(screen.getByTestId('analysis-kthLargest'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('analysis-sheet-name'), { target: { value: 'Stats' } });
    fireEvent.click(screen.getByTestId('analysis-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(host.exec).toHaveBeenCalledWith({
      cmd: 'analysis',
      sheet: 0,
      tool: {
        tool: 'descriptive',
        byRows: false,
        labels: true,
        confidence: 0.95,
        kthLargest: 2,
        kthSmallest: null,
      },
      input: 'A1:B10',
      input2: null,
      dest: null,
      sheetName: 'Stats',
    });
    expect(onCreated).toHaveBeenCalledWith(2);
  });

  it('builds a t-test with a second range and an output cell on another sheet', async () => {
    const { host } = setup(async () => ({ sheet: 1 }));
    fireEvent.change(screen.getByTestId('analysis-tool'), { target: { value: 'tTestEqual' } });
    fireEvent.change(screen.getByTestId('analysis-input2'), { target: { value: 'C1:C10' } });
    fireEvent.change(screen.getByTestId('analysis-alpha'), { target: { value: '0,01' } });
    fireEvent.click(screen.getByTestId('analysis-out-cell'));
    fireEvent.change(screen.getByTestId('analysis-cell'), { target: { value: 'Data!$E$3' } });
    fireEvent.click(screen.getByTestId('analysis-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith({
        cmd: 'analysis',
        sheet: 0,
        tool: { tool: 'tTestEqual', labels: false, alpha: 0.01, hypothesized: 0 },
        input: 'A1:B10',
        input2: 'C1:C10',
        dest: { sheet: 1, row: 2, col: 4 },
        sheetName: null,
      }),
    );
  });

  it('random numbers need no input range and use the seed', async () => {
    const { host } = setup(async () => ({ sheet: 0 }));
    fireEvent.change(screen.getByTestId('analysis-tool'), { target: { value: 'random' } });
    expect(screen.queryByTestId('analysis-input')).toBeNull();
    fireEvent.change(screen.getByTestId('analysis-dist'), { target: { value: 'binomial' } });
    fireEvent.change(screen.getByTestId('analysis-seed'), { target: { value: '42' } });
    fireEvent.click(screen.getByTestId('analysis-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          input: null,
          tool: {
            tool: 'random',
            columns: 1,
            count: 10,
            seed: 42,
            dist: { kind: 'binomial', p: 0.5, trials: 10 },
          },
        }),
      ),
    );
  });

  it('patterned numbers take from / to / step / repeats and no count or seed', async () => {
    const { host } = setup(async () => ({ sheet: 0 }));
    fireEvent.change(screen.getByTestId('analysis-tool'), { target: { value: 'random' } });
    fireEvent.change(screen.getByTestId('analysis-dist'), { target: { value: 'patterned' } });
    expect(screen.queryByTestId('analysis-count')).toBeNull();
    expect(screen.queryByTestId('analysis-seed')).toBeNull();
    fireEvent.change(screen.getByTestId('analysis-p2'), { target: { value: '5' } });
    fireEvent.change(screen.getByTestId('analysis-p3'), { target: { value: '0,5' } });
    fireEvent.change(screen.getByTestId('analysis-p4'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('analysis-p5'), { target: { value: '3' } });
    fireEvent.click(screen.getByTestId('analysis-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          tool: expect.objectContaining({
            tool: 'random',
            count: 0,
            dist: { kind: 'patterned', from: 1, to: 5, step: 0.5, repeatEach: 2, repeatSeq: 3 },
          }),
        }),
      ),
    );
  });

  it('asks for chart outputs and regression plots', async () => {
    const { host } = setup(async () => ({ sheet: 1 }));
    fireEvent.change(screen.getByTestId('analysis-tool'), { target: { value: 'histogram' } });
    fireEvent.click(screen.getByTestId('analysis-chart'));
    fireEvent.click(screen.getByTestId('analysis-ok'));
    await waitFor(() =>
      expect(host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          tool: { tool: 'histogram', labels: false, pareto: false, cumulative: false, chart: true },
        }),
      ),
    );
    cleanup();
    const second = setup(async () => ({ sheet: 1 }));
    fireEvent.change(screen.getByTestId('analysis-tool'), { target: { value: 'regression' } });
    fireEvent.change(screen.getByTestId('analysis-input2'), { target: { value: 'C1:C10' } });
    fireEvent.click(screen.getByTestId('analysis-residualPlots'));
    fireEvent.click(screen.getByTestId('analysis-lineFitPlots'));
    fireEvent.click(screen.getByTestId('analysis-normalPlot'));
    fireEvent.click(screen.getByTestId('analysis-ok'));
    await waitFor(() =>
      expect(second.host.exec).toHaveBeenCalledWith(
        expect.objectContaining({
          tool: expect.objectContaining({
            tool: 'regression',
            residualPlots: true,
            lineFitPlots: true,
            normalPlot: true,
          }),
        }),
      ),
    );
  });

  it('shows engine and form errors in the dialog', async () => {
    const { host, fail, onClose } = setup(async () => {
      throw new Error('spreadsheet error: analysis: nonNumeric');
    });
    fireEvent.click(screen.getByTestId('analysis-ok'));
    expect((await screen.findByTestId('analysis-error')).textContent).toBe(
      'The input range contains non-numeric data.',
    );
    expect(fail).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.change(screen.getByTestId('analysis-tool'), { target: { value: 'zTest' } });
    fireEvent.click(screen.getByTestId('analysis-ok'));
    expect((await screen.findByTestId('analysis-error')).textContent).toBe(
      'Enter a number in every numeric field.',
    );
    fireEvent.change(screen.getByTestId('analysis-tool'), { target: { value: 'fourier' } });
    fireEvent.click(screen.getByTestId('analysis-out-cell'));
    fireEvent.change(screen.getByTestId('analysis-cell'), { target: { value: 'Nope!A1' } });
    fireEvent.click(screen.getByTestId('analysis-ok'));
    expect((await screen.findByTestId('analysis-error')).textContent).toBe(
      'Enter a valid output cell, e.g. F1 or Sheet2!A1.',
    );
    expect(host.exec).toHaveBeenCalledTimes(1);
  });

  it('parses error codes', () => {
    expect(analysisErrorCode(new Error('spreadsheet error: analysis: overlap'))).toBe('overlap');
    expect(analysisErrorCode(new Error('analysis: whatever'))).toBeNull();
    expect(analysisErrorCode('boom')).toBeNull();
  });
});
