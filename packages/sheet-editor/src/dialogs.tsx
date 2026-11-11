/**
 * BlueSheet dialogs and popovers: AutoFilter menu, conditional
 * formatting, data validation, charts, find/replace, go to, paste special,
 * messages and a generic anchored menu. Every action goes through
 * the engine command bridge (`SheetHost.exec`).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  CellAt,
  CfRuleSpec,
  CfStyle,
  ChartInfo,
  ChartMapOptions,
  ChartTemplate,
  PasteMode,
  SheetHost,
  SheetIcon,
  SheetRange,
} from '@blueoffice/platform';
import { ChartTemplateList, Dialog, deleteChartTemplate, loadChartTemplates } from '@blueoffice/ui';
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import {
  DataBarEditor,
  IconSetEditor,
  defaultBarOptions,
  defaultIconOptions,
  iconCount,
} from './cfRules';
import { layoutTexts } from './chartStyles';
import { MapChartOptions, mapOptionsOf } from './mapChart';
import { FilterExtras } from './sortFilter';
import styles from './SheetEditor.module.css';

interface Common {
  host: SheetHost;
  sheet: number;
  onClose: () => void;
  /** Called after the workbook changed. */
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}

function Buttons({
  onCancel,
  onOk,
  okLabel,
  okId,
}: {
  onCancel: () => void;
  onOk: () => void;
  okLabel?: string;
  okId?: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.btn} onClick={onCancel}>
        {t('sheetEditor.dlg.cancel')}
      </button>
      <button type="button" className={styles.btnPrimary} data-testid={okId} onClick={onOk}>
        {okLabel ?? t('sheetEditor.dlg.ok')}
      </button>
    </>
  );
}

// ----------------------------------------------------------------- menu popover

export interface MenuItem {
  id: string;
  label: string;
  /** Swatch / preview drawn before the label. */
  preview?: ReactNode;
}

/** A small menu anchored at a screen point; closes on outside click / Escape. */
export function Popover({
  x,
  y,
  onClose,
  children,
  testId,
}: {
  x: number;
  y: number;
  onClose: () => void;
  children: ReactNode;
  testId?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const down = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('mousedown', down);
    window.addEventListener('keydown', key);
    return () => {
      window.removeEventListener('mousedown', down);
      window.removeEventListener('keydown', key);
    };
  }, [onClose]);
  return (
    <div
      ref={ref}
      className={styles.popover}
      style={{ left: Math.max(4, x), top: Math.max(4, y) }}
      data-testid={testId}
      role="menu"
    >
      {children}
    </div>
  );
}

export function Menu({
  x,
  y,
  items,
  onPick,
  onClose,
  testId,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onPick: (id: string) => void;
  onClose: () => void;
  testId?: string;
}) {
  return (
    <Popover x={x} y={y} onClose={onClose} testId={testId}>
      {items.map((i) => (
        <button
          key={i.id}
          type="button"
          role="menuitem"
          className={styles.menuItem}
          data-menu={i.id}
          onClick={() => {
            onPick(i.id);
            onClose();
          }}
        >
          {i.preview}
          <span>{i.label}</span>
        </button>
      ))}
    </Popover>
  );
}

/** Colour palette (own selection of tones) + "none". */
export const PALETTE = [
  '000000',
  '434A54',
  '8A9099',
  'D9DDE2',
  'FFFFFF',
  'D64545',
  'F28C28',
  'FFEE00',
  '2FA36B',
  '1FA7A8',
  '2E6DB4',
  '7E57C2',
  'FADBD8',
  'FDF0C4',
  'D8F3E3',
  'DCE8F6',
];

export function ColorMenu({
  x,
  y,
  onPick,
  onClose,
  noneLabel,
}: {
  x: number;
  y: number;
  onPick: (hex: string) => void;
  onClose: () => void;
  noneLabel: string;
}) {
  return (
    <Popover x={x} y={y} onClose={onClose} testId="color-menu">
      <div className={styles.swatches}>
        {PALETTE.map((c) => (
          <button
            key={c}
            type="button"
            aria-label={`#${c}`}
            data-color={c}
            className={styles.swatch}
            style={{ background: `#${c}` }}
            onClick={() => {
              onPick(c);
              onClose();
            }}
          />
        ))}
      </div>
      <button
        type="button"
        className={styles.menuItem}
        onClick={() => {
          onPick('');
          onClose();
        }}
      >
        {noneLabel}
      </button>
    </Popover>
  );
}

// ----------------------------------------------------------------- AutoFilter menu

type FilterDates = {
  year: number;
  checked: boolean;
  months: { month: number; checked: boolean }[];
}[];

/** Relative date and average filters offered by the menu (§18.18.26). */
const DYNAMIC_FILTERS = [
  'today',
  'yesterday',
  'tomorrow',
  'thisWeek',
  'lastWeek',
  'nextWeek',
  'thisMonth',
  'lastMonth',
  'nextMonth',
  'thisQuarter',
  'thisYear',
  'lastYear',
  'yearToDate',
  'aboveAverage',
  'belowAverage',
];

