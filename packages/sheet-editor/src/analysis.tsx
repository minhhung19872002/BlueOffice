/**
 * Analysis tools dialog (Data ▸ Statistical analysis): descriptive statistics, ANOVA, F/t/z tests,
 * correlation, covariance, smoothing, histogram, rank and percentile, random numbers,
 * sampling, regression and Fourier analysis. The dialog only collects options; the engine
 * command `analysis` reads the ranges, computes the table and writes it (one undo step).
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  ANALYSIS_ERROR_CODES,
  type AnalysisErrorCode,
  type AnalysisRandomDist,
  type AnalysisTool,
  type CellAt,
  type SheetHost,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import { parseA1 } from './grid';
import styles from './SheetEditor.module.css';

export const ANALYSIS_TOOL_IDS = [
  'anovaSingle',
  'anovaTwoFactorRep',
  'anovaTwoFactor',
  'correlation',
  'covariance',
  'descriptive',
  'expSmoothing',
  'fTest',
  'fourier',
  'histogram',
  'movingAverage',
  'random',
  'rankPercentile',
  'regression',
  'sampling',
  'tTestPaired',
  'tTestEqual',
  'tTestUnequal',
  'zTest',
] as const;
export type AnalysisToolId = (typeof ANALYSIS_TOOL_IDS)[number];

const DISTS = [
  'uniform',
  'normal',
  'bernoulli',
  'binomial',
  'poisson',
  'patterned',
  'discrete',
] as const;
type DistKind = (typeof DISTS)[number];

const TWO_SAMPLE: AnalysisToolId[] = [
  'fTest',
  'tTestPaired',
  'tTestEqual',
  'tTestUnequal',
  'zTest',
];
const BY_ROWS: AnalysisToolId[] = [
  'descriptive',
  'correlation',
  'covariance',
  'anovaSingle',
  'rankPercentile',
];
const ALPHA: AnalysisToolId[] = [
  'anovaSingle',
  'anovaTwoFactor',
  'anovaTwoFactorRep',
  ...TWO_SAMPLE,
];

/** Parses a decimal typed with `.` or `,`; `NaN` when it is not one. */
const num = (s: string): number => (s.trim() === '' ? NaN : Number(s.trim().replace(',', '.')));

/** The option values of the form (strings as typed). */
export interface AnalysisForm {
  tool: AnalysisToolId;
  input: string;
  input2: string;
  labels: boolean;
  byRows: boolean;
  alpha: string;
  hypothesized: string;
  varA: string;
  varB: string;
  summaryConfidence: boolean;
  confidence: string;
  kthLargest: string;
  kthSmallest: string;
  damping: string;
  interval: string;
  stdErrors: boolean;
  pareto: boolean;
  cumulative: boolean;
  /** Chart output (histogram, exponential smoothing, moving average). */
  chart: boolean;
  rowsPerSample: string;
  columns: string;
  count: string;
  seed: string;
  dist: DistKind;
  p1: string;
  p2: string;
  /** Patterned distribution: step, times each value, times the sequence. */
  p3: string;
  p4: string;
  p5: string;
  sampling: 'periodic' | 'random';
  period: string;
  constant: boolean;
  regConfidence: boolean;
  residuals: boolean;
  stdResiduals: boolean;
  residualPlots: boolean;
  lineFitPlots: boolean;
  normalPlot: boolean;
  inverse: boolean;
}

/** Default parameters of each distribution (`p1`, `p2`). */
const DIST_DEFAULTS: Record<DistKind, [string, string]> = {
  uniform: ['0', '1'],
  normal: ['0', '1'],
  bernoulli: ['0.5', ''],
  binomial: ['0.5', '10'],
  poisson: ['1', ''],
  patterned: ['1', '10'],
  discrete: ['', ''],
};

/** Labels of the distribution parameters. */
const DIST_PARAMS: Record<DistKind, string[]> = {
  uniform: ['lo', 'hi'],
  normal: ['mean', 'sd'],
  bernoulli: ['p'],
  binomial: ['p', 'trials'],
  poisson: ['lambda'],
  patterned: ['from', 'to', 'step', 'repeatEach', 'repeatSeq'],
  discrete: [],
};

