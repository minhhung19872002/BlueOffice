/**
 * "Format chart" task pane (CORE-CHART-005 / CORE-CHART-007) shared by BlueSheet, BlueSlide and
 * BlueDoc: a chart element picker and, for the picked element, the options the chart model
 * holds (ECMA-376 Part 1, §21.2.2 — axes, gridlines, titles, legend, series, data labels, data
 * table, trend lines, error bars, drop / high-low lines, up/down bars). The pane only reads the
 * `elements` of the host's chart info and sends `elements` patches through the host's chart
 * design command: every change is one undo step of the host.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  ChartAxisId,
  ChartAxisInfo,
  ChartAxisPatch,
  ChartDataLabels,
  ChartElementsInfo,
  ChartElementsPatch,
  ChartErrorBars,
  ChartSeriesInfo,
  ChartSeriesPatch,
  ChartTrendline,
  ChartTrendlineKind,
} from '@blueoffice/platform';
import { useId, useState, type ReactNode } from 'react';
import styles from './ChartFormat.module.css';

type T = ReturnType<typeof useTranslation>['t'];

/** Parses a number typed with a decimal point or comma; `null` when it is not a number. */
export function parseNumber(text: string): number | null {
  const s = text.trim().replace(/\s/g, '').replace(',', '.');
  if (!s || !/^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(s)) return null;
  const v = Number(s);
  return Number.isFinite(v) ? v : null;
}

/**
 * Custom error amounts as typed: numbers separated by `;` or spaces (decimal comma allowed), or
 * else a range reference.
 */
export function parseAmounts(text: string): { ref: string | null; values: number[] } {
  const s = text.trim();
  if (!s) return { ref: null, values: [] };
  const parts = s.split(/[;\s]+/).filter(Boolean);
  const nums = parts.map(parseNumber);
  if (nums.every((n): n is number => n !== null)) return { ref: null, values: nums };
  return { ref: s, values: [] };
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className={styles.row}>
      <span>{label}</span>
      {children}
    </label>
  );
}

