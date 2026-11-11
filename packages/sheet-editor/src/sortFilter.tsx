/**
 * BlueSheet sort & filter dialogs (SHEET-DATA-001): the multi-level sort dialog
 * (sort on values, cell colour, font colour or conditional-format icon; ascending,
 * descending or a custom list; top to bottom or left to right), the extra AutoFilter
 * menu entries (top / bottom N, period date filters, icon filters) and the Advanced
 * Filter dialog. Every action goes through the engine command bridge (`SheetHost.exec`).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  AdvancedFilterResult,
  FilterSpec,
  SheetHost,
  SheetIcon,
  SheetRange,
  SortKeySpec,
  SortRegion,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState } from 'react';
import { a1, colName, parseA1, rangeA1, rangeOf } from './grid';
import styles from './SheetEditor.module.css';

interface Common {
  host: SheetHost;
  sheet: number;
  onClose: () => void;
  /** Called after the workbook changed. */
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}

function Footer({
  onCancel,
  onOk,
  okId,
}: {
  onCancel: () => void;
  onOk: () => void;
  okId: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.btn} onClick={onCancel}>
        {t('sheetEditor.dlg.cancel')}
      </button>
      <button type="button" className={styles.btnPrimary} data-testid={okId} onClick={onOk}>
        {t('sheetEditor.dlg.ok')}
      </button>
    </>
  );
}

/** `3Arrows` icon 0 → "3Arrows – icon 1". */
export function iconLabel(t: (k: string, o?: Record<string, unknown>) => string, i: SheetIcon) {
  return t('sheetEditor.sort.iconName', { set: i.set, n: i.id + 1 });
}

// ----------------------------------------------------------------- sort

/**
 * Asked when a quick sort runs on one column of a wider block of data: sort the whole
 * block (the default, rows stay together) or only the selected cells.
 */
export function SortWarningDialog({
  onSort,
  onClose,
}: {
  onSort: (expand: boolean) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [expand, setExpand] = useState(true);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.sortWarning.title')}
      testId="sort-warning"
      footer={
        <>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="sort-warning-ok"
            onClick={() => onSort(expand)}
          >
            {t('sheetEditor.sortWarning.sort')}
          </button>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
        </>
      }
    >
      <p className={styles.hint}>{t('sheetEditor.sortWarning.message')}</p>
      <label className={styles.check}>
        <input
          type="radio"
          name="sort-warning"
          data-testid="sort-warning-expand"
          checked={expand}
          onChange={() => setExpand(true)}
        />
        {t('sheetEditor.sortWarning.expand')}
      </label>
      <label className={styles.check}>
        <input
          type="radio"
          name="sort-warning"
          data-testid="sort-warning-current"
          checked={!expand}
          onChange={() => setExpand(false)}
        />
        {t('sheetEditor.sortWarning.current')}
      </label>
    </Dialog>
  );
}

type SortOn = 'value' | 'cellColor' | 'fontColor' | 'icon';

export interface SortLevel {
  col: number;
  on: SortOn;
  /** `custom`: entries of `list` first, in that order. */
  order: 'asc' | 'desc' | 'custom';
  color?: string;
  /** `set:id` of an icon key. */
  icon?: string;
  /** Custom list entries (one per line in the editor). */
  list?: string;
}

/** Engine sort keys of the dialog's levels (levels missing their colour / icon are skipped). */
export function levelKeys(levels: SortLevel[]): SortKeySpec[] {
  const keys: SortKeySpec[] = [];
  for (const l of levels) {
    const descending = l.order === 'desc';
    if (l.on === 'value') {
      const list = (l.list ?? '')
        .split(/\r?\n/)
        .map((s) => s.trim())
        .filter(Boolean);
      keys.push(
        l.order === 'custom' && list.length > 0
          ? { col: l.col, customList: list }
          : { col: l.col, descending },
      );
    } else if (l.on === 'icon') {
      const [set, id] = (l.icon ?? '').split(':');
      if (set && id !== undefined) {
        keys.push({ col: l.col, descending, by: 'icon', iconSet: set, iconId: Number(id) });
      }
    } else if (l.color) {
      keys.push({ col: l.col, descending, by: l.on, color: l.color });
    }
  }
  return keys;
}