/** Form fields holding the distribution parameters, in order. */
const PARAM_FIELDS = ['p1', 'p2', 'p3', 'p4', 'p5'] as const;

const seedOf = (s: string): number =>
  s.trim() === '' ? Math.floor(Math.random() * 2 ** 31) : Math.trunc(num(s));

function dist(f: AnalysisForm): AnalysisRandomDist {
  const a = num(f.p1);
  const b = num(f.p2);
  switch (f.dist) {
    case 'uniform':
      return { kind: 'uniform', lo: a, hi: b };
    case 'normal':
      return { kind: 'normal', mean: a, sd: b };
    case 'bernoulli':
      return { kind: 'bernoulli', p: a };
    case 'binomial':
      return { kind: 'binomial', p: a, trials: Math.trunc(b) };
    case 'poisson':
      return { kind: 'poisson', lambda: a };
    case 'patterned':
      return {
        kind: 'patterned',
        from: a,
        to: b,
        step: num(f.p3),
        repeatEach: Math.trunc(num(f.p4)),
        repeatSeq: Math.trunc(num(f.p5)),
      };
    case 'discrete':
      return { kind: 'discrete' };
  }
}

/** The `tool` JSON of the form; `null` when a number field is not a number. */
export function toolSpec(f: AnalysisForm): AnalysisTool | null {
  const alpha = num(f.alpha);
  const hypothesized = num(f.hypothesized);
  const opt = (s: string) => (s.trim() === '' ? null : Math.trunc(num(s)));
  let spec: AnalysisTool;
  switch (f.tool) {
    case 'descriptive':
      spec = {
        tool: 'descriptive',
        byRows: f.byRows,
        labels: f.labels,
        confidence: f.summaryConfidence ? num(f.confidence) / 100 : null,
        kthLargest: opt(f.kthLargest),
        kthSmallest: opt(f.kthSmallest),
      };
      break;
    case 'correlation':
    case 'covariance':
    case 'rankPercentile':
      spec = { tool: f.tool, byRows: f.byRows, labels: f.labels };
      break;
    case 'anovaSingle':
      spec = { tool: 'anovaSingle', byRows: f.byRows, labels: f.labels, alpha };
      break;
    case 'anovaTwoFactor':
      spec = { tool: 'anovaTwoFactor', labels: f.labels, alpha };
      break;
    case 'anovaTwoFactorRep':
      spec = { tool: 'anovaTwoFactorRep', rowsPerSample: Math.trunc(num(f.rowsPerSample)), alpha };
      break;
    case 'fTest':
      spec = { tool: 'fTest', labels: f.labels, alpha };
      break;
    case 'tTestPaired':
    case 'tTestEqual':
    case 'tTestUnequal':
      spec = { tool: f.tool, labels: f.labels, alpha, hypothesized };
      break;
    case 'zTest':
      spec = {
        tool: 'zTest',
        labels: f.labels,
        alpha,
        hypothesized,
        varA: num(f.varA),
        varB: num(f.varB),
      };
      break;
    case 'expSmoothing':
      spec = {
        tool: 'expSmoothing',
        labels: f.labels,
        damping: num(f.damping),
        stdErrors: f.stdErrors,
        chart: f.chart,
      };
      break;
    case 'movingAverage':
      spec = {
        tool: 'movingAverage',
        labels: f.labels,
        interval: Math.trunc(num(f.interval)),
        stdErrors: f.stdErrors,
        chart: f.chart,
      };
      break;
    case 'histogram':
      spec = {
        tool: 'histogram',
        labels: f.labels,
        pareto: f.pareto,
        cumulative: f.cumulative,
        chart: f.chart,
      };
      break;
    case 'random':
      spec = {
        tool: 'random',
        columns: Math.trunc(num(f.columns)),
        // A pattern is written whole (the count does not apply).
        count: f.dist === 'patterned' ? 0 : Math.trunc(num(f.count)),
        seed: seedOf(f.seed),
        dist: dist(f),
      };
      break;
    case 'sampling':
      spec = {
        tool: 'sampling',
        labels: f.labels,
        method:
          f.sampling === 'periodic'
            ? { kind: 'periodic', period: Math.trunc(num(f.period)) }
            : { kind: 'random', count: Math.trunc(num(f.count)) },
        seed: seedOf(f.seed),
      };
      break;
    case 'regression':
      spec = {
        tool: 'regression',
        labels: f.labels,
        constant: f.constant,
        confidence: f.regConfidence ? num(f.confidence) / 100 : 0.95,
        residuals: f.residuals,
        stdResiduals: f.stdResiduals,
        residualPlots: f.residualPlots,
        lineFitPlots: f.lineFitPlots,
        normalPlot: f.normalPlot,
      };
      break;
    case 'fourier':
      spec = { tool: 'fourier', labels: f.labels, inverse: f.inverse };
      break;
  }
  const bad = Object.values(spec).some((v) => typeof v === 'number' && !Number.isFinite(v));
  const badDist =
    spec.tool === 'random' &&
    Object.values(spec.dist).some((v) => typeof v === 'number' && !Number.isFinite(v));
  return bad || badDist ? null : spec;
}

