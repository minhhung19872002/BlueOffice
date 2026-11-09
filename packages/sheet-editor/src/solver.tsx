/**
 * BlueSheet Solver dialog: optimise a formula cell by changing variable cells under
 * constraints, with a choice of method (linear, smooth nonlinear, evolutionary), per-method
 * options and optional reports written on new sheets. Everything goes through the engine's
 * `solve` command (`SheetHost.exec`).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  SheetHost,
  SolverMethod,
  SolverOptions,
  SolverRelation,
  SolverReportKind,
  SolverResult,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './SheetEditor.module.css';

interface SolverRow {
  lhs: string;
  relation: SolverRelation;
  rhs: string;
}

const RELATIONS: SolverRelation[] = ['le', 'ge', 'eq', 'int', 'bin', 'dif'];
const METHODS: SolverMethod[] = ['simplex', 'nonlinear', 'evolutionary'];
const REPORTS: SolverReportKind[] = ['answer', 'sensitivity', 'limits'];

/** Relations without a right side. */
const unary = (r: SolverRelation) => r === 'int' || r === 'bin' || r === 'dif';

/** Option fields as typed (strings; empty = method default). */
interface OptionText {
  maxTime: string;
  maxIterations: string;
  convergence: string;
  starts: string;
  seed: string;
  population: string;
  mutation: string;
  maxSubproblems: string;
}

const NO_OPTIONS: OptionText = {
  maxTime: '',
  maxIterations: '',
  convergence: '',
  starts: '',
  seed: '',
  population: '',
  mutation: '',
  maxSubproblems: '',
};

/** A typed number (`,` or `.` decimal), or undefined when empty / not a number. */
function parseNum(s: string): number | undefined {
  const v = Number(s.trim().replace(',', '.'));
  return s.trim() === '' || !Number.isFinite(v) ? undefined : v;
}

/** The `options` object for `method` (only the fields that apply and were filled in). */
export function solverOptions(
  method: SolverMethod,
  o: OptionText,
  multistart: boolean,
): SolverOptions {
  const out: SolverOptions = {};
  const time = parseNum(o.maxTime);
  if (time !== undefined && time > 0) out.maxTimeMs = Math.round(time * 1000);
  const int = (s: string) => {
    const v = parseNum(s);
    return v === undefined || v < 1 ? undefined : Math.round(v);
  };
  if (method !== 'evolutionary') {
    const sub = int(o.maxSubproblems);
    if (sub !== undefined) out.maxSubproblems = sub;
  }
  if (method === 'simplex') return out;
  const iters = int(o.maxIterations);
  if (iters !== undefined) out.maxIterations = iters;
  const conv = parseNum(o.convergence);
  if (conv !== undefined && conv > 0) out.convergence = conv;
  const seed = parseNum(o.seed);
  const withSeed = method === 'evolutionary' || multistart;
  if (withSeed && seed !== undefined && seed >= 0) out.seed = Math.round(seed);
  if (method === 'nonlinear' && multistart) {
    out.multistart = true;
    const starts = int(o.starts);
    if (starts !== undefined) out.starts = starts;
  }
  if (method === 'evolutionary') {
    const pop = int(o.population);
    if (pop !== undefined) out.population = pop;
    const mut = parseNum(o.mutation);
    if (mut !== undefined && mut >= 0 && mut <= 1) out.mutation = mut;
  }
  return out;
}

