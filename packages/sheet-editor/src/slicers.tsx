/**
 * BlueSheet slicers and timelines (SHEET-SLICER-001, SHEET-TIMELINE-001; [MS-XLSX] slicer and
 * timeline parts) and the filter buttons of pivot charts (SHEET-PIVOT-002): the frames drawn
 * over the grid with the colours of their style, the dialogs that insert them over a pivot
 * table, their options and report connections. Every change is one undo step through the
 * command bridge (`insertPivotSlicer`, `slicerOptions`, `slicerConnect`, `moveSlicer`,
 * `insertTimeline`, `timelineOptions`, `timelineConnect`, `moveTimeline`, `pivotFieldItems`).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  FrameColors,
  FrameConnection,
  PivotInfo,
  SlicerInfo,
  TimelineInfo,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { Filter, FunnelX, Link2, Settings2, Trash2, ZoomIn, ZoomOut } from 'lucide-react';
import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import styles from './SheetEditor.module.css';

type T = ReturnType<typeof useTranslation>['t'];

/** Built-in slicer style names ([MS-XLSX] `slicer/@style`) offered in the options dialog. */
export const SLICER_STYLE_NAMES: readonly string[] = [
  ...[1, 2, 3, 4, 5, 6].map((n) => `SlicerStyleLight${n}`),
  'SlicerStyleOther1',
  'SlicerStyleOther2',
  ...[1, 2, 3, 4, 5, 6].map((n) => `SlicerStyleDark${n}`),
];

/** Built-in timeline style names ([MS-XLSX] `timeline/@style`). */
export const TIMELINE_STYLE_NAMES: readonly string[] = [
  ...[1, 2, 3, 4, 5, 6].map((n) => `TimeSlicerStyleLight${n}`),
  ...[1, 2, 3, 4, 5, 6].map((n) => `TimeSlicerStyleDark${n}`),
];

/** Shown name of a built-in style: its family and number (`Dark 3`); other names as they are. */
export function frameStyleLabel(name: string, t: T): string {
  const m = /(Light|Dark|Other)(\d+)$/.exec(name);
  if (!m) return name;
  const family = m[1] === 'Light' ? 'light' : m[1] === 'Dark' ? 'dark' : 'other';
  return t(`sheetSlicer.style.${family}`, { n: m[2] });
}

/** The fields of `pivot` a slicer can filter: source columns that are not grouped. */
export function slicerFields(pivot: PivotInfo): number[] {
  return pivot.fields
    .map((f, i) => ({ f, i }))
    .filter(({ f }) => !f.derived && !f.formula && !f.group && !f.measure)
    .map(({ i }) => i);
}

/** The date fields of `pivot` a timeline can filter. */
export function timelineFields(pivot: PivotInfo): number[] {
  return pivot.fields
    .map((f, i) => ({ f, i }))
    .filter(({ f }) => f.date && !f.derived && !f.formula)
    .map(({ i }) => i);
}

/**
 * The next choice of a slicer after a click on `label`: a plain click keeps only that item (or
 * clears the filter when it was the only one chosen); with Ctrl the item is added or removed.
 * An empty list clears the filter.
 */
export function nextSlicerChoice(info: SlicerInfo, label: string, multi: boolean): string[] {
  const chosen = info.items.filter((i) => i.selected).map((i) => i.label);
  const all = chosen.length === info.items.length;
  if (multi) {
    if (all) return [label];
    return chosen.includes(label) ? chosen.filter((l) => l !== label) : [...chosen, label];
  }
  return !all && chosen.length === 1 && chosen[0] === label ? [] : [label];
}

/** First and last day of the periods `from`..`to` of a timeline (either order). */
export function periodRange(
  info: TimelineInfo,
  from: number,
  to: number,
): { start: number; end: number } | null {
  const a = info.periods[Math.min(from, to)];
  const b = info.periods[Math.max(from, to)];
  return a && b ? { start: a.start, end: b.end } : null;
}

/** The label of a timeline's selection: its first and last selected period. */
export function selectionLabel(info: TimelineInfo, t: T): string {
  if (!info.filtered) return t('sheetSlicer.allPeriods');
  const on = info.periods.filter((p) => p.selected);
  const first = on[0];
  const last = on[on.length - 1];
  if (!first || !last) return t('sheetSlicer.allPeriods');
  return first === last ? first.label : `${first.label} – ${last.label}`;
}

