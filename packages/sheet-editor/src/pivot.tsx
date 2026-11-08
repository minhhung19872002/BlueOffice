/**
 * Pivot tables: the "Insert → Pivot table" dialog and the field list side
 * panel (drag fields between Filters / Columns / Rows / Values). The report is
 * computed by the engine (`pivotCreate`, `pivotUpdate`, `pivotRefresh`); the
 * UI only edits the areas.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  PivotAreas,
  PivotFunction,
  PivotInfo,
  PivotLabels,
  SheetHost,
  SheetRange,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { RefreshCw, Settings, Settings2, Sigma, X } from 'lucide-react';
import { useEffect, useState, type DragEvent } from 'react';
import { Popover } from './dialogs';
import { parseA1, rangeA1, rangeOf } from './grid';
import { areasOf, PIVOT_FUNCTIONS } from './pivot-dialogs';
import styles from './SheetEditor.module.css';

export { PIVOT_FUNCTIONS };

type Area = 'filters' | 'cols' | 'rows' | 'values';
const AREAS: Area[] = ['filters', 'cols', 'rows', 'values'];
const DRAG_TYPE = 'application/x-bo-pivot-field';

/** Localized captions for report cells. */
export function usePivotLabels(): PivotLabels {
  const { t } = useTranslation();
  return {
    grandTotal: t('sheetEditor.pivot.labels.grandTotal'),
    values: t('sheetEditor.pivot.labels.values'),
    all: t('sheetEditor.pivot.labels.all'),
    multiple: t('sheetEditor.pivot.labels.multiple'),
    blank: t('sheetEditor.pivot.labels.blank'),
    column: t('sheetEditor.pivot.labels.column'),
    name: t('sheetEditor.pivot.labels.name'),
    subtotal: t('sheetEditor.pivot.labels.subtotal'),
    rowLabels: t('sheetEditor.pivot.labels.rowLabels'),
    colLabels: t('sheetEditor.pivot.labels.colLabels'),
    months: t('sheetEditor.pivot.labels.months').split('|'),
    quarter: t('sheetEditor.pivot.labels.quarter'),
    yearsField: t('sheetEditor.pivot.labels.yearsField'),
    quartersField: t('sheetEditor.pivot.labels.quartersField'),
    monthsField: t('sheetEditor.pivot.labels.monthsField'),
    group: t('sheetEditor.pivot.labels.group'),
  };
}

const contains = (r: SheetRange, row: number, col: number) =>
  row >= r.r0 && row <= r.r1 && col >= r.c0 && col <= r.c1;

/**
 * The row / column field and item indices shown by the selected report cells
 * (labels matched against the items of the pivot's axis fields, innermost
 * field first). `null` when no item label is selected.
 */
export function pivotSelectionItems(
  p: PivotInfo,
  texts: string[],
): { field: number; items: number[] } | null {
  const wanted = texts.map((x) => x.trim()).filter((x) => x !== '');
  for (const field of [...p.rows].reverse().concat([...p.cols].reverse())) {
    const items = p.fields[field]?.items ?? [];
    const hit = items.flatMap((it, i) => (wanted.includes(it) ? [i] : []));
    if (hit.length > 0) return { field, items: hit };
  }
  return null;
}

/** The pivot table whose cells contain the given cell. */
export function pivotAt(list: PivotInfo[], row: number, col: number): PivotInfo | null {
  return list.find((p) => contains(p.area, row, col)) ?? null;
}

// ----------------------------------------------------------------- insert dialog

