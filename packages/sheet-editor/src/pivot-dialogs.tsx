/**
 * Pivot table dialogs: field settings (subtotals, layout, sorting, filters,
 * grouping), value field settings (summary, "show values as", number
 * format), report options (layout, grand totals, style), calculated fields,
 * move and change source. Every dialog sends the engine commands
 * (`pivotUpdate`, `pivotGroup`, `pivotCalcField`, `pivotMove`,
 * `pivotChangeSource`); the report itself is computed by the engine.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  PIVOT_BASE_NEXT,
  PIVOT_BASE_PREVIOUS,
  PIVOT_DATE_FILTERS,
  type PivotAreas,
  type PivotDateFilterType,
  type PivotFieldFilter,
  type PivotFilterType,
  type PivotFunction,
  type PivotInfo,
  type PivotLabels,
  type PivotLayout,
  type PivotSettings,
  type PivotShowAs,
  type PivotSort,
  type SheetHost,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState, type ReactNode } from 'react';
import { parseA1 } from './grid';
import styles from './SheetEditor.module.css';

export const PIVOT_FUNCTIONS: PivotFunction[] = [
  'sum',
  'count',
  'average',
  'max',
  'min',
  'product',
  'countNums',
  'stdDev',
  'stdDevp',
  'var',
  'varp',
];

/** The areas of a pivot table as `pivotUpdate` takes them. */
export const areasOf = (p: PivotInfo): PivotAreas => ({
  rows: [...p.rows],
  cols: [...p.cols],
  values: p.values.map((v) => ({
    field: v.field,
    function: v.function,
    name: v.name ?? undefined,
    showAs: v.showAs,
    baseField: v.baseField,
    baseItem: v.baseItem,
    numFmt: v.numFmt ?? undefined,
  })),
  filters: p.filters.map((f) => ({ field: f.field, items: [...f.items] })),
});

export const SHOW_AS: PivotShowAs[] = [
  'normal',
  'percentOfTotal',
  'percentOfCol',
  'percentOfRow',
  'percent',
  'percentOfParentRow',
  'percentOfParentCol',
  'percentOfParent',
  'difference',
  'percentDiff',
  'runTotal',
  'percentOfRunningTotal',
  'rankAscending',
  'rankDescending',
  'index',
];

/** Calculations relative to a base field (and, for some, a base item). */
const NEEDS_BASE_FIELD: PivotShowAs[] = [
  'percent',
  'percentOfParent',
  'difference',
  'percentDiff',
  'runTotal',
  'percentOfRunningTotal',
  'rankAscending',
  'rankDescending',
];
const NEEDS_BASE_ITEM: PivotShowAs[] = ['percent', 'difference', 'percentDiff'];

/** Number formats offered for value fields (codes of `bo_sheet::style::NUMBER_PRESETS`). */
const NUM_FORMATS: { id: string; code: string }[] = [
  { id: 'general', code: 'General' },
  { id: 'number', code: '#,##0.00' },
  { id: 'integer', code: '#,##0' },
  { id: 'currencyVnd', code: '#,##0 [$₫-42A]' },
  { id: 'percent', code: '0%' },
  { id: 'percent2', code: '0.00%' },
  { id: 'date', code: 'dd/mm/yyyy' },
];

const LABEL_OPS = [
  'Equal',
  'NotEqual',
  'BeginsWith',
  'NotBeginsWith',
  'EndsWith',
  'NotEndsWith',
  'Contains',
  'NotContains',
  'GreaterThan',
  'GreaterThanOrEqual',
  'LessThan',
  'LessThanOrEqual',
  'Between',
  'NotBetween',
] as const;
const VALUE_OPS = [
  'Equal',
  'NotEqual',
  'GreaterThan',
  'GreaterThanOrEqual',
  'LessThan',
  'LessThanOrEqual',
  'Between',
  'NotBetween',
] as const;
type Op = (typeof LABEL_OPS)[number];
const opKey = (op: Op) => op.charAt(0).toLowerCase() + op.slice(1);

interface Common {
  host: SheetHost;
  sheet: number;
  pivot: PivotInfo;
  labels: PivotLabels;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}

/** `pivotUpdate` with the current areas and some changes. */
async function update(c: Common, patch: Partial<PivotAreas> & PivotSettings) {
  await c.host.exec({
    cmd: 'pivotUpdate',
    sheet: c.sheet,
    index: c.pivot.index,
    labels: c.labels,
    ...areasOf(c.pivot),
    ...patch,
  });
}

