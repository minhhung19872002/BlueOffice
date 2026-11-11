/**
 * The outline bar beside the BlueSheet grid (SHEET-DATA-002), as a spreadsheet shows it
 * once rows or columns are grouped: a band left of the row headers and one above the
 * column headers. Level buttons 1…n+1 sit where the bands meet the headers (button k
 * shows levels below k, the last one everything); each group has a bracket over its
 * detail and a −/+ button at its summary row / column that hides / shows the detail.
 * Positions come from the drawn grid geometry, so the bar scrolls and zooms with it.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { SheetOutlineGroup, SheetOutlineInfo } from '@blueoffice/platform';
import { colName } from './grid';
import type { GridGeometry } from './gridView';
import css from './outlineBar.module.css';

/** Width (px) of one level lane of the bar — the 24 px spacing of its 16 px buttons. */
export const OUTLINE_LANE = 24;
const BTN = 16;

/** Size (px) of the row band (left) and column band (top); 0 without an outline. */
export function outlineBarSize(o: SheetOutlineInfo | undefined | null): {
  left: number;
  top: number;
} {
  if (!o) return { left: 0, top: 0 };
  return {
    left: o.rowLevels > 0 ? (o.rowLevels + 1) * OUTLINE_LANE : 0,
    top: o.colLevels > 0 ? (o.colLevels + 1) * OUTLINE_LANE : 0,
  };
}

interface Span {
  start: number;
  end: number;
}

/** Screen spans (px, from the cells' origin) of the visible rows or columns. */
export function visibleSpans(g: GridGeometry, rows: boolean, scale: number): Map<number, Span> {
  const out = new Map<number, Span>();
  for (const pane of rows ? g.rows : g.cols) {
    for (const s of pane.segs) out.set(s.index, { start: s.start * scale, end: s.end * scale });
  }
  return out;
}

/** What to draw for one group: its bracket (when open) and its button, in px. */
export interface GroupMark {
  group: SheetOutlineGroup;
  /** Bracket span along the axis (only when the detail is shown and visible). */
  line: Span | null;
  /** Centre of the +/− button along the axis (null when the summary is off screen). */
  button: number | null;
}

/** The marks of the groups with something on screen. */
export function groupMarks(groups: SheetOutlineGroup[], spans: Map<number, Span>): GroupMark[] {
  const out: GroupMark[] = [];
  const keys = [...spans.keys()].sort((a, b) => a - b);
  for (const group of groups) {
    let line: Span | null = null;
    if (!group.collapsed) {
      for (const k of keys) {
        if (k < group.first) continue;
        if (k > group.last) break;
        const s = spans.get(k);
        if (!s) continue;
        line = line ? { start: line.start, end: s.end } : { ...s };
      }
    }
    const s = group.summary === null ? undefined : spans.get(group.summary);
    const button = s ? (s.start + s.end) / 2 : null;
    if (line || button !== null) out.push({ group, line, button });
  }
  return out;
}

export function OutlineBar({
  outline,
  geometry,
  scale,
  width,
  height,
  onLevel,
  onToggle,
}: {
  outline: SheetOutlineInfo;
  geometry: GridGeometry | null;
  /** CSS px per point of the drawn grid. */
  scale: number;
  /** Size (px) of the grid viewport next to the bar. */
  width: number;
  height: number;
  onLevel: (rows: boolean, level: number) => void;
  onToggle: (rows: boolean, group: SheetOutlineGroup) => void;
}) {
  const { t } = useTranslation();
  const { left, top } = outlineBarSize(outline);
  if (left === 0 && top === 0) return null;
  const headerW = (geometry?.headerW ?? 0) * scale;
  const headerH = (geometry?.headerH ?? 0) * scale;
  const lane = (level: number) => (level - 1) * OUTLINE_LANE + OUTLINE_LANE / 2;

  const band = (rows: boolean) => {
    const levels = rows ? outline.rowLevels : outline.colLevels;
    if (levels === 0) return null;
    const marks = geometry
      ? groupMarks(rows ? outline.rows : outline.cols, visibleSpans(geometry, rows, scale))
      : [];
    const name = (k: 'level' | 'show' | 'hide', n: number | string) =>
      t(`sheetOutline.${rows ? 'row' : 'col'}.${k}`, { n });
    const label = (i: number) => (rows ? i + 1 : colName(i));
    // Along = position on the grid axis (the drawn grid's own coordinates, headers
    // included); across = the level lane.
    const at = (along: number, across: number, w: number, h: number) =>
      rows
        ? { left: across, top: along, width: w, height: h }
        : { left: along, top: across, width: w, height: h };
    return (
      <div
        className={`${css.bar} ${rows ? css.left : css.top}`}
        data-testid={rows ? 'outline-bar-rows' : 'outline-bar-cols'}
        role="toolbar"
        aria-label={t(rows ? 'sheetOutline.row.bar' : 'sheetOutline.col.bar')}
        aria-orientation={rows ? 'vertical' : 'horizontal'}
        style={rows ? { left: 0, top, width: left, height } : { left, top: 0, width, height: top }}
      >
        {Array.from({ length: levels + 1 }, (_, i) => i + 1).map((n) => (
          <button
            key={`l${n}`}
            type="button"
            className={`${css.button} ${css.level}`}
            data-testid={`outline-${rows ? 'row' : 'col'}-level-${n}`}
            aria-label={name('level', n)}
            title={name('level', n)}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onLevel(rows, n)}
            style={
              rows
                ? { left: lane(n) - BTN / 2, top: Math.max(0, (headerH - BTN) / 2) }
                : { left: Math.max(0, (headerW - BTN) / 2), top: lane(n) - BTN / 2 }
            }
          >
            {n}
          </button>
        ))}
        {marks.map(({ group, line, button }) => {
          const key = `${group.level}:${group.first}`;
          const c = lane(group.level);
          const after = group.summary !== null && group.summary > group.last;
          return (
            <span key={key}>
              {line && (
                <span
                  className={css.line}
                  style={
                    rows
                      ? {
                          left: c,
                          top: line.start + 2,
                          width: 4,
                          height: Math.max(0, line.end - line.start - 2),
                          borderLeftWidth: 1,
                          // A tick at the end away from the summary.
                          borderTopWidth: after ? 1 : 0,
                          borderBottomWidth: after ? 0 : 1,
                        }
                      : {
                          left: line.start + 2,
                          top: c,
                          width: Math.max(0, line.end - line.start - 2),
                          height: 4,
                          borderTopWidth: 1,
                          borderLeftWidth: after ? 1 : 0,
                          borderRightWidth: after ? 0 : 1,
                        }
                  }
                />
              )}
              {button !== null && (
                <button
                  type="button"
                  className={css.button}
                  data-testid={`outline-${rows ? 'row' : 'col'}-toggle-${group.summary}`}
                  aria-expanded={!group.collapsed}
                  aria-label={name(group.collapsed ? 'show' : 'hide', label(group.summary ?? 0))}
                  title={name(group.collapsed ? 'show' : 'hide', label(group.summary ?? 0))}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => onToggle(rows, group)}
                  style={at(button - BTN / 2, c - BTN / 2, BTN, BTN)}
                >
                  {group.collapsed ? '+' : '−'}
                </button>
              )}
            </span>
          );
        })}
      </div>
    );
  };

  return (
    <>
      {left > 0 && top > 0 && (
        <div
          className={`${css.bar} ${css.corner}`}
          style={{ left: 0, top: 0, width: left, height: top }}
        />
      )}
      {band(true)}
      {band(false)}
    </>
  );
}