function frameVars(colors: FrameColors | undefined, rect: Box): CSSProperties {
  const vars: Record<string, string | number> = {
    left: rect.x,
    top: rect.y,
    width: rect.w,
    height: rect.h,
  };
  if (colors) {
    vars['--frame-on'] = colors.selected;
    vars['--frame-on-text'] = colors.selectedText;
    vars['--frame-head'] = colors.header;
    vars['--frame-head-text'] = colors.headerText;
    vars['--frame-border'] = colors.border;
  }
  return vars as CSSProperties;
}

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

interface FrameCommon {
  rect: Box;
  selected: boolean;
  /** The sheet can be edited (move, size, options, delete). */
  editable: boolean;
  onSelect: () => void;
  /** Mouse down on the header (`resize` false) or the corner handle (`resize` true). */
  onDragStart: (e: ReactMouseEvent, resize: boolean) => void;
  onSettings: () => void;
  onDelete: () => void;
}

function FrameTools({
  c,
  prefix,
  children,
}: {
  c: FrameCommon;
  prefix: string;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <span className={styles.frameTools} onMouseDown={(e) => e.stopPropagation()}>
      {children}
      {c.editable && (
        <>
          <button
            type="button"
            data-testid={`${prefix}-settings`}
            title={t('sheetSlicer.settings')}
            aria-label={t('sheetSlicer.settings')}
            onClick={c.onSettings}
          >
            <Settings2 size={14} />
          </button>
          <button
            type="button"
            data-testid={`${prefix}-delete`}
            title={t('sheetSlicer.delete')}
            aria-label={t('sheetSlicer.delete')}
            onClick={c.onDelete}
          >
            <Trash2 size={14} />
          </button>
        </>
      )}
    </span>
  );
}

/** A slicer over the grid: the header (drag to move), then one button per item. */
export function SlicerFrame({
  info,
  onChoose,
  onClear,
  ...c
}: FrameCommon & {
  info: SlicerInfo;
  onChoose: (label: string, multi: boolean) => void;
  onClear: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className={`${styles.slicer} ${c.selected ? styles.frameSelected : ''}`}
      data-testid="slicer"
      data-style={info.style ?? ''}
      data-selected={c.selected || undefined}
      role="group"
      aria-label={info.caption}
      tabIndex={0}
      style={frameVars(info.colors, c.rect)}
      onMouseDown={(e) => {
        e.stopPropagation();
        c.onSelect();
      }}
      onKeyDown={(e) => {
        if (e.key === 'Delete' && c.editable && e.target === e.currentTarget) {
          e.preventDefault();
          c.onDelete();
        }
      }}
    >
      <div
        className={styles.slicerHeader}
        data-testid="slicer-header"
        onMouseDown={(e) => {
          if (c.editable && e.button === 0) c.onDragStart(e, false);
        }}
      >
        <span className={styles.frameCaption}>{info.caption}</span>
        <FrameTools c={c} prefix="slicer">
          <button
            type="button"
            data-testid="slicer-clear"
            title={t('sheetEditor.slicer.clear')}
            aria-label={t('sheetEditor.slicer.clear')}
            disabled={info.items.every((i) => i.selected)}
            onClick={onClear}
          >
            <FunnelX size={14} />
          </button>
        </FrameTools>
      </div>
      <div
        className={styles.slicerItems}
        style={{ gridTemplateColumns: `repeat(${info.columns}, 1fr)` }}
      >
        {info.items.map((item) => (
          <button
            key={item.label}
            type="button"
            className={item.selected ? styles.slicerOn : styles.slicerOff}
            aria-pressed={item.selected}
            onClick={(e) => onChoose(item.label, e.ctrlKey || e.metaKey)}
          >
            {item.label || t('sheetEditor.slicer.blank')}
          </button>
        ))}
      </div>
      {c.selected && c.editable && (
        <div
          className={styles.frameHandle}
          data-testid="slicer-resize"
          title={t('sheetSlicer.resize')}
          onMouseDown={(e) => c.onDragStart(e, true)}
        />
      )}
    </div>
  );
}

/**
 * A timeline over the grid: the header with the time level (years … days; the zoom buttons
 * step through the levels), the selected range, then the strip of periods (click one, Shift
 * click to extend the range). The strip scrolls sideways and shows the selection when it
 * changes.
 */