export function PivotDialog({
  host,
  sheet,
  range,
  sheets,
  labels,
  onClose,
  onDone,
  onCreated,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  range: SheetRange;
  sheets: string[];
  labels: PivotLabels;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  onCreated: (sheet: number, index: number) => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [source, setSource] = useState('');
  const [summary, setSummary] = useState<{ fields: string[]; records: number } | null>(null);
  const [dest, setDest] = useState<'new' | 'existing'>('new');
  const [destSheet, setDestSheet] = useState(sheet);
  const [location, setLocation] = useState('');
  const [error, setError] = useState('');

  useEffect(() => {
    void host
      .exec<{ range: SheetRange; fields: string[]; records: number }>({
        cmd: 'pivotSource',
        sheet,
        range,
      })
      .then((r) => {
        if (!r) return;
        setSource(rangeA1(r.range));
        setSummary({ fields: r.fields, records: r.records });
      })
      .catch(fail);
    // Loaded once per dialog.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, sheet]);

  const ok = async () => {
    const src = parseA1(source);
    if (!src) {
      setError(t('sheetEditor.pivot.badRef'));
      return;
    }
    let at = null;
    if (dest === 'existing') {
      const l = parseA1(location);
      if (!l) {
        setError(t('sheetEditor.pivot.badRef'));
        return;
      }
      at = { sheet: destSheet, row: l.active.row, col: l.active.col };
    }
    try {
      const r = rangeOf(src);
      const res = await host.exec<{ sheet: number; index: number }>({
        cmd: 'pivotCreate',
        sheet,
        range: { r0: r.r0, c0: r.c0, r1: r.r1, c1: r.c1 },
        dest: at,
        labels,
      });
      await onDone();
      if (res) onCreated(res.sheet, res.index);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.pivot.insertTitle')}
      testId="pivot-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="pivot-ok"
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.pivot.source')}
          <input
            data-testid="pivot-source"
            value={source}
            onChange={(e) => setSource(e.target.value)}
          />
        </label>
        {summary && (
          <p className={styles.hint}>
            {summary.fields.length > 0
              ? t('sheetEditor.pivot.fieldsFound', {
                  n: summary.fields.length,
                  records: summary.records,
                })
              : t('sheetEditor.pivot.noFields')}
          </p>
        )}
        <p className={styles.hint}>{t('sheetEditor.pivot.destination')}</p>
        <label className={styles.check}>
          <input
            type="radio"
            name="pivot-dest"
            checked={dest === 'new'}
            onChange={() => setDest('new')}
          />
          {t('sheetEditor.pivot.newSheet')}
        </label>
        <label className={styles.check}>
          <input
            type="radio"
            name="pivot-dest"
            data-testid="pivot-dest-existing"
            checked={dest === 'existing'}
            onChange={() => setDest('existing')}
          />
          {t('sheetEditor.pivot.existingSheet')}
        </label>
        {dest === 'existing' && (
          <>
            <label>
              {t('sheetEditor.pivot.sheet')}
              <select value={destSheet} onChange={(e) => setDestSheet(Number(e.target.value))}>
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
                data-testid="pivot-location"
                value={location}
                onChange={(e) => setLocation(e.target.value)}
              />
            </label>
          </>
        )}
        {error && <p className={styles.error}>{error}</p>}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- field list panel

interface Drag {
  field: number;
  from: Area | 'list';
  /** Position in the source area. */
  idx: number;
}

/** A field whose items are all numbers goes to Values when ticked. */
const numeric = (items: string[]) =>
  items.length > 0 && items.every((s) => s !== '' && !Number.isNaN(Number(s)));

export function PivotPanel({
  host,
  sheet,
  pivot,
  labels,
  onChanged,
  onRefresh,
  onClose,
  onSettings,
  onOptions,
  onCalc,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  pivot: PivotInfo;
  labels: PivotLabels;
  onChanged: () => void | Promise<void>;
  onRefresh: () => void;
  onClose: () => void;
  /** Field settings of a chip (row / column field or value). */
  onSettings: (area: 'rows' | 'cols' | 'values', idx: number) => void;
  onOptions: () => void;
  onCalc: () => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [areas, setAreas] = useState<PivotAreas>(() => areasOf(pivot));
  const [over, setOver] = useState<Area | null>(null);
  const [filterMenu, setFilterMenu] = useState<{
    field: number;
    x: number;
    y: number;
    items: boolean[];
  } | null>(null);
  // Server state replaces the local copy whenever the engine sends a new one.
  const [shown, setShown] = useState(pivot);
  if (shown !== pivot) {
    setShown(pivot);
    setAreas(areasOf(pivot));
  }

  const fieldName = (f: number) => pivot.fields[f]?.name ?? '';
  const valueName = (field: number, fn: PivotFunction) =>
    t('sheetEditor.pivot.valueName', {
      fn: t(`sheetEditor.pivot.fn.${fn}`),
      field: fieldName(field),
    });

  const apply = async (next: PivotAreas) => {
    setAreas(next);
    try {
      await host.exec({ cmd: 'pivotUpdate', sheet, index: pivot.index, labels, ...next });
      await onChanged();
    } catch (e) {
      fail(e);
    }
  };

  /** Removes a field from the axis areas (a field is on at most one axis). */
  const withoutAxis = (a: PivotAreas, field: number): PivotAreas => ({
    ...a,
    rows: a.rows.filter((f) => f !== field),
    cols: a.cols.filter((f) => f !== field),
    filters: a.filters.filter((f) => f.field !== field),
  });

  const removeFrom = (a: PivotAreas, area: Area, idx: number): PivotAreas => {
    const next = { ...a };
    if (area === 'rows') next.rows = a.rows.filter((_, i) => i !== idx);
    if (area === 'cols') next.cols = a.cols.filter((_, i) => i !== idx);
    if (area === 'filters') next.filters = a.filters.filter((_, i) => i !== idx);
    if (area === 'values') next.values = a.values.filter((_, i) => i !== idx);
    return next;
  };

  const addTo = (a: PivotAreas, area: Area, field: number, at?: number): PivotAreas => {
    if (area === 'values') {
      const fn: PivotFunction = numeric(pivot.fields[field]?.items ?? []) ? 'sum' : 'count';
      const values = [...a.values];
      values.splice(at ?? values.length, 0, { field, function: fn, name: valueName(field, fn) });
      return { ...a, values };
    }
    const b = withoutAxis(a, field);
    if (area === 'rows') {
      const rows = [...b.rows];
      rows.splice(at ?? rows.length, 0, field);
      return { ...b, rows };
    }
    if (area === 'cols') {
      const cols = [...b.cols];
      cols.splice(at ?? cols.length, 0, field);
      return { ...b, cols };
    }
    return { ...b, filters: [...b.filters, { field, items: [] }] };
  };

  const used = (f: number) =>
    areas.rows.includes(f) ||
    areas.cols.includes(f) ||
    areas.filters.some((x) => x.field === f) ||
    areas.values.some((x) => x.field === f);

  const toggle = (f: number) => {
    if (used(f)) {
      void apply({ ...withoutAxis(areas, f), values: areas.values.filter((v) => v.field !== f) });
    } else {
      const fl = pivot.fields[f];
      // Numbers go to Values; dates and text to Rows; calculated fields are values only.
      const toValues =
        !!fl && (fl.formula !== null || (!fl.date && !fl.derived && numeric(fl.items)));
      void apply(addTo(areas, toValues ? 'values' : 'rows', f));
    }
  };

  const onDragStart = (e: DragEvent, d: Drag) => {
    e.dataTransfer.setData(DRAG_TYPE, JSON.stringify(d));
    e.dataTransfer.setData('text/plain', fieldName(d.field));
    e.dataTransfer.effectAllowed = 'move';
  };
  const onDrop = (e: DragEvent, area: Area) => {
    e.preventDefault();
    setOver(null);
    const raw = e.dataTransfer.getData(DRAG_TYPE);
    if (!raw) return;
    const d = JSON.parse(raw) as Drag;
    const base = d.from === 'list' ? areas : removeFrom(areas, d.from, d.idx);
    void apply(addTo(base, area, d.field));
  };
  const dropOutside = (e: DragEvent) => {
    // Dropping a chip on the field list removes it from its area.
    e.preventDefault();
    const raw = e.dataTransfer.getData(DRAG_TYPE);
    if (!raw) return;
    const d = JSON.parse(raw) as Drag;
    if (d.from !== 'list') void apply(removeFrom(areas, d.from, d.idx));
  };

  const chips = (area: Area): { field: number; label: string }[] => {
    switch (area) {
      case 'rows':
        return areas.rows.map((f) => ({ field: f, label: fieldName(f) }));
      case 'cols':
        return areas.cols.map((f) => ({ field: f, label: fieldName(f) }));
      case 'filters':
        return areas.filters.map((f) => ({ field: f.field, label: fieldName(f.field) }));
      default:
        return areas.values.map((v) => ({
          field: v.field,
          label: v.name ?? valueName(v.field, v.function ?? 'sum'),
        }));
    }
  };

  return (
    <aside
      className={styles.pivotPanel}
      data-testid="pivot-panel"
      aria-label={t('sheetEditor.pivot.panelTitle')}
    >
      <div className={styles.pivotHead}>
        <strong>{t('sheetEditor.pivot.panelTitle')}</strong>
        <span className={styles.pivotTools}>
          <button
            type="button"
            className={styles.iconBtn}
            title={t('sheetEditor.pivot.refresh')}
            aria-label={t('sheetEditor.pivot.refresh')}
            data-testid="pivot-refresh"
            onClick={onRefresh}
          >
            <RefreshCw size={14} />
          </button>
          <button
            type="button"
            className={styles.iconBtn}
            title={t('sheetEditor.pivot.calcField')}
            aria-label={t('sheetEditor.pivot.calcField')}
            data-testid="pivot-calc"
            onClick={onCalc}
          >
            <Sigma size={14} />
          </button>
          <button
            type="button"
            className={styles.iconBtn}
            title={t('sheetEditor.pivot.options')}
            aria-label={t('sheetEditor.pivot.options')}
            data-testid="pivot-options"
            onClick={onOptions}
          >
            <Settings2 size={14} />
          </button>
          <button
            type="button"
            className={styles.iconBtn}
            title={t('sheetEditor.pivot.close')}
            aria-label={t('sheetEditor.pivot.close')}
            onClick={onClose}
          >
            <X size={14} />
          </button>
        </span>
      </div>
      <p className={styles.hint}>{pivot.name}</p>
      <p className={styles.hint}>{t('sheetEditor.pivot.chooseFields')}</p>
      <div
        className={styles.pivotFields}
        onDragOver={(e) => e.preventDefault()}
        onDrop={dropOutside}
      >
        {pivot.fields.map((f, i) => (
          <label
            key={i}
            className={styles.pivotField}
            draggable
            onDragStart={(e) => onDragStart(e, { field: i, from: 'list', idx: -1 })}
          >
            <input
              type="checkbox"
              data-testid={`pivot-field-${i}`}
              checked={used(i)}
              onChange={() => toggle(i)}
            />
            <span>{f.name}</span>
          </label>
        ))}
      </div>
      <p className={styles.hint}>{t('sheetEditor.pivot.dragHint')}</p>
      <div className={styles.pivotAreas}>
        {AREAS.map((area) => (
          <div
            key={area}
            className={over === area ? styles.pivotAreaOver : styles.pivotArea}
            data-testid={`pivot-area-${area}`}
            onDragOver={(e) => {
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              setOver(area);
            }}
            onDragLeave={() => setOver((o) => (o === area ? null : o))}
            onDrop={(e) => onDrop(e, area)}
          >
            <div className={styles.pivotAreaTitle}>{t(`sheetEditor.pivot.areas.${area}`)}</div>
            {chips(area).map((c, idx) => (
              <div
                key={`${c.field}-${idx}`}
                className={styles.pivotChip}
                draggable
                data-testid={`pivot-chip-${area}-${idx}`}
                onDragStart={(e) => onDragStart(e, { field: c.field, from: area, idx })}
              >
                <span className={styles.pivotChipLabel}>{c.label}</span>
                {area === 'values' && (
                  <select
                    aria-label={t('sheetEditor.pivot.summarize')}
                    data-testid={`pivot-fn-${idx}`}
                    value={areas.values[idx]?.function ?? 'sum'}
                    onChange={(e) => {
                      const fn = e.target.value as PivotFunction;
                      const values = areas.values.map((v, i) =>
                        i === idx ? { ...v, function: fn, name: valueName(v.field, fn) } : v,
                      );
                      void apply({ ...areas, values });
                    }}
                  >
                    {PIVOT_FUNCTIONS.map((fn) => (
                      <option key={fn} value={fn}>
                        {t(`sheetEditor.pivot.fn.${fn}`)}
                      </option>
                    ))}
                  </select>
                )}
                {area === 'filters' && (
                  <button
                    type="button"
                    className={styles.iconBtn}
                    aria-label={t('sheetEditor.pivot.filterItems')}
                    data-testid={`pivot-filter-${idx}`}
                    onClick={(e) => {
                      const sel = areas.filters[idx]?.items ?? [];
                      const items = (pivot.fields[c.field]?.items ?? []).map(
                        (_, i) => sel.length === 0 || sel.includes(i),
                      );
                      setFilterMenu({ field: c.field, x: e.clientX, y: e.clientY + 4, items });
                    }}
                  >
                    ▾
                  </button>
                )}
                {area !== 'filters' && (
                  <button
                    type="button"
                    className={styles.iconBtn}
                    title={t('sheetEditor.pivot.settings')}
                    aria-label={t('sheetEditor.pivot.settings')}
                    data-testid={`pivot-settings-${area}-${idx}`}
                    onClick={() => onSettings(area, idx)}
                  >
                    <Settings size={12} />
                  </button>
                )}
                <button
                  type="button"
                  className={styles.iconBtn}
                  aria-label={t('sheetEditor.pivot.remove')}
                  onClick={() => void apply(removeFrom(areas, area, idx))}
                >
                  <X size={12} />
                </button>
              </div>
            ))}
          </div>
        ))}
      </div>
      {areas.rows.length + areas.cols.length + areas.values.length === 0 && (
        <p className={styles.hint}>{t('sheetEditor.pivot.empty')}</p>
      )}
      {filterMenu && (
        <Popover
          x={filterMenu.x}
          y={filterMenu.y}
          onClose={() => setFilterMenu(null)}
          testId="pivot-filter-menu"
        >
          <div className={styles.filterMenu}>
            <strong>{t('sheetEditor.pivot.filterItems')}</strong>
            <div className={styles.valueList}>
              {(pivot.fields[filterMenu.field]?.items ?? []).map((label, i) => (
                <label key={i} className={styles.check}>
                  <input
                    type="checkbox"
                    checked={filterMenu.items[i] ?? false}
                    onChange={() =>
                      setFilterMenu({
                        ...filterMenu,
                        items: filterMenu.items.map((v, j) => (j === i ? !v : v)),
                      })
                    }
                  />
                  {label || labels.blank}
                </label>
              ))}
            </div>
            <div className={styles.row}>
              <button
                type="button"
                className={styles.btnPrimary}
                data-testid="pivot-filter-ok"
                onClick={() => {
                  const chosen = filterMenu.items.flatMap((v, i) => (v ? [i] : []));
                  const all = chosen.length === filterMenu.items.length;
                  const filters = areas.filters.map((f) =>
                    f.field === filterMenu.field ? { ...f, items: all ? [] : chosen } : f,
                  );
                  setFilterMenu(null);
                  void apply({ ...areas, filters });
                }}
              >
                {t('sheetEditor.dlg.ok')}
              </button>
            </div>
          </div>
        </Popover>
      )}
    </aside>
  );
}