/** Solver: optimise a formula cell by changing variable cells under constraints. */
export function SolverDialog({
  host,
  sheet,
  onClose,
  onDone,
  onCreated,
  objective: initial,
}: {
  host: SheetHost;
  sheet: number;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  /** Called with the first report sheet written. */
  onCreated?: (sheet: number) => void;
  objective: string;
}) {
  const { t } = useTranslation();
  const [objective, setObjective] = useState(initial);
  const [goal, setGoal] = useState<'max' | 'min' | 'value'>('max');
  const [target, setTarget] = useState('0');
  const [variables, setVariables] = useState('');
  const [rows, setRows] = useState<SolverRow[]>([]);
  const [nonNegative, setNonNegative] = useState(true);
  const [method, setMethod] = useState<SolverMethod>('simplex');
  const [opts, setOpts] = useState<OptionText>(NO_OPTIONS);
  const [multistart, setMultistart] = useState(false);
  const [reports, setReports] = useState<SolverReportKind[]>([]);
  const [result, setResult] = useState<SolverResult | null>(null);
  const [error, setError] = useState('');
  const spec = () => ({
    cmd: 'solve' as const,
    sheet,
    objective,
    goal: goal === 'value' ? { value: parseNum(target) ?? 0 } : goal,
    variables: variables
      .split(/[;,]/)
      .map((v) => v.trim())
      .filter((v) => v !== ''),
    constraints: rows.map((r) => ({
      lhs: r.lhs,
      relation: r.relation,
      rhs: unary(r.relation) ? null : r.rhs,
    })),
    nonNegative,
    method,
    options: solverOptions(method, opts, multistart),
  });
  const solve = async (apply: boolean) => {
    try {
      const r = await host.exec<SolverResult>({
        ...spec(),
        apply,
        ...(apply && reports.length > 0 ? { reports } : {}),
      });
      setError('');
      if (apply) {
        await onDone();
        const first = r.reports?.[0];
        if (first) onCreated?.(first.sheet);
        onClose();
      } else {
        setResult(r);
      }
    } catch {
      setError(t('sheetEditor.solver.invalid'));
    }
  };
  const changed = () => setResult(null);
  const update = (i: number, patch: Partial<SolverRow>) => {
    setRows(rows.map((r, k) => (k === i ? { ...r, ...patch } : r)));
    changed();
  };
  const opt = (key: keyof OptionText, label: string, testId: string) => (
    <label>
      {label}
      <input
        data-testid={testId}
        inputMode="decimal"
        value={opts[key]}
        onChange={(e) => {
          setOpts({ ...opts, [key]: e.target.value });
          changed();
        }}
      />
    </label>
  );
  const usable =
    result !== null &&
    (result.status === 'optimal' || result.status === 'limit') &&
    result.values.length > 0;
  const message = (r: SolverResult) => {
    const value = Number(r.objective.toPrecision(12));
    if (r.status === 'optimal') {
      const key =
        method === 'nonlinear' ? 'foundLocal' : method === 'evolutionary' ? 'foundBest' : 'found';
      return t(`sheetEditor.solver.${key}`, { value });
    }
    if (r.status === 'limit') {
      return r.values.length > 0
        ? t('sheetEditor.solver.status.limit', { value })
        : t('sheetEditor.solver.status.limitEmpty');
    }
    return t(`sheetEditor.solver.status.${r.status}`);
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.solver.title')}
      testId="solver-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          {usable ? (
            <button
              type="button"
              className={styles.btnPrimary}
              data-testid="solver-keep"
              onClick={() => void solve(true)}
            >
              {t('sheetEditor.solver.keep')}
            </button>
          ) : (
            <button
              type="button"
              className={styles.btnPrimary}
              data-testid="solver-solve"
              onClick={() => void solve(false)}
            >
              {t('sheetEditor.solver.solve')}
            </button>
          )}
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.solver.objective')}
          <input
            data-testid="solver-objective"
            value={objective}
            onChange={(e) => {
              setObjective(e.target.value);
              changed();
            }}
          />
        </label>
        <label>
          {t('sheetEditor.solver.goal')}
          <select
            data-testid="solver-goal"
            value={goal}
            onChange={(e) => {
              setGoal(e.target.value as 'max' | 'min' | 'value');
              changed();
            }}
          >
            {(['max', 'min', 'value'] as const).map((g) => (
              <option key={g} value={g}>
                {t(`sheetEditor.solver.goals.${g}`)}
              </option>
            ))}
          </select>
        </label>
        {goal === 'value' && (
          <label>
            {t('sheetEditor.solver.valueOf')}
            <input
              data-testid="solver-value"
              value={target}
              onChange={(e) => {
                setTarget(e.target.value);
                changed();
              }}
            />
          </label>
        )}
        <label>
          {t('sheetEditor.solver.variables')}
          <input
            data-testid="solver-variables"
            value={variables}
            onChange={(e) => {
              setVariables(e.target.value);
              changed();
            }}
          />
        </label>
        <fieldset>
          <legend>{t('sheetEditor.solver.constraints')}</legend>
          {rows.map((r, i) => (
            <div key={i} className={styles.row} data-testid="solver-constraint">
              <input
                aria-label={t('sheetEditor.solver.cell')}
                value={r.lhs}
                onChange={(e) => update(i, { lhs: e.target.value })}
              />
              <select
                aria-label={t('sheetEditor.solver.relation')}
                value={r.relation}
                onChange={(e) => update(i, { relation: e.target.value as SolverRelation })}
              >
                {RELATIONS.map((rel) => (
                  <option key={rel} value={rel}>
                    {t(`sheetEditor.solver.relations.${rel}`)}
                  </option>
                ))}
              </select>
              {!unary(r.relation) && (
                <input
                  aria-label={t('sheetEditor.solver.limit')}
                  value={r.rhs}
                  onChange={(e) => update(i, { rhs: e.target.value })}
                />
              )}
              <button
                type="button"
                className={styles.btn}
                aria-label={t('sheetEditor.solver.remove')}
                onClick={() => {
                  setRows(rows.filter((_, k) => k !== i));
                  changed();
                }}
              >
                ×
              </button>
            </div>
          ))}
          <button
            type="button"
            className={styles.btn}
            data-testid="solver-add"
            onClick={() => setRows([...rows, { lhs: '', relation: 'le', rhs: '' }])}
          >
            {t('sheetEditor.solver.add')}
          </button>
        </fieldset>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={nonNegative}
            onChange={(e) => {
              setNonNegative(e.target.checked);
              changed();
            }}
          />{' '}
          {t('sheetEditor.solver.nonNegative')}
        </label>
        <label>
          {t('sheetEditor.solver.method')}
          <select
            data-testid="solver-method"
            value={method}
            onChange={(e) => {
              setMethod(e.target.value as SolverMethod);
              changed();
            }}
          >
            {METHODS.map((m) => (
              <option key={m} value={m}>
                {t(`sheetEditor.solver.methods.${m}`)}
              </option>
            ))}
          </select>
        </label>
        <p className={styles.hint} data-testid="solver-method-hint">
          {t(`sheetEditor.solver.methodHints.${method}`)}
        </p>
        <fieldset data-testid="solver-options">
          <legend>{t('sheetEditor.solver.options')}</legend>
          {opt('maxTime', t('sheetEditor.solver.maxTime'), 'solver-max-time')}
          {method !== 'evolutionary' &&
            opt('maxSubproblems', t('sheetEditor.solver.maxSubproblems'), 'solver-max-sub')}
          {method !== 'simplex' &&
            opt(
              'maxIterations',
              t(`sheetEditor.solver.maxIterations.${method}`),
              'solver-max-iter',
            )}
          {method !== 'simplex' &&
            opt('convergence', t('sheetEditor.solver.convergence'), 'solver-convergence')}
          {method === 'nonlinear' && (
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="solver-multistart"
                checked={multistart}
                onChange={(e) => {
                  setMultistart(e.target.checked);
                  changed();
                }}
              />{' '}
              {t('sheetEditor.solver.multistart')}
            </label>
          )}
          {method === 'nonlinear' &&
            multistart &&
            opt('starts', t('sheetEditor.solver.starts'), 'solver-starts')}
          {method === 'evolutionary' &&
            opt('population', t('sheetEditor.solver.population'), 'solver-population')}
          {method === 'evolutionary' &&
            opt('mutation', t('sheetEditor.solver.mutation'), 'solver-mutation')}
          {(method === 'evolutionary' || (method === 'nonlinear' && multistart)) &&
            opt('seed', t('sheetEditor.solver.seed'), 'solver-seed')}
        </fieldset>
        {error && <p role="alert">{error}</p>}
        {result && (
          <p data-testid="solver-result" role="status">
            {message(result)}
          </p>
        )}
        {usable && (
          <fieldset data-testid="solver-reports">
            <legend>{t('sheetEditor.solver.reports')}</legend>
            {REPORTS.map((k) => (
              <label key={k} className={styles.check}>
                <input
                  type="checkbox"
                  data-testid={`solver-report-${k}`}
                  checked={reports.includes(k)}
                  onChange={(e) =>
                    setReports(
                      e.target.checked
                        ? REPORTS.filter((x) => x === k || reports.includes(x))
                        : reports.filter((x) => x !== k),
                    )
                  }
                />{' '}
                {t(`sheetEditor.solver.reportKinds.${k}`)}
              </label>
            ))}
          </fieldset>
        )}
      </div>
    </Dialog>
  );
}