export function TimelineFrame({
  info,
  onRange,
  onClear,
  onLevel,
  ...c
}: FrameCommon & {
  info: TimelineInfo;
  onRange: (start: number, end: number) => void;
  onClear: () => void;
  onLevel: (level: number) => void;
}) {
  const { t } = useTranslation();
  const anchor = useRef<number | null>(null);
  const strip = useRef<HTMLDivElement | null>(null);
  const firstOn = info.filtered ? info.periods.findIndex((p) => p.selected) : -1;
  // Bring the selection into view when it or the level changes.
  useEffect(() => {
    const el = strip.current;
    if (!el || firstOn < 0) return;
    const btn = el.children[firstOn];
    if (btn instanceof HTMLElement) el.scrollLeft = Math.max(0, btn.offsetLeft - 24);
  }, [firstOn, info.level, info.periods.length]);
  const levels = [0, 1, 2, 3];
  return (
    <div
      className={`${styles.slicer} ${c.selected ? styles.frameSelected : ''}`}
      data-testid="timeline"
      data-style={info.style ?? ''}
      data-level={info.level}
      data-selected={c.selected || undefined}
      role="group"
      aria-label={info.caption}
      tabIndex={0}
      style={frameVars(info.colors, c.rect)}
      onMouseDown={(e) => {
        e.stopPropagation();
        c.onSelect();
      }}
      onKeyDown={(e) => {
        if (e.key === 'Delete' && c.editable && e.target === e.currentTarget) {
          e.preventDefault();
          c.onDelete();
        }
      }}
    >
      <div
        className={styles.slicerHeader}
        data-testid="timeline-header"
        onMouseDown={(e) => {
          if (c.editable && e.button === 0) c.onDragStart(e, false);
        }}
      >
        <span className={styles.frameCaption}>{info.caption}</span>
        <FrameTools c={c} prefix="timeline">
          <button
            type="button"
            data-testid="timeline-zoom-out"
            title={t('sheetSlicer.zoomOut')}
            aria-label={t('sheetSlicer.zoomOut')}
            disabled={!c.editable || info.level <= 0}
            onClick={() => onLevel(info.level - 1)}
          >
            <ZoomOut size={14} />
          </button>
          <select
            data-testid="timeline-level"
            aria-label={t('sheetSlicer.level')}
            value={info.level}
            disabled={!c.editable}
            onChange={(e) => onLevel(Number(e.target.value))}
          >
            {levels.map((l) => (
              <option key={l} value={l}>
                {t(`sheetEditor.timeline.levels.${l}`)}
              </option>
            ))}
          </select>
          <button
            type="button"
            data-testid="timeline-zoom-in"
            title={t('sheetSlicer.zoomIn')}
            aria-label={t('sheetSlicer.zoomIn')}
            disabled={!c.editable || info.level >= 3}
            onClick={() => onLevel(info.level + 1)}
          >
            <ZoomIn size={14} />
          </button>
          <button
            type="button"
            data-testid="timeline-clear"
            title={t('sheetEditor.slicer.clear')}
            aria-label={t('sheetEditor.slicer.clear')}
            disabled={!info.filtered}
            onClick={onClear}
          >
            <FunnelX size={14} />
          </button>
        </FrameTools>
      </div>
      <div className={styles.timelineRange} data-testid="timeline-range">
        {selectionLabel(info, t)}
      </div>
      <div className={styles.timelineStrip} ref={strip} data-testid="timeline-strip">
        {info.periods.map((p, k) => (
          <button
            key={p.label}
            type="button"
            className={info.filtered && p.selected ? styles.slicerOn : styles.slicerOff}
            aria-pressed={info.filtered && p.selected}
            onClick={(e) => {
              const from = e.shiftKey && anchor.current !== null ? anchor.current : k;
              if (!e.shiftKey) anchor.current = k;
              const r = periodRange(info, from, k);
              if (r) onRange(r.start, r.end);
            }}
          >
            {p.label}
          </button>
        ))}
      </div>
      {c.selected && c.editable && (
        <div
          className={styles.frameHandle}
          data-testid="timeline-resize"
          title={t('sheetSlicer.resize')}
          onMouseDown={(e) => c.onDragStart(e, true)}
        />
      )}
    </div>
  );
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

/**
 * Insert › Slicer / Timeline for a pivot table: the fields to make frames for (one slicer per
 * ticked field; a timeline takes date fields only).
 */
export function InsertFramesDialog({
  pivot,
  kind,
  onInsert,
  onClose,
}: {
  pivot: PivotInfo;
  kind: 'slicer' | 'timeline';
  /** Insert a frame for every field; rejects with the engine's message. */
  onInsert: (fields: number[]) => Promise<void>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const fields = kind === 'slicer' ? slicerFields(pivot) : timelineFields(pivot);
  const [picked, setPicked] = useState<number[]>([]);
  const [error, setError] = useState('');
  const ok = async () => {
    if (picked.length === 0) {
      setError(t('sheetSlicer.pickField'));
      return;
    }
    try {
      await onInsert([...picked].sort((a, b) => a - b));
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(kind === 'slicer' ? 'sheetSlicer.insertSlicers' : 'sheetSlicer.insertTimelines')}
      testId="frame-insert-dialog"
      footer={<Footer onClose={onClose} onOk={() => void ok()} testId="frame-insert-ok" />}
    >
      <div className={styles.form}>
        {fields.length === 0 && (
          <p>{t(kind === 'slicer' ? 'sheetSlicer.noFields' : 'sheetSlicer.noDateFields')}</p>
        )}
        <div className={styles.valueList} role="group" aria-label={t('sheetSlicer.fields')}>
          {fields.map((i) => (
            <label key={i} className={styles.check}>
              <input
                type="checkbox"
                data-testid={`frame-field-${i}`}
                checked={picked.includes(i)}
                onChange={(e) =>
                  setPicked((p) => (e.target.checked ? [...p, i] : p.filter((x) => x !== i)))
                }
              />
              {pivot.fields[i]?.column ?? pivot.fields[i]?.name}
            </label>
          ))}
        </div>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** What the options dialog of a slicer or timeline changes. */
export interface FrameOptions {
  caption: string;
  /** Slicers only. */
  columns?: number;
  style: string;
  /** Names of the connected pivot tables (pivot slicers and timelines). */
  pivots?: string[];
}

/**
 * Options of a slicer or timeline: caption, columns (slicer), style, and the report
 * connections — the pivot tables on the same data it filters (at least one).
 */
export function FrameOptionsDialog({
  kind,
  caption,
  columns,
  style,
  connections,
  onApply,
  onClose,
}: {
  kind: 'slicer' | 'timeline';
  caption: string;
  columns?: number;
  style: string | null;
  connections: FrameConnection[];
  /** Apply the changes; rejects with the engine's message. */
  onApply: (o: FrameOptions) => Promise<void>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [cap, setCap] = useState(caption);
  const [cols, setCols] = useState(String(columns ?? 1));
  const [st, setSt] = useState(style ?? '');
  const [on, setOn] = useState(() => connections.filter((c) => c.connected).map((c) => c.name));
  const [error, setError] = useState('');
  const names = kind === 'slicer' ? SLICER_STYLE_NAMES : TIMELINE_STYLE_NAMES;
  const ok = async () => {
    const n = Number(cols);
    if (kind === 'slicer' && (!Number.isInteger(n) || n < 1 || n > 50)) {
      setError(t('sheetSlicer.badColumns'));
      return;
    }
    if (connections.length > 0 && on.length === 0) {
      setError(t('sheetSlicer.needConnection'));
      return;
    }
    try {
      await onApply({
        caption: cap,
        columns: kind === 'slicer' ? n : undefined,
        style: st,
        pivots: connections.length > 0 ? on : undefined,
      });
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(kind === 'slicer' ? 'sheetSlicer.slicerOptions' : 'sheetSlicer.timelineOptions')}
      testId="frame-options-dialog"
      footer={<Footer onClose={onClose} onOk={() => void ok()} testId="frame-options-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetSlicer.caption')}
          <input data-testid="frame-caption" value={cap} onChange={(e) => setCap(e.target.value)} />
        </label>
        {kind === 'slicer' && (
          <label>
            {t('sheetSlicer.columns')}
            <input
              data-testid="frame-columns"
              type="number"
              min={1}
              max={50}
              value={cols}
              onChange={(e) => setCols(e.target.value)}
            />
          </label>
        )}
        <label>
          {t('sheetSlicer.styleLabel')}
          <select data-testid="frame-style" value={st} onChange={(e) => setSt(e.target.value)}>
            {!names.includes(st) && <option value={st}>{st || t('sheetSlicer.style.none')}</option>}
            {names.map((n) => (
              <option key={n} value={n}>
                {frameStyleLabel(n, t)}
              </option>
            ))}
          </select>
        </label>
        {connections.length > 0 && (
          <fieldset className={styles.frameConnections}>
            <legend>
              <Link2 size={13} aria-hidden /> {t('sheetSlicer.connections')}
            </legend>
            {connections.map((c) => (
              <label key={`${c.sheet}:${c.name}`} className={styles.check}>
                <input
                  type="checkbox"
                  data-testid={`frame-connection-${c.name}`}
                  checked={on.includes(c.name)}
                  onChange={(e) =>
                    setOn((p) =>
                      e.target.checked ? [...p, c.name] : p.filter((x) => x !== c.name),
                    )
                  }
                />
                {c.name} ({c.sheetName})
              </label>
            ))}
          </fieldset>
        )}
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** A field of a pivot chart with a filter button: its items and the hidden ones. */
export interface ChartFilterField {
  field: number;
  name: string;
  /** `filter`: report filter; `axis`: category (row) field; `legend`: series (column) field. */
  area: 'filter' | 'axis' | 'legend';
  items: string[];
  hidden: number[];
}

/** The fields of `pivot` a pivot chart shows filter buttons for, with their hidden items. */
export function chartFilterFields(pivot: PivotInfo): ChartFilterField[] {
  const of = (field: number, area: ChartFilterField['area']): ChartFilterField | null => {
    const f = pivot.fields[field];
    if (!f) return null;
    let hidden: number[];
    if (area === 'filter') {
      const shown = pivot.filters.find((x) => x.field === field)?.items ?? [];
      hidden = shown.length === 0 ? [] : f.items.map((_, i) => i).filter((i) => !shown.includes(i));
    } else {
      hidden = pivot.fieldOptions[field]?.hidden ?? [];
    }
    return { field, name: f.column ?? f.name, area, items: f.items, hidden };
  };
  return [
    ...pivot.filters.map((f) => of(f.field, 'filter')),
    ...pivot.rows.map((f) => of(f, 'axis')),
    ...pivot.cols.map((f) => of(f, 'legend')),
  ].filter((x): x is ChartFilterField => x !== null);
}

/**
 * The filter buttons of a pivot chart (one per report filter, category and series field of its
 * pivot table): a button opens the field's items as a check list; applying hides the unticked
 * items in the pivot table, and the chart follows.
 */
export function PivotChartFilters({
  pivot,
  rect,
  onApply,
}: {
  pivot: PivotInfo;
  /** The chart's box over the grid. */
  rect: Box;
  /** Hide `hidden` (item indexes) of `field`; rejects with the engine's message. */
  onApply: (field: number, hidden: number[]) => Promise<void>;
}) {
  const { t } = useTranslation();
  const fields = chartFilterFields(pivot);
  const [open, setOpen] = useState<number | null>(null);
  const [off, setOff] = useState<number[]>([]);
  const [error, setError] = useState('');
  if (fields.length === 0) return null;
  const current = fields.find((f) => f.field === open) ?? null;
  const apply = async () => {
    if (!current) return;
    if (off.length >= current.items.length) {
      setError(t('sheetSlicer.keepOne'));
      return;
    }
    try {
      await onApply(
        current.field,
        [...off].sort((a, b) => a - b),
      );
      setOpen(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <div
      className={styles.chartFilters}
      data-testid="pivot-chart-filters"
      style={{ left: rect.x + 4, top: rect.y + 4, maxWidth: Math.max(80, rect.w - 8) }}
      onMouseDown={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
    >
      {fields.map((f) => (
        <button
          key={`${f.area}${f.field}`}
          type="button"
          className={f.hidden.length > 0 ? styles.chartFilterOn : styles.chartFilterOff}
          data-testid={`pivot-chart-filter-${f.field}`}
          data-area={f.area}
          aria-haspopup="dialog"
          aria-expanded={open === f.field}
          title={t(`sheetSlicer.chartField.${f.area}`, { name: f.name })}
          onClick={() => {
            setError('');
            setOff(f.hidden);
            setOpen(open === f.field ? null : f.field);
          }}
        >
          {f.name}
          <Filter size={11} aria-hidden />
        </button>
      ))}
      {current && (
        <div
          className={styles.chartFilterMenu}
          role="dialog"
          aria-label={t('sheetSlicer.chartItems', { name: current.name })}
          data-testid="pivot-chart-filter-menu"
        >
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="pivot-chart-filter-all"
              checked={off.length === 0}
              onChange={(e) => setOff(e.target.checked ? [] : current.items.map((_, i) => i))}
            />
            {t('sheetSlicer.selectAll')}
          </label>
          <div className={styles.valueList}>
            {current.items.map((label, i) => (
              <label key={i} className={styles.check}>
                <input
                  type="checkbox"
                  checked={!off.includes(i)}
                  onChange={(e) =>
                    setOff((p) => (e.target.checked ? p.filter((x) => x !== i) : [...p, i]))
                  }
                />
                {label || t('sheetEditor.slicer.blank')}
              </label>
            ))}
          </div>
          {error && (
            <p className={styles.error} role="alert">
              {error}
            </p>
          )}
          <div className={styles.chartFilterActions}>
            <button type="button" className={styles.btn} onClick={() => setOpen(null)}>
              {t('sheetEditor.dlg.cancel')}
            </button>
            <button
              type="button"
              className={styles.btnPrimary}
              data-testid="pivot-chart-filter-ok"
              onClick={() => void apply()}
            >
              {t('sheetEditor.dlg.ok')}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