interface KeyChoices {
  cell: string[];
  font: string[];
  icons: SheetIcon[];
}

export function SortDialog({
  host,
  sheet,
  range,
  onClose,
  onDone,
  fail,
}: Common & { range: SheetRange }) {
  const { t } = useTranslation();
  const [region, setRegion] = useState<SheetRange>(range);
  const [header, setHeader] = useState(true);
  const [levels, setLevels] = useState<SortLevel[]>([{ col: range.c0, on: 'value', order: 'asc' }]);
  const [names, setNames] = useState<string[]>([]);
  const [leftToRight, setLeftToRight] = useState(false);
  const [lists, setLists] = useState<string[][]>([]);
  /** Colours and icons present per key (column, or row when sorting left to right). */
  const [choices, setChoices] = useState<Record<number, KeyChoices>>({});

  useEffect(() => {
    void (async () => {
      // The current region of a single cell, and the guess whether its first row is labels.
      const g = await host.exec<SortRegion>({ cmd: 'sortRegion', sheet, range });
      const r = g.range;
      setRegion(r);
      setHeader(g.hasHeader);
      setLevels([{ col: Math.max(r.c0, Math.min(range.c0, r.c1)), on: 'value', order: 'asc' }]);
      const n: string[] = [];
      for (let c = r.c0; c <= r.c1 && c < r.c0 + 200; c++) {
        n.push((await host.cell(sheet, r.r0, c)).display);
      }
      setNames(n);
      setLists((await host.exec<string[][]>({ cmd: 'sortLists' })) ?? []);
    })().catch(fail);
  }, [host, sheet, range, fail]);

  const loadChoices = (key: number) => {
    if (choices[key]) return;
    void host
      .exec<Partial<KeyChoices>>({ cmd: 'sortColors', sheet, range: region, key, leftToRight })
      .then(
        (r) =>
          r &&
          setChoices((c) => ({
            ...c,
            [key]: { cell: r.cell ?? [], font: r.font ?? [], icons: r.icons ?? [] },
          })),
      )
      .catch(fail);
  };

  const keyLabel = (c: number) => {
    if (leftToRight) return t('sheetEditor.sort.row', { n: c + 1 });
    const name = names[c - region.c0];
    return header && name ? name : t('sheetEditor.sort.column', { name: colName(c) });
  };
  const keys: number[] = [];
  const [lo, hi] = leftToRight ? [region.r0, region.r1] : [region.c0, region.c1];
  for (let c = lo; c <= hi && c < lo + 200; c++) keys.push(c);
  const patch = (i: number, p: Partial<SortLevel>) =>
    setLevels(levels.map((x, j) => (j === i ? { ...x, ...p } : x)));

  const ok = async () => {
    try {
      await host.exec({
        cmd: 'sort',
        sheet,
        range: region,
        keys: levelKeys(levels),
        hasHeader: header,
        leftToRight,
      });
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
      title={t('sheetEditor.sort.title')}
      width={620}
      testId="sort-dialog"
      footer={<Footer onCancel={onClose} onOk={() => void ok()} okId="sort-ok" />}
    >
      <p className={styles.hint}>
        {a1({ row: region.r0, col: region.c0 })}:{a1({ row: region.r1, col: region.c1 })}
      </p>
      <label className={styles.check}>
        <input type="checkbox" checked={header} onChange={(e) => setHeader(e.target.checked)} />
        {leftToRight ? t('sheetEditor.sort.headerCol') : t('sheetEditor.sort.header')}
      </label>
      <label className={styles.check}>
        <input
          type="checkbox"
          data-testid="sort-left-to-right"
          checked={leftToRight}
          onChange={(e) => {
            const on = e.target.checked;
            setLeftToRight(on);
            setChoices({});
            setLevels([{ col: on ? region.r0 : region.c0, on: 'value', order: 'asc' }]);
          }}
        />
        {t('sheetEditor.sort.leftToRight')}
      </label>
      {levels.map((l, i) => {
        const ch = choices[l.col];
        const colours = l.on === 'cellColor' ? ch?.cell : ch?.font;
        return (
          <div key={i} className={styles.row} data-testid={`sort-level-${i}`}>
            <span className={styles.levelLabel}>
              {i === 0 ? t('sheetEditor.sort.by') : t('sheetEditor.sort.thenBy')}
            </span>
            <select
              aria-label={t('sheetEditor.sort.columnLabel')}
              data-testid={`sort-col-${i}`}
              value={l.col}
              onChange={(e) => {
                const col = Number(e.target.value);
                if (l.on !== 'value') loadChoices(col);
                patch(i, { col, color: undefined, icon: undefined });
              }}
            >
              {keys.map((c) => (
                <option key={c} value={c}>
                  {keyLabel(c)}
                </option>
              ))}
            </select>
            <select
              aria-label={t('sheetEditor.sort.sortOn')}
              data-testid={`sort-by-${i}`}
              value={l.on}
              onChange={(e) => {
                const on = e.target.value as SortOn;
                if (on !== 'value') loadChoices(l.col);
                patch(i, {
                  on,
                  color: undefined,
                  icon: undefined,
                  order: l.order === 'custom' ? 'asc' : l.order,
                });
              }}
            >
              <option value="value">{t('sheetEditor.sort.onValue')}</option>
              <option value="cellColor">{t('sheetEditor.sort.onCellColor')}</option>
              <option value="fontColor">{t('sheetEditor.sort.onFontColor')}</option>
              <option value="icon">{t('sheetEditor.sort.onIcon')}</option>
            </select>
            {(l.on === 'cellColor' || l.on === 'fontColor') && (
              <select
                aria-label={t('sheetEditor.sort.color')}
                data-testid={`sort-color-${i}`}
                value={l.color ?? ''}
                onChange={(e) => patch(i, { color: e.target.value })}
              >
                <option value="">—</option>
                {colours?.map((h) => (
                  <option key={h} value={h} style={{ background: `#${h}` }}>
                    #{h}
                  </option>
                ))}
              </select>
            )}
            {l.on === 'icon' && (
              <select
                aria-label={t('sheetEditor.sort.icon')}
                data-testid={`sort-icon-${i}`}
                value={l.icon ?? ''}
                onChange={(e) => patch(i, { icon: e.target.value })}
              >
                <option value="">—</option>
                {ch?.icons.map((ic) => (
                  <option key={`${ic.set}:${ic.id}`} value={`${ic.set}:${ic.id}`}>
                    {iconLabel(t, ic)}
                  </option>
                ))}
              </select>
            )}
            <select
              aria-label={t('sheetEditor.sort.order')}
              data-testid={`sort-order-${i}`}
              value={l.order}
              onChange={(e) => patch(i, { order: e.target.value as SortLevel['order'] })}
            >
              {l.on === 'value' ? (
                <>
                  <option value="asc">{t('sheetEditor.sort.asc')}</option>
                  <option value="desc">{t('sheetEditor.sort.desc')}</option>
                  <option value="custom">{t('sheetEditor.sort.customList')}</option>
                </>
              ) : (
                <>
                  <option value="asc">{t('sheetEditor.sort.onTop')}</option>
                  <option value="desc">{t('sheetEditor.sort.onBottom')}</option>
                </>
              )}
            </select>
            {levels.length > 1 && (
              <button
                type="button"
                className={styles.btn}
                onClick={() => setLevels(levels.filter((_, j) => j !== i))}
              >
                {t('sheetEditor.sort.remove')}
              </button>
            )}
            {l.on === 'value' && l.order === 'custom' && (
              <div className={styles.row}>
                <select
                  aria-label={t('sheetEditor.sort.listPreset')}
                  data-testid={`sort-list-preset-${i}`}
                  value=""
                  onChange={(e) => {
                    const preset = lists[Number(e.target.value)];
                    if (preset) patch(i, { list: preset.join('\n') });
                  }}
                >
                  <option value="">{t('sheetEditor.sort.listPreset')}</option>
                  {lists.map((l2, k) => (
                    <option key={k} value={k}>
                      {l2.join(', ')}
                    </option>
                  ))}
                </select>
                <textarea
                  aria-label={t('sheetEditor.sort.listEntries')}
                  placeholder={t('sheetEditor.sort.listHint')}
                  data-testid={`sort-list-${i}`}
                  rows={4}
                  value={l.list ?? ''}
                  onChange={(e) => patch(i, { list: e.target.value })}
                />
              </div>
            )}
          </div>
        );
      })}
      <div className={styles.row}>
        <button
          type="button"
          className={styles.btn}
          data-testid="sort-add-level"
          disabled={levels.length >= 64}
          onClick={() =>
            setLevels([
              ...levels,
              { col: leftToRight ? region.r0 : region.c0, on: 'value', order: 'asc' },
            ])
          }
        >
          {t('sheetEditor.sort.addLevel')}
        </button>
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- AutoFilter menu extras

/** Period filters of `ST_DynamicFilterType` (§18.18.26) not in the main condition list. */
export const PERIOD_FILTERS = [
  'lastQuarter',
  'nextQuarter',
  'nextYear',
  'Q1',
  'Q2',
  'Q3',
  'Q4',
  ...Array.from({ length: 12 }, (_, i) => `M${i + 1}`),
];

/**
 * Number, date-period and icon filters of the AutoFilter drop-down: top / bottom N
 * items or percent, quarters / months of any year and the remaining relative periods,
 * and the conditional-format icons of the column.
 */
export function FilterExtras({
  icons,
  top10,
  onFilter,
}: {
  icons: SheetIcon[];
  top10: { top: boolean; percent: boolean; val: number } | null;
  onFilter: (f: FilterSpec) => void;
}) {
  const { t } = useTranslation();
  const [top, setTop] = useState(top10?.top ?? true);
  const [n, setN] = useState(String(top10?.val ?? 10));
  const [percent, setPercent] = useState(top10?.percent ?? false);
  const periodLabel = (p: string) =>
    p.startsWith('Q')
      ? t('sheetEditor.filter.quarter', { n: p.slice(1) })
      : p.startsWith('M')
        ? t('sheetEditor.filter.monthAny', { n: p.slice(1) })
        : t(`sheetEditor.filter.dyn.${p}`);
  const sets = [...new Set(icons.map((i) => i.set))];
  return (
    <div data-testid="filter-extras">
      <div className={styles.row}>
        <span>{t('sheetEditor.filter.topN')}</span>
        <select
          aria-label={t('sheetEditor.filter.topDir')}
          data-testid="filter-top-dir"
          value={top ? 'top' : 'bottom'}
          onChange={(e) => setTop(e.target.value === 'top')}
        >
          <option value="top">{t('sheetEditor.filter.top')}</option>
          <option value="bottom">{t('sheetEditor.filter.bottom')}</option>
        </select>
        <input
          type="number"
          min={1}
          style={{ width: 56 }}
          aria-label={t('sheetEditor.filter.topCount')}
          data-testid="filter-top-n"
          value={n}
          onChange={(e) => setN(e.target.value)}
        />
        <select
          aria-label={t('sheetEditor.filter.topUnit')}
          data-testid="filter-top-unit"
          value={percent ? 'percent' : 'items'}
          onChange={(e) => setPercent(e.target.value === 'percent')}
        >
          <option value="items">{t('sheetEditor.filter.items')}</option>
          <option value="percent">{t('sheetEditor.filter.percent')}</option>
        </select>
        <button
          type="button"
          className={styles.btn}
          data-testid="filter-top-apply"
          onClick={() => {
            const val = Number(n);
            if (Number.isFinite(val) && val > 0) onFilter({ top10: { top, percent, val } });
          }}
        >
          {t('sheetEditor.filter.apply')}
        </button>
      </div>
      <div className={styles.row}>
        <select
          aria-label={t('sheetEditor.filter.period')}
          data-testid="filter-period"
          value=""
          onChange={(e) => e.target.value && onFilter({ dynamic: e.target.value })}
        >
          <option value="">{t('sheetEditor.filter.period')}</option>
          {PERIOD_FILTERS.map((p) => (
            <option key={p} value={p}>
              {periodLabel(p)}
            </option>
          ))}
        </select>
      </div>
      {icons.length > 0 && (
        <div className={styles.row} data-testid="filter-icons">
          <span>{t('sheetEditor.filter.byIcon')}</span>
          {icons.map((i) => (
            <button
              key={`${i.set}:${i.id}`}
              type="button"
              className={styles.btn}
              data-testid={`filter-icon-${i.set}-${i.id}`}
              onClick={() => onFilter({ icon: { set: i.set, id: i.id } })}
            >
              {iconLabel(t, i)}
            </button>
          ))}
          {sets.map((set) => (
            <button
              key={`none-${set}`}
              type="button"
              className={styles.btn}
              data-testid={`filter-icon-${set}-none`}
              onClick={() => onFilter({ icon: { set } })}
            >
              {t('sheetEditor.filter.noIcon')}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// ----------------------------------------------------------------- Advanced Filter

/** `A1:C9` → range; `null` when not a reference. */
export function parseRange(text: string): SheetRange | null {
  const s = parseA1(text);
  return s ? rangeOf(s) : null;
}

export function AdvancedFilterDialog({
  host,
  sheet,
  range,
  onClose,
  onDone,
  fail,
}: Common & { range: SheetRange }) {
  const { t } = useTranslation();
  const [list, setList] = useState(rangeA1(range));
  const [criteria, setCriteria] = useState('');
  const [copy, setCopy] = useState(false);
  const [copyTo, setCopyTo] = useState('');
  const [unique, setUnique] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    void host
      .exec<SheetRange>({ cmd: 'region', sheet, range })
      .then((r) => r && setList(rangeA1(r)))
      .catch(fail);
  }, [host, sheet, range, fail]);

  const ok = async () => {
    const l = parseRange(list);
    const c = parseRange(criteria);
    const d = copy ? parseRange(copyTo) : null;
    if (!l || !c || (copy && !d)) {
      setError(t('sheetEditor.adv.badRange'));
      return;
    }
    try {
      await host.exec<AdvancedFilterResult>({
        cmd: 'advancedFilter',
        sheet,
        list: l,
        criteria: c,
        copyTo: d,
        unique,
      });
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
      title={t('sheetEditor.adv.title')}
      width={460}
      testId="advanced-filter-dialog"
      footer={<Footer onCancel={onClose} onOk={() => void ok()} okId="adv-ok" />}
    >
      <p className={styles.hint}>{t('sheetEditor.adv.hint')}</p>
      <label className={styles.check}>
        <input
          type="radio"
          name="adv-mode"
          data-testid="adv-in-place"
          checked={!copy}
          onChange={() => setCopy(false)}
        />
        {t('sheetEditor.adv.inPlace')}
      </label>
      <label className={styles.check}>
        <input
          type="radio"
          name="adv-mode"
          data-testid="adv-copy"
          checked={copy}
          onChange={() => setCopy(true)}
        />
        {t('sheetEditor.adv.copy')}
      </label>
      <label className={styles.row}>
        <span className={styles.levelLabel}>{t('sheetEditor.adv.list')}</span>
        <input data-testid="adv-list" value={list} onChange={(e) => setList(e.target.value)} />
      </label>
      <label className={styles.row}>
        <span className={styles.levelLabel}>{t('sheetEditor.adv.criteria')}</span>
        <input
          data-testid="adv-criteria"
          value={criteria}
          onChange={(e) => setCriteria(e.target.value)}
        />
      </label>
      <label className={styles.row}>
        <span className={styles.levelLabel}>{t('sheetEditor.adv.copyTo')}</span>
        <input
          data-testid="adv-copy-to"
          disabled={!copy}
          value={copyTo}
          onChange={(e) => setCopyTo(e.target.value)}
        />
      </label>
      <label className={styles.check}>
        <input
          type="checkbox"
          data-testid="adv-unique"
          checked={unique}
          onChange={(e) => setUnique(e.target.checked)}
        />
        {t('sheetEditor.adv.unique')}
      </label>
      {error && (
        <p className={styles.hint} role="alert" data-testid="adv-error">
          {error}
        </p>
      )}
    </Dialog>
  );
}