/** Which range fields a tool shows: `[first label key, second label key]` (null = hidden). */
function rangeLabels(f: AnalysisForm): [string | null, string | null] {
  if (TWO_SAMPLE.includes(f.tool)) return ['variable1', 'variable2'];
  if (f.tool === 'regression') return ['inputY', 'inputX'];
  if (f.tool === 'histogram') return ['input', 'bins'];
  if (f.tool === 'random') return [f.dist === 'discrete' ? 'valueProb' : null, null];
  return ['input', null];
}

/** Error code of an `analysis: <code>` engine error. */
export function analysisErrorCode(e: unknown): AnalysisErrorCode | null {
  const message = e instanceof Error ? e.message : String(e);
  const m = /analysis: (\w+)/.exec(message);
  const code = m?.[1];
  return code && (ANALYSIS_ERROR_CODES as readonly string[]).includes(code)
    ? (code as AnalysisErrorCode)
    : null;
}

export function AnalysisDialog({
  host,
  sheet,
  sheets,
  input: initial,
  onClose,
  onDone,
  onCreated,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  /** Sheet names (for `Sheet2!B3` output cells). */
  sheets: string[];
  /** The selection in A1 notation. */
  input: string;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  /** The output was written to `sheet` (switch to it). */
  onCreated?: (sheet: number) => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [f, setF] = useState<AnalysisForm>({
    tool: 'descriptive',
    input: initial,
    input2: '',
    labels: false,
    byRows: false,
    alpha: '0.05',
    hypothesized: '0',
    varA: '',
    varB: '',
    summaryConfidence: false,
    confidence: '95',
    kthLargest: '',
    kthSmallest: '',
    damping: '0.3',
    interval: '3',
    stdErrors: false,
    pareto: false,
    cumulative: false,
    chart: false,
    rowsPerSample: '2',
    columns: '1',
    count: '10',
    seed: '',
    dist: 'uniform',
    p1: '0',
    p2: '1',
    p3: '1',
    p4: '1',
    p5: '1',
    sampling: 'periodic',
    period: '2',
    constant: true,
    regConfidence: false,
    residuals: false,
    stdResiduals: false,
    residualPlots: false,
    lineFitPlots: false,
    normalPlot: false,
    inverse: false,
  });
  const [output, setOutput] = useState<'newSheet' | 'cell'>('newSheet');
  const [outCell, setOutCell] = useState('');
  const [sheetName, setSheetName] = useState('');
  const [error, setError] = useState('');
  const set = <K extends keyof AnalysisForm>(k: K, v: AnalysisForm[K]) => {
    setF((old) => ({ ...old, [k]: v }));
    setError('');
  };
  const tl = (k: string) => t(`sheetEditor.analysis.${k}`);
  const [range1, range2] = rangeLabels(f);

  const dest = (): CellAt | null | undefined => {
    if (output === 'newSheet') return null;
    const text = outCell.trim();
    const bang = text.lastIndexOf('!');
    let target = sheet;
    if (bang >= 0) {
      const name = text.slice(0, bang).replace(/^'|'$/g, '').replace(/''/g, "'");
      target = sheets.findIndex((s) => s.toLowerCase() === name.toLowerCase());
      if (target < 0) return undefined;
    }
    const sel = parseA1(bang >= 0 ? text.slice(bang + 1) : text);
    return sel ? { sheet: target, row: sel.anchor.row, col: sel.anchor.col } : undefined;
  };

  const ok = async () => {
    const tool = toolSpec(f);
    if (!tool) {
      setError(tl('badNumber'));
      return;
    }
    const at = dest();
    if (at === undefined) {
      setError(tl('badOutput'));
      return;
    }
    try {
      const r = await host.exec<{ sheet: number }>({
        cmd: 'analysis',
        sheet,
        tool,
        input: range1 ? f.input : null,
        input2: range2 ? f.input2 : null,
        dest: at,
        sheetName: output === 'newSheet' ? sheetName : null,
      });
      await onDone();
      onCreated?.(r.sheet);
      onClose();
    } catch (e) {
      const code = analysisErrorCode(e);
      if (code) setError(t(`sheetEditor.analysis.errors.${code}`));
      else fail(e);
    }
  };

  const text = (k: keyof AnalysisForm, label: string, testId?: string) => (
    <label>
      {label}
      <input
        data-testid={testId ?? `analysis-${k}`}
        value={f[k] as string}
        onChange={(e) => set(k, e.target.value as never)}
      />
    </label>
  );
  const check = (k: keyof AnalysisForm, label: string) => (
    <label className={styles.check}>
      <input
        type="checkbox"
        data-testid={`analysis-${k}`}
        checked={f[k] as boolean}
        onChange={(e) => set(k, e.target.checked as never)}
      />{' '}
      {label}
    </label>
  );
  const tool = f.tool;
  const twoSample = TWO_SAMPLE.includes(tool);

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={tl('title')}
      width={460}
      testId="analysis-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="analysis-ok"
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {tl('tool')}
          <select
            data-testid="analysis-tool"
            value={tool}
            onChange={(e) => set('tool', e.target.value as AnalysisToolId)}
          >
            {ANALYSIS_TOOL_IDS.map((id) => (
              <option key={id} value={id}>
                {t(`sheetEditor.analysis.tools.${id}`)}
              </option>
            ))}
          </select>
        </label>
        {range1 && text('input', tl(range1))}
        {range2 && text('input2', tl(range2))}
        {BY_ROWS.includes(tool) && (
          <label>
            {tl('groupedBy')}
            <select
              data-testid="analysis-byRows"
              value={f.byRows ? 'rows' : 'columns'}
              onChange={(e) => set('byRows', e.target.value === 'rows')}
            >
              <option value="columns">{tl('columns')}</option>
              <option value="rows">{tl('rows')}</option>
            </select>
          </label>
        )}
        {tool !== 'random' && tool !== 'anovaTwoFactorRep' && check('labels', tl('labels'))}
        {tool === 'anovaTwoFactorRep' && text('rowsPerSample', tl('rowsPerSample'))}
        {ALPHA.includes(tool) && text('alpha', tl('alpha'))}
        {twoSample && tool !== 'fTest' && text('hypothesized', tl('hypothesized'))}
        {tool === 'zTest' && text('varA', tl('varA'))}
        {tool === 'zTest' && text('varB', tl('varB'))}
        {tool === 'descriptive' && (
          <>
            {check('summaryConfidence', tl('confidenceMean'))}
            {f.summaryConfidence && text('confidence', tl('confidence'))}
            {text('kthLargest', tl('kthLargest'))}
            {text('kthSmallest', tl('kthSmallest'))}
          </>
        )}
        {tool === 'expSmoothing' && text('damping', tl('damping'))}
        {tool === 'movingAverage' && text('interval', tl('interval'))}
        {(tool === 'expSmoothing' || tool === 'movingAverage') &&
          check('stdErrors', tl('stdErrors'))}
        {tool === 'histogram' && (
          <>
            {check('pareto', tl('pareto'))}
            {check('cumulative', tl('cumulative'))}
          </>
        )}
        {(tool === 'histogram' || tool === 'expSmoothing' || tool === 'movingAverage') &&
          check('chart', tl('chartOutput'))}
        {tool === 'random' && (
          <>
            {text('columns', tl('variables'))}
            {f.dist !== 'patterned' && text('count', tl('count'))}
            <label>
              {tl('distribution')}
              <select
                data-testid="analysis-dist"
                value={f.dist}
                onChange={(e) => {
                  const d = e.target.value as DistKind;
                  const [p1, p2] = DIST_DEFAULTS[d];
                  setF((old) => ({ ...old, dist: d, p1, p2, p3: '1', p4: '1', p5: '1' }));
                }}
              >
                {DISTS.map((d) => (
                  <option key={d} value={d}>
                    {t(`sheetEditor.analysis.dists.${d}`)}
                  </option>
                ))}
              </select>
            </label>
            {DIST_PARAMS[f.dist].map((p, i) =>
              text(PARAM_FIELDS[i] ?? 'p1', t(`sheetEditor.analysis.params.${p}`)),
            )}
            {f.dist !== 'patterned' && text('seed', tl('seed'))}
          </>
        )}
        {tool === 'sampling' && (
          <>
            <label>
              {tl('samplingMethod')}
              <select
                data-testid="analysis-sampling"
                value={f.sampling}
                onChange={(e) => set('sampling', e.target.value as 'periodic' | 'random')}
              >
                <option value="periodic">{tl('periodic')}</option>
                <option value="random">{tl('randomSample')}</option>
              </select>
            </label>
            {f.sampling === 'periodic'
              ? text('period', tl('period'))
              : text('count', tl('sampleCount'))}
            {f.sampling === 'random' && text('seed', tl('seed'))}
          </>
        )}
        {tool === 'regression' && (
          <>
            {check('constant', tl('constant'))}
            {check('regConfidence', tl('confidenceLevel'))}
            {f.regConfidence && text('confidence', tl('confidence'))}
            {check('residuals', tl('residuals'))}
            {check('stdResiduals', tl('stdResiduals'))}
            {check('residualPlots', tl('residualPlots'))}
            {check('lineFitPlots', tl('lineFitPlots'))}
            {check('normalPlot', tl('normalPlot'))}
          </>
        )}
        {tool === 'fourier' && check('inverse', tl('inverse'))}
        <fieldset>
          <legend>{tl('output')}</legend>
          <label className={styles.check}>
            <input
              type="radio"
              name="analysis-output"
              data-testid="analysis-out-sheet"
              checked={output === 'newSheet'}
              onChange={() => setOutput('newSheet')}
            />{' '}
            {tl('newSheet')}
          </label>
          {output === 'newSheet' && (
            <input
              aria-label={tl('sheetName')}
              data-testid="analysis-sheet-name"
              placeholder={tl('sheetName')}
              value={sheetName}
              onChange={(e) => setSheetName(e.target.value)}
            />
          )}
          <label className={styles.check}>
            <input
              type="radio"
              name="analysis-output"
              data-testid="analysis-out-cell"
              checked={output === 'cell'}
              onChange={() => setOutput('cell')}
            />{' '}
            {tl('outputCell')}
          </label>
          {output === 'cell' && (
            <input
              aria-label={tl('outputCell')}
              data-testid="analysis-cell"
              value={outCell}
              onChange={(e) => {
                setOutCell(e.target.value);
                setError('');
              }}
            />
          )}
        </fieldset>
        {error && (
          <p role="alert" data-testid="analysis-error">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
