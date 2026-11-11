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
  FrameElementFormat,
  PivotInfo,
  SlicerInfo,
  TimelineInfo,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import {
  Filter,
  FunnelX,
  Link2,
  ListChecks,
  Settings2,
  Trash2,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
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
 * clears the filter when it was the only one chosen); with Ctrl (or Multi-Select) the item is
 * added or removed — with every item chosen (no filter) that click takes it out. An empty list
 * clears the filter.
 */
export function nextSlicerChoice(info: SlicerInfo, label: string, multi: boolean): string[] {
  const chosen = info.items.filter((i) => i.selected).map((i) => i.label);
  const all = chosen.length === info.items.length;
  if (multi) {
    return chosen.includes(label) ? chosen.filter((l) => l !== label) : [...chosen, label];
  }
  return !all && chosen.length === 1 && chosen[0] === label ? [] : [label];
}

/**
 * The choice of a slicer after a Shift click: every item from the last clicked one (nchor) to
 * label, in the slicer's order. Without an anchor (or an unknown one) only label.
 */
export function slicerRangeChoice(
  info: SlicerInfo,
  anchor: string | null,
  label: string,
): string[] {
  const labels = info.items.map((i) => i.label);
  const a = anchor === null ? -1 : labels.indexOf(anchor);
  const b = labels.indexOf(label);
  if (b < 0) return [];
  if (a < 0) return [label];
  return labels.slice(Math.min(a, b), Math.max(a, b) + 1);
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
    if (colors.background) vars['--frame-bg'] = colors.background;
    if (colors.unselected) vars['--frame-off'] = colors.unselected;
    if (colors.unselectedText) vars['--frame-off-text'] = colors.unselectedText;
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
  edit = true,
  className = styles.frameTools,
}: {
  c: FrameCommon;
  prefix: string;
  children?: ReactNode;
  /** Show the settings and delete buttons (an editable frame). */
  edit?: boolean;
  className?: string;
}) {
  const { t } = useTranslation();
  return (
    <span className={className} onMouseDown={(e) => e.stopPropagation()}>
      {children}
      {edit && c.editable && (
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

/**
 * A slicer over the grid: the header (drag to move) with the Multi-Select and Clear Filter
 * buttons, then one button per item. A click keeps only that item; Ctrl click (or any click
 * while Multi-Select is on) adds or removes it; Shift click takes the items from the last
 * clicked one. Alt+S toggles Multi-Select and Alt+C clears the filter while the slicer has the
 * focus.
 */
export function SlicerFrame({
  info,
  onChoose,
  onClear,
  ...c
}: FrameCommon & {
  info: SlicerInfo;
  /** The items to keep (empty: no filter). */
  onChoose: (items: string[]) => void;
  onClear: () => void;
}) {
  const { t } = useTranslation();
  const [multi, setMulti] = useState(false);
  const anchor = useRef<string | null>(null);
  const filtered = !info.items.every((i) => i.selected);
  const click = (label: string, e: ReactMouseEvent) => {
    if (e.shiftKey) {
      onChoose(slicerRangeChoice(info, anchor.current, label));
      return;
    }
    anchor.current = label;
    onChoose(nextSlicerChoice(info, label, multi || e.ctrlKey || e.metaKey));
  };
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
        } else if (e.altKey && (e.key === 's' || e.key === 'S')) {
          e.preventDefault();
          setMulti((m) => !m);
        } else if (e.altKey && (e.key === 'c' || e.key === 'C')) {
          e.preventDefault();
          if (filtered) onClear();
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
        <FrameTools c={c} prefix="slicer" edit={false}>
          <button
            type="button"
            data-testid="slicer-multi"
            title={t('sheetSlicer.multiSelect')}
            aria-label={t('sheetSlicer.multiSelect')}
            aria-pressed={multi}
            onClick={() => setMulti((m) => !m)}
          >
            <ListChecks size={14} />
          </button>
          <button
            type="button"
            data-testid="slicer-clear"
            title={t('sheetEditor.slicer.clear')}
            aria-label={t('sheetEditor.slicer.clear')}
            disabled={!filtered}
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
            data-testid="slicer-item"
            className={item.selected ? styles.slicerOn : styles.slicerOff}
            aria-pressed={item.selected}
            onClick={(e) => click(item.label, e)}
          >
            {item.label || t('sheetEditor.slicer.blank')}
          </button>
        ))}
      </div>
      {c.selected && c.editable && (
        // Like the header of a spreadsheet slicer, which holds only Multi-Select and Clear
        // Filter: the options and delete buttons sit over the selected frame.
        <FrameTools c={c} prefix="slicer" className={styles.frameFloat} />
      )}
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

/** The index of the first period of `periods` a strip scrolled to `day` shows (0 without one). */
export function periodAt(periods: TimelineInfo['periods'], day: number | null | undefined): number {
  if (day === null || day === undefined) return 0;
  const k = periods.findIndex((p) => p.end >= day);
  return k < 0 ? Math.max(0, periods.length - 1) : k;
}

/**
 * A timeline over the grid: the header (caption, time level — the zoom buttons step through
 * years … days — and Clear Filter), the label of the selected range, then the strip of periods:
 * click one, drag across several, or Shift click to extend the range from the last one clicked.
 * Timeline Tools › Show hides the header, the selection label, the time level or the scroll bar.
 * The strip shows the selection when it changes; where it is scrolled to is kept with the file.
 */
export function TimelineFrame({
  info,
  onRange,
  onClear,
  onLevel,
  onScroll,
  ...c
}: FrameCommon & {
  info: TimelineInfo;
  onRange: (start: number, end: number) => void;
  onClear: () => void;
  onLevel: (level: number) => void;
  /** The strip was scrolled: the first day it shows (`null`: the start). */
  onScroll?: (day: number | null) => void;
}) {
  const { t } = useTranslation();
  const anchor = useRef<number | null>(null);
  const strip = useRef<HTMLDivElement | null>(null);
  const [drag, setDrag] = useState<{ from: number; to: number } | null>(null);
  const dragRef = useRef<{ from: number; to: number } | null>(null);
  const scrollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const showHeader = info.showHeader !== false;
  const showLabel = info.showSelectionLabel !== false;
  const showLevel = info.showTimeLevel !== false;
  const showBar = info.showScrollbar !== false;
  const firstOn = info.filtered ? info.periods.findIndex((p) => p.selected) : -1;
  const lastOn = info.filtered ? info.periods.findLastIndex((p) => p.selected) : -1;
  // Bring the selection into view when it or the level changes; without one, where the strip
  // was scrolled to.
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    const k = firstOn >= 0 ? firstOn : periodAt(info.periods, info.scrollDay);
    const btn = el.children[k];
    if (btn instanceof HTMLElement)
      el.scrollLeft = Math.max(0, btn.offsetLeft - (firstOn >= 0 ? 24 : 6));
    // Only when the selection or the periods change, not on every scroll.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firstOn, info.level, info.periods.length]);
  /**
   * A drag over the periods from period `from` (the fixed end) to `to`: it ends where the mouse
   * button goes up.
   */
  const startDrag = (from: number, to = from) => {
    const d = { from, to };
    dragRef.current = d;
    setDrag(d);
    window.addEventListener(
      'mouseup',
      () => {
        const done = dragRef.current;
        dragRef.current = null;
        setDrag(null);
        if (!done) return;
        anchor.current = done.from;
        const r = periodRange(info, done.from, done.to);
        if (r) onRange(r.start, r.end);
      },
      { once: true },
    );
  };
  useEffect(
    () => () => {
      if (scrollTimer.current) clearTimeout(scrollTimer.current);
    },
    [],
  );
  const scrolled = () => {
    if (!onScroll) return;
    if (scrollTimer.current) clearTimeout(scrollTimer.current);
    scrollTimer.current = setTimeout(() => {
      const el = strip.current;
      if (!el) return;
      const kids = Array.from(el.children) as HTMLElement[];
      const k = Math.max(
        0,
        kids.findIndex((b) => b.offsetLeft + b.offsetWidth > el.scrollLeft + 6),
      );
      const day = k === 0 ? null : (info.periods[k]?.start ?? null);
      const was = info.scrollDay ?? null;
      if (day !== was && periodAt(info.periods, was) !== k) onScroll(day);
    }, 250);
  };
  const inDrag = (k: number) =>
    drag !== null && k >= Math.min(drag.from, drag.to) && k <= Math.max(drag.from, drag.to);
  const levels = [0, 1, 2, 3];
  const levelTools = showLevel && (
    <>
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
    </>
  );
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
        // Without a header the frame moves by its edge.
        if (!showHeader && c.editable && e.button === 0 && e.target === e.currentTarget)
          c.onDragStart(e, false);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Delete' && c.editable && e.target === e.currentTarget) {
          e.preventDefault();
          c.onDelete();
        } else if (e.altKey && (e.key === 'c' || e.key === 'C')) {
          e.preventDefault();
          if (info.filtered) onClear();
        }
      }}
    >
      {showHeader && (
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
      )}
      {(showLabel || showLevel || (!showHeader && c.editable)) && (
        // The selection label at the left, the time level at the right (the frame's settings
        // and delete buttons too when there is no header).
        <div className={styles.timelineBar}>
          <span
            className={styles.timelineRange}
            data-testid={showLabel ? 'timeline-range' : undefined}
          >
            {showLabel ? selectionLabel(info, t) : ''}
          </span>
          {showHeader ? (
            <span className={styles.frameTools} onMouseDown={(e) => e.stopPropagation()}>
              {levelTools}
            </span>
          ) : (
            <FrameTools c={c} prefix="timeline">
              {levelTools}
            </FrameTools>
          )}
        </div>
      )}
      <div
        className={`${styles.timelineStrip} ${showBar ? '' : styles.timelineNoBar}`}
        ref={strip}
        data-testid="timeline-strip"
        data-scrollbar={showBar ? 'shown' : 'hidden'}
        onScroll={scrolled}
      >
        {info.periods.map((p, k) => (
          <button
            key={p.label}
            type="button"
            data-testid="timeline-period"
            className={`${info.filtered && p.selected ? styles.slicerOn : styles.slicerOff} ${
              inDrag(k) ? styles.timelineDrag : ''
            }`}
            aria-pressed={info.filtered && p.selected}
            onMouseDown={(e) => {
              if (e.button !== 0 || e.shiftKey) return;
              // A drag over the periods chooses a range (committed on mouse up).
              e.preventDefault();
              startDrag(k);
            }}
            onMouseEnter={() => {
              const d = dragRef.current;
              if (!d) return;
              dragRef.current = { from: d.from, to: k };
              setDrag(dragRef.current);
            }}
            onClick={(e) => {
              // Mouse clicks are handled on mouse up (drag); this is Shift click and the
              // keyboard (Enter / Space).
              if (!e.shiftKey && e.detail !== 0) return;
              const from = e.shiftKey && anchor.current !== null ? anchor.current : k;
              if (!e.shiftKey) anchor.current = k;
              const r = periodRange(info, from, k);
              if (r) onRange(r.start, r.end);
            }}
          >
            {p.label}
            {/* The ends of the selection: drag one to move that end (Shift click does it
                from the keyboard too). */}
            {c.editable && drag === null && k === firstOn && (
              <span
                className={`${styles.timelineEdge} ${styles.timelineEdgeStart}`}
                data-testid="timeline-edge-start"
                title={t('sheetSlicer.dragStart')}
                aria-hidden
                onMouseDown={(e) => {
                  if (e.button !== 0) return;
                  e.preventDefault();
                  e.stopPropagation();
                  startDrag(lastOn, k);
                }}
              />
            )}
            {c.editable && drag === null && k === lastOn && (
              <span
                className={`${styles.timelineEdge} ${styles.timelineEdgeEnd}`}
                data-testid="timeline-edge-end"
                title={t('sheetSlicer.dragEnd')}
                aria-hidden
                onMouseDown={(e) => {
                  if (e.button !== 0) return;
                  e.preventDefault();
                  e.stopPropagation();
                  startDrag(firstOn, k);
                }}
              />
            )}
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

/** The elements a new slicer or timeline style formats, in the dialog's order. */
export const FRAME_STYLE_ELEMENTS = ['whole', 'header', 'selected', 'unselected'] as const;
export type FrameStyleElement = (typeof FRAME_STYLE_ELEMENTS)[number];

/** A new slicer or timeline style: its name and the format of each element it sets. */
export interface NewFrameStyle {
  name: string;
  formats: Partial<Record<FrameStyleElement, FrameElementFormat>>;
}

/** The first free default name of a new style (`Slicer style 1`, `Timeline style 2`, …). */
export function nextFrameStyleName(
  kind: 'slicer' | 'timeline',
  taken: readonly string[],
  t: T,
): string {
  const key = kind === 'slicer' ? 'sheetSlicer.newStyle.slicerName' : 'sheetSlicer.newStyle.timelineName';
  const lower = taken.map((n) => n.toLowerCase());
  for (let n = 1; ; n++) {
    const name = t(key, { n });
    if (!lower.includes(name.toLowerCase())) return name;
  }
}

/** Does an element format set anything? */
export function hasFormat(f: FrameElementFormat | undefined): boolean {
  return !!f && (!!f.fill || !!f.text || !!f.border || !!f.bold || !!f.italic);
}

/** The colours of `base` with the formats of a new style over them (the dialog's preview). */
export function previewColors(
  base: FrameColors,
  formats: NewFrameStyle['formats'],
): FrameColors {
  const c: FrameColors = { ...base };
  const w = formats.whole;
  if (w?.fill) c.background = w.fill;
  if (w?.border) c.border = w.border;
  if (w?.text) {
    c.headerText = w.text;
    c.unselectedText = w.text;
  }
  const h = formats.header;
  if (h?.fill) c.header = h.fill;
  if (h?.text) c.headerText = h.text;
  const s = formats.selected;
  if (s?.fill) c.selected = s.fill;
  if (s?.text) c.selectedText = s.text;
  const u = formats.unselected;
  if (u?.fill) c.unselected = u.fill;
  if (u?.text) c.unselectedText = u.text;
  return c;
}

const ELEMENT_LABEL: Record<'slicer' | 'timeline', Record<FrameStyleElement, string>> = {
  slicer: {
    whole: 'sheetSlicer.newStyle.element.whole',
    header: 'sheetSlicer.newStyle.element.header',
    selected: 'sheetSlicer.newStyle.element.selected',
    unselected: 'sheetSlicer.newStyle.element.unselected',
  },
  timeline: {
    whole: 'sheetSlicer.newStyle.element.wholeTimeline',
    header: 'sheetSlicer.newStyle.element.header',
    selected: 'sheetSlicer.newStyle.element.selectedTime',
    unselected: 'sheetSlicer.newStyle.element.unselectedTime',
  },
};

/**
 * New Slicer Style / New Timeline Style, as spreadsheet applications offer it: a name, the list
 * of the style's elements (whole frame, header, selected and unselected items), the format of the
 * element picked in the list (fill, text colour, bold / italic, border; Clear takes it away) and
 * a preview. OK creates the style in the workbook (one undo step); frames keep their style until
 * one is chosen for them.
 */
export function NewFrameStyleDialog({
  kind,
  taken,
  base,
  onCreate,
  onClose,
}: {
  kind: 'slicer' | 'timeline';
  /** Names of the workbook's styles of this kind (the default name avoids them). */
  taken: readonly string[];
  /** The colours the preview starts from (the frame's style). */
  base: FrameColors;
  /** Create the style; rejects with the engine's message. */
  onCreate: (s: NewFrameStyle) => Promise<void>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(() => nextFrameStyleName(kind, taken, t));
  const [element, setElement] = useState<FrameStyleElement>('whole');
  const [formats, setFormats] = useState<NewFrameStyle['formats']>({});
  const [error, setError] = useState('');
  const f = formats[element] ?? {};
  const set = (patch: Partial<FrameElementFormat>) => {
    const next: FrameElementFormat = { ...f, ...patch };
    for (const k of Object.keys(next) as (keyof FrameElementFormat)[]) {
      if (next[k] === undefined || next[k] === false) delete next[k];
    }
    setFormats((all) => ({ ...all, [element]: next }));
  };
  const colors = previewColors(base, formats);
  const ok = async () => {
    if (!name.trim()) {
      setError(t('sheetSlicer.newStyle.nameNeeded'));
      return;
    }
    const used: NewFrameStyle['formats'] = {};
    for (const e of FRAME_STYLE_ELEMENTS) {
      const x = formats[e];
      if (x && hasFormat(x)) used[e] = x;
    }
    try {
      await onCreate({ name: name.trim(), formats: used });
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  const colorRow = (
    key: 'fill' | 'text' | 'border',
    use: string,
    label: string,
    fallback: string,
  ) => (
    <div className={styles.check}>
      <label className={styles.check}>
        <input
          type="checkbox"
          data-testid={`frame-style-use-${key}`}
          checked={!!f[key]}
          onChange={(e) => set({ [key]: e.target.checked ? fallback : undefined })}
        />
        {use}
      </label>
      <input
        type="color"
        data-testid={`frame-style-${key}`}
        aria-label={label}
        value={f[key] ?? fallback}
        disabled={!f[key]}
        onChange={(e) => set({ [key]: e.target.value.toUpperCase() })}
      />
    </div>
  );
  const sample = [1, 2, 3].map((n) => t('sheetSlicer.newStyle.sample', { n }));
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(
        kind === 'slicer' ? 'sheetSlicer.newStyle.slicerTitle' : 'sheetSlicer.newStyle.timelineTitle',
      )}
      testId="frame-style-dialog"
      footer={<Footer onClose={onClose} onOk={() => void ok()} testId="frame-style-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetSlicer.newStyle.name')}
          <input
            data-testid="frame-style-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <div className={styles.frameStyleBody}>
          <fieldset className={styles.frameConnections}>
            <legend>{t('sheetSlicer.newStyle.elements')}</legend>
            <div role="radiogroup" aria-label={t('sheetSlicer.newStyle.elements')}>
              {FRAME_STYLE_ELEMENTS.map((e) => (
                <label key={e} className={styles.check}>
                  <input
                    type="radio"
                    name="frame-style-element"
                    data-testid={`frame-style-element-${e}`}
                    checked={element === e}
                    onChange={() => setElement(e)}
                  />
                  {t(ELEMENT_LABEL[kind][e])}
                  {hasFormat(formats[e]) && (
                    <em className={styles.frameStyleMark}>
                      {' '}
                      ({t('sheetSlicer.newStyle.formatted')})
                    </em>
                  )}
                </label>
              ))}
            </div>
          </fieldset>
          <fieldset className={styles.frameConnections} data-testid="frame-style-format">
            <legend>{t('sheetSlicer.newStyle.format')}</legend>
            {colorRow('fill', t('sheetSlicer.newStyle.useFill'), t('sheetSlicer.newStyle.fill'), '#DDEBF7')}
            {colorRow('text', t('sheetSlicer.newStyle.useText'), t('sheetSlicer.newStyle.text'), '#1F4E79')}
            {colorRow(
              'border',
              t('sheetSlicer.newStyle.useBorder'),
              t('sheetSlicer.newStyle.border'),
              '#1F4E79',
            )}
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="frame-style-bold"
                checked={!!f.bold}
                onChange={(e) => set({ bold: e.target.checked })}
              />
              {t('sheetSlicer.newStyle.bold')}
            </label>
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="frame-style-italic"
                checked={!!f.italic}
                onChange={(e) => set({ italic: e.target.checked })}
              />
              {t('sheetSlicer.newStyle.italic')}
            </label>
            <button
              type="button"
              className={styles.btn}
              data-testid="frame-style-clear"
              disabled={!hasFormat(formats[element])}
              onClick={() => setFormats((all) => ({ ...all, [element]: {} }))}
            >
              {t('sheetSlicer.newStyle.clear')}
            </button>
          </fieldset>
        </div>
        <fieldset className={styles.frameConnections}>
          <legend>{t('sheetSlicer.newStyle.preview')}</legend>
          <div
            className={`${styles.slicer} ${styles.frameStylePreview}`}
            data-testid="frame-style-preview"
            style={frameVars(colors, { x: 0, y: 0, w: 180, h: 112 })}
          >
            <div className={styles.slicerHeader}>
              <span
                className={styles.frameCaption}
                style={{
                  fontWeight: formats.header?.bold ? 700 : undefined,
                  fontStyle: formats.header?.italic ? 'italic' : undefined,
                }}
              >
                {name}
              </span>
            </div>
            <div className={styles.slicerItems} style={{ gridTemplateColumns: '1fr' }}>
              {sample.map((label, i) => (
                <span
                  key={label}
                  className={i === 0 ? styles.slicerOn : styles.slicerOff}
                  data-testid={i === 0 ? 'frame-style-preview-on' : undefined}
                >
                  {label}
                </span>
              ))}
            </div>
          </div>
        </fieldset>
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
  /** Timelines: Timeline Tools › Show. */
  show?: TimelineShow;
}

/** The parts of a timeline Timeline Tools › Show draws. */
export interface TimelineShow {
  header: boolean;
  selectionLabel: boolean;
  timeLevel: boolean;
  scrollbar: boolean;
}

const SHOW_KEYS: (keyof TimelineShow)[] = ['header', 'selectionLabel', 'timeLevel', 'scrollbar'];

/** The Show options of a listed timeline (absent flags: shown). */
export function timelineShowOf(info: TimelineInfo): TimelineShow {
  return {
    header: info.showHeader !== false,
    selectionLabel: info.showSelectionLabel !== false,
    timeLevel: info.showTimeLevel !== false,
    scrollbar: info.showScrollbar !== false,
  };
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
  customStyles,
  show,
  colors,
  onCreateStyle,
  onApply,
  onClose,
}: {
  kind: 'slicer' | 'timeline';
  caption: string;
  columns?: number;
  style: string | null;
  connections: FrameConnection[];
  /** The workbook's own styles of this kind. */
  customStyles?: string[];
  /** Timelines: what is shown now. */
  show?: TimelineShow;
  /** The frame's colours (the New Style preview starts from them). */
  colors?: FrameColors;
  /** New style…: create a custom style (one undo step); rejects with the engine's message. */
  onCreateStyle?: (s: NewFrameStyle) => Promise<void>;
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
  const [shown, setShown] = useState<TimelineShow | undefined>(show);
  // Styles made with New style… while the dialog is open (the list from the engine follows).
  const [made, setMade] = useState<string[]>([]);
  const [newStyle, setNewStyle] = useState(false);
  const builtIn = kind === 'slicer' ? SLICER_STYLE_NAMES : TIMELINE_STYLE_NAMES;
  const customs = [...(customStyles ?? []), ...made.filter((m) => !customStyles?.includes(m))];
  const names = [...customs, ...builtIn];
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
        show: shown,
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
        <div className={styles.frameStyleRow}>
          <label>
            {t('sheetSlicer.styleLabel')}
            <select data-testid="frame-style" value={st} onChange={(e) => setSt(e.target.value)}>
              {!names.includes(st) && (
                <option value={st}>{st || t('sheetSlicer.style.none')}</option>
              )}
              {customs.length > 0 && (
                <optgroup label={t('sheetSlicer.style.custom')}>
                  {customs.map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </optgroup>
              )}
              {builtIn.map((n) => (
                <option key={n} value={n}>
                  {frameStyleLabel(n, t)}
                </option>
              ))}
            </select>
          </label>
          {onCreateStyle && (
            <button
              type="button"
              className={styles.btn}
              data-testid="frame-style-new"
              onClick={() => setNewStyle(true)}
            >
              {t('sheetSlicer.newStyle.button')}
            </button>
          )}
        </div>
        {newStyle && onCreateStyle && (
          <NewFrameStyleDialog
            kind={kind}
            taken={customs}
            base={
              colors ?? {
                selected: '#BDD7EE',
                selectedText: '#000000',
                header: '#FFFFFF',
                headerText: '#262626',
                border: '#BFBFBF',
              }
            }
            onCreate={async (s) => {
              await onCreateStyle(s);
              setMade((m) => [...m, s.name]);
              // The new style is offered as the choice (applied with OK, like any other).
              setSt(s.name);
            }}
            onClose={() => setNewStyle(false)}
          />
        )}
        {shown && (
          <fieldset className={styles.frameConnections} data-testid="timeline-show">
            <legend>{t('sheetSlicer.show.title')}</legend>
            {SHOW_KEYS.map((k) => (
              <label key={k} className={styles.check}>
                <input
                  type="checkbox"
                  data-testid={`timeline-show-${k}`}
                  checked={shown[k]}
                  onChange={(e) => setShown({ ...shown, [k]: e.target.checked })}
                />
                {t(`sheetSlicer.show.${k}`)}
              </label>
            ))}
          </fieldset>
        )}
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
