// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  CalcState,
  ErrorFinding,
  EvalTrace,
  SheetCommand,
  SheetHost,
  TraceArrow,
  WatchRow,
} from '@blueoffice/platform';
import {
  arrowLine,
  CalcOptionsDialog,
  CalcStatus,
  DEFAULT_RULES,
  ErrorCheckDialog,
  ErrorMarks,
  EvaluateFormulaDialog,
  nextTraces,
  parseIteration,
  WatchWindow,
} from '../src/audit';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

function fakeHost(answer: (cmd: SheetCommand) => unknown) {
  const calls: SheetCommand[] = [];
  const host = {
    exec: vi.fn(async (cmd: SheetCommand) => {
      calls.push(cmd);
      return answer(cmd);
    }),
    setInputs: vi.fn(async () => {}),
    cell: vi.fn(async () => ({ address: 'C2', input: ' 42 ', display: ' 42 ' })),
  } as unknown as SheetHost;
  return { host, calls };
}

const fail = (e: unknown) => {
  throw e;
};

describe('helpers', () => {
  it('adds a level to an existing trace, else starts one', () => {
    const one = nextTraces([], { dependents: false, sheet: 0, row: 2, col: 1 });
    expect(one).toEqual([{ dependents: false, sheet: 0, row: 2, col: 1, levels: 1 }]);
    const two = nextTraces(one, { dependents: false, sheet: 0, row: 2, col: 1 });
    expect(two[0]!.levels).toBe(2);
    const other = nextTraces(two, { dependents: true, sheet: 0, row: 2, col: 1 });
    expect(other).toHaveLength(2);
  });

  it('validates iteration settings', () => {
    expect(parseIteration('100', '0.001')).toEqual([100, 0.001]);
    expect(parseIteration('5', '0,5')).toEqual([5, 0.5]);
    expect(parseIteration('0', '0.001')).toBeNull();
    expect(parseIteration('2.5', '0.001')).toBeNull();
    expect(parseIteration('20000', '0.001')).toBeNull();
    expect(parseIteration('10', '-1')).toBeNull();
    expect(parseIteration('10', '')).toBeNull();
  });

  it('draws arrows between centres; other sheets become an icon', () => {
    const a: TraceArrow = {
      from: { sheet: 0, r0: 0, c0: 0, r1: 1, c1: 0 },
      to: { sheet: 0, row: 0, col: 2 },
      level: 1,
      error: false,
    };
    const from = { x: 0, y: 0, w: 60, h: 40 };
    const to = { x: 120, y: 0, w: 60, h: 20 };
    const l = arrowLine(a, from, to)!;
    // Centres: (30, 20) → (150, 10); the two-cell source gets a box.
    expect([l.x1, l.y1, l.x2, l.y2]).toEqual([30, 20, 150, 10]);
    expect(l.box).toEqual(from);
    expect(l.external).toBe(false);
    const ext = arrowLine({ ...a, from: { ...a.from, sheet: 1 } }, null, to)!;
    expect(ext.external).toBe(true);
    expect([ext.x2, ext.y2]).toEqual([150, 10]);
    expect(arrowLine(a, null, null)).toBeNull();
  });
});

const outer: EvalTrace = {
  cell: 'Sheet1!C1',
  formula: '=A1*B1',
  steps: [
    {
      before: '',
      part: 'A1',
      after: '*B1',
      shown: '7',
      error: false,
      cell: { sheet: 0, row: 0, col: 0, label: 'Sheet1!A1' },
    },
    { before: '7*', part: 'B1', after: '', shown: '2', error: false, cell: null },
    { before: '', part: '7*2', after: '', shown: '14', error: false, cell: null },
  ],
  result: '14',
  truncated: false,
};
const inner: EvalTrace = {
  cell: 'Sheet1!A1',
  formula: '=3+4',
  steps: [{ before: '', part: '3+4', after: '', shown: '7', error: false, cell: null }],
  result: '7',
  truncated: false,
};

