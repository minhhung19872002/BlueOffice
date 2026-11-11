/**
 * BlueSheet formula auditing and calculation (SHEET-AUDIT-001): trace arrows drawn
 * over the grid, step-by-step formula evaluation, error checking, the watch window,
 * calculation options (ECMA-376 Part 1, §18.2.2 `calcPr`) and the calculation status.
 * Everything goes through the engine command bridge (`SheetHost.exec`, the commands of
 * `@blueoffice/platform` `AuditCommand`); {@link useAudit} holds the UI state the
 * editor wires into its ribbon, overlay, dialogs and status bar.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  AuditCell,
  CalcMode,
  CalcSettings,
  CalcState,
  ErrorFinding,
  ErrorRule,
  ErrorRules,
  EvalTrace,
  SheetHost,
  SheetView,
  TraceArrow,
  TraceArrows,
  WatchRow,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import {
  ArrowRightFromLine,
  ArrowRightToLine,
  Calculator,
  Eraser,
  Eye,
  Footprints,
  RefreshCw,
  RotateCw,
  Settings2,
  TriangleAlert,
} from 'lucide-react';
import { type SetStateAction, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { a1, type Range } from './grid';
import { ConfirmDialog } from './homeControls';
import styles from './SheetEditor.module.css';
import css from './audit.module.css';

/** Ribbon icons of the Formulas tab. */
export const AuditIcons = {
  precedents: ArrowRightToLine,
  dependents: ArrowRightFromLine,
  removeArrows: Eraser,
  errors: TriangleAlert,
  evaluate: Footprints,
  watch: Eye,
  options: Settings2,
  calcNow: RefreshCw,
  calcSheet: RotateCw,
  calcFull: Calculator,
};

/** Highest "maximum iterations" accepted by the options dialog (own limit). */
export const MAX_ITERATIONS = 10000;
/** Most cells one "add watch" takes from the selection. */
const MAX_WATCH_ADD = 50;

export const DEFAULT_RULES: ErrorRules = {
  evalError: true,
  inconsistentFormula: true,
  numberAsText: true,
  // Off by default, as in common spreadsheet error checking: blank inputs are normal.
  emptyRef: false,
  unlockedFormula: true,
};

const RULES: ErrorRule[] = [
  'evalError',
  'inconsistentFormula',
  'numberAsText',
  'emptyRef',
  'unlockedFormula',
];

/** One trace the user asked for: precedents or dependents of a cell, `levels` deep. */
export interface AuditTrace {
  dependents: boolean;
  sheet: number;
  row: number;
  col: number;
  levels: number;
}

/** Screen rectangle (CSS pixels, relative to the grid). */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type AuditDialog =
  | { kind: 'evaluate'; cell: AuditCell }
  | { kind: 'errors' }
  | { kind: 'options' }
  | { kind: 'circular'; labels: string };

/** Add a level to the trace of this cell, or start a new one. */
export function nextTraces(traces: AuditTrace[], add: Omit<AuditTrace, 'levels'>): AuditTrace[] {
  const i = traces.findIndex(
    (x) =>
      x.dependents === add.dependents &&
      x.sheet === add.sheet &&
      x.row === add.row &&
      x.col === add.col,
  );
  if (i < 0) return [...traces, { ...add, levels: 1 }];
  return traces.map((x, j) => (j === i ? { ...x, levels: x.levels + 1 } : x));
}

/** Validates the iteration fields of the options dialog. */
export function parseIteration(count: string, delta: string): [number, number] | null {
  const n = Number(count.trim());
  const d = Number(delta.trim().replace(',', '.'));
  if (!Number.isInteger(n) || n < 1 || n > MAX_ITERATIONS) return null;
  if (!Number.isFinite(d) || d < 0 || delta.trim() === '') return null;
  return [n, d];
}