export function FilterMenu({
  host,
  sheet,
  col,
  x,
  y,
  onClose,
  onDone,
  fail,
  range,
  onNotice,
}: Common & {
  col: number;
  x: number;
  y: number;
  /** The AutoFilter range (header row included): what the menu's sort orders. */
  range: SheetRange;
  /** "x of y records found" after a filter is applied. */
  onNotice?: (text: string) => void;
}) {
  const { t } = useTranslation();
  const [values, setValues] = useState<{ text: string; checked: boolean }[]>([]);
  const [dates, setDates] = useState<FilterDates>([]);
  const [colors, setColors] = useState<{ cell: string[]; font: string[] }>({ cell: [], font: [] });
  const [icons, setIcons] = useState<SheetIcon[]>([]);
  const [top10, setTop10] = useState<{ top: boolean; percent: boolean; val: number } | null>(null);
  const [search, setSearch] = useState('');
  const [op, setOp] = useState('');
  const [arg, setArg] = useState('');

  useEffect(() => {
    void host
      .exec<{
        values: { text: string; checked: boolean }[];
        dates?: FilterDates;
        dynamic?: string | null;
        colors?: { cell: string[]; font: string[] };
        icons?: SheetIcon[];
        top10?: { top: boolean; percent: boolean; val: number } | null;
      }>({ cmd: 'filterValues', sheet, col })
      .then((r) => {
        setValues(r.values);
        setDates(r.dates ?? []);
        if (r.colors) setColors(r.colors);
        setIcons(r.icons ?? []);
        setTop10(r.top10 ?? null);
        if (r.dynamic) setOp(`dyn:${r.dynamic}`);
      })
      .catch(fail);
  }, [host, sheet, col, fail]);

  const shown = useMemo(
    () => values.filter((v) => v.text.toLowerCase().includes(search.toLowerCase())),
    [values, search],
  );
  const run = async (filter: Parameters<SheetHost['exec']>[0]) => {
    try {
      const r = await host.exec<{ hidden?: number; total?: number } | null>(filter);
      await onDone();
      if (filter.cmd === 'setFilter' && typeof r?.total === 'number')
        onNotice?.(
          t('sheetEditor.filter.found', { shown: r.total - (r.hidden ?? 0), total: r.total }),
        );
      onClose();
    } catch (e) {
      fail(e);
    }
  };
  const apply = () => {
    if (op.startsWith('dyn:')) {
      void run({ cmd: 'setFilter', sheet, col, filter: { dynamic: op.slice(4) } });
      return;
    }
    if (op) {
      void run({
        cmd: 'setFilter',
        sheet,
        col,
        filter: { conditions: [{ operator: op, value: arg }] },
      });
      return;
    }
    const datesAll = dates.every((y) => y.checked && y.months.every((m) => m.checked));
    if (dates.length > 0 && !datesAll) {
      const picked = dates.flatMap((y) =>
        y.months.every((m) => m.checked)
          ? [{ year: y.year }]
          : y.months.filter((m) => m.checked).map((m) => ({ year: y.year, month: m.month })),
      );
      void run({ cmd: 'setFilter', sheet, col, filter: { dates: picked } });
      return;
    }
    // With a search, only the matching items that are ticked stay shown (the search
    // result replaces the filter); without one, every ticked item.
    const kept = (search ? shown : values).filter((v) => v.checked);
    const all = kept.length === values.length;
    void run({
      cmd: 'setFilter',
      sheet,
      col,
      filter: all ? null : { values: kept.map((v) => v.text) },
    });
  };
  const toggleAll = (on: boolean) =>
    setValues(values.map((v) => (shown.includes(v) ? { ...v, checked: on } : v)));

  return (
    <Popover x={x} y={y} onClose={onClose} testId="filter-menu">
      <div className={styles.filterMenu}>
        <button
          type="button"
          className={styles.menuItem}
          onClick={() =>
            void run({
              cmd: 'sort',
              sheet,
              range,
              keys: [{ col }],
              hasHeader: true,
            })
          }
        >
          {t('sheetEditor.sortAsc')}
        </button>
        <button
          type="button"
          className={styles.menuItem}
          onClick={() =>
            void run({
              cmd: 'sort',
              sheet,
              range,
              keys: [{ col, descending: true }],
              hasHeader: true,
            })
          }
        >
          {t('sheetEditor.sortDesc')}
        </button>
        <button
          type="button"
          className={styles.menuItem}
          onClick={() => void run({ cmd: 'setFilter', sheet, col, filter: null })}
        >
          {t('sheetEditor.filter.clear')}
        </button>
        <div className={styles.row}>
          <select
            aria-label={t('sheetEditor.filter.condition')}
            value={op}
            onChange={(e) => setOp(e.target.value)}
          >
            <option value="">{t('sheetEditor.filter.byValues')}</option>
            {[
              'equal',
              'notEqual',
              'greaterThan',
              'greaterThanOrEqual',
              'lessThan',
              'lessThanOrEqual',
              'contains',
              'notContains',
              'beginsWith',
              'endsWith',
            ].map((o) => (
              <option key={o} value={o}>
                {t(`sheetEditor.op.${o}`)}
              </option>
            ))}
            <optgroup label={t('sheetEditor.filter.dynamic')}>
              {DYNAMIC_FILTERS.map((d) => (
                <option key={d} value={`dyn:${d}`}>
                  {t(`sheetEditor.filter.dyn.${d}`)}
                </option>
              ))}
            </optgroup>
          </select>
          {op && !op.startsWith('dyn:') && (
            <input
              aria-label={t('sheetEditor.filter.value')}
              value={arg}
              onChange={(e) => setArg(e.target.value)}
            />
          )}
        </div>
        <FilterExtras
          key={top10 ? 'top10' : 'none'}
          icons={icons}
          top10={top10}
          onFilter={(filter) => void run({ cmd: 'setFilter', sheet, col, filter })}
        />
        {(colors.cell.length > 0 || colors.font.length > 1) && (
          <div className={styles.row} data-testid="filter-colors">
            <span>{t('sheetEditor.filter.byColor')}</span>
            {colors.cell.map((h) => (
              <button
                key={`c${h}`}
                type="button"
                className={styles.swatch}
                title={t('sheetEditor.filter.cellColor', { color: `#${h}` })}
                aria-label={t('sheetEditor.filter.cellColor', { color: `#${h}` })}
                data-color={h}
                style={{ background: `#${h}` }}
                onClick={() => void run({ cmd: 'setFilter', sheet, col, filter: { cellColor: h } })}
              />
            ))}
            {colors.font.length > 1 &&
              colors.font.map((h) => (
                <button
                  key={`f${h}`}
                  type="button"
                  className={styles.swatch}
                  title={t('sheetEditor.filter.fontColor', { color: `#${h}` })}
                  aria-label={t('sheetEditor.filter.fontColor', { color: `#${h}` })}
                  style={{ color: `#${h}`, fontWeight: 700 }}
                  onClick={() =>
                    void run({ cmd: 'setFilter', sheet, col, filter: { fontColor: h } })
                  }
                >
                  A
                </button>
              ))}
          </div>
        )}
        {!op && dates.length > 0 && (
          <div className={styles.valueList} data-testid="filter-dates">
            {dates.map((y, yi) => (
              <div key={y.year}>
                <label className={styles.check}>
                  <input
                    type="checkbox"
                    checked={y.months.every((m) => m.checked)}
                    onChange={(e) =>
                      setDates(
                        dates.map((d, j) =>
                          j === yi
                            ? {
                                ...d,
                                checked: e.target.checked,
                                months: d.months.map((m) => ({ ...m, checked: e.target.checked })),
                              }
                            : d,
                        ),
                      )
                    }
                  />
                  {y.year}
                </label>
                {y.months.map((m, mi) => (
                  <label key={m.month} className={styles.check} style={{ paddingLeft: 18 }}>
                    <input
                      type="checkbox"
                      checked={m.checked}
                      onChange={(e) =>
                        setDates(
                          dates.map((d, j) =>
                            j === yi
                              ? {
                                  ...d,
                                  months: d.months.map((x, k) =>
                                    k === mi ? { ...x, checked: e.target.checked } : x,
                                  ),
                                }
                              : d,
                          ),
                        )
                      }
                    />
                    {t('sheetEditor.filter.month', { n: m.month })}
                  </label>
                ))}
              </div>
            ))}
          </div>
        )}
        {!op && dates.length === 0 && (
          <>
            <input
              placeholder={t('sheetEditor.filter.search')}
              aria-label={t('sheetEditor.filter.search')}
              data-testid="filter-search"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={shown.every((v) => v.checked)}
                onChange={(e) => toggleAll(e.target.checked)}
              />
              {t('sheetEditor.filter.selectAll')}
            </label>
            <div className={styles.valueList}>
              {shown.map((v) => (
                <label key={v.text} className={styles.check}>
                  <input
                    type="checkbox"
                    checked={v.checked}
                    onChange={(e) =>
                      setValues(
                        values.map((x) => (x === v ? { ...x, checked: e.target.checked } : x)),
                      )
                    }
                  />
                  {v.text || t('sheetEditor.filter.blank')}
                </label>
              ))}
            </div>
          </>
        )}
        <div className={styles.row}>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="filter-ok"
            onClick={apply}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </div>
      </div>
    </Popover>
  );
}