describe('EvaluateFormulaDialog', () => {
  it('steps through, steps in and out', async () => {
    const { host, calls } = fakeHost((cmd) =>
      cmd.cmd === 'evaluateFormula' ? (cmd.row === 0 && cmd.col === 2 ? outer : inner) : null,
    );
    wrap(
      <EvaluateFormulaDialog host={host} cell={{ sheet: 0, row: 0, col: 2 }} onClose={() => {}} />,
    );
    expect((await screen.findByTestId('eval-part')).textContent).toBe('A1');
    expect(screen.getByTestId('eval-text').textContent).toBe('A1*B1');
    // A1 holds a formula: step in shows its own evaluation.
    fireEvent.click(screen.getByTestId('eval-step-in'));
    await waitFor(() => expect(screen.getAllByTestId('eval-frame')).toHaveLength(2));
    expect(calls).toContainEqual({ cmd: 'evaluateFormula', sheet: 0, row: 0, col: 0 });
    expect(screen.getByTestId('eval-part').textContent).toBe('3+4');
    fireEvent.click(screen.getByTestId('eval-next'));
    expect(screen.getByTestId('eval-result').textContent).toBe('7');
    // Step out: back in C1 with A1 evaluated, B1 underlined.
    fireEvent.click(screen.getByTestId('eval-step-out'));
    expect(screen.getAllByTestId('eval-frame')).toHaveLength(1);
    expect(screen.getByTestId('eval-text').textContent).toBe('7*B1');
    expect((screen.getByTestId('eval-step-in') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('eval-next'));
    expect(screen.getByTestId('eval-part').textContent).toBe('7*2');
    fireEvent.click(screen.getByTestId('eval-next'));
    expect(screen.getByTestId('eval-result').textContent).toBe('14');
    // At the end the button starts over.
    expect(screen.getByTestId('eval-next').textContent).toBe('Start over');
  });

  it('says so when the cell has no formula', async () => {
    const { host } = fakeHost(() => null);
    wrap(
      <EvaluateFormulaDialog host={host} cell={{ sheet: 0, row: 4, col: 4 }} onClose={() => {}} />,
    );
    expect(await screen.findByTestId('eval-empty')).toBeTruthy();
  });
});

describe('ErrorCheckDialog', () => {
  const findings: ErrorFinding[] = [
    { row: 1, col: 1, rule: 'inconsistentFormula', suggestion: 'A2*2' },
    { row: 1, col: 2, rule: 'numberAsText', number: 42 },
  ];

  it('walks the findings, fixes and ignores them', async () => {
    let list = findings;
    const { host, calls } = fakeHost((cmd) => (cmd.cmd === 'errorCheck' ? list : { ok: true }));
    const onGoto = vi.fn();
    const onDone = vi.fn();
    wrap(
      <ErrorCheckDialog
        host={host}
        sheet={0}
        rules={DEFAULT_RULES}
        onRules={() => {}}
        onGoto={onGoto}
        onEvaluate={() => {}}
        onDone={onDone}
        onClose={() => {}}
        fail={fail}
      />,
    );
    expect((await screen.findByTestId('err-cell')).textContent).toBe('B2');
    expect(onGoto).toHaveBeenCalledWith(1, 1);
    expect(screen.getByTestId('err-finding').textContent).toContain('=A2*2');
    // Fix: the neighbours' formula is entered.
    list = [findings[1]!];
    fireEvent.click(screen.getByTestId('err-fix'));
    await waitFor(() => expect(host.setInputs).toHaveBeenCalledWith(0, [[1, 1, '=A2*2']]));
    expect((await screen.findByTestId('err-cell')).textContent).toBe('C2');
    // Ignore: recorded in the workbook.
    list = [];
    fireEvent.click(screen.getByTestId('err-ignore'));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: 'ignoreError',
        sheet: 0,
        row: 1,
        col: 2,
        rule: 'numberAsText',
      }),
    );
    expect(await screen.findByTestId('err-none')).toBeTruthy();
    expect(onDone).toHaveBeenCalledTimes(2);
  });

  it('passes the rules and toggles one', async () => {
    const { host, calls } = fakeHost(() => []);
    const onRules = vi.fn();
    wrap(
      <ErrorCheckDialog
        host={host}
        sheet={3}
        rules={{ ...DEFAULT_RULES, emptyRef: false }}
        onRules={onRules}
        onGoto={() => {}}
        onEvaluate={() => {}}
        onDone={() => {}}
        onClose={() => {}}
        fail={fail}
      />,
    );
    await screen.findByTestId('err-none');
    expect(calls[0]).toEqual({
      cmd: 'errorCheck',
      sheet: 3,
      rules: { ...DEFAULT_RULES, emptyRef: false },
    });
    fireEvent.click(screen.getByTestId('err-rules-toggle'));
    fireEvent.click(screen.getByTestId('err-rule-evalError'));
    expect(onRules).toHaveBeenCalledWith({ ...DEFAULT_RULES, emptyRef: false, evalError: false });
  });
});

const state = (over: Partial<CalcState> = {}): CalcState => ({
  options: {
    mode: 'auto',
    iterate: false,
    iterateCount: 100,
    iterateDelta: 0.001,
    fullPrecision: true,
    calcOnSave: true,
    fullCalcOnLoad: false,
  },
  needsCalc: false,
  circular: [],
  ...over,
});