/** Auditing state and actions of one editor. */
export function useAudit({
  host,
  sheet,
  revision,
  loadSeq = 0,
  onDone,
  fail,
}: {
  host: SheetHost | null | undefined;
  sheet: number;
  revision: number;
  /** Changes when another workbook is loaded (its saved watches are read). */
  loadSeq?: number;
  /** The workbook changed (refresh the grid). */
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}) {
  const [traces, setTraces] = useState<AuditTrace[]>([]);
  const [arrows, setArrows] = useState<TraceArrow[]>([]);
  const [dialog, setDialog] = useState<AuditDialog | null>(null);
  const [watchOpen, setWatchOpen] = useState(false);
  const [watches, setWatchState] = useState<AuditCell[]>([]);
  const [rules, setRules] = useState<ErrorRules>(DEFAULT_RULES);
  const [calc, setCalc] = useState<CalcState | null>(null);
  const seenCircular = useRef('');

  // Arrows of every trace, refreshed after each change of the workbook.
  useEffect(() => {
    if (!host) return;
    let cancelled = false;
    void Promise.all(
      traces.map((x) =>
        host.exec<TraceArrows>({
          cmd: 'traceArrows',
          sheet: x.sheet,
          row: x.row,
          col: x.col,
          dependents: x.dependents,
          levels: x.levels,
        }),
      ),
    )
      .then((all) => !cancelled && setArrows(all.flatMap((r) => r?.arrows ?? [])))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, traces, revision]);

  // Calculation state (status bar, circular reference warning).
  useEffect(() => {
    if (!host) return;
    let cancelled = false;
    void host
      .exec<CalcState>({ cmd: 'calcOptions' })
      .then((s) => {
        // Hosts without the auditing commands answer something else.
        if (cancelled || !s || !Array.isArray(s.circular) || !s.options) return;
        setCalc(s);
        const labels = s.circular.map((c) => c.label).join(', ');
        if (labels !== '' && labels !== seenCircular.current && !s.options.iterate) {
          setDialog((d) => d ?? { kind: 'circular', labels });
        }
        seenCircular.current = labels;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, revision]);

  const trace = useCallback(
    async (dependents: boolean, cell: AuditCell): Promise<'none' | 'ok'> => {
      if (!host) return 'none';
      const probe = await host.exec<TraceArrows>({
        cmd: 'traceArrows',
        ...cell,
        dependents,
        levels: 1,
      });
      if (!probe || probe.arrows.length === 0) return 'none';
      setTraces((t) => nextTraces(t, { dependents, ...cell }));
      return 'ok';
    },
    [host],
  );

  const calculate = useCallback(
    async (scope: 'workbook' | 'sheet' | 'full') => {
      if (!host) return;
      try {
        const s = await host.exec<CalcState>(
          scope === 'sheet'
            ? { cmd: 'calculate', sheet }
            : { cmd: 'calculate', full: scope === 'full' },
        );
        if (s && Array.isArray(s.circular)) setCalc(s);
        await onDone();
      } catch (e) {
        fail(e);
      }
    },
    [host, sheet, onDone, fail],
  );

  const removeArrows = useCallback(() => setTraces([]), []);

  // The watched cells are saved with the workbook (`cellWatches`): read on open,
  // written back after each change made in the watch window.
  useEffect(() => {
    if (!host) return;
    let cancelled = false;
    void host
      .exec<AuditCell[]>({ cmd: 'cellWatches' })
      .then((w) => !cancelled && Array.isArray(w) && setWatchState(w))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, loadSeq]);
  const persistWatches = useRef(false);
  useEffect(() => {
    if (!host || !persistWatches.current) return;
    persistWatches.current = false;
    void host.exec({ cmd: 'setCellWatches', cells: watches }).catch(() => {});
  }, [host, watches]);
  const setWatches = useCallback((next: SetStateAction<AuditCell[]>) => {
    persistWatches.current = true;
    setWatchState(next);
  }, []);

  const addWatches = useCallback(
    (s: number, r: Range) => {
      setWatches((w) => {
        const out = [...w];
        let added = 0;
        for (let row = r.r0; row <= r.r1 && added < MAX_WATCH_ADD; row++) {
          for (let col = r.c0; col <= r.c1 && added < MAX_WATCH_ADD; col++) {
            if (!out.some((x) => x.sheet === s && x.row === row && x.col === col)) {
              out.push({ sheet: s, row, col });
              added++;
            }
          }
        }
        return out;
      });
      setWatchOpen(true);
    },
    [setWatches],
  );

  return {
    traces,
    arrows,
    dialog,
    setDialog,
    watchOpen,
    setWatchOpen,
    watches,
    setWatches,
    addWatches,
    rules,
    setRules,
    calc,
    setCalc,
    trace,
    removeArrows,
    calculate,
  };
}

// ------------------------------------------------------------------ arrows

/** Trace arrows over the grid: precedent → dependent, with a dot at the source. */
export function TraceArrowsOverlay({
  host,
  view,
  arrows,
  toScreen,
  width,
  height,
}: {
  host: SheetHost;
  view: SheetView;
  arrows: TraceArrow[];
  /** Engine rectangle (points) → CSS pixels. */
  toScreen: (r: Box | null) => Box | null;
  width: number;
  height: number;
}) {
  const { t } = useTranslation();
  const [lines, setLines] = useState<ArrowLine[]>([]);
  useEffect(() => {
    let cancelled = false;
    const here = arrows.filter((a) => a.from.sheet === view.sheet || a.to.sheet === view.sheet);
    void Promise.all(
      here.map(async (a) => {
        const from =
          a.from.sheet === view.sheet
            ? toScreen(await host.rangeRect(view, a.from.r0, a.from.c0, a.from.r1, a.from.c1))
            : null;
        const to =
          a.to.sheet === view.sheet
            ? toScreen(await host.rangeRect(view, a.to.row, a.to.col, a.to.row, a.to.col))
            : null;
        return arrowLine(a, from, to);
      }),
    )
      .then((all) => !cancelled && setLines(all.filter((x): x is ArrowLine => x !== null)))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, view, arrows, toScreen]);
  if (lines.length === 0) return null;
  return (
    <svg
      className={css.arrows}
      width={width}
      height={height}
      data-testid="trace-arrows"
      aria-hidden="true"
    >
      <defs>
        <marker
          id="bo-audit-head"
          viewBox="0 0 8 8"
          refX="7"
          refY="4"
          markerWidth="7"
          markerHeight="7"
          orient="auto"
        >
          <path d="M0,0 L8,4 L0,8 z" className={css.head} />
        </marker>
        <marker
          id="bo-audit-head-err"
          viewBox="0 0 8 8"
          refX="7"
          refY="4"
          markerWidth="7"
          markerHeight="7"
          orient="auto"
        >
          <path d="M0,0 L8,4 L0,8 z" className={css.headError} />
        </marker>
      </defs>
      {lines.map((l, i) => (
        <g
          key={i}
          className={l.error ? css.error : css.arrow}
          data-testid="trace-arrow"
          data-level={l.level}
          data-external={l.external ? 'true' : undefined}
        >
          {l.box && (
            <rect x={l.box.x} y={l.box.y} width={l.box.w} height={l.box.h} className={css.box} />
          )}
          <line
            x1={l.x1}
            y1={l.y1}
            x2={l.x2}
            y2={l.y2}
            strokeDasharray={l.external ? '4 3' : undefined}
            markerEnd={`url(#${l.error ? 'bo-audit-head-err' : 'bo-audit-head'})`}
          />
          <circle cx={l.x1} cy={l.y1} r={2.5} className={css.dot} />
          {l.external && (
            <g transform={`translate(${l.icon.x - 9},${l.icon.y - 7})`} className={css.sheetIcon}>
              <title>{t('sheetAudit.otherSheet')}</title>
              <rect x={0} y={0} width={18} height={14} rx={1.5} />
              <path d="M0,5 H18 M0,9.5 H18 M6,0 V14 M12,0 V14" />
            </g>
          )}
        </g>
      ))}
    </svg>
  );
}

interface ArrowLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  box: Box | null;
  error: boolean;
  level: number;
  external: boolean;
  icon: { x: number; y: number };
}

const center = (b: Box) => ({ x: b.x + b.w / 2, y: b.y + b.h / 2 });

/** Geometry of one arrow; an end on another sheet is drawn as a sheet icon. */
export function arrowLine(a: TraceArrow, from: Box | null, to: Box | null): ArrowLine | null {
  const multi = a.from.r0 !== a.from.r1 || a.from.c0 !== a.from.c1;
  const base = { error: a.error, level: a.level, box: multi ? from : null };
  if (from && to) {
    const p = center(from);
    const q = center(to);
    return { ...base, x1: p.x, y1: p.y, x2: q.x, y2: q.y, external: false, icon: q };
  }
  if (to) {
    // Precedent on another sheet: icon up and to the left of the formula cell.
    const q = center(to);
    const icon = { x: Math.max(10, q.x - 48), y: Math.max(8, q.y - 28) };
    return { ...base, box: null, x1: icon.x, y1: icon.y, x2: q.x, y2: q.y, external: true, icon };
  }
  if (from) {
    // Dependent on another sheet: icon up and to the right of the source.
    const p = center(from);
    const icon = { x: p.x + 48, y: Math.max(8, p.y - 28) };
    return { ...base, x1: p.x, y1: p.y, x2: icon.x - 10, y2: icon.y, external: true, icon };
  }
  return null;
}

// ------------------------------------------------------------------ evaluate formula

interface Frame {
  trace: EvalTrace;
  at: number;
}

/** Step-by-step evaluation, with step in / step out for referenced formula cells. */
export function EvaluateFormulaDialog({
  host,
  cell,
  onClose,
}: {
  host: SheetHost;
  cell: AuditCell;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [frames, setFrames] = useState<Frame[] | null>(null);
  const [empty, setEmpty] = useState(false);
  useEffect(() => {
    let cancelled = false;
    void host
      .exec<EvalTrace | null>({ cmd: 'evaluateFormula', ...cell })
      .then((tr) => {
        if (cancelled) return;
        if (tr) setFrames([{ trace: tr, at: 0 }]);
        else setEmpty(true);
      })
      .catch(() => !cancelled && setEmpty(true));
    return () => {
      cancelled = true;
    };
  }, [host, cell]);

  const top = frames?.[frames.length - 1] ?? null;
  const step = top && top.at < top.trace.steps.length ? top.trace.steps[top.at] : null;
  const setTop = (f: Frame) => setFrames((fs) => (fs ? [...fs.slice(0, -1), f] : fs));

  const evaluate = () => {
    if (!top) return;
    setTop({ ...top, at: step ? top.at + 1 : 0 });
  };
  const stepIn = async () => {
    if (!step?.cell) return;
    const { sheet, row, col } = step.cell;
    const tr = await host.exec<EvalTrace | null>({ cmd: 'evaluateFormula', sheet, row, col });
    if (tr) setFrames((fs) => [...(fs ?? []), { trace: tr, at: 0 }]);
  };
  const stepOut = () =>
    setFrames((fs) => {
      if (!fs || fs.length < 2) return fs;
      const parent = fs[fs.length - 2]!;
      // The stepped-into reference is now evaluated in the parent.
      return [...fs.slice(0, -2), { ...parent, at: parent.at + 1 }];
    });

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetAudit.eval.title')}
      testId="evaluate-formula"
      width={560}
      footer={
        <>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="eval-next"
            disabled={!top}
            onClick={evaluate}
          >
            {step || !top ? t('sheetAudit.eval.evaluate') : t('sheetAudit.eval.restart')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="eval-step-in"
            disabled={!step?.cell}
            onClick={() => void stepIn()}
          >
            {t('sheetAudit.eval.stepIn')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="eval-step-out"
            disabled={!frames || frames.length < 2}
            onClick={stepOut}
          >
            {t('sheetAudit.eval.stepOut')}
          </button>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetAudit.close')}
          </button>
        </>
      }
    >
      {empty && <p data-testid="eval-empty">{t('sheetAudit.noFormula')}</p>}
      {frames?.map((f, i) => {
        const s = f.at < f.trace.steps.length ? f.trace.steps[f.at] : null;
        const current = i === frames.length - 1;
        return (
          <div key={i} className={current ? css.frame : css.frameParent} data-testid="eval-frame">
            <div className={css.frameHead}>
              {f.trace.cell} <code>{f.trace.formula}</code>
            </div>
            <div className={css.evalText} data-testid={current ? 'eval-text' : undefined}>
              {s ? (
                <>
                  {s.before}
                  <u data-testid={current ? 'eval-part' : undefined}>{s.part}</u>
                  {s.after}
                </>
              ) : (
                <strong data-testid={current ? 'eval-result' : undefined}>{f.trace.result}</strong>
              )}
            </div>
          </div>
        );
      })}
      {top && (
        <p className={styles.hint}>
          {step ? t('sheetAudit.eval.hint') : t('sheetAudit.eval.done')}
          {step?.cell ? ` ${t('sheetAudit.eval.canStepIn', { cell: step.cell.label })}` : ''}
        </p>
      )}
    </Dialog>
  );
}

// ------------------------------------------------------------------ error checking

/** Walks the findings of a sheet one at a time; ignore, fix, rules. */
export function ErrorCheckDialog({
  host,
  sheet,
  rules,
  onRules,
  onGoto,
  onEvaluate,
  onDone,
  onClose,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  rules: ErrorRules;
  onRules: (r: ErrorRules) => void;
  /** Select the flagged cell in the grid. */
  onGoto: (row: number, col: number) => void;
  /** Open step-by-step evaluation of a cell. */
  onEvaluate: (row: number, col: number) => void;
  onDone: () => void | Promise<void>;
  onClose: () => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [list, setList] = useState<ErrorFinding[] | null>(null);
  const [index, setIndex] = useState(0);
  const [showRules, setShowRules] = useState(false);

  const reload = useCallback(
    async (keep: number) => {
      const l = (await host.exec<ErrorFinding[]>({ cmd: 'errorCheck', sheet, rules })) ?? [];
      setList(l);
      setIndex(Math.max(0, Math.min(keep, l.length - 1)));
    },
    [host, sheet, rules],
  );
  useEffect(() => {
    let cancelled = false;
    void host
      .exec<ErrorFinding[]>({ cmd: 'errorCheck', sheet, rules })
      .then((l) => {
        if (cancelled) return;
        setList(l ?? []);
        setIndex(0);
      })
      .catch(fail);
    return () => {
      cancelled = true;
    };
  }, [host, sheet, rules, fail]);

  const f = list && list.length > 0 ? list[Math.min(index, list.length - 1)] : null;
  useEffect(() => {
    if (f) onGoto(f.row, f.col);
    // Only when the finding shown changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [f?.row, f?.col]);

  const act = async (g: () => Promise<unknown>) => {
    try {
      await g();
      await onDone();
      await reload(index);
    } catch (e) {
      fail(e);
    }
  };
  const ignore = () =>
    act(async () => {
      if (f) await host.exec({ cmd: 'ignoreError', sheet, row: f.row, col: f.col, rule: f.rule });
    });
  const fix = () =>
    act(async () => {
      if (!f) return;
      if (f.rule === 'inconsistentFormula' && f.suggestion) {
        await host.setInputs(sheet, [[f.row, f.col, `=${f.suggestion}`]]);
      } else if (f.rule === 'numberAsText') {
        const c = await host.cell(sheet, f.row, f.col);
        await host.setInputs(sheet, [[f.row, f.col, c.input.trim()]]);
      }
    });
  const reset = () => act(() => host.exec({ cmd: 'resetIgnoredErrors' }));

  const detail = (x: ErrorFinding) => {
    switch (x.rule) {
      case 'inconsistentFormula':
        return t('sheetAudit.errors.detail.inconsistentFormula', {
          formula: `=${x.suggestion ?? ''}`,
        });
      case 'numberAsText':
        return t('sheetAudit.errors.detail.numberAsText', { number: x.number ?? '' });
      case 'emptyRef':
        return t('sheetAudit.errors.detail.emptyRef', { cell: x.empty ?? '' });
      default:
        return t(`sheetAudit.errors.detail.${x.rule}`);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetAudit.errors.title')}
      testId="error-check"
      width={480}
      footer={
        <>
          <button
            type="button"
            className={styles.btn}
            data-testid="err-prev"
            disabled={!f || index === 0}
            onClick={() => setIndex(index - 1)}
          >
            {t('sheetAudit.errors.previous')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="err-next"
            disabled={!f || !list || index >= list.length - 1}
            onClick={() => setIndex(index + 1)}
          >
            {t('sheetAudit.errors.next')}
          </button>
          <button type="button" className={styles.btnPrimary} onClick={onClose}>
            {t('sheetAudit.close')}
          </button>
        </>
      }
    >
      {list && !f && <p data-testid="err-none">{t('sheetAudit.errors.none')}</p>}
      {f && list && (
        <div className={styles.form} data-testid="err-finding" data-rule={f.rule}>
          <p className={styles.hint}>
            {t('sheetAudit.errors.count', { index: index + 1, count: list.length })}
          </p>
          <strong data-testid="err-cell">{a1({ row: f.row, col: f.col })}</strong>
          <p data-testid="err-rule">{t(`sheetAudit.errors.rules.${f.rule}`)}</p>
          <p className={styles.hint}>{detail(f)}</p>
          <div className={css.actions}>
            {(f.rule === 'inconsistentFormula' || f.rule === 'numberAsText') && (
              <button
                type="button"
                className={styles.btn}
                data-testid="err-fix"
                onClick={() => void fix()}
              >
                {t(
                  f.rule === 'numberAsText'
                    ? 'sheetAudit.errors.convert'
                    : 'sheetAudit.errors.copyFormula',
                )}
              </button>
            )}
            {f.rule === 'evalError' && (
              <button
                type="button"
                className={styles.btn}
                data-testid="err-steps"
                onClick={() => onEvaluate(f.row, f.col)}
              >
                {t('sheetAudit.errors.showSteps')}
              </button>
            )}
            <button
              type="button"
              className={styles.btn}
              data-testid="err-ignore"
              onClick={() => void ignore()}
            >
              {t('sheetAudit.errors.ignore')}
            </button>
          </div>
        </div>
      )}
      <button
        type="button"
        className={styles.btn}
        data-testid="err-rules-toggle"
        aria-expanded={showRules}
        onClick={() => setShowRules((s) => !s)}
      >
        {t('sheetAudit.errors.options')}
      </button>
      {showRules && (
        <div className={styles.form} data-testid="err-rules">
          {RULES.map((r) => (
            <label key={r} className={styles.check}>
              <input
                type="checkbox"
                data-testid={`err-rule-${r}`}
                checked={rules[r]}
                onChange={(e) => onRules({ ...rules, [r]: e.target.checked })}
              />
              {t(`sheetAudit.errors.rules.${r}`)}
            </label>
          ))}
          <button
            type="button"
            className={styles.btn}
            data-testid="err-reset"
            onClick={() => void reset()}
          >
            {t('sheetAudit.errors.reset')}
          </button>
        </div>
      )}
    </Dialog>
  );
}

// ------------------------------------------------------------------ watch window

/** Floating list of watched cells, refreshed after every change. */
export function WatchWindow({
  host,
  watches,
  revision,
  selection,
  sheet,
  onAdd,
  onRemove,
  onGoto,
  onClose,
}: {
  host: SheetHost;
  watches: AuditCell[];
  revision: number;
  /** The current selection (added by "add watch"). */
  selection: Range;
  sheet: number;
  onAdd: (sheet: number, r: Range) => void;
  onRemove: (index: number) => void;
  onGoto: (c: AuditCell) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<WatchRow[]>([]);
  const [picked, setPicked] = useState<number | null>(null);
  useEffect(() => {
    let cancelled = false;
    const ask =
      watches.length === 0
        ? Promise.resolve<WatchRow[]>([])
        : host.exec<WatchRow[]>({ cmd: 'watchValues', cells: watches });
    void ask.then((r) => !cancelled && setRows(r ?? [])).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, watches, revision]);
  const cols = ['book', 'sheet', 'name', 'cell', 'value', 'formula'] as const;
  return (
    <div
      className={css.watch}
      role="dialog"
      aria-label={t('sheetAudit.watchWin.title')}
      data-testid="watch-window"
    >
      <div className={css.watchHead}>
        <strong>{t('sheetAudit.watchWin.title')}</strong>
        <button
          type="button"
          className={styles.btn}
          data-testid="watch-add"
          onClick={() => onAdd(sheet, selection)}
        >
          {t('sheetAudit.watchWin.add')}
        </button>
        <button
          type="button"
          className={styles.btn}
          data-testid="watch-remove"
          disabled={picked === null}
          onClick={() => {
            if (picked === null) return;
            onRemove(picked);
            setPicked(null);
          }}
        >
          {t('sheetAudit.watchWin.remove')}
        </button>
        <button
          type="button"
          className={styles.btn}
          aria-label={t('sheetAudit.close')}
          onClick={onClose}
        >
          ×
        </button>
      </div>
      {watches.length === 0 ? (
        <p className={styles.hint}>{t('sheetAudit.watchWin.empty')}</p>
      ) : (
        <table className={css.watchTable}>
          <thead>
            <tr>
              {cols.map((c) => (
                <th key={c}>{t(`sheetAudit.watchWin.${c}`)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr
                key={i}
                data-testid="watch-row"
                aria-selected={picked === i}
                className={picked === i ? css.picked : undefined}
                onClick={() => setPicked(i)}
                onDoubleClick={() => watches[i] && onGoto(watches[i]!)}
              >
                {cols.map((c) => (
                  <td key={c} data-col={c}>
                    {r[c] ?? ''}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

// ------------------------------------------------------------------ calculation options

/** Calculation mode, iteration and precision (§18.2.2 `calcPr`). */
export function CalcOptionsDialog({
  host,
  onClose,
  onDone,
  fail,
}: {
  host: SheetHost;
  onClose: () => void;
  onDone: (s: CalcState) => void | Promise<void>;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [opts, setOpts] = useState<CalcSettings | null>(null);
  const [count, setCount] = useState('100');
  const [delta, setDelta] = useState('0.001');
  const [error, setError] = useState('');
  // Turning precision as displayed on asks first, like the spreadsheet users know:
  // stored numbers are rounded for good.
  const [askPrecision, setAskPrecision] = useState(false);
  useEffect(() => {
    let cancelled = false;
    void host
      .exec<CalcState>({ cmd: 'calcOptions' })
      .then((s) => {
        if (cancelled || !s) return;
        setOpts(s.options);
        setCount(String(s.options.iterateCount));
        setDelta(String(s.options.iterateDelta));
      })
      .catch(fail);
    return () => {
      cancelled = true;
    };
  }, [host, fail]);

  const save = async () => {
    if (!opts) return;
    const it = parseIteration(count, delta);
    if (!it) {
      setError(t('sheetAudit.opts.invalid', { max: MAX_ITERATIONS }));
      return;
    }
    try {
      const s = await host.exec<CalcState>({
        cmd: 'setCalcOptions',
        options: { ...opts, iterateCount: it[0], iterateDelta: it[1] },
      });
      if (s) await onDone(s);
      onClose();
    } catch (e) {
      fail(e);
    }
  };

  const modes: CalcMode[] = ['auto', 'autoNoTable', 'manual'];
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetAudit.opts.title')}
      testId="calc-options"
      width={440}
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="calc-ok"
            disabled={!opts}
            onClick={() => void save()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      {opts && (
        <div className={styles.form}>
          <fieldset className={css.group}>
            <legend>{t('sheetAudit.opts.mode')}</legend>
            {modes.map((m) => (
              <label key={m} className={styles.check}>
                <input
                  type="radio"
                  name="calc-mode"
                  data-testid={`calc-mode-${m}`}
                  checked={opts.mode === m}
                  onChange={() => setOpts({ ...opts, mode: m })}
                />
                {t(`sheetAudit.opts.${m}`)}
              </label>
            ))}
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="calc-on-save"
                checked={opts.calcOnSave}
                disabled={opts.mode !== 'manual'}
                onChange={(e) => setOpts({ ...opts, calcOnSave: e.target.checked })}
              />
              {t('sheetAudit.opts.calcOnSave')}
            </label>
          </fieldset>
          <fieldset className={css.group}>
            <legend>{t('sheetAudit.opts.iteration')}</legend>
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="calc-iterate"
                checked={opts.iterate}
                onChange={(e) => setOpts({ ...opts, iterate: e.target.checked })}
              />
              {t('sheetAudit.opts.iterate')}
            </label>
            <label>
              {t('sheetAudit.opts.maxIterations')}
              <input
                data-testid="calc-count"
                inputMode="numeric"
                value={count}
                disabled={!opts.iterate}
                onChange={(e) => setCount(e.target.value)}
              />
            </label>
            <label>
              {t('sheetAudit.opts.maxChange')}
              <input
                data-testid="calc-delta"
                inputMode="decimal"
                value={delta}
                disabled={!opts.iterate}
                onChange={(e) => setDelta(e.target.value)}
              />
            </label>
          </fieldset>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="calc-precision"
              checked={!opts.fullPrecision}
              onChange={(e) => {
                if (e.target.checked) setAskPrecision(true);
                else setOpts({ ...opts, fullPrecision: true });
              }}
            />
            {t('sheetAudit.opts.precision')}
          </label>
          {!opts.fullPrecision && (
            <p className={styles.hint}>{t('sheetAudit.opts.precisionWarn')}</p>
          )}
          {error && (
            <p className={styles.error} role="alert" data-testid="calc-error">
              {error}
            </p>
          )}
          {askPrecision && (
            <ConfirmDialog
              title={t('sheetAudit.opts.precisionConfirmTitle')}
              message={t('sheetAudit.opts.precisionConfirm')}
              testId="calc-precision-confirm"
              onClose={() => setAskPrecision(false)}
              onOk={() => {
                setAskPrecision(false);
                setOpts({ ...opts, fullPrecision: false });
              }}
            />
          )}
        </div>
      )}
    </Dialog>
  );
}

// ------------------------------------------------------------------ status & warning

/** Status-bar part: "calculate" when results are out of date, circular references. */
export function CalcStatus({
  state,
  onCalculate,
}: {
  state: CalcState | null;
  onCalculate: () => void;
}) {
  const { t } = useTranslation();
  const circular = useMemo(() => state?.circular?.map((c) => c.label) ?? [], [state]);
  if (!state) return null;
  return (
    <>
      {circular.length > 0 && (
        <span data-testid="status-circular" className={css.circular}>
          {t('sheetAudit.status.circular', {
            cells: circular.slice(0, 3).join(', ') + (circular.length > 3 ? '…' : ''),
          })}
        </span>
      )}
      {state.needsCalc && (
        <button
          type="button"
          className={css.calcButton}
          data-testid="status-calculate"
          onClick={onCalculate}
        >
          {t('sheetAudit.status.calculate')}
        </button>
      )}
    </>
  );
}

/** Warning shown when a calculation finds a new circular reference. */
export function CircularWarning({
  labels,
  onOptions,
  onClose,
}: {
  labels: string;
  onOptions: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetAudit.circular.title')}
      testId="circular-warning"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onOptions}>
            {t('sheetAudit.options')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="circular-ok"
            onClick={onClose}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <p>{t('sheetAudit.circular.body')}</p>
      <p className={styles.hint}>{t('sheetAudit.circular.cells', { cells: labels })}</p>
    </Dialog>
  );
}

/** Most cells marked at once by background error checking. */
const MAX_MARKS = 400;

/**
 * Background error checking: a small green triangle in the top-left corner of every cell a
 * rule flags (on the visible part of the sheet), and, when the active cell is one of them,
 * the warning button beside it whose menu names the problem and offers the fixes (use the
 * neighbours' formula, convert to number, show calculation steps), ignore, edit in the
 * formula bar and the rules. The list is asked for again after every change.
 */
export function ErrorMarks({
  host,
  view,
  revision,
  rules,
  toScreen,
  active,
  onDone,
  onEdit,
  onEvaluate,
  onOptions,
  fail,
}: {
  host: SheetHost;
  view: SheetView;
  revision: number;
  rules: ErrorRules;
  toScreen: (r: Box | null) => Box | null;
  /** The active cell (its finding gets the warning button). */
  active: { row: number; col: number };
  onDone: () => void | Promise<void>;
  /** Start editing the active cell in the formula bar. */
  onEdit: () => void;
  onEvaluate: (row: number, col: number) => void;
  onOptions: () => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [marks, setMarks] = useState<{ f: ErrorFinding; box: Box }[]>([]);
  // The cell whose warning menu is open (moving to another cell closes it).
  const [openAt, setOpenAt] = useState<string | null>(null);
  const sheet = view.sheet;
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const list = (await host.exec<ErrorFinding[]>({ cmd: 'errorCheck', sheet, rules })) ?? [];
      // One mark per cell (the first rule that flags it).
      const seen = new Set<string>();
      const cells = list.filter((f) => {
        const k = `${f.row}:${f.col}`;
        if (seen.has(k)) return false;
        seen.add(k);
        return true;
      });
      const boxes = await Promise.all(
        cells.slice(0, MAX_MARKS).map(async (f) => ({
          f,
          box: toScreen(await host.rangeRect(view, f.row, f.col, f.row, f.col)),
        })),
      );
      if (!cancelled) setMarks(boxes.filter((m): m is { f: ErrorFinding; box: Box } => !!m.box));
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, sheet, view, rules, revision, toScreen]);
  const here = marks.find((m) => m.f.row === active.row && m.f.col === active.col);
  const key = `${sheet}:${active.row}:${active.col}`;
  const open = openAt === key;
  const setOpen = (o: boolean) => setOpenAt(o ? key : null);

  const act = (g: () => Promise<unknown>) => {
    setOpen(false);
    void g()
      .then(() => onDone())
      .catch(fail);
  };
  return (
    <>
      {marks.map(({ f, box }) => (
        <div
          key={`${f.row}:${f.col}`}
          className={css.errorMark}
          data-testid="error-mark"
          data-cell={a1({ row: f.row, col: f.col })}
          style={{ left: box.x + 1, top: box.y + 1 }}
        />
      ))}
      {here && (
        <div
          className={css.errorTag}
          style={{ left: Math.max(0, here.box.x - 22), top: here.box.y }}
        >
          <button
            type="button"
            className={css.errorTagButton}
            data-testid="error-tag"
            aria-label={t('sheetAudit.errors.tag')}
            title={t(`sheetAudit.errors.rules.${here.f.rule}`)}
            aria-haspopup="menu"
            aria-expanded={open}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => setOpen(!open)}
          >
            <TriangleAlert size={14} aria-hidden="true" />
          </button>
          {open && (
            <div className={css.errorTagMenu} role="menu" data-testid="error-tag-menu">
              <p className={css.errorTagTitle}>{t(`sheetAudit.errors.rules.${here.f.rule}`)}</p>
              {here.f.rule === 'inconsistentFormula' && here.f.suggestion && (
                <button
                  type="button"
                  role="menuitem"
                  data-action="copyFormula"
                  onClick={() =>
                    act(() =>
                      host.setInputs(sheet, [[here.f.row, here.f.col, `=${here.f.suggestion}`]]),
                    )
                  }
                >
                  {t('sheetAudit.errors.copyFormula')}
                </button>
              )}
              {here.f.rule === 'numberAsText' && (
                <button
                  type="button"
                  role="menuitem"
                  data-action="convert"
                  onClick={() =>
                    act(async () => {
                      const c = await host.cell(sheet, here.f.row, here.f.col);
                      await host.setInputs(sheet, [[here.f.row, here.f.col, c.input.trim()]]);
                    })
                  }
                >
                  {t('sheetAudit.errors.convert')}
                </button>
              )}
              {here.f.rule === 'evalError' && (
                <button
                  type="button"
                  role="menuitem"
                  data-action="evaluate"
                  onClick={() => {
                    setOpen(false);
                    onEvaluate(here.f.row, here.f.col);
                  }}
                >
                  {t('sheetAudit.errors.showSteps')}
                </button>
              )}
              <button
                type="button"
                role="menuitem"
                data-action="ignore"
                onClick={() =>
                  act(() =>
                    host.exec({
                      cmd: 'ignoreError',
                      sheet,
                      row: here.f.row,
                      col: here.f.col,
                      rule: here.f.rule,
                    }),
                  )
                }
              >
                {t('sheetAudit.errors.ignore')}
              </button>
              <button
                type="button"
                role="menuitem"
                data-action="edit"
                onClick={() => {
                  setOpen(false);
                  onEdit();
                }}
              >
                {t('sheetAudit.errors.editInBar')}
              </button>
              <button
                type="button"
                role="menuitem"
                data-action="options"
                onClick={() => {
                  setOpen(false);
                  onOptions();
                }}
              >
                {t('sheetAudit.errors.options')}
              </button>
            </div>
          )}
        </div>
      )}
    </>
  );
}