function Footer({
  onClose,
  onOk,
  testId,
}: {
  onClose: () => void;
  onOk: () => void;
  testId: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.btn} onClick={onClose}>
        {t('sheetEditor.dlg.cancel')}
      </button>
      <button type="button" className={styles.btnPrimary} data-testid={testId} onClick={onOk}>
        {t('sheetEditor.dlg.ok')}
      </button>
    </>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <fieldset className={styles.pivotSection}>
      <legend>{title}</legend>
      {children}
    </fieldset>
  );
}

const num = (s: string): number | null => {
  const v = Number(s.replace(',', '.'));
  return s.trim() === '' || Number.isNaN(v) ? null : v;
};

// ----------------------------------------------------------------- row / column field

type FilterMode = 'none' | 'top' | 'bottom' | 'label' | 'value' | 'date';
const isDateKind = (k: string) => (PIVOT_DATE_FILTERS as readonly string[]).includes(k);
/** A date filter value for the date input: serials (1900 system) become ISO dates. */
const isoDate = (v: string): string => {
  const n = Number(v);
  if (v.trim() === '' || Number.isNaN(n)) return v;
  // Serial 25569 = 1970-01-01 (ECMA-376 Part 1, §18.17.4.1).
  return new Date(Math.round((n - 25569) * 86400000)).toISOString().slice(0, 10);
};