describe('CalcOptionsDialog', () => {
  it('sends the chosen settings', async () => {
    const { host, calls } = fakeHost((cmd) => (cmd.cmd === 'calcOptions' ? state() : state()));
    const onDone = vi.fn();
    const onClose = vi.fn();
    wrap(<CalcOptionsDialog host={host} onClose={onClose} onDone={onDone} fail={fail} />);
    fireEvent.click(await screen.findByTestId('calc-mode-manual'));
    fireEvent.click(screen.getByTestId('calc-iterate'));
    fireEvent.change(screen.getByTestId('calc-count'), { target: { value: '0' } });
    fireEvent.click(screen.getByTestId('calc-ok'));
    expect((await screen.findByTestId('calc-error')).textContent).toContain('10000');
    fireEvent.change(screen.getByTestId('calc-count'), { target: { value: '50' } });
    fireEvent.change(screen.getByTestId('calc-delta'), { target: { value: '0.01' } });
    fireEvent.click(screen.getByTestId('calc-precision'));
    fireEvent.click(screen.getByTestId('calc-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(calls).toContainEqual({
      cmd: 'setCalcOptions',
      options: {
        mode: 'manual',
        iterate: true,
        iterateCount: 50,
        iterateDelta: 0.01,
        fullPrecision: false,
        calcOnSave: true,
        fullCalcOnLoad: false,
      },
    });
    expect(onDone).toHaveBeenCalled();
  });
});

describe('WatchWindow and status', () => {
  it('lists watched cells and adds the selection', async () => {
    const rows: WatchRow[] = [
      { book: 'b.xlsx', sheet: 'Sheet1', name: 'Rate', cell: 'B2', value: '5%', formula: null },
    ];
    const { host, calls } = fakeHost(() => rows);
    const onAdd = vi.fn();
    const onRemove = vi.fn();
    wrap(
      <WatchWindow
        host={host}
        watches={[{ sheet: 0, row: 1, col: 1 }]}
        revision={0}
        selection={{ r0: 2, c0: 0, r1: 3, c1: 0 }}
        sheet={0}
        onAdd={onAdd}
        onRemove={onRemove}
        onGoto={() => {}}
        onClose={() => {}}
      />,
    );
    const row = await screen.findByTestId('watch-row');
    expect(row.textContent).toContain('Rate');
    expect(row.textContent).toContain('5%');
    expect(calls[0]).toEqual({ cmd: 'watchValues', cells: [{ sheet: 0, row: 1, col: 1 }] });
    fireEvent.click(screen.getByTestId('watch-add'));
    expect(onAdd).toHaveBeenCalledWith(0, { r0: 2, c0: 0, r1: 3, c1: 0 });
    fireEvent.click(row);
    fireEvent.click(screen.getByTestId('watch-remove'));
    expect(onRemove).toHaveBeenCalledWith(0);
  });

  it('shows circular references and the calculate button', () => {
    const onCalculate = vi.fn();
    wrap(
      <CalcStatus
        state={state({
          needsCalc: true,
          circular: [{ sheet: 0, row: 0, col: 0, label: 'Sheet1!A1' }],
        })}
        onCalculate={onCalculate}
      />,
    );
    expect(screen.getByTestId('status-circular').textContent).toBe('Circular reference: Sheet1!A1');
    fireEvent.click(screen.getByTestId('status-calculate'));
    expect(onCalculate).toHaveBeenCalled();
  });
});

describe('ErrorMarks', () => {
  it('marks the flagged cells and offers the fixes of the active one', async () => {
    const findings: ErrorFinding[] = [
      { row: 0, col: 0, rule: 'evalError' },
      { row: 0, col: 1, rule: 'numberAsText', number: 42 },
    ];
    const { host, calls } = fakeHost((cmd) => (cmd.cmd === 'errorCheck' ? findings : null));
    (host as unknown as { rangeRect: unknown }).rangeRect = vi.fn(
      async (_v: unknown, row: number, col: number) => ({ x: col * 60, y: row * 20, w: 60, h: 20 }),
    );
    const onDone = vi.fn();
    const onEvaluate = vi.fn();
    wrap(
      <ErrorMarks
        host={host}
        view={{ sheet: 0, scrollX: 0, scrollY: 0, width: 800, height: 600, zoom: 1 } as never}
        revision={1}
        rules={DEFAULT_RULES}
        toScreen={(r) => r}
        active={{ row: 0, col: 1 }}
        onDone={onDone}
        onEdit={() => {}}
        onEvaluate={onEvaluate}
        onOptions={() => {}}
        fail={fail}
      />,
    );
    await waitFor(() => expect(screen.getAllByTestId('error-mark')).toHaveLength(2));
    expect(calls[0]).toEqual({ cmd: 'errorCheck', sheet: 0, rules: DEFAULT_RULES });
    // The active cell (B1, a number stored as text): convert it.
    fireEvent.click(screen.getByTestId('error-tag'));
    expect(screen.getByTestId('error-tag-menu').textContent).toContain(
      'A number is stored as text',
    );
    fireEvent.click(screen.getByTestId('error-tag-menu').querySelector('[data-action="convert"]')!);
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(host.setInputs).toHaveBeenCalledWith(0, [[0, 1, '42']]);
  });
});