function Check({
  label,
  checked,
  onChange,
  testId,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  testId: string;
  disabled?: boolean;
}) {
  return (
    <label className={styles.check}>
      <input
        type="checkbox"
        data-testid={testId}
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

function Choice({
  label,
  value,
  options,
  onChange,
  testId,
  disabled,
}: {
  label: string;
  value: string;
  options: [string, string][];
  onChange: (v: string) => void;
  testId: string;
  disabled?: boolean;
}) {
  return (
    <Row label={label}>
      <select
        data-testid={testId}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      >
        {options.map(([v, text]) => (
          <option key={v} value={v}>
            {text}
          </option>
        ))}
      </select>
    </Row>
  );
}

/** A text field committed on Enter or when it loses focus (one change = one command). */
function Field({
  label,
  value,
  onCommit,
  testId,
  placeholder,
  list,
  disabled,
}: {
  label: string;
  value: string;
  onCommit: (v: string) => void;
  testId: string;
  placeholder?: string;
  list?: string[];
  disabled?: boolean;
}) {
  const [draft, setDraft] = useState(value);
  // The chart's value changed (the command ran, undo, another chart): the field shows it.
  const [seen, setSeen] = useState(value);
  if (seen !== value) {
    setSeen(value);
    setDraft(value);
  }
  // A plain id for the datalist (React's ids hold colons).
  const id = `cf-list-${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const commit = () => {
    if (draft === value) return;
    onCommit(draft);
    // The field shows the chart's value: the typed one comes back once the chart has taken it.
    setDraft(value);
  };
  return (
    <Row label={label}>
      <>
        <input
          type="text"
          data-testid={testId}
          value={draft}
          placeholder={placeholder}
          disabled={disabled}
          list={list ? id : undefined}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            if (e.key === 'Escape') setDraft(value);
          }}
        />
        {list && (
          <datalist id={id}>
            {list.map((v) => (
              <option key={v} value={v} />
            ))}
          </datalist>
        )}
      </>
    </Row>
  );
}

/** A number field; empty = automatic (`null`) when `auto` is given as its placeholder. */
function Num({
  label,
  value,
  onCommit,
  testId,
  auto,
  disabled,
}: {
  label: string;
  value: number | null | undefined;
  onCommit: (v: number | null) => void;
  testId: string;
  auto?: string;
  disabled?: boolean;
}) {
  const text = value === null || value === undefined ? '' : String(value);
  return (
    <Field
      label={label}
      value={text}
      testId={testId}
      placeholder={auto}
      disabled={disabled}
      onCommit={(s) => {
        if (!s.trim()) {
          if (auto !== undefined) onCommit(null);
          return;
        }
        const v = parseNumber(s);
        if (v !== null) onCommit(v);
      }}
    />
  );
}

function Color({
  label,
  value,
  onChange,
  testId,
  t,
}: {
  label: string;
  value: string | null;
  onChange: (v: string | null) => void;
  testId: string;
  t: T;
}) {
  const hex = value && /^#[0-9a-f]{6}$/i.test(value) ? value : '#808080';
  return (
    <Row label={label}>
      <span className={styles.color}>
        <input
          type="color"
          data-testid={testId}
          value={hex}
          aria-label={label}
          onChange={(e) => onChange(e.target.value.toUpperCase())}
        />
        <button
          type="button"
          data-testid={`${testId}-auto`}
          disabled={value === null}
          onClick={() => onChange(null)}
        >
          {t('chartFormat.automatic')}
        </button>
      </span>
    </Row>
  );
}

const shown = (d: ChartDataLabels | null | undefined) =>
  !!d && !!(d.value || d.percent || d.category || d.series);

const NUM_FORMATS = ['General', '0', '0.00', '#,##0', '#,##0.00', '0%', '0.0%', '0.00E+00'];
const UNITS = [
  'hundreds',
  'thousands',
  'tenThousands',
  'hundredThousands',
  'millions',
  'tenMillions',
  'hundredMillions',
  'billions',
  'trillions',
];
const SYMBOLS = [
  'none',
  'circle',
  'square',
  'diamond',
  'triangle',
  'x',
  'plus',
  'star',
  'dash',
  'dot',
];
const TREND_KINDS: ChartTrendlineKind[] = ['linear', 'exp', 'log', 'poly', 'power', 'movingAvg'];
const LINE_LIKE = ['line', 'scatter', 'radar'];
const PIE_LIKE = ['pie', 'doughnut', 'ofPie'];

/** Label positions a series of `kind` can take. */
function labelPositions(kind: string, stacked: boolean): string[] {
  if (kind === 'bar')
    return stacked ? ['ctr', 'inEnd', 'inBase'] : ['outEnd', 'inEnd', 'inBase', 'ctr'];
  if (kind === 'line' || kind === 'scatter') return ['t', 'b', 'l', 'r', 'ctr'];
  if (kind === 'pie' || kind === 'ofPie') return ['bestFit', 'outEnd', 'inEnd', 'ctr'];
  return [];
}

interface SectionProps {
  info: ChartElementsInfo;
  apply: (p: ChartElementsPatch) => void;
  t: T;
}

function AxisSection({ info, apply, t, id }: SectionProps & { id: ChartAxisId }) {
  const a: ChartAxisInfo | null | undefined = info.axes?.[id];
  if (!a) return null;
  const set = (p: ChartAxisPatch) => apply({ axes: { [id]: p } });
  const value = a.kind === 'value';
  const auto = t('chartFormat.automatic');
  const cross = a.crossesAt !== null ? 'value' : (a.crosses ?? 'auto');
  const tick = (key: string): [string, string] => [key, t(`chartFormat.tick.${key}`)];
  return (
    <>
      <Check
        label={t('chartFormat.axis.show')}
        checked={!a.deleted}
        testId="cf-axis-show"
        onChange={(v) => set({ deleted: !v })}
      />
      <Field
        label={t('chartFormat.axis.title')}
        value={a.title ?? ''}
        testId="cf-axis-title"
        onCommit={(v) => set({ title: v.trim() || null })}
      />
      {id === 'cat' && !value && (
        <>
          <Choice
            label={t('chartFormat.axis.type')}
            value={a.kind}
            testId="cf-axis-kind"
            options={[
              ['category', t('chartFormat.axis.category')],
              ['date', t('chartFormat.axis.date')],
            ]}
            onChange={(v) => set({ kind: v === 'date' ? 'date' : 'category' })}
          />
          {a.kind === 'date' && (
            <Choice
              label={t('chartFormat.axis.baseUnit')}
              value={a.baseUnit ?? 'days'}
              testId="cf-axis-base-unit"
              options={['days', 'months', 'years'].map((u) => [u, t(`chartFormat.baseUnit.${u}`)])}
              onChange={(v) => set({ baseUnit: v === 'days' ? null : v })}
            />
          )}
        </>
      )}
      {value && (
        <>
          <Num
            label={t('chartFormat.axis.min')}
            value={a.min}
            auto={auto}
            testId="cf-axis-min"
            onCommit={(v) => set({ min: v })}
          />
          <Num
            label={t('chartFormat.axis.max')}
            value={a.max}
            auto={auto}
            testId="cf-axis-max"
            onCommit={(v) => set({ max: v })}
          />
          <Num
            label={t('chartFormat.axis.majorUnit')}
            value={a.majorUnit}
            auto={auto}
            testId="cf-axis-major"
            onCommit={(v) => set({ majorUnit: v })}
          />
          <Num
            label={t('chartFormat.axis.minorUnit')}
            value={a.minorUnit}
            auto={auto}
            testId="cf-axis-minor"
            onCommit={(v) => set({ minorUnit: v })}
          />
        </>
      )}
      {value && id !== 'cat' && (
        <>
          <Check
            label={t('chartFormat.axis.log')}
            checked={a.logBase !== null}
            testId="cf-axis-log"
            onChange={(v) => set({ logBase: v ? 10 : null })}
          />
          {a.logBase !== null && (
            <Num
              label={t('chartFormat.axis.logBase')}
              value={a.logBase}
              testId="cf-axis-log-base"
              onCommit={(v) => v !== null && set({ logBase: v })}
            />
          )}
        </>
      )}
      <Check
        label={t(value ? 'chartFormat.axis.reverseValues' : 'chartFormat.axis.reverseCategories')}
        checked={a.reverse}
        testId="cf-axis-reverse"
        onChange={(v) => set({ reverse: v })}
      />
      <Choice
        label={t('chartFormat.axis.crosses')}
        value={cross}
        testId="cf-axis-crosses"
        options={[
          ['auto', auto],
          ['min', t('chartFormat.axis.crossesMin')],
          ['max', t('chartFormat.axis.crossesMax')],
          ['value', t('chartFormat.axis.crossesAt')],
        ]}
        onChange={(v) => {
          if (v === 'value') set({ crossesAt: a.crossesAt ?? 0 });
          else if (v === 'auto') set({ crosses: null, crossesAt: null });
          else set({ crosses: v });
        }}
      />
      {cross === 'value' && (
        <Num
          label={t('chartFormat.axis.crossesValue')}
          value={a.crossesAt}
          testId="cf-axis-crosses-at"
          onCommit={(v) => v !== null && set({ crossesAt: v })}
        />
      )}
      {value && (
        <>
          <Choice
            label={t('chartFormat.axis.dispUnit')}
            value={a.dispUnit ?? 'none'}
            testId="cf-axis-disp-unit"
            options={[
              ['none', t('chartFormat.none')],
              ...UNITS.map((u): [string, string] => [u, t(`chartFormat.unit.${u}`)]),
            ]}
            onChange={(v) => set({ dispUnit: v === 'none' ? null : v })}
          />
          <Check
            label={t('chartFormat.axis.dispUnitLabel')}
            checked={a.dispUnitLabel}
            disabled={!a.dispUnit}
            testId="cf-axis-disp-unit-label"
            onChange={(v) => set({ dispUnitLabel: v })}
          />
        </>
      )}
      <Field
        label={t('chartFormat.numFmt')}
        value={a.numFmt ?? ''}
        placeholder={auto}
        list={NUM_FORMATS}
        testId="cf-axis-num-fmt"
        onCommit={(v) => set({ numFmt: v.trim() || null })}
      />
      <Choice
        label={t('chartFormat.axis.majorTick')}
        value={a.majorTick ?? 'out'}
        testId="cf-axis-major-tick"
        options={[tick('out'), tick('in'), tick('cross'), tick('none')]}
        onChange={(v) => set({ majorTick: v === 'out' ? null : v })}
      />
      <Choice
        label={t('chartFormat.axis.minorTick')}
        value={a.minorTick ?? 'none'}
        testId="cf-axis-minor-tick"
        options={[tick('none'), tick('in'), tick('out'), tick('cross')]}
        onChange={(v) => set({ minorTick: v === 'none' ? null : v })}
      />
      <Choice
        label={t('chartFormat.axis.labelPos')}
        value={a.labelPos ?? 'nextTo'}
        testId="cf-axis-label-pos"
        options={['nextTo', 'high', 'low', 'none'].map((p) => [p, t(`chartFormat.axisLabel.${p}`)])}
        onChange={(v) => set({ labelPos: v === 'nextTo' ? null : v })}
      />
      {id !== 'val2' && (
        <>
          <Check
            label={t('chartFormat.axis.majorGrid')}
            checked={a.majorGrid}
            testId="cf-axis-major-grid"
            onChange={(v) => set({ majorGrid: v })}
          />
          <Check
            label={t('chartFormat.axis.minorGrid')}
            checked={a.minorGrid}
            testId="cf-axis-minor-grid"
            onChange={(v) => set({ minorGrid: v })}
          />
        </>
      )}
    </>
  );
}

function SeriesSection({
  info,
  apply,
  t,
  s,
  go,
}: SectionProps & { s: ChartSeriesInfo; go: (element: string, series: number) => void }) {
  const set = (p: Omit<ChartSeriesPatch, 'index'>) => apply({ series: [{ index: s.index, ...p }] });
  const group = info.groups[s.plot];
  const line = LINE_LIKE.includes(s.kind);
  return (
    <>
      {line ? (
        <Color
          label={t('chartFormat.series.line')}
          value={s.line ?? s.fill}
          testId="cf-series-line"
          t={t}
          onChange={(v) => set({ line: v, fill: null })}
        />
      ) : (
        <Color
          label={t('chartFormat.series.fill')}
          value={s.fill}
          testId="cf-series-fill"
          t={t}
          onChange={(v) => set({ fill: v })}
        />
      )}
      {s.lineWidth !== null && (
        <Num
          label={t('chartFormat.series.lineWidth')}
          value={s.lineWidth}
          auto={t('chartFormat.automatic')}
          testId="cf-series-line-width"
          onCommit={(v) => set({ lineWidth: v })}
        />
      )}
      {s.marker && (
        <>
          <Choice
            label={t('chartFormat.series.marker')}
            value={s.marker.symbol}
            testId="cf-series-marker"
            options={SYMBOLS.map((m) => [m, t(`chartFormat.marker.${m}`)])}
            onChange={(v) => set({ marker: { symbol: v, size: s.marker?.size ?? 5 } })}
          />
          <Num
            label={t('chartFormat.series.markerSize')}
            value={s.marker.size ?? 5}
            disabled={s.marker.symbol === 'none'}
            testId="cf-series-marker-size"
            onCommit={(v) =>
              v !== null &&
              set({ marker: { symbol: s.marker?.symbol ?? 'circle', size: Math.round(v) } })
            }
          />
        </>
      )}
      {(s.kind === 'line' || s.kind === 'scatter') && !group?.threeD && (
        <Check
          label={t('chartFormat.series.smooth')}
          checked={s.smooth}
          testId="cf-series-smooth"
          onChange={(v) => set({ smooth: v })}
        />
      )}
      {PIE_LIKE.includes(s.kind) && (
        <Num
          label={t('chartFormat.series.explosion')}
          value={s.explosion}
          testId="cf-series-explosion"
          onCommit={(v) => v !== null && set({ explosion: Math.max(0, Math.round(v)) })}
        />
      )}
      {s.canSecondary && (
        <Check
          label={t('chartFormat.series.secondary')}
          checked={s.secondary}
          testId="cf-series-secondary"
          onChange={(v) => set({ secondary: v })}
        />
      )}
      {s.kind === 'bar' && group && !group.threeD && (
        <>
          <Num
            label={t('chartFormat.series.gapWidth')}
            value={group.gapWidth}
            testId="cf-series-gap-width"
            onCommit={(v) =>
              v !== null && apply({ groups: [{ plot: s.plot, gapWidth: Math.round(v) }] })
            }
          />
          <Num
            label={t('chartFormat.series.overlap')}
            value={group.overlap}
            disabled={group.stacked}
            testId="cf-series-overlap"
            onCommit={(v) =>
              v !== null && apply({ groups: [{ plot: s.plot, overlap: Math.round(v) }] })
            }
          />
        </>
      )}
      <button
        type="button"
        className={styles.button}
        data-testid="cf-series-labels"
        onClick={() => go('labels', s.index)}
      >
        {t('chartFormat.element.labels')}…
      </button>
      {s.canTrend && (
        <>
          <button
            type="button"
            className={styles.button}
            data-testid="cf-series-trendline"
            onClick={() => go('trendline', s.index)}
          >
            {t('chartFormat.element.trendline')}…
          </button>
          <button
            type="button"
            className={styles.button}
            data-testid="cf-series-error-bars"
            onClick={() => go('errorBars', s.index)}
          >
            {t('chartFormat.element.errorBars')}…
          </button>
        </>
      )}
    </>
  );
}

function SeriesPicker({
  t,
  series,
  value,
  onChange,
  all,
}: {
  t: T;
  series: ChartSeriesInfo[];
  value: number | null;
  onChange: (v: number | null) => void;
  /** Label of the "whole chart" choice, when there is one. */
  all?: string;
}) {
  return (
    <Row label={t('chartFormat.series.pick')}>
      <select
        data-testid="cf-series-pick"
        value={value === null ? 'all' : String(value)}
        onChange={(e) => onChange(e.target.value === 'all' ? null : Number(e.target.value))}
      >
        {all !== undefined && <option value="all">{all}</option>}
        {series.map((s) => (
          <option key={s.index} value={s.index}>
            {s.name || t('chartFormat.series.unnamed', { n: s.index + 1 })}
          </option>
        ))}
      </select>
    </Row>
  );
}

function LabelsSection({
  info,
  apply,
  t,
  series,
  setSeries,
}: SectionProps & { series: number | null; setSeries: (v: number | null) => void }) {
  const [point, setPoint] = useState<number | null>(null);
  const s = series === null ? null : (info.series.find((x) => x.index === series) ?? null);
  const own =
    s && point !== null ? s.pointLabels.find((p) => p.point === point)?.labels : undefined;
  // What the scope shows now: its own labels, else those it follows.
  const cur: ChartDataLabels = (point !== null ? own : undefined) ?? s?.labels ?? info.labels ?? {};
  const inherits = s ? (point !== null ? own === undefined : s.labels === null) : false;
  const kind = (s ?? info.series[0])?.kind ?? 'bar';
  const stacked = !!info.groups[(s ?? info.series[0])?.plot ?? 0]?.stacked;
  const pie = PIE_LIKE.includes(kind);
  const send = (next: ChartDataLabels | null) => {
    if (!s) apply({ labels: next });
    else if (point === null) apply({ series: [{ index: s.index, labels: next }] });
    else apply({ series: [{ index: s.index, pointLabels: [{ point, labels: next }] }] });
  };
  const set = (p: ChartDataLabels) => send({ ...cur, ...p });
  const on = shown(cur);
  const positions = labelPositions(kind, stacked);
  return (
    <>
      <SeriesPicker
        t={t}
        series={info.series}
        value={series}
        onChange={(v) => {
          setSeries(v);
          setPoint(null);
        }}
        all={t('chartFormat.labels.allSeries')}
      />
      {s && (
        <Row label={t('chartFormat.labels.point')}>
          <select
            data-testid="cf-labels-point"
            value={point === null ? 'all' : String(point)}
            onChange={(e) => setPoint(e.target.value === 'all' ? null : Number(e.target.value))}
          >
            <option value="all">{t('chartFormat.labels.allPoints')}</option>
            {Array.from({ length: s.points }, (_, i) => (
              <option key={i} value={i}>
                {s.categories[i] || String(i + 1)}
              </option>
            ))}
          </select>
        </Row>
      )}
      <Check
        label={t('chartFormat.labels.show')}
        checked={on}
        testId="cf-labels-on"
        onChange={(v) =>
          send(v ? { ...cur, ...(pie ? { percent: true } : { value: true }) } : s ? {} : null)
        }
      />
      {s && (
        <button
          type="button"
          className={styles.button}
          data-testid="cf-labels-inherit"
          disabled={inherits}
          onClick={() => send(null)}
        >
          {t(point === null ? 'chartFormat.labels.followChart' : 'chartFormat.labels.followSeries')}
        </button>
      )}
      <Check
        label={t('chartFormat.labels.value')}
        checked={!!cur.value}
        testId="cf-labels-value"
        onChange={(v) => set({ value: v })}
      />
      <Check
        label={t('chartFormat.labels.category')}
        checked={!!cur.category}
        testId="cf-labels-category"
        onChange={(v) => set({ category: v })}
      />
      <Check
        label={t('chartFormat.labels.series')}
        checked={!!cur.series}
        testId="cf-labels-series"
        onChange={(v) => set({ series: v })}
      />
      {pie && (
        <>
          <Check
            label={t('chartFormat.labels.percent')}
            checked={!!cur.percent}
            testId="cf-labels-percent"
            onChange={(v) => set({ percent: v })}
          />
          <Check
            label={t('chartFormat.labels.leaderLines')}
            checked={cur.leaderLines !== false}
            disabled={point !== null}
            testId="cf-labels-leader"
            onChange={(v) => set({ leaderLines: v ? null : false })}
          />
        </>
      )}
      {positions.length > 0 && (
        <Choice
          label={t('chartFormat.labels.position')}
          value={cur.pos ?? 'auto'}
          testId="cf-labels-pos"
          disabled={!on}
          options={[
            ['auto', t('chartFormat.automatic')],
            ...positions.map((p): [string, string] => [p, t(`chartFormat.labelPos.${p}`)]),
          ]}
          onChange={(v) => set({ pos: v === 'auto' ? null : v })}
        />
      )}
      <Field
        label={t('chartFormat.numFmt')}
        value={cur.numFmt ?? ''}
        placeholder={t('chartFormat.automatic')}
        list={NUM_FORMATS}
        disabled={!on}
        testId="cf-labels-num-fmt"
        onCommit={(v) => set({ numFmt: v.trim() || null })}
      />
      <Field
        label={t('chartFormat.labels.separator')}
        value={cur.separator ?? ''}
        placeholder="; "
        disabled={!on}
        testId="cf-labels-separator"
        onCommit={(v) => set({ separator: v || null })}
      />
    </>
  );
}

function TrendlineSection({
  info,
  apply,
  t,
  series,
  setSeries,
}: SectionProps & { series: number | null; setSeries: (v: number | null) => void }) {
  const list = info.series.filter((s) => s.canTrend);
  const s = list.find((x) => x.index === series) ?? list[0];
  const [at, setAt] = useState(0);
  if (!s) return <p className={styles.hint}>{t('chartFormat.trendline.unavailable')}</p>;
  const lines = s.trendlines;
  const cur: ChartTrendline | undefined = lines[Math.min(at, lines.length - 1)];
  const index = cur ? lines.indexOf(cur) : -1;
  const send = (next: ChartTrendline[]) =>
    apply({ series: [{ index: s.index, trendlines: next }] });
  const set = (p: Partial<ChartTrendline>) =>
    cur && send(lines.map((l, i) => (i === index ? { ...l, ...p } : l)));
  const name = (l: ChartTrendline) => l.name || t(`chartFormat.trendline.kind.${l.kind}`);
  const moving = cur?.kind === 'movingAvg';
  const canIntercept = !!cur && ['linear', 'poly', 'exp'].includes(cur.kind);
  return (
    <>
      <SeriesPicker
        t={t}
        series={list}
        value={s.index}
        onChange={(v) => {
          setSeries(v);
          setAt(0);
        }}
      />
      <ul className={styles.list} data-testid="cf-trend-list">
        {lines.map((l, i) => (
          <li key={i}>
            <button
              type="button"
              aria-pressed={i === index}
              data-testid={`cf-trend-item-${i}`}
              onClick={() => setAt(i)}
            >
              {name(l)}
            </button>
            <button
              type="button"
              aria-label={t('chartFormat.trendline.remove', { name: name(l) })}
              title={t('chartFormat.trendline.remove', { name: name(l) })}
              data-testid={`cf-trend-remove-${i}`}
              onClick={() => {
                send(lines.filter((_, k) => k !== i));
                setAt(0);
              }}
            >
              ✕
            </button>
          </li>
        ))}
      </ul>
      <button
        type="button"
        className={styles.button}
        data-testid="cf-trend-add"
        onClick={() => {
          send([...lines, { kind: 'linear' }]);
          setAt(lines.length);
        }}
      >
        {t('chartFormat.trendline.add')}
      </button>
      {cur && (
        <>
          <Choice
            label={t('chartFormat.trendline.type')}
            value={cur.kind}
            testId="cf-trend-kind"
            options={TREND_KINDS.map((k) => [k, t(`chartFormat.trendline.kind.${k}`)])}
            onChange={(v) => {
              const kind = TREND_KINDS.find((k) => k === v) ?? 'linear';
              set({ kind, order: kind === 'poly' || kind === 'movingAvg' ? 2 : 0 });
            }}
          />
          {cur.kind === 'poly' && (
            <Num
              label={t('chartFormat.trendline.order')}
              value={cur.order ?? 2}
              testId="cf-trend-order"
              onCommit={(v) =>
                v !== null && set({ order: Math.min(6, Math.max(2, Math.round(v))) })
              }
            />
          )}
          {moving && (
            <Num
              label={t('chartFormat.trendline.period')}
              value={cur.order ?? 2}
              testId="cf-trend-period"
              onCommit={(v) => v !== null && set({ order: Math.max(2, Math.round(v)) })}
            />
          )}
          <Field
            label={t('chartFormat.trendline.name')}
            value={cur.name ?? ''}
            placeholder={t('chartFormat.automatic')}
            testId="cf-trend-name"
            onCommit={(v) => set({ name: v.trim() || null })}
          />
          {!moving && (
            <>
              <Num
                label={t('chartFormat.trendline.forward')}
                value={cur.forward ?? 0}
                testId="cf-trend-forward"
                onCommit={(v) => v !== null && set({ forward: v > 0 ? v : null })}
              />
              <Num
                label={t('chartFormat.trendline.backward')}
                value={cur.backward ?? 0}
                testId="cf-trend-backward"
                onCommit={(v) => v !== null && set({ backward: v > 0 ? v : null })}
              />
              {canIntercept && (
                <>
                  <Check
                    label={t('chartFormat.trendline.setIntercept')}
                    checked={cur.intercept !== null && cur.intercept !== undefined}
                    testId="cf-trend-intercept-on"
                    onChange={(v) => set({ intercept: v ? (cur.kind === 'exp' ? 1 : 0) : null })}
                  />
                  {cur.intercept !== null && cur.intercept !== undefined && (
                    <Num
                      label={t('chartFormat.trendline.intercept')}
                      value={cur.intercept}
                      testId="cf-trend-intercept"
                      onCommit={(v) => v !== null && set({ intercept: v })}
                    />
                  )}
                </>
              )}
              <Check
                label={t('chartFormat.trendline.dispEq')}
                checked={!!cur.dispEq}
                testId="cf-trend-eq"
                onChange={(v) => set({ dispEq: v })}
              />
              <Check
                label={t('chartFormat.trendline.dispR2')}
                checked={!!cur.dispR2}
                testId="cf-trend-r2"
                onChange={(v) => set({ dispR2: v })}
              />
            </>
          )}
        </>
      )}
    </>
  );
}

function ErrorBarsSection({
  info,
  apply,
  t,
  series,
  setSeries,
}: SectionProps & { series: number | null; setSeries: (v: number | null) => void }) {
  const list = info.series.filter((s) => s.canTrend);
  const s = list.find((x) => x.index === series) ?? list[0];
  if (!s) return <p className={styles.hint}>{t('chartFormat.errorBars.unavailable')}</p>;
  const e = s.errorBars;
  const send = (next: ChartErrorBars | null) =>
    apply({ series: [{ index: s.index, errorBars: next }] });
  const set = (p: Partial<ChartErrorBars>) => e && send({ ...e, ...p });
  const amounts = (ref: string | null | undefined, values: (number | null)[] | undefined) =>
    ref ?? (values ?? []).filter((v): v is number => v !== null).join('; ');
  const custom = (side: 'plus' | 'minus', text: string) => {
    const a = parseAmounts(text);
    set(
      side === 'plus' ? { plusRef: a.ref, plus: a.values } : { minusRef: a.ref, minus: a.values },
    );
  };
  return (
    <>
      <SeriesPicker t={t} series={list} value={s.index} onChange={setSeries} />
      <Check
        label={t('chartFormat.errorBars.show')}
        checked={!!e}
        testId="cf-err-on"
        onChange={(v) => send(v ? { kind: 'both', valueType: 'fixedVal', value: 1 } : null)}
      />
      {e && (
        <>
          {s.kind === 'scatter' || s.kind === 'bubble' ? (
            <Choice
              label={t('chartFormat.errorBars.axis')}
              value={e.dir === 'x' ? 'x' : 'y'}
              testId="cf-err-dir"
              options={[
                ['y', t('chartFormat.errorBars.axisY')],
                ['x', t('chartFormat.errorBars.axisX')],
              ]}
              onChange={(v) => set({ dir: v === 'x' ? 'x' : null })}
            />
          ) : null}
          <Choice
            label={t('chartFormat.errorBars.direction')}
            value={e.kind}
            testId="cf-err-kind"
            options={['both', 'plus', 'minus'].map((k) => [
              k,
              t(`chartFormat.errorBars.kind.${k}`),
            ])}
            onChange={(v) => set({ kind: v })}
          />
          <Choice
            label={t('chartFormat.errorBars.endStyle')}
            value={e.noEndCap ? 'none' : 'cap'}
            testId="cf-err-cap"
            options={[
              ['cap', t('chartFormat.errorBars.cap')],
              ['none', t('chartFormat.errorBars.noCap')],
            ]}
            onChange={(v) => set({ noEndCap: v === 'none' })}
          />
          <Choice
            label={t('chartFormat.errorBars.amount')}
            value={e.valueType}
            testId="cf-err-type"
            options={['fixedVal', 'percentage', 'stdDev', 'stdErr', 'cust'].map((k) => [
              k,
              t(`chartFormat.errorBars.type.${k}`),
            ])}
            onChange={(v) =>
              // Custom bars start from the fixed amount on both sides.
              v === 'cust'
                ? set({
                    valueType: v,
                    plus: [e.value ?? 1],
                    minus: [e.value ?? 1],
                    plusRef: null,
                    minusRef: null,
                  })
                : set({ valueType: v, value: v === 'percentage' ? 5 : 1 })
            }
          />
          {['fixedVal', 'percentage', 'stdDev'].includes(e.valueType) && (
            <Num
              label={t('chartFormat.errorBars.value')}
              value={e.value ?? 1}
              testId="cf-err-value"
              onCommit={(v) => v !== null && v >= 0 && set({ value: v })}
            />
          )}
          {e.valueType === 'cust' && (
            <>
              <Field
                label={t('chartFormat.errorBars.plus')}
                value={amounts(e.plusRef, e.plus)}
                testId="cf-err-plus"
                onCommit={(v) => custom('plus', v)}
              />
              <Field
                label={t('chartFormat.errorBars.minus')}
                value={amounts(e.minusRef, e.minus)}
                testId="cf-err-minus"
                onCommit={(v) => custom('minus', v)}
              />
              <p className={styles.hint}>{t('chartFormat.errorBars.customHint')}</p>
            </>
          )}
        </>
      )}
    </>
  );
}

/** The element ids the picker offers for a chart. */
export function chartElementIds(info: ChartElementsInfo): string[] {
  const ids = ['chartArea'];
  if (info.axes) ids.push('plotArea');
  ids.push('title', 'legend');
  if (info.axes) {
    ids.push('axis.cat', 'axis.val');
    if (info.axes.val2) ids.push('axis.val2');
    ids.push('gridlines');
  }
  for (const s of info.series) ids.push(`series.${s.index}`);
  ids.push('labels');
  if (info.has.dataTable) ids.push('dataTable');
  if (info.series.some((s) => s.canTrend)) ids.push('trendline', 'errorBars');
  if (info.has.dropLines || info.has.upDownBars) ids.push('lines');
  return ids;
}

/** Name of an element in the picker. */
export function chartElementName(id: string, info: ChartElementsInfo, t: T): string {
  if (id.startsWith('series.')) {
    const s = info.series.find((x) => x.index === Number(id.slice(7)));
    const name = s?.name || t('chartFormat.series.unnamed', { n: (s?.index ?? 0) + 1 });
    return t('chartFormat.element.series', { name });
  }
  if (id === 'axis.cat') {
    return t(info.has.scatter ? 'chartFormat.element.axisX' : 'chartFormat.element.axisCat');
  }
  if (id === 'axis.val') return t('chartFormat.element.axisVal');
  if (id === 'axis.val2') return t('chartFormat.element.axisVal2');
  return t(`chartFormat.element.${id}`);
}

export interface ChartFormatPaneProps {
  /**
   * Elements of the selected chart (`elements` of the host's chart info); `undefined` = no chart
   * is selected, `null` = the selected chart has no element options (chartex charts).
   */
  info: ChartElementsInfo | null | undefined;
  /** Sends one change through the host's chart design command. */
  apply: (patch: ChartElementsPatch) => void;
  /** Element to show (set by the host when a command opens the pane on one). */
  element?: string;
}

export function ChartFormatPane({ info, apply, element }: ChartFormatPaneProps) {
  const { t } = useTranslation();
  const [picked, setPicked] = useState(element ?? 'chartArea');
  const [series, setSeries] = useState<number | null>(null);
  // The host asked for another element.
  const [asked, setAsked] = useState(element);
  if (asked !== element) {
    setAsked(element);
    if (element) setPicked(element);
  }
  if (!info) {
    return (
      <div className={styles.pane} data-testid="chart-format-pane">
        <p className={styles.empty}>
          {t(info === null ? 'chartFormat.unsupported' : 'chartFormat.noChart')}
        </p>
      </div>
    );
  }
  const ids = chartElementIds(info);
  const el = ids.includes(picked) ? picked : 'chartArea';
  const props: SectionProps = { info, apply, t };
  const go = (id: string, s: number) => {
    setSeries(s);
    setPicked(id);
  };
  const axis = (id: ChartAxisId) => <AxisSection {...props} id={id} />;
  const grid = (id: 'cat' | 'val', minor: boolean) => {
    const a = info.axes?.[id];
    const key = minor ? 'minorGrid' : 'majorGrid';
    return (
      <Check
        label={t(`chartFormat.gridlines.${id}${minor ? 'Minor' : 'Major'}`)}
        checked={!!a?.[key]}
        testId={`cf-grid-${id}-${minor ? 'minor' : 'major'}`}
        onChange={(v) => apply({ axes: { [id]: { [key]: v } } })}
      />
    );
  };
  let body: ReactNode = null;
  if (el === 'chartArea') {
    body = (
      <>
        <Color
          label={t('chartFormat.fill')}
          value={info.chartArea.fill}
          testId="cf-area-fill"
          t={t}
          onChange={(v) => apply({ chartArea: { fill: v } })}
        />
        <Color
          label={t('chartFormat.border')}
          value={info.chartArea.line}
          testId="cf-area-line"
          t={t}
          onChange={(v) => apply({ chartArea: { line: v } })}
        />
        <Check
          label={t('chartFormat.rounded')}
          checked={info.chartArea.rounded}
          testId="cf-area-rounded"
          onChange={(v) => apply({ chartArea: { rounded: v } })}
        />
      </>
    );
  } else if (el === 'plotArea') {
    body = (
      <Color
        label={t('chartFormat.fill')}
        value={info.plotArea.fill}
        testId="cf-plot-fill"
        t={t}
        onChange={(v) => apply({ plotArea: { fill: v } })}
      />
    );
  } else if (el === 'title') {
    body = (
      <>
        <Field
          label={t('chartFormat.titleText')}
          value={info.title.text}
          testId="cf-title-text"
          onCommit={(v) => apply({ title: { text: v } })}
        />
        <Check
          label={t('chartFormat.overlay')}
          checked={info.title.overlay}
          disabled={!info.title.text}
          testId="cf-title-overlay"
          onChange={(v) => apply({ title: { overlay: v } })}
        />
      </>
    );
  } else if (el === 'legend') {
    body = (
      <>
        <Choice
          label={t('chartFormat.legend.position')}
          value={info.legend.pos}
          testId="cf-legend-pos"
          options={['none', 'r', 't', 'l', 'b', 'tr'].map((p) => [
            p,
            t(`chartFormat.legend.pos.${p}`),
          ])}
          onChange={(v) => apply({ legend: { pos: v } })}
        />
        <Check
          label={t('chartFormat.overlay')}
          checked={info.legend.overlay}
          disabled={info.legend.pos === 'none'}
          testId="cf-legend-overlay"
          onChange={(v) => apply({ legend: { overlay: v } })}
        />
      </>
    );
  } else if (el === 'axis.cat') body = axis('cat');
  else if (el === 'axis.val') body = axis('val');
  else if (el === 'axis.val2') body = axis('val2');
  else if (el === 'gridlines') {
    body = (
      <>
        {grid('val', false)}
        {grid('val', true)}
        {grid('cat', false)}
        {grid('cat', true)}
        <Color
          label={t('chartFormat.gridlines.color')}
          value={info.gridlines.color}
          testId="cf-grid-color"
          t={t}
          onChange={(v) => apply({ gridlines: { color: v } })}
        />
        <Num
          label={t('chartFormat.gridlines.width')}
          value={info.gridlines.width}
          auto={t('chartFormat.automatic')}
          testId="cf-grid-width"
          onCommit={(v) => apply({ gridlines: { width: v } })}
        />
      </>
    );
  } else if (el.startsWith('series.')) {
    const s = info.series.find((x) => x.index === Number(el.slice(7)));
    body = s ? <SeriesSection {...props} s={s} go={go} /> : null;
  } else if (el === 'labels') {
    body = <LabelsSection {...props} series={series} setSeries={setSeries} />;
  } else if (el === 'dataTable') {
    body = (
      <>
        <Check
          label={t('chartFormat.dataTable.show')}
          checked={info.dataTable.show}
          testId="cf-table-show"
          onChange={(v) => apply({ dataTable: { show: v } })}
        />
        <Check
          label={t('chartFormat.dataTable.keys')}
          checked={info.dataTable.keys}
          disabled={!info.dataTable.show}
          testId="cf-table-keys"
          onChange={(v) => apply({ dataTable: { keys: v } })}
        />
      </>
    );
  } else if (el === 'trendline') {
    body = <TrendlineSection {...props} series={series} setSeries={setSeries} />;
  } else if (el === 'errorBars') {
    body = <ErrorBarsSection {...props} series={series} setSeries={setSeries} />;
  } else if (el === 'lines') {
    const any = (key: 'dropLines' | 'hiLowLines' | 'upDownBars') =>
      info.groups.some((g) => g[key] && !(key === 'hiLowLines' && g.kind === 'stock'));
    body = (
      <>
        <Check
          label={t('chartFormat.lines.drop')}
          checked={any('dropLines')}
          disabled={!info.has.dropLines}
          testId="cf-lines-drop"
          onChange={(v) => apply({ groups: [{ dropLines: v }] })}
        />
        <Check
          label={t('chartFormat.lines.hiLow')}
          checked={any('hiLowLines')}
          disabled={!info.has.lines}
          testId="cf-lines-hilow"
          onChange={(v) => apply({ groups: [{ hiLowLines: v }] })}
        />
        <Check
          label={t('chartFormat.lines.upDown')}
          checked={any('upDownBars')}
          disabled={!info.has.upDownBars}
          testId="cf-lines-updown"
          onChange={(v) => apply({ groups: [{ upDownBars: v }] })}
        />
      </>
    );
  }
  return (
    <div className={styles.pane} data-testid="chart-format-pane">
      <label className={styles.picker}>
        <span>{t('chartFormat.element.label')}</span>
        <select
          data-testid="chart-format-element"
          value={el}
          onChange={(e) => setPicked(e.target.value)}
        >
          {ids.map((id) => (
            <option key={id} value={id}>
              {chartElementName(id, info, t)}
            </option>
          ))}
        </select>
      </label>
      <fieldset className={styles.section} data-testid={`cf-section-${el.replace('.', '-')}`}>
        <legend>{chartElementName(el, info, t)}</legend>
        {body}
      </fieldset>
    </div>
  );
}