export function PivotFieldDialog({ field, ...c }: Common & { field: number }) {
  const { t } = useTranslation();
  const p = c.pivot;
  const info = p.fields[field];
  const opts = p.fieldOptions[field] ?? {
    subtotal: true,
    subtotalTop: true,
    blankRow: false,
    repeatLabels: false,
    sort: 'manual' as PivotSort,
    sortBy: null,
    hidden: [],
  };
  const current = p.fieldFilters.find((f) => f.field === field);
  const [subtotal, setSubtotal] = useState(opts.subtotal);
  const [subtotalTop, setSubtotalTop] = useState(opts.subtotalTop);
  const [blankRow, setBlankRow] = useState(opts.blankRow);
  const [repeatLabels, setRepeatLabels] = useState(opts.repeatLabels);
  const [showAll, setShowAll] = useState(opts.showAll ?? false);
  const [sort, setSort] = useState<PivotSort>(opts.sort);
  const [sortBy, setSortBy] = useState(opts.sortBy === null ? 'label' : String(opts.sortBy));
  const initialMode: FilterMode = !current
    ? 'none'
    : isDateKind(current.kind)
      ? 'date'
      : ['count', 'percent', 'sum'].includes(current.kind)
        ? current.top
          ? 'top'
          : 'bottom'
        : current.kind.startsWith('caption')
          ? 'label'
          : 'value';
  const [mode, setMode] = useState<FilterMode>(initialMode);
  const [topType, setTopType] = useState<'count' | 'percent' | 'sum'>(
    current && ['count', 'percent', 'sum'].includes(current.kind)
      ? (current.kind as 'count' | 'percent' | 'sum')
      : 'count',
  );
  const opOf = (k?: PivotFilterType): Op => {
    const rest = k?.replace(/^caption|^value/, '') as Op | undefined;
    return rest && (LABEL_OPS as readonly string[]).includes(rest) ? rest : 'Equal';
  };
  const [op, setOp] = useState<Op>(opOf(current?.kind));
  const [count, setCount] = useState(String(current?.count || 10));
  const dateValue = (v: string | undefined) =>
    current && isDateKind(current.kind) ? isoDate(v ?? '') : (v ?? '');
  const [value1, setValue1] = useState(dateValue(current?.value1));
  const [value2, setValue2] = useState(dateValue(current?.value2));
  const [measure, setMeasure] = useState(current?.measure ?? 0);
  const [dateKind, setDateKind] = useState<PivotDateFilterType>(
    current && isDateKind(current.kind) ? (current.kind as PivotDateFilterType) : 'dateBetween',
  );
  const dateFixed = dateKind.startsWith('date');
  const group = info?.group ?? null;
  const groupable = !!info && !info.derived && info.formula === null;
  const [groupBy, setGroupBy] = useState<'none' | 'dates' | 'numbers'>(
    group?.by === 'dates' || group?.by === 'numbers' ? group.by : 'none',
  );
  const [parts, setParts] = useState(
    group?.by === 'dates'
      ? { years: group.years, quarters: group.quarters, months: group.months, days: group.days }
      : { years: false, quarters: false, months: true, days: false },
  );
  const [start, setStart] = useState(group?.by === 'numbers' ? String(group.start) : '');
  const [end, setEnd] = useState(group?.by === 'numbers' ? String(group.end) : '');
  const [step, setStep] = useState(group?.by === 'numbers' ? String(group.interval) : '10');
  const [error, setError] = useState('');
  const hasValues = p.values.length > 0;

  const ok = async () => {
    let filter: PivotFieldFilter | null = null;
    if (mode === 'top' || mode === 'bottom') {
      filter = {
        field,
        kind: topType,
        measure,
        value1: '',
        value2: '',
        top: mode === 'top',
        count: num(count) ?? 10,
      };
    } else if (mode === 'date') {
      filter = {
        field,
        kind: dateKind,
        measure: 0,
        value1: dateFixed ? value1 : '',
        value2: dateKind.endsWith('Between') ? value2 : '',
        top: true,
        count: 0,
      };
    } else if (mode === 'label' || mode === 'value') {
      filter = {
        field,
        kind: `${mode === 'label' ? 'caption' : 'value'}${op}` as PivotFilterType,
        measure,
        value1,
        value2: op.endsWith('Between') ? value2 : '',
        top: true,
        count: 0,
      };
    }
    try {
      await update(c, {
        fieldOptions: [
          {
            field,
            subtotal,
            subtotalTop,
            blankRow,
            repeatLabels,
            showAll,
            sort,
            sortBy: sortBy === 'label' ? null : Number(sortBy),
            hidden: opts.hidden,
          },
        ],
        fieldFilters: [
          ...p.fieldFilters.filter((f) => f.field !== field),
          ...(filter ? [filter] : []),
        ],
      });
      const before =
        group?.by === 'dates'
          ? `dates:${group.years}${group.quarters}${group.months}${group.days}`
          : group?.by === 'numbers'
            ? `numbers:${group.start}:${group.end}:${group.interval}`
            : 'none';
      const after =
        groupBy === 'dates'
          ? `dates:${parts.years}${parts.quarters}${parts.months}${parts.days}`
          : groupBy === 'numbers'
            ? `numbers:${start}:${end}:${step}`
            : 'none';
      if (groupable && before !== after) {
        await c.host.exec({
          cmd: 'pivotGroup',
          sheet: c.sheet,
          index: p.index,
          field,
          by: groupBy,
          ...parts,
          start: num(start),
          end: num(end),
          interval: num(step) ?? 1,
          labels: c.labels,
        });
      }
      await c.onDone();
      c.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const ops = mode === 'label' ? LABEL_OPS : VALUE_OPS;
  return (
    <Dialog
      open
      width={460}
      onOpenChange={(o) => !o && c.onClose()}
      title={t('sheetEditor.pivot.fieldSettings', { field: info?.name ?? '' })}
      testId="pivot-field-dialog"
      footer={<Footer onClose={c.onClose} onOk={() => void ok()} testId="pivot-field-ok" />}
    >
      <div className={styles.form}>
        <Section title={t('sheetEditor.pivot.subtotals')}>
          <label className={styles.check}>
            <input
              type="radio"
              name="pivot-subtotal"
              checked={subtotal}
              data-testid="pivot-subtotal-auto"
              onChange={() => setSubtotal(true)}
            />
            {t('sheetEditor.pivot.subtotalAuto')}
          </label>
          <label className={styles.check}>
            <input
              type="radio"
              name="pivot-subtotal"
              checked={!subtotal}
              data-testid="pivot-subtotal-none"
              onChange={() => setSubtotal(false)}
            />
            {t('sheetEditor.pivot.subtotalNone')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={subtotalTop}
              disabled={!subtotal}
              data-testid="pivot-subtotal-top"
              onChange={(e) => setSubtotalTop(e.target.checked)}
            />
            {t('sheetEditor.pivot.subtotalTop')}
          </label>
        </Section>
        <Section title={t('sheetEditor.pivot.layoutSection')}>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={repeatLabels}
              data-testid="pivot-repeat-labels"
              onChange={(e) => setRepeatLabels(e.target.checked)}
            />
            {t('sheetEditor.pivot.repeatLabels')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={showAll}
              data-testid="pivot-show-all"
              onChange={(e) => setShowAll(e.target.checked)}
            />
            {t('sheetEditor.pivot.showAll')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={blankRow}
              data-testid="pivot-blank-row"
              onChange={(e) => setBlankRow(e.target.checked)}
            />
            {t('sheetEditor.pivot.blankRow')}
          </label>
        </Section>
        <Section title={t('sheetEditor.pivot.sortSection')}>
          <div className={styles.row}>
            <select
              aria-label={t('sheetEditor.pivot.sortSection')}
              data-testid="pivot-sort"
              value={sort}
              onChange={(e) => setSort(e.target.value as PivotSort)}
            >
              {(['manual', 'ascending', 'descending'] as const).map((s) => (
                <option key={s} value={s}>
                  {t(`sheetEditor.pivot.sort.${s}`)}
                </option>
              ))}
            </select>
            <select
              aria-label={t('sheetEditor.pivot.sortBy')}
              data-testid="pivot-sort-by"
              value={sortBy}
              disabled={sort === 'manual'}
              onChange={(e) => setSortBy(e.target.value)}
            >
              <option value="label">{t('sheetEditor.pivot.sortByLabel')}</option>
              {p.values.map((v, i) => (
                <option key={i} value={String(i)}>
                  {v.name ?? p.fields[v.field]?.name}
                </option>
              ))}
            </select>
          </div>
        </Section>
        <Section title={t('sheetEditor.pivot.filterSection')}>
          <div className={styles.row}>
            <select
              aria-label={t('sheetEditor.pivot.filterSection')}
              data-testid="pivot-filter-mode"
              value={mode}
              onChange={(e) => setMode(e.target.value as FilterMode)}
            >
              {(['none', 'top', 'bottom', 'label', 'value', 'date'] as const)
                .filter((m) => hasValues || m === 'none' || m === 'label' || m === 'date')
                .filter((m) => m !== 'date' || !!info?.date)
                .map((m) => (
                  <option key={m} value={m}>
                    {t(`sheetEditor.pivot.filterMode.${m}`)}
                  </option>
                ))}
            </select>
            {(mode === 'top' || mode === 'bottom') && (
              <>
                <input
                  aria-label={t('sheetEditor.pivot.count')}
                  data-testid="pivot-filter-count"
                  size={4}
                  value={count}
                  onChange={(e) => setCount(e.target.value)}
                />
                <select
                  aria-label={t('sheetEditor.pivot.topType')}
                  value={topType}
                  onChange={(e) => setTopType(e.target.value as 'count' | 'percent' | 'sum')}
                >
                  {(['count', 'percent', 'sum'] as const).map((k) => (
                    <option key={k} value={k}>
                      {t(`sheetEditor.pivot.topTypes.${k}`)}
                    </option>
                  ))}
                </select>
              </>
            )}
            {mode === 'date' && (
              <select
                aria-label={t('sheetEditor.pivot.dateFilter')}
                data-testid="pivot-date-filter"
                value={dateKind}
                onChange={(e) => setDateKind(e.target.value as PivotDateFilterType)}
              >
                {PIVOT_DATE_FILTERS.map((k) => (
                  <option key={k} value={k}>
                    {t(`sheetEditor.pivot.dateFilters.${k}`)}
                  </option>
                ))}
              </select>
            )}
            {(mode === 'label' || mode === 'value') && (
              <select
                aria-label={t('sheetEditor.pivot.operator')}
                data-testid="pivot-filter-op"
                value={op}
                onChange={(e) => setOp(e.target.value as Op)}
              >
                {ops.map((o) => (
                  <option key={o} value={o}>
                    {t(`sheetEditor.pivot.ops.${opKey(o)}`)}
                  </option>
                ))}
              </select>
            )}
          </div>
          {(mode === 'top' || mode === 'bottom' || mode === 'value') && (
            <label>
              {t('sheetEditor.pivot.filterBy')}
              <select value={measure} onChange={(e) => setMeasure(Number(e.target.value))}>
                {p.values.map((v, i) => (
                  <option key={i} value={i}>
                    {v.name ?? p.fields[v.field]?.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          {mode === 'date' && dateFixed && (
            <div className={styles.row}>
              <input
                type="date"
                aria-label={t('sheetEditor.pivot.value1')}
                data-testid="pivot-date-value1"
                value={value1}
                onChange={(e) => setValue1(e.target.value)}
              />
              {dateKind.endsWith('Between') && (
                <>
                  {t('sheetEditor.pivot.and')}
                  <input
                    type="date"
                    aria-label={t('sheetEditor.pivot.value2')}
                    data-testid="pivot-date-value2"
                    value={value2}
                    onChange={(e) => setValue2(e.target.value)}
                  />
                </>
              )}
            </div>
          )}
          {(mode === 'label' || mode === 'value') && (
            <div className={styles.row}>
              <input
                aria-label={t('sheetEditor.pivot.value1')}
                data-testid="pivot-filter-value1"
                value={value1}
                onChange={(e) => setValue1(e.target.value)}
              />
              {op.endsWith('Between') && (
                <>
                  {t('sheetEditor.pivot.and')}
                  <input
                    aria-label={t('sheetEditor.pivot.value2')}
                    value={value2}
                    onChange={(e) => setValue2(e.target.value)}
                  />
                </>
              )}
            </div>
          )}
        </Section>
        {groupable && (
          <Section title={t('sheetEditor.pivot.groupSection')}>
            <select
              aria-label={t('sheetEditor.pivot.groupSection')}
              data-testid="pivot-group-by"
              value={groupBy}
              onChange={(e) => setGroupBy(e.target.value as 'none' | 'dates' | 'numbers')}
            >
              {(['none', 'dates', 'numbers'] as const).map((g) => (
                <option key={g} value={g}>
                  {t(`sheetEditor.pivot.group.${g}`)}
                </option>
              ))}
            </select>
            {groupBy === 'dates' && (
              <div className={styles.row}>
                {(['years', 'quarters', 'months', 'days'] as const).map((k) => (
                  <label key={k} className={styles.check}>
                    <input
                      type="checkbox"
                      data-testid={`pivot-group-${k}`}
                      checked={parts[k]}
                      onChange={(e) => setParts({ ...parts, [k]: e.target.checked })}
                    />
                    {t(`sheetEditor.pivot.group.${k}`)}
                  </label>
                ))}
              </div>
            )}
            {groupBy === 'numbers' && (
              <>
                <label>
                  {t('sheetEditor.pivot.group.start')}
                  <input
                    data-testid="pivot-group-start"
                    placeholder={t('sheetEditor.pivot.group.auto')}
                    value={start}
                    onChange={(e) => setStart(e.target.value)}
                  />
                </label>
                <label>
                  {t('sheetEditor.pivot.group.end')}
                  <input
                    data-testid="pivot-group-end"
                    placeholder={t('sheetEditor.pivot.group.auto')}
                    value={end}
                    onChange={(e) => setEnd(e.target.value)}
                  />
                </label>
                <label>
                  {t('sheetEditor.pivot.group.interval')}
                  <input
                    data-testid="pivot-group-interval"
                    value={step}
                    onChange={(e) => setStep(e.target.value)}
                  />
                </label>
              </>
            )}
          </Section>
        )}
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- value field

export function PivotValueDialog({ idx, ...c }: Common & { idx: number }) {
  const { t } = useTranslation();
  const p = c.pivot;
  const v = p.values[idx];
  const [name, setName] = useState(v?.name ?? '');
  const [fn, setFn] = useState<PivotFunction>(v?.function ?? 'sum');
  const [showAs, setShowAs] = useState<PivotShowAs>(v?.showAs ?? 'normal');
  const axisFields = [...p.rows, ...p.cols];
  const [baseField, setBaseField] = useState(
    v && axisFields.includes(v.baseField) ? v.baseField : (axisFields[0] ?? 0),
  );
  const [baseItem, setBaseItem] = useState(v?.baseItem ?? 0);
  const preset = NUM_FORMATS.find((f) => f.code === v?.numFmt);
  const [fmt, setFmt] = useState(v?.numFmt ? (preset ? preset.code : 'custom') : '');
  const [custom, setCustom] = useState(v?.numFmt ?? '');
  const [error, setError] = useState('');
  const fieldName = p.fields[v?.field ?? 0]?.name ?? '';
  const autoName = (f: PivotFunction) =>
    t('sheetEditor.pivot.valueName', { fn: t(`sheetEditor.pivot.fn.${f}`), field: fieldName });

  const ok = async () => {
    if (!v) return;
    const values = areasOf(p).values.map((x, i) =>
      i === idx
        ? {
            ...x,
            name: name.trim() || undefined,
            function: fn,
            showAs,
            baseField: NEEDS_BASE_FIELD.includes(showAs) ? baseField : 0,
            baseItem: NEEDS_BASE_ITEM.includes(showAs) ? baseItem : 0,
            numFmt: fmt === 'custom' ? custom.trim() || undefined : fmt || undefined,
          }
        : x,
    );
    try {
      await update(c, { values });
      await c.onDone();
      c.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const items = p.fields[baseField]?.items ?? [];
  return (
    <Dialog
      open
      width={440}
      onOpenChange={(o) => !o && c.onClose()}
      title={t('sheetEditor.pivot.valueSettings', { field: fieldName })}
      testId="pivot-value-dialog"
      footer={<Footer onClose={c.onClose} onOk={() => void ok()} testId="pivot-value-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.pivot.customName')}
          <input
            data-testid="pivot-value-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label>
          {t('sheetEditor.pivot.summarize')}
          <select
            data-testid="pivot-value-fn"
            value={fn}
            onChange={(e) => {
              const f = e.target.value as PivotFunction;
              if (name === autoName(fn) || name === '') setName(autoName(f));
              setFn(f);
            }}
          >
            {PIVOT_FUNCTIONS.map((f) => (
              <option key={f} value={f}>
                {t(`sheetEditor.pivot.fn.${f}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('sheetEditor.pivot.showAs')}
          <select
            data-testid="pivot-show-as"
            value={showAs}
            onChange={(e) => setShowAs(e.target.value as PivotShowAs)}
          >
            {SHOW_AS.map((s) => (
              <option key={s} value={s}>
                {t(`sheetEditor.pivot.showAsOpt.${s}`)}
              </option>
            ))}
          </select>
        </label>
        {NEEDS_BASE_FIELD.includes(showAs) && (
          <label>
            {t('sheetEditor.pivot.baseField')}
            <select
              data-testid="pivot-base-field"
              value={baseField}
              onChange={(e) => {
                setBaseField(Number(e.target.value));
                setBaseItem(0);
              }}
            >
              {axisFields.map((f) => (
                <option key={f} value={f}>
                  {p.fields[f]?.name}
                </option>
              ))}
            </select>
          </label>
        )}
        {NEEDS_BASE_ITEM.includes(showAs) && (
          <label>
            {t('sheetEditor.pivot.baseItem')}
            <select
              data-testid="pivot-base-item"
              value={baseItem}
              onChange={(e) => setBaseItem(Number(e.target.value))}
            >
              <option value={PIVOT_BASE_PREVIOUS}>{t('sheetEditor.pivot.previous')}</option>
              <option value={PIVOT_BASE_NEXT}>{t('sheetEditor.pivot.next')}</option>
              {items.map((label, i) => (
                <option key={i} value={i}>
                  {label || c.labels.blank}
                </option>
              ))}
            </select>
          </label>
        )}
        <label>
          {t('sheetEditor.pivot.numFmt')}
          <select data-testid="pivot-num-fmt" value={fmt} onChange={(e) => setFmt(e.target.value)}>
            <option value="">{t('sheetEditor.pivot.numFmtSource')}</option>
            {NUM_FORMATS.map((f) => (
              <option key={f.id} value={f.code}>
                {t(`sheetEditor.numFmt.${f.id}`)}
              </option>
            ))}
            <option value="custom">{t('sheetEditor.pivot.numFmtCustom')}</option>
          </select>
        </label>
        {fmt === 'custom' && (
          <label>
            {t('sheetEditor.pivot.numFmtCode')}
            <input
              data-testid="pivot-num-fmt-code"
              value={custom}
              onChange={(e) => setCustom(e.target.value)}
            />
          </label>
        )}
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- report options

export const LAYOUTS: PivotLayout[] = ['compact', 'outline', 'tabular'];

/** "Light 16" style caption of a built-in style name. */
export function styleLabel(name: string, t: (k: string) => string): string {
  const m = /^PivotStyle(Light|Medium|Dark)(\d+)$/.exec(name);
  if (!m) return name;
  return `${t(`sheetEditor.pivot.styleFamily.${(m[1] ?? '').toLowerCase()}`)} ${m[2] ?? ''}`;
}

export function PivotOptionsDialog(c: Common) {
  const { t } = useTranslation();
  const p = c.pivot;
  const [layout, setLayout] = useState<PivotLayout>(p.layout);
  const [rowTotals, setRowTotals] = useState(p.rowGrandTotals);
  const [colTotals, setColTotals] = useState(p.colGrandTotals);
  const [style, setStyle] = useState(
    p.style ?? {
      name: '',
      rowHeaders: true,
      colHeaders: true,
      rowStripes: false,
      colStripes: false,
      lastColumn: true,
    },
  );
  const [names, setNames] = useState<string[]>([]);
  const [error, setError] = useState('');
  useEffect(() => {
    void c.host
      .exec<{ pivotStyles: string[] }>({ cmd: 'presets' })
      .then((r) => setNames(r?.pivotStyles ?? []))
      .catch(c.fail);
    // Loaded once per dialog.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const ok = async () => {
    try {
      await update(c, {
        layout,
        rowGrandTotals: rowTotals,
        colGrandTotals: colTotals,
        ...(style.name ? { style } : {}),
      });
      await c.onDone();
      c.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  const all = names.includes(style.name) || !style.name ? names : [style.name, ...names];
  return (
    <Dialog
      open
      width={440}
      onOpenChange={(o) => !o && c.onClose()}
      title={t('sheetEditor.pivot.options')}
      testId="pivot-options-dialog"
      footer={<Footer onClose={c.onClose} onOk={() => void ok()} testId="pivot-options-ok" />}
    >
      <div className={styles.form}>
        <Section title={t('sheetEditor.pivot.reportLayout')}>
          {LAYOUTS.map((l) => (
            <label key={l} className={styles.check}>
              <input
                type="radio"
                name="pivot-layout"
                data-testid={`pivot-layout-${l}`}
                checked={layout === l}
                onChange={() => setLayout(l)}
              />
              {t(`sheetEditor.pivot.layoutForm.${l}`)}
            </label>
          ))}
        </Section>
        <Section title={t('sheetEditor.pivot.grandTotals')}>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="pivot-row-grand-totals"
              checked={rowTotals}
              onChange={(e) => setRowTotals(e.target.checked)}
            />
            {t('sheetEditor.pivot.rowGrandTotals')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="pivot-col-grand-totals"
              checked={colTotals}
              onChange={(e) => setColTotals(e.target.checked)}
            />
            {t('sheetEditor.pivot.colGrandTotals')}
          </label>
        </Section>
        <Section title={t('sheetEditor.pivot.style')}>
          <select
            aria-label={t('sheetEditor.pivot.style')}
            data-testid="pivot-style"
            value={style.name}
            onChange={(e) => setStyle({ ...style, name: e.target.value })}
          >
            {all.map((n) => (
              <option key={n} value={n}>
                {styleLabel(n, (k) => t(k))}
              </option>
            ))}
          </select>
          <div className={styles.row}>
            {(['rowHeaders', 'colHeaders', 'rowStripes', 'colStripes', 'lastColumn'] as const).map(
              (k) => (
                <label key={k} className={styles.check}>
                  <input
                    type="checkbox"
                    data-testid={`pivot-style-${k}`}
                    checked={style[k]}
                    onChange={(e) => setStyle({ ...style, [k]: e.target.checked })}
                  />
                  {t(`sheetEditor.pivot.styleOpts.${k}`)}
                </label>
              ),
            )}
          </div>
        </Section>
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- calculated field

export function PivotCalcDialog(c: Common) {
  const { t } = useTranslation();
  const p = c.pivot;
  const calcs = p.fields.flatMap((f, i) => (f.formula !== null ? [i] : []));
  const [field, setField] = useState<number | null>(null);
  const [name, setName] = useState('');
  const [formula, setFormula] = useState('=');
  const [error, setError] = useState('');
  const pick = (f: number | null) => {
    setField(f);
    setName(f === null ? '' : (p.fields[f]?.name ?? ''));
    setFormula(f === null ? '=' : `=${p.fields[f]?.formula ?? ''}`);
  };
  const run = async (remove: boolean) => {
    try {
      await c.host.exec({
        cmd: 'pivotCalcField',
        sheet: c.sheet,
        index: p.index,
        field,
        name,
        formula,
        remove,
        labels: c.labels,
      });
      await c.onDone();
      c.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  const source = p.fields.flatMap((f, i) => (!f.derived ? [i] : []));
  return (
    <Dialog
      open
      width={460}
      onOpenChange={(o) => !o && c.onClose()}
      title={t('sheetEditor.pivot.calcField')}
      testId="pivot-calc-dialog"
      footer={
        <>
          {field !== null && (
            <button type="button" className={styles.btn} onClick={() => void run(true)}>
              {t('sheetEditor.pivot.calcRemove')}
            </button>
          )}
          <Footer onClose={c.onClose} onOk={() => void run(false)} testId="pivot-calc-ok" />
        </>
      }
    >
      <div className={styles.form}>
        {calcs.length > 0 && (
          <label>
            {t('sheetEditor.pivot.calcExisting')}
            <select
              value={field ?? ''}
              onChange={(e) => pick(e.target.value === '' ? null : Number(e.target.value))}
            >
              <option value="">{t('sheetEditor.pivot.calcNew')}</option>
              {calcs.map((f) => (
                <option key={f} value={f}>
                  {p.fields[f]?.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <label>
          {t('sheetEditor.pivot.calcName')}
          <input
            data-testid="pivot-calc-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label>
          {t('sheetEditor.pivot.calcFormula')}
          <input
            data-testid="pivot-calc-formula"
            value={formula}
            onChange={(e) => setFormula(e.target.value)}
          />
        </label>
        <p className={styles.hint}>{t('sheetEditor.pivot.calcHint')}</p>
        <div className={styles.row}>
          {t('sheetEditor.pivot.calcInsert')}
          {source.map((f) => {
            const n = p.fields[f]?.name ?? '';
            const ref = /^[\p{L}_][\p{L}\p{N}_.]*$/u.test(n) ? n : `'${n.replace(/'/g, "''")}'`;
            return (
              <button
                key={f}
                type="button"
                className={styles.btn}
                onClick={() => setFormula((x) => x + ref)}
              >
                {n}
              </button>
            );
          })}
        </div>
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- calculated items

export function PivotCalcItemDialog(c: Common) {
  const { t } = useTranslation();
  const p = c.pivot;
  const axis = [...p.rows, ...p.cols].filter((f) => !p.fields[f]?.derived);
  const [field, setField] = useState<number>(axis[0] ?? 0);
  const existing = p.calcItems.filter((ci) => ci.field === field);
  const [old, setOld] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [formula, setFormula] = useState('=');
  const [error, setError] = useState('');
  const pick = (n: string | null) => {
    setOld(n);
    const ci = p.calcItems.find((x) => x.field === field && x.name === n);
    setName(ci?.name ?? '');
    setFormula(`=${ci?.formula ?? ''}`);
  };
  const run = async (remove: boolean) => {
    try {
      await c.host.exec({
        cmd: 'pivotCalcItem',
        sheet: c.sheet,
        index: p.index,
        field,
        old: old ?? undefined,
        name,
        formula,
        remove,
        labels: c.labels,
      });
      await c.onDone();
      c.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  const calcNames = new Set(existing.map((ci) => ci.name));
  const items = (p.fields[field]?.items ?? []).filter((it) => !calcNames.has(it));
  return (
    <Dialog
      open
      width={460}
      onOpenChange={(o) => !o && c.onClose()}
      title={t('sheetEditor.pivot.calcItem')}
      testId="pivot-calc-item-dialog"
      footer={
        <>
          {old !== null && (
            <button type="button" className={styles.btn} onClick={() => void run(true)}>
              {t('sheetEditor.pivot.calcRemove')}
            </button>
          )}
          <Footer onClose={c.onClose} onOk={() => void run(false)} testId="pivot-calc-item-ok" />
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.pivot.calcItemField')}
          <select
            data-testid="pivot-calc-item-field"
            value={field}
            onChange={(e) => {
              setField(Number(e.target.value));
              setOld(null);
            }}
          >
            {axis.map((f) => (
              <option key={f} value={f}>
                {p.fields[f]?.name}
              </option>
            ))}
          </select>
        </label>
        {existing.length > 0 && (
          <label>
            {t('sheetEditor.pivot.calcExisting')}
            <select value={old ?? ''} onChange={(e) => pick(e.target.value || null)}>
              <option value="">{t('sheetEditor.pivot.calcNew')}</option>
              {existing.map((ci) => (
                <option key={ci.name} value={ci.name}>
                  {ci.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <label>
          {t('sheetEditor.pivot.calcName')}
          <input
            data-testid="pivot-calc-item-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label>
          {t('sheetEditor.pivot.calcFormula')}
          <input
            data-testid="pivot-calc-item-formula"
            value={formula}
            onChange={(e) => setFormula(e.target.value)}
          />
        </label>
        <p className={styles.hint}>{t('sheetEditor.pivot.calcItemHint')}</p>
        <div className={styles.row}>
          {t('sheetEditor.pivot.calcInsert')}
          {items.map((n) => {
            const ref = /^[\p{L}_][\p{L}\p{N}_.]*$/u.test(n) ? n : `'${n.replace(/'/g, "''")}'`;
            return (
              <button
                key={n}
                type="button"
                className={styles.btn}
                onClick={() => setFormula((x) => x + ref)}
              >
                {n}
              </button>
            );
          })}
        </div>
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- move / change source

export function PivotMoveDialog({
  sheets,
  onMoved,
  ...c
}: Common & { sheets: string[]; onMoved: (sheet: number, index: number) => void }) {
  const { t } = useTranslation();
  const [dest, setDest] = useState(c.sheet);
  const [cell, setCell] = useState('');
  const [error, setError] = useState('');
  const ok = async () => {
    const at = parseA1(cell);
    if (!at) {
      setError(t('sheetEditor.pivot.badRef'));
      return;
    }
    try {
      const r = await c.host.exec<{ sheet: number; index: number }>({
        cmd: 'pivotMove',
        sheet: c.sheet,
        index: c.pivot.index,
        dest: { sheet: dest, row: at.active.row, col: at.active.col },
        labels: c.labels,
      });
      await c.onDone();
      if (r) onMoved(r.sheet, r.index);
      c.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && c.onClose()}
      title={t('sheetEditor.pivot.move')}
      testId="pivot-move-dialog"
      footer={<Footer onClose={c.onClose} onOk={() => void ok()} testId="pivot-move-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.pivot.sheet')}
          <select value={dest} onChange={(e) => setDest(Number(e.target.value))}>
            {sheets.map((n, i) => (
              <option key={i} value={i}>
                {n}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('sheetEditor.pivot.location')}
          <input
            data-testid="pivot-move-cell"
            value={cell}
            onChange={(e) => setCell(e.target.value)}
          />
        </label>
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}

export function PivotSourceDialog(c: Common) {
  const { t } = useTranslation();
  const [source, setSource] = useState(c.pivot.source ?? '');
  const [error, setError] = useState('');
  const ok = async () => {
    try {
      await c.host.exec({
        cmd: 'pivotChangeSource',
        sheet: c.sheet,
        index: c.pivot.index,
        source,
        labels: c.labels,
      });
      await c.onDone();
      c.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && c.onClose()}
      title={t('sheetEditor.pivot.changeSource')}
      testId="pivot-source-dialog"
      footer={<Footer onClose={c.onClose} onOk={() => void ok()} testId="pivot-source-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.pivot.source')}
          <input
            data-testid="pivot-change-source"
            value={source}
            onChange={(e) => setSource(e.target.value)}
          />
        </label>
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}
