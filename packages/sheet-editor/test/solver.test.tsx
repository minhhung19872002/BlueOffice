// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SheetCommand, SheetHost, SolverResult } from '@blueoffice/platform';
import { SolverDialog, solverOptions } from '../src/solver';

afterEach(cleanup);

const i18n = createI18n('en-US');

function fakeHost(result: SolverResult) {
  const calls: SheetCommand[] = [];
  const host = {
    exec: vi.fn(async (cmd: SheetCommand) => {
      calls.push(cmd);
      if (cmd.cmd === 'solve' && cmd.apply && cmd.reports) {
        return { ...result, reports: cmd.reports.map((kind, i) => ({ kind, sheet: i + 1 })) };
      }
      return result;
    }),
  } as unknown as SheetHost;
  return { host, calls };
}

const optimal: SolverResult = { status: 'optimal', objective: 25, values: [[[0, 0], 5]] };

function setup(result: SolverResult = optimal) {
  const { host, calls } = fakeHost(result);
  const onDone = vi.fn();
  const onClose = vi.fn();
  const onCreated = vi.fn();
  render(
    <I18nextProvider i18n={i18n}>
      <SolverDialog
        host={host}
        sheet={0}
        objective="C1"
        onClose={onClose}
        onDone={onDone}
        onCreated={onCreated}
      />
    </I18nextProvider>,
  );
  fireEvent.change(screen.getByTestId('solver-variables'), { target: { value: 'A1:B1' } });
  return { calls, onDone, onClose, onCreated };
}

const last = (calls: SheetCommand[]) =>
  calls[calls.length - 1] as Extract<SheetCommand, { cmd: 'solve' }>;

describe('SolverDialog', () => {
  it('sends the method, its options and alldifferent constraints', async () => {
    const { calls } = setup();
    fireEvent.change(screen.getByTestId('solver-method'), { target: { value: 'evolutionary' } });
    // Evolutionary shows population / mutation / seed, not the subproblem limit.
    expect(screen.queryByTestId('solver-max-sub')).toBeNull();
    fireEvent.change(screen.getByTestId('solver-population'), { target: { value: '30' } });
    fireEvent.change(screen.getByTestId('solver-mutation'), { target: { value: '0,1' } });
    fireEvent.change(screen.getByTestId('solver-seed'), { target: { value: '7' } });
    fireEvent.change(screen.getByTestId('solver-max-time'), { target: { value: '2.5' } });
    fireEvent.click(screen.getByTestId('solver-add'));
    const row = screen.getByTestId('solver-constraint');
    fireEvent.change(row.querySelector('input')!, { target: { value: 'A1:B1' } });
    fireEvent.change(row.querySelector('select')!, { target: { value: 'dif' } });
    // No right side for a permutation constraint.
    expect(row.querySelectorAll('input')).toHaveLength(1);
    fireEvent.click(screen.getByTestId('solver-solve'));
    await waitFor(() => expect(calls).toHaveLength(1));
    const cmd = last(calls);
    expect(cmd.method).toBe('evolutionary');
    expect(cmd.options).toEqual({ maxTimeMs: 2500, population: 30, mutation: 0.1, seed: 7 });
    expect(cmd.constraints).toEqual([{ lhs: 'A1:B1', relation: 'dif', rhs: null }]);
    expect(cmd.apply).toBe(false);
    expect((await screen.findByTestId('solver-result')).textContent).toContain(
      'Best solution found: objective 25.',
    );
  });

  it('shows multistart options only for the nonlinear method', () => {
    setup();
    expect(screen.queryByTestId('solver-multistart')).toBeNull();
    fireEvent.change(screen.getByTestId('solver-method'), { target: { value: 'nonlinear' } });
    expect(screen.getByTestId('solver-method-hint').textContent).toContain('local optimum');
    expect(screen.queryByTestId('solver-seed')).toBeNull();
    fireEvent.click(screen.getByTestId('solver-multistart'));
    expect(screen.getByTestId('solver-starts')).toBeTruthy();
    expect(screen.getByTestId('solver-seed')).toBeTruthy();
  });

  it('keeps the solution with the chosen reports and opens the first report sheet', async () => {
    const { calls, onDone, onClose, onCreated } = setup();
    fireEvent.change(screen.getByTestId('solver-method'), { target: { value: 'nonlinear' } });
    expect(screen.queryByTestId('solver-reports')).toBeNull();
    fireEvent.click(screen.getByTestId('solver-solve'));
    expect((await screen.findByTestId('solver-result')).textContent).toContain(
      'Local optimum found: objective 25.',
    );
    fireEvent.click(screen.getByTestId('solver-report-limits'));
    fireEvent.click(screen.getByTestId('solver-report-answer'));
    fireEvent.click(screen.getByTestId('solver-keep'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const cmd = last(calls);
    expect(cmd.apply).toBe(true);
    // Reports keep their canonical order whatever the click order.
    expect(cmd.reports).toEqual(['answer', 'limits']);
    expect(onDone).toHaveBeenCalled();
    expect(onCreated).toHaveBeenCalledWith(1);
  });

  it('explains the new statuses', async () => {
    setup({ status: 'missingBounds', objective: 0, values: [] });
    fireEvent.click(screen.getByTestId('solver-solve'));
    expect((await screen.findByTestId('solver-result')).textContent).toContain(
      'needs a lower and an upper limit',
    );
    expect(screen.queryByTestId('solver-keep')).toBeNull();
  });

  it('builds options per method', () => {
    const text = {
      maxTime: '',
      maxIterations: '40',
      convergence: '1e-5',
      starts: '3',
      seed: '2',
      population: '9',
      mutation: '0.2',
      maxSubproblems: '100',
    };
    expect(solverOptions('simplex', text, false)).toEqual({ maxSubproblems: 100 });
    expect(solverOptions('nonlinear', text, false)).toEqual({
      maxSubproblems: 100,
      maxIterations: 40,
      convergence: 1e-5,
    });
    expect(solverOptions('nonlinear', text, true)).toMatchObject({
      multistart: true,
      starts: 3,
      seed: 2,
    });
    expect(solverOptions('evolutionary', { ...text, mutation: '3' }, false)).toEqual({
      maxIterations: 40,
      convergence: 1e-5,
      seed: 2,
      population: 9,
    });
  });
});