// ----------------------------------------------------------------- conditional formatting

const CF_KINDS = [
  'cellIs',
  'containsText',
  'expression',
  'top10',
  'duplicateValues',
  'colorScale',
  'dataBar',
  'iconSet',
] as const;
export function CfDialog({
  host,
  sheet,
  range,
  onClose,
  onDone,
  fail,
}: Common & { range: SheetRange }) {
  const { t } = useTranslation();
  const [kind, setKind] = useState<(typeof CF_KINDS)[number]>('cellIs');
  const [operator, setOperator] = useState('greaterThan');
  const [f1, setF1] = useState('');
  const [f2, setF2] = useState('');
  const [fill, setFill] = useState('FADBD8');
  const [font, setFont] = useState('9B1C1C');
  const [bold, setBold] = useState(false);
  const [c1, setC1] = useState('FADBD8');
  const [c2, setC2] = useState('FDF0C4');
  const [c3, setC3] = useState('D8F3E3');
  const [three, setThree] = useState(true);
  const [iconSet, setIconSet] = useState('3Arrows');
  const [reverse, setReverse] = useState(false);
  const [barOpts, setBarOpts] = useState(defaultBarOptions);
  const [iconOpts, setIconOpts] = useState(() => defaultIconOptions(iconCount('3Arrows')));
  const [pivot, setPivot] = useState(false);

  const style: CfStyle = { fill, fontColor: font, bold };
  const rule = (): CfRuleSpec => {
    switch (kind) {
      case 'cellIs':
        return { kind, operator, formula1: f1, formula2: f2 || undefined, style };
      case 'containsText':
        return { kind, text: f1, style };
      case 'expression':
        return { kind, formula: f1, style };
      case 'top10':
        return { kind, rank: Number(f1) || 10, bottom: operator === 'bottom', style };
      case 'duplicateValues':
        return { kind, style };
      case 'colorScale':
        return three
          ? { kind, minColor: c1, midColor: c2, maxColor: c3 }
          : { kind, minColor: c1, maxColor: c3 };
      case 'dataBar':
        return { kind, color: c1, options: barOpts };
      case 'iconSet':
        return { kind, set: iconSet, reverse, options: iconOpts };
    }
  };
  const ok = async () => {
    try {
      await host.exec({ cmd: 'addCf', sheet, range, rule: rule(), pivot });
      await onDone();
      onClose();
    } catch (e) {
      fail(e);
    }
  };
  const color = (v: string, set: (s: string) => void, label: string) => (
    <label>
      {label}
      <input
        type="color"
        value={`#${v}`}
        onChange={(e) => set(e.target.value.slice(1).toUpperCase())}
      />
    </label>
  );
  const highlight = ['cellIs', 'containsText', 'expression', 'top10', 'duplicateValues'].includes(
    kind,
  );

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.cf.title')}
      width={460}
      testId="cf-dialog"
      footer={<Buttons onCancel={onClose} onOk={() => void ok()} okId="cf-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.cf.kind')}
          <select value={kind} onChange={(e) => setKind(e.target.value as typeof kind)}>
            {CF_KINDS.map((k) => (
              <option key={k} value={k}>
                {t(`sheetEditor.cf.kinds.${k}`)}
              </option>
            ))}
          </select>
        </label>
        {kind === 'cellIs' && (
          <>
            <label>
              {t('sheetEditor.cf.operator')}
              <select value={operator} onChange={(e) => setOperator(e.target.value)}>
                {[
                  'greaterThan',
                  'greaterThanOrEqual',
                  'lessThan',
                  'lessThanOrEqual',
                  'equal',
                  'notEqual',
                  'between',
                  'notBetween',
                ].map((o) => (
                  <option key={o} value={o}>
                    {t(`sheetEditor.op.${o}`)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t('sheetEditor.cf.value')}
              <input data-testid="cf-value" value={f1} onChange={(e) => setF1(e.target.value)} />
            </label>
            {(operator === 'between' || operator === 'notBetween') && (
              <label>
                {t('sheetEditor.cf.value2')}
                <input value={f2} onChange={(e) => setF2(e.target.value)} />
              </label>
            )}
          </>
        )}
        {kind === 'containsText' && (
          <label>
            {t('sheetEditor.cf.text')}
            <input value={f1} onChange={(e) => setF1(e.target.value)} />
          </label>
        )}
        {kind === 'expression' && (
          <label>
            {t('sheetEditor.cf.formula')}
            <input placeholder="=$A1>100" value={f1} onChange={(e) => setF1(e.target.value)} />
          </label>
        )}
        {kind === 'top10' && (
          <>
            <label>
              {t('sheetEditor.cf.rank')}
              <input
                type="number"
                min={1}
                value={f1 || '10'}
                onChange={(e) => setF1(e.target.value)}
              />
            </label>
            <label>
              {t('sheetEditor.cf.direction')}
              <select
                value={operator === 'bottom' ? 'bottom' : 'top'}
                onChange={(e) => setOperator(e.target.value)}
              >
                <option value="top">{t('sheetEditor.cf.top')}</option>
                <option value="bottom">{t('sheetEditor.cf.bottom')}</option>
              </select>
            </label>
          </>
        )}
        {highlight && (
          <>
            {color(fill, setFill, t('sheetEditor.fmt.fillColor'))}
            {color(font, setFont, t('sheetEditor.fmt.color'))}
            <label className={styles.check}>
              <input type="checkbox" checked={bold} onChange={(e) => setBold(e.target.checked)} />
              {t('sheetEditor.bold')}
            </label>
          </>
        )}
        {kind === 'colorScale' && (
          <>
            {color(c1, setC1, t('sheetEditor.cf.minColor'))}
            {three && color(c2, setC2, t('sheetEditor.cf.midColor'))}
            {color(c3, setC3, t('sheetEditor.cf.maxColor'))}
            <label className={styles.check}>
              <input type="checkbox" checked={three} onChange={(e) => setThree(e.target.checked)} />
              {t('sheetEditor.cf.threeColors')}
            </label>
          </>
        )}
        {kind === 'dataBar' && (
          <DataBarEditor
            color={c1}
            options={barOpts}
            onChange={(c, o) => {
              setC1(c);
              setBarOpts(o);
            }}
          />
        )}
        {kind === 'iconSet' && (
          <IconSetEditor
            set={iconSet}
            reverse={reverse}
            options={iconOpts}
            onChange={(s, r, o) => {
              setIconSet(s);
              setReverse(r);
              setIconOpts(o);
            }}
          />
        )}
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="cf-pivot"
            checked={pivot}
            onChange={(e) => setPivot(e.target.checked)}
          />
          {t('sheetEditor.cfm.pivotValues')}
        </label>
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- data validation

export function ValidationDialog({
  host,
  sheet,
  range,
  onClose,
  onDone,
  fail,
}: Common & { range: SheetRange }) {
  const { t } = useTranslation();
  const [kind, setKind] = useState('list');
  const [operator, setOperator] = useState('between');
  const [f1, setF1] = useState('');
  const [f2, setF2] = useState('');
  const [message, setMessage] = useState('');
  const [style, setStyle] = useState('stop');
  const ok = async () => {
    try {
      await host.exec({
        cmd: 'setValidation',
        sheet,
        range,
        spec: {
          kind,
          operator: kind === 'list' || kind === 'custom' ? undefined : operator,
          formula1: f1,
          formula2: f2 || undefined,
          error: message || undefined,
          errorStyle: style,
          allowBlank: true,
        },
      });
      await onDone();
      onClose();
    } catch (e) {
      fail(e);
    }
  };
  const numeric = !['list', 'custom', 'none'].includes(kind);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.dv.title')}
      testId="dv-dialog"
      footer={<Buttons onCancel={onClose} onOk={() => void ok()} okId="dv-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.dv.allow')}
          <select value={kind} onChange={(e) => setKind(e.target.value)}>
            {['list', 'whole', 'decimal', 'date', 'textLength', 'custom', 'none'].map((k) => (
              <option key={k} value={k}>
                {t(`sheetEditor.dv.kinds.${k}`)}
              </option>
            ))}
          </select>
        </label>
        {numeric && (
          <label>
            {t('sheetEditor.cf.operator')}
            <select value={operator} onChange={(e) => setOperator(e.target.value)}>
              {[
                'between',
                'notBetween',
                'equal',
                'notEqual',
                'greaterThan',
                'greaterThanOrEqual',
                'lessThan',
                'lessThanOrEqual',
              ].map((o) => (
                <option key={o} value={o}>
                  {t(`sheetEditor.op.${o}`)}
                </option>
              ))}
            </select>
          </label>
        )}
        {kind !== 'none' && (
          <label>
            {kind === 'list' ? t('sheetEditor.dv.source') : t('sheetEditor.cf.value')}
            <input
              data-testid="dv-source"
              placeholder={kind === 'list' ? 'Có,Không  |  =$A$1:$A$5' : ''}
              value={f1}
              onChange={(e) => setF1(e.target.value)}
            />
          </label>
        )}
        {numeric && (operator === 'between' || operator === 'notBetween') && (
          <label>
            {t('sheetEditor.cf.value2')}
            <input value={f2} onChange={(e) => setF2(e.target.value)} />
          </label>
        )}
        {kind !== 'none' && (
          <>
            <label>
              {t('sheetEditor.dv.errorStyle')}
              <select value={style} onChange={(e) => setStyle(e.target.value)}>
                {['stop', 'warning', 'information'].map((s) => (
                  <option key={s} value={s}>
                    {t(`sheetEditor.dv.styles.${s}`)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t('sheetEditor.dv.message')}
              <input value={message} onChange={(e) => setMessage(e.target.value)} />
            </label>
          </>
        )}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- charts

export const CHART_TYPES = [
  'column',
  'stackedColumn',
  'percentColumn',
  'bar',
  'stackedBar',
  'percentBar',
  'line',
  'stackedLine',
  'percentLine',
  'area',
  'stackedArea',
  'percentArea',
  'pie',
  'explodedPie',
  'doughnut',
  'pieOfPie',
  'barOfPie',
  'scatter',
  'bubble',
  'radar',
  'radarMarker',
  'radarFilled',
  'stock',
  'stockUpDown',
  'stockVolume',
  'stockVolumeUpDown',
  'surface',
  'surfaceWireframe',
  'waterfall',
  'treemap',
  'sunburst',
  'histogram',
  'pareto',
  'boxWhisker',
  'funnel',
  'regionMap',
  'column3D',
  'clusteredColumn3D',
  'stackedColumn3D',
  'percentColumn3D',
  'cylinder3D',
  'cone3D',
  'pyramid3D',
  'bar3D',
  'stackedBar3D',
  'percentBar3D',
  'line3D',
  'area3D',
  'stackedArea3D',
  'percentArea3D',
  'pie3D',
  'explodedPie3D',
  'surface3D',
  'surfaceWireframe3D',
  'combo',
];

export function ChartDialog({
  host,
  sheet,
  index,
  onClose,
  onDone,
  fail,
}: Common & { index: number | null }) {
  const { t } = useTranslation();
  const [charts, setCharts] = useState<ChartInfo[]>([]);
  const [sel, setSel] = useState(index ?? 0);
  const [kind, setKind] = useState('column');
  const [title, setTitle] = useState('');
  const [legend, setLegend] = useState('r');
  const [xTitle, setXTitle] = useState('');
  const [yTitle, setYTitle] = useState('');
  // Region map options (CORE-CHART-006).
  const [mapOpts, setMapOpts] = useState<ChartMapOptions>(() => mapOptionsOf(null));
  // Saved chart templates (CORE-CHART-007): one may replace the type and formatting.
  const [templates, setTemplates] = useState(() => loadChartTemplates<ChartTemplate>());
  const [template, setTemplate] = useState<string | null>(null);

  const show = (c: ChartInfo | undefined) => {
    if (!c) return;
    setKind(c.kind);
    setTitle(c.title ?? '');
    setLegend(c.legend ?? 'none');
    setXTitle(c.xTitle ?? '');
    setYTitle(c.yTitle ?? '');
    setMapOpts(mapOptionsOf(c.map));
  };
  useEffect(() => {
    void host
      .exec<ChartInfo[]>({ cmd: 'charts', sheet })
      .then((list) => {
        setCharts(list);
        show(list[index ?? 0]);
      })
      .catch(fail);
    // Loaded once per dialog.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, sheet, fail]);

  const ok = async () => {
    try {
      await host.exec({ cmd: 'editChart', sheet, index: sel, kind, title, legend, xTitle, yTitle });
      if (kind === 'regionMap')
        await host.exec({ cmd: 'setChartMap', sheet, index: sel, ...mapOpts });
      const chosen = templates.find((x) => x.name === template);
      if (chosen) {
        await host.exec({
          cmd: 'chartDesign',
          sheet,
          index: sel,
          template: chosen,
          texts: layoutTexts(t),
        });
      }
      await onDone();
      onClose();
    } catch (e) {
      fail(e);
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.chart.editTitle')}
      testId="chart-dialog"
      footer={<Buttons onCancel={onClose} onOk={() => void ok()} okId="chart-ok" />}
    >
      {charts.length === 0 ? (
        <p className={styles.hint}>{t('sheetEditor.chart.none')}</p>
      ) : (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.chart.which')}
            <select
              value={sel}
              onChange={(e) => {
                setSel(Number(e.target.value));
                show(charts[Number(e.target.value)]);
              }}
            >
              {charts.map((c) => (
                <option key={c.index} value={c.index}>
                  {c.title || t('sheetEditor.chart.untitled', { n: c.index + 1 })}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sheetEditor.chart.type')}
            <select value={kind} onChange={(e) => setKind(e.target.value)}>
              {CHART_TYPES.map((k) => (
                <option key={k} value={k}>
                  {t(`sheetEditor.chart.types.${k}`)}
                </option>
              ))}
            </select>
          </label>
          <ChartTemplateList
            templates={templates}
            value={template}
            onChange={setTemplate}
            onDelete={(name) => {
              setTemplates(deleteChartTemplate<ChartTemplate>(name));
              if (template === name) setTemplate(null);
            }}
            labels={{
              title: t('chartDesign.templates'),
              none: t('chartDesign.noTemplates'),
              noTemplate: t('chartDesign.noTemplate'),
              remove: t('chartDesign.removeTemplate'),
            }}
          />
          <label>
            {t('sheetEditor.chart.chartTitle')}
            <input
              data-testid="chart-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </label>
          <label>
            {t('sheetEditor.chart.legend')}
            <select value={legend} onChange={(e) => setLegend(e.target.value)}>
              {['none', 'r', 'b', 't', 'l'].map((l) => (
                <option key={l} value={l}>
                  {t(`sheetEditor.chart.legendPos.${l}`)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sheetEditor.chart.xTitle')}
            <input value={xTitle} onChange={(e) => setXTitle(e.target.value)} />
          </label>
          <label>
            {t('sheetEditor.chart.yTitle')}
            <input value={yTitle} onChange={(e) => setYTitle(e.target.value)} />
          </label>
          {kind === 'regionMap' && (
            <MapChartOptions
              value={mapOpts}
              info={charts.find((c) => c.index === sel)?.map}
              onChange={setMapOpts}
            />
          )}
        </div>
      )}
    </Dialog>
  );
}

// ----------------------------------------------------------------- find / replace

export function FindDialog({
  host,
  sheet,
  replace: withReplace,
  onClose,
  onDone,
  fail,
  onSelect,
}: Common & { replace: boolean; onSelect: (at: CellAt) => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [repl, setRepl] = useState('');
  const [matchCase, setMatchCase] = useState(false);
  const [wholeCell, setWholeCell] = useState(false);
  const [inFormulas, setInFormulas] = useState(false);
  const [allSheets, setAllSheets] = useState(false);
  const [matches, setMatches] = useState<CellAt[]>([]);
  const [index, setIndex] = useState(-1);
  const [message, setMessage] = useState('');

  const opts = { matchCase, wholeCell, sheet: allSheets ? null : sheet };
  const search = async () => {
    const r = await host.exec<{ matches: CellAt[] }>({ cmd: 'find', query, inFormulas, ...opts });
    setMatches(r.matches);
    setMessage(
      r.matches.length
        ? t('sheetEditor.find.count', { count: r.matches.length })
        : t('sheetEditor.find.none'),
    );
    return r.matches;
  };
  const next = async () => {
    try {
      const m = await search();
      if (!m.length) return;
      const i = (index + 1) % m.length;
      setIndex(i);
      onSelect(m[i]!);
    } catch (e) {
      fail(e);
    }
  };
  const replaceOne = async () => {
    try {
      const at = matches[index];
      if (!at) return next();
      await host.exec({ cmd: 'replace', query, with: repl, at, ...opts });
      await onDone();
      setIndex(index - 1);
      await next();
    } catch (e) {
      fail(e);
    }
  };
  const replaceAll = async () => {
    try {
      const r = await host.exec<{ count: number }>({ cmd: 'replace', query, with: repl, ...opts });
      await onDone();
      setMessage(t('sheetEditor.find.replaced', { count: r.count }));
      setMatches([]);
      setIndex(-1);
    } catch (e) {
      fail(e);
    }
  };
  const check = (v: boolean, set: (b: boolean) => void, label: string) => (
    <label className={styles.check}>
      <input type="checkbox" checked={v} onChange={(e) => set(e.target.checked)} />
      {label}
    </label>
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={withReplace ? t('sheetEditor.find.replaceTitle') : t('sheetEditor.find.title')}
      width={460}
      testId="find-dialog"
      footer={
        <>
          {withReplace && (
            <>
              <button type="button" className={styles.btn} onClick={() => void replaceAll()}>
                {t('sheetEditor.find.replaceAll')}
              </button>
              <button type="button" className={styles.btn} onClick={() => void replaceOne()}>
                {t('sheetEditor.find.replace')}
              </button>
            </>
          )}
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="find-next"
            onClick={() => void next()}
          >
            {t('sheetEditor.find.next')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.find.query')}
          <input
            autoFocus
            data-testid="find-query"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setIndex(-1);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void next();
            }}
          />
        </label>
        {withReplace && (
          <label>
            {t('sheetEditor.find.with')}
            <input value={repl} onChange={(e) => setRepl(e.target.value)} />
          </label>
        )}
        {check(matchCase, setMatchCase, t('sheetEditor.find.matchCase'))}
        {check(wholeCell, setWholeCell, t('sheetEditor.find.wholeCell'))}
        {check(inFormulas, setInFormulas, t('sheetEditor.find.inFormulas'))}
        {check(allSheets, setAllSheets, t('sheetEditor.find.allSheets'))}
        <p className={styles.hint} data-testid="find-message">
          {message}
        </p>
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- go to

export function GoToDialog({
  host,
  sheet,
  onClose,
  fail,
  onGo,
}: Omit<Common, 'onDone'> & { onGo: (sheet: number, r: SheetRange) => void }) {
  const { t } = useTranslation();
  const [names, setNames] = useState<{ name: string; ref: string }[]>([]);
  const [text, setText] = useState('');
  const [error, setError] = useState('');
  useEffect(() => {
    void host.exec<{ name: string; ref: string }[]>({ cmd: 'names' }).then(setNames).catch(fail);
  }, [host, fail]);
  const go = async (ref: string) => {
    try {
      const r = await host.exec<{ sheet: number; range: SheetRange } | null>({
        cmd: 'resolve',
        sheet,
        text: ref,
      });
      if (!r) {
        setError(t('sheetEditor.goto.invalid'));
        return;
      }
      onGo(r.sheet, r.range);
      onClose();
    } catch (e) {
      fail(e);
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.goto.title')}
      testId="goto-dialog"
      footer={<Buttons onCancel={onClose} onOk={() => void go(text)} okId="goto-ok" />}
    >
      <div className={styles.valueList}>
        {names.map((n) => (
          <button
            key={n.name}
            type="button"
            className={styles.menuItem}
            onDoubleClick={() => void go(n.name)}
            onClick={() => setText(n.name)}
          >
            <span>{n.name}</span>
            <span className={styles.hint}>{n.ref}</span>
          </button>
        ))}
      </div>
      <label className={styles.form}>
        {t('sheetEditor.goto.reference')}
        <input
          autoFocus
          data-testid="goto-ref"
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setError('');
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void go(text);
          }}
        />
      </label>
      {error && <p className={styles.error}>{error}</p>}
    </Dialog>
  );
}

// ----------------------------------------------------------------- paste special

export function PasteSpecialDialog({
  onClose,
  onPaste,
}: {
  onClose: () => void;
  onPaste: (mode: PasteMode, transpose: boolean, skipBlanks: boolean) => void;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<PasteMode>('all');
  const [transpose, setTranspose] = useState(false);
  const [skip, setSkip] = useState(false);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.pasteSpecial.title')}
      testId="paste-special"
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => {
            onPaste(mode, transpose, skip);
            onClose();
          }}
          okId="paste-special-ok"
        />
      }
    >
      <div className={styles.form} role="radiogroup">
        {(['all', 'values', 'formulas', 'formats'] as const).map((m) => (
          <label key={m} className={styles.check}>
            <input
              type="radio"
              name="paste-mode"
              checked={mode === m}
              onChange={() => setMode(m)}
            />
            {t(`sheetEditor.pasteSpecial.${m}`)}
          </label>
        ))}
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={transpose}
            onChange={(e) => setTranspose(e.target.checked)}
          />
          {t('sheetEditor.pasteSpecial.transpose')}
        </label>
        <label className={styles.check}>
          <input type="checkbox" checked={skip} onChange={(e) => setSkip(e.target.checked)} />
          {t('sheetEditor.pasteSpecial.skipBlanks')}
        </label>
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- message

export function MessageDialog({
  title,
  message,
  onClose,
}: {
  title: string;
  message: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={title}
      testId="message-dialog"
      footer={
        <button type="button" className={styles.btnPrimary} onClick={onClose}>
          {t('sheetEditor.dlg.ok')}
        </button>
      }
    >
      <p>{message}</p>
    </Dialog>
  );
}

// ----------------------------------------------------------------- goal seek

interface GoalSeekResult {
  found: boolean;
  input: number;
  result: number;
  iterations: number;
}

/** What-if analysis: find the input that makes a formula reach a value. */
export function GoalSeekDialog({
  host,
  sheet,
  onClose,
  onDone,
  target: initial,
}: Omit<Common, 'fail'> & { target: string }) {
  const { t } = useTranslation();
  const [target, setTarget] = useState(initial);
  const [goal, setGoal] = useState('');
  const [changing, setChanging] = useState('');
  const [result, setResult] = useState<GoalSeekResult | null>(null);
  const [error, setError] = useState('');
  const run = async (apply: boolean) => {
    const value = Number(goal.replace(',', '.'));
    if (goal.trim() === '' || !Number.isFinite(value)) {
      setError(t('sheetEditor.goalSeek.badGoal'));
      return;
    }
    try {
      const r = await host.exec<GoalSeekResult>({
        cmd: 'goalSeek',
        sheet,
        target,
        goal: value,
        changing,
        apply,
      });
      setError('');
      if (apply) {
        await onDone();
        onClose();
      } else {
        setResult(r);
      }
    } catch {
      setError(t('sheetEditor.goalSeek.invalid'));
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.goalSeek.title')}
      testId="goalseek-dialog"
      footer={
        result?.found ? (
          <Buttons
            onCancel={onClose}
            onOk={() => void run(true)}
            okLabel={t('sheetEditor.goalSeek.apply')}
            okId="goalseek-apply"
          />
        ) : (
          <Buttons
            onCancel={onClose}
            onOk={() => void run(false)}
            okLabel={t('sheetEditor.goalSeek.find')}
            okId="goalseek-find"
          />
        )
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.goalSeek.target')}
          <input
            data-testid="goalseek-target"
            value={target}
            onChange={(e) => {
              setTarget(e.target.value);
              setResult(null);
            }}
          />
        </label>
        <label>
          {t('sheetEditor.goalSeek.goal')}
          <input
            data-testid="goalseek-goal"
            inputMode="decimal"
            value={goal}
            onChange={(e) => {
              setGoal(e.target.value);
              setResult(null);
            }}
          />
        </label>
        <label>
          {t('sheetEditor.goalSeek.changing')}
          <input
            data-testid="goalseek-changing"
            value={changing}
            onChange={(e) => {
              setChanging(e.target.value);
              setResult(null);
            }}
          />
        </label>
        {error && <p role="alert">{error}</p>}
        {result && (
          <p data-testid="goalseek-result" role="status">
            {result.found
              ? t('sheetEditor.goalSeek.found', {
                  input: Number(result.input.toPrecision(10)),
                  result: Number(result.result.toPrecision(10)),
                })
              : t('sheetEditor.goalSeek.notFound', {
                  result: Number(result.result.toPrecision(10)),
                })}
          </p>
        )}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- data table

/** What-if data table over the selection: input cells for the top row and/or left column. */
export function DataTableDialog({
  host,
  sheet,
  onClose,
  onDone,
  range,
}: Omit<Common, 'fail'> & { range: SheetRange }) {
  const { t } = useTranslation();
  const [rowInput, setRowInput] = useState('');
  const [colInput, setColInput] = useState('');
  const [error, setError] = useState('');
  const ok = async () => {
    try {
      await host.exec({
        cmd: 'dataTable',
        sheet,
        range,
        rowInput: rowInput.trim() || null,
        colInput: colInput.trim() || null,
      });
      await onDone();
      onClose();
    } catch {
      setError(t('sheetEditor.dataTable.invalid'));
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.dataTable.title')}
      testId="datatable-dialog"
      footer={<Buttons onCancel={onClose} onOk={() => void ok()} okId="datatable-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.dataTable.rowInput')}
          <input
            data-testid="datatable-row"
            value={rowInput}
            onChange={(e) => setRowInput(e.target.value)}
          />
        </label>
        <label>
          {t('sheetEditor.dataTable.colInput')}
          <input
            data-testid="datatable-col"
            value={colInput}
            onChange={(e) => setColInput(e.target.value)}
          />
        </label>
        {error && <p role="alert">{error}</p>}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- solver

export { SolverDialog } from './solver';
