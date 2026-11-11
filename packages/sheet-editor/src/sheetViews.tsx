/**
 * BlueSheet workbook views (SHEET-VIEW-001): Normal, Page break preview (page regions
 * with "Page N" labels, manual breaks solid and automatic breaks dashed, breaks dragged
 * with the mouse), Page layout (the printed pages with their margins and header /
 * footer areas), Show formulas, Custom views (ECMA-376 Part 1, §18.3.1.25
 * `customSheetView`, §18.2.2 `customWorkbookView`) and temporary sheet views that stay
 * on this device. The engine stores the view type and show formulas in the sheet view
 * (§18.3.1.87) and paginates; this module only draws and forwards commands
 * (`@blueoffice/platform` `SheetViewCommand`).
 */
import { GlyphStore, PX_PER_PT, renderPage } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type {
  AppliedCustomView,
  BreakPreview,
  CustomViewInfo,
  PageLayoutPage,
  PrintPreviewInfo,
  ScreenRect,
  SheetCommand,
  SheetHost,
  SheetRange,
  SheetView,
  SheetViewMode,
  SheetViewState,
} from '@blueoffice/platform';
import { Dialog, type RibbonGroup, type RibbonItem } from '@blueoffice/ui';
import {
  BookmarkPlus,
  FileText,
  Glasses,
  Grid3x3,
  ListTree,
  SeparatorHorizontal,
  SquareFunction,
} from 'lucide-react';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type MutableRefObject,
} from 'react';
import styles from './SheetEditor.module.css';
import css from './sheetViews.module.css';

type T = ReturnType<typeof useTranslation>['t'];
type Item = (id: string, label: string, icon: ReactNode, size?: 'large' | 'small') => RibbonItem;
type Exec = <R>(cmd: SheetCommand) => Promise<R | null>;

/** Most page spans measured on screen per print area and axis (own limit). */
export const MAX_SPANS = 400;

// ------------------------------------------------------------------ state

/**
 * View type and show formulas of `sheet`, re-read after every change (`revision`). The
 * zoom kept in the file for the current view type is handed to `setZoom` when another
 * sheet or workbook is shown (`loadKey`).
 */
export function useSheetViews({
  host,
  sheet,
  revision,
  loadKey,
  zoom,
  setZoom,
  exec,
}: {
  host: SheetHost | undefined;
  sheet: number;
  revision: number;
  loadKey: number;
  zoom: number;
  setZoom: (z: number) => void;
  exec: Exec;
}) {
  const [state, setState] = useState<SheetViewState | null>(null);
  useEffect(() => {
    if (!host) return;
    let live = true;
    void host
      .exec<SheetViewState>({ cmd: 'sheetViewState', sheet })
      .then((s) => live && setState(s))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [host, sheet, revision]);
  useEffect(() => {
    if (!host || loadKey === 0) return;
    let live = true;
    void host
      .exec<SheetViewState>({ cmd: 'sheetViewState', sheet })
      .then((s) => live && setZoom(s.zoom / 100))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [host, sheet, loadKey, setZoom]);

  const mode: SheetViewMode = state?.view ?? 'normal';
  const setMode = useCallback(
    async (view: SheetViewMode) => {
      if (view === mode) return;
      // The zoom of the view being left is kept for it (§18.3.1.87 zoomScale*).
      await exec({ cmd: 'setSheetView', sheet, zoom: Math.round(zoom * 100) });
      const s = await exec<SheetViewState>({ cmd: 'setSheetView', sheet, view });
      if (s) {
        setState(s);
        setZoom(s.zoom / 100);
      }
    },
    [exec, sheet, zoom, mode, setZoom],
  );
  const toggleFormulas = useCallback(async () => {
    const s = await exec<SheetViewState>({
      cmd: 'setSheetView',
      sheet,
      showFormulas: !(state?.showFormulas ?? false),
    });
    if (s) setState(s);
  }, [exec, sheet, state]);
  /** Ctrl+8: show / hide the outline bar (`showOutlineSymbols`). */
  const toggleOutlineSymbols = useCallback(async () => {
    const s = await exec<SheetViewState>({
      cmd: 'setSheetView',
      sheet,
      outlineSymbols: !(state?.outlineSymbols ?? true),
    });
    if (s) setState(s);
  }, [exec, sheet, state]);
  return { state, mode, setMode, toggleFormulas, toggleOutlineSymbols };
}

/** A temporary view kept by the editor for this session (never written to the file). */
export interface TempView {
  name: string;
  sheet: number;
  zoom: number;
  scroll: { x: number; y: number };
  active: { row: number; col: number };
}

/** The editor's temporary sheet views. */
export function useTempViews() {
  const [views, setViews] = useState<TempView[]>([]);
  const add = useCallback(
    (v: TempView) => setViews((list) => [...list.filter((x) => x.name !== v.name), v].slice(-20)),
    [],
  );
  const remove = useCallback(
    (name: string) => setViews((list) => list.filter((x) => x.name !== name)),
    [],
  );
  return { views, add, remove };
}

// ------------------------------------------------------------------ ribbon

/** Groups added to the View tab. */
export function sheetViewTabGroups(t: T, item: Item): RibbonGroup[] {
  const s = 16;
  return [
    {
      id: 'workbookViews',
      label: t('sheetViews.groupViews'),
      items: [
        item('view.normal', t('sheetViews.normal'), <Grid3x3 size={18} />),
        item(
          'view.pageBreakPreview',
          t('sheetViews.pageBreakPreview'),
          <SeparatorHorizontal size={18} />,
        ),
        item('view.pageLayout', t('sheetViews.pageLayout'), <FileText size={18} />),
        item('view.customViews', t('sheetViews.customViews'), <BookmarkPlus size={s} />, 'small'),
      ],
    },
    {
      id: 'viewShow',
      label: t('sheetViews.groupShow'),
      items: [
        item(
          'view.showFormulas',
          t('sheetViews.showFormulas'),
          <SquareFunction size={s} />,
          'small',
        ),
        item('view.outlineSymbols', t('sheetOutline.symbols'), <ListTree size={s} />, 'small'),
      ],
    },
    {
      id: 'sheetView',
      label: t('sheetViews.groupTemp'),
      items: [item('view.tempViews', t('sheetViews.tempViews'), <Glasses size={s} />, 'small')],
    },
  ];
}

// ------------------------------------------------------------------ page break preview

/** A page break the user is dragging. */
interface BreakDrag {
  rows: boolean;
  area: number;
  /** First row / column of the next page at the break (absent: a new break). */
  from?: number;
  /** Pointer position in the overlay (px). */
  x: number;
  y: number;
}

/** Where a dragged break goes: the row / column boundary nearest the pointer. */
export function boundaryAt(
  rows: boolean,
  index: number | null,
  cell: ScreenRect | null,
  ptX: number,
  ptY: number,
): number {
  if (index === null) return 0;
  if (!cell) return index;
  const past = rows ? ptY > cell.y + cell.h / 2 : ptX > cell.x + cell.w / 2;
  return past ? index + 1 : index;
}

/** Page regions, page numbers and page breaks drawn over the grid. */
export function PageBreakOverlay({
  host,
  view,
  revision,
  scale,
  width,
  height,
  frozen,
  onMove,
  onAreas,
}: {
  host: SheetHost;
  view: SheetView;
  revision: number;
  /** CSS pixels per point. */
  scale: number;
  width: number;
  height: number;
  frozen: { rows: number; cols: number };
  onMove: (m: { rows: boolean; area: number; from?: number; to: number }) => void;
  /** A print area edge was dragged: the print areas as they are now. */
  onAreas?: (areas: SheetRange[]) => void;
}) {
  const { t } = useTranslation();
  const [pv, setPv] = useState<BreakPreview | null>(null);
  const [rects, setRects] = useState<{
    origin: { x: number; y: number };
    areas: { cols: (ScreenRect | null)[]; rows: (ScreenRect | null)[] }[];
  } | null>(null);
  const [drag, setDrag] = useState<BreakDrag | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const sheet = view.sheet;

  useEffect(() => {
    let live = true;
    void host
      .exec<BreakPreview>({ cmd: 'pageBreakPreview', sheet })
      .then((r) => live && setPv(r))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [host, sheet, revision]);

  useEffect(() => {
    if (!pv) return;
    let live = true;
    void (async () => {
      const a1 = await host.rangeRect(view, 0, 0, 0, 0);
      // The top-left corner of the cells (right of / below the headers), in points.
      const origin = {
        x: a1 ? a1.x + (frozen.cols > 0 ? 0 : view.scrollX) : 0,
        y: a1 ? a1.y + (frozen.rows > 0 ? 0 : view.scrollY) : 0,
      };
      const areas = await Promise.all(
        pv.areas.map(async (a) => ({
          cols: await Promise.all(
            a.cols.slice(0, MAX_SPANS).map(([c0, c1]) => host.rangeRect(view, a.r0, c0, a.r0, c1)),
          ),
          rows: await Promise.all(
            a.rows.slice(0, MAX_SPANS).map(([r0, r1]) => host.rangeRect(view, r0, a.c0, r1, a.c0)),
          ),
        })),
      );
      if (live) setRects({ origin, areas });
    })().catch(() => {});
    return () => {
      live = false;
    };
  }, [host, view, pv, frozen.rows, frozen.cols]);

  const startDrag =
    (rows: boolean, area: number, from: number, done?: (to: number) => void) =>
    (e: ReactPointerEvent<SVGLineElement>) => {
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();
      const box = svgRef.current?.getBoundingClientRect();
      if (!box) return;
      const at = (ev: { clientX: number; clientY: number }) => ({
        x: ev.clientX - box.left,
        y: ev.clientY - box.top,
      });
      setDrag({ rows, area, from, ...at(e) });
      const move = (ev: PointerEvent) => setDrag((d) => d && { ...d, ...at(ev) });
      const up = (ev: PointerEvent) => {
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
        setDrag(null);
        const p = at(ev);
        const ptX = p.x / scale;
        const ptY = p.y / scale;
        void (async () => {
          const hit = await host.hitTest(view, ptX, ptY);
          const index = rows ? ('row' in hit ? hit.row : null) : 'col' in hit ? hit.col : null;
          const cell =
            index === null
              ? null
              : await host.rangeRect(
                  view,
                  rows ? index : 0,
                  rows ? 0 : index,
                  rows ? index : 0,
                  rows ? 0 : index,
                );
          const to = boundaryAt(rows, index, cell, ptX, ptY);
          if (to === from) return;
          if (done) done(to);
          else onMove({ rows, area, from, to });
        })().catch(() => {});
      };
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
    };

  if (!pv || !rects) return null;
  const px = (r: ScreenRect | null) =>
    r && { x: r.x * scale, y: r.y * scale, w: r.w * scale, h: r.h * scale };
  const ox = rects.origin.x * scale;
  const oy = rects.origin.y * scale;
  let veil = `M${ox},${oy}H${width}V${height}H${ox}Z`;
  const shapes: ReactNode[] = [];
  pv.areas.forEach((a, ai) => {
    const ar = rects.areas[ai];
    if (!ar) return;
    const cols = ar.cols.map(px);
    const rows = ar.rows.map(px);
    const first = (v: typeof cols) => v.find((r) => r !== null) ?? null;
    const last = (v: typeof cols) => [...v].reverse().find((r) => r !== null) ?? null;
    const c0 = first(cols);
    const c1 = last(cols);
    const r0 = first(rows);
    const r1 = last(rows);
    if (!c0 || !c1 || !r0 || !r1) return;
    const box = { x: c0.x, y: r0.y, w: c1.x + c1.w - c0.x, h: r1.y + r1.h - r0.y };
    veil += `M${box.x},${box.y}h${box.w}v${box.h}h${-box.w}Z`;
    shapes.push(
      <rect
        key={`area-${ai}`}
        className={css.area}
        x={box.x}
        y={box.y}
        width={box.w}
        height={box.h}
      />,
    );
    for (const p of a.pages) {
      const c = cols[p.col];
      const r = rows[p.row];
      if (!c || !r) continue;
      if (c.x > width || r.y > height || c.x + c.w < 0 || r.y + r.h < 0) continue;
      const size = Math.max(14, Math.min(72, Math.min(c.w, r.h) / 5));
      shapes.push(
        <text
          key={`page-${ai}-${p.number}`}
          className={css.pageLabel}
          x={c.x + c.w / 2}
          y={r.y + r.h / 2}
          fontSize={size}
          data-testid={`page-label-${p.number}`}
        >
          {t('sheetViews.pageLabel', { n: p.number })}
        </text>,
      );
    }
    const line = (
      key: string,
      isRows: boolean,
      at: number,
      manual: boolean,
      x1: number,
      y1: number,
      x2: number,
      y2: number,
    ) => (
      <g key={key}>
        <line
          className={manual ? css.manual : css.auto}
          x1={x1}
          y1={y1}
          x2={x2}
          y2={y2}
          data-testid={`page-break-${isRows ? 'row' : 'col'}-${at}`}
          data-manual={manual ? 'true' : 'false'}
        />
        <line
          className={isRows ? css.handleRow : css.handleCol}
          x1={x1}
          y1={y1}
          x2={x2}
          y2={y2}
          data-testid={`page-break-handle-${isRows ? 'row' : 'col'}-${at}`}
          onPointerDown={startDrag(isRows, ai, at)}
        >
          <title>{t(manual ? 'sheetViews.breakManual' : 'sheetViews.breakAuto')}</title>
        </line>
      </g>
    );
    // The print area's edges can be dragged too (the print area follows).
    if (onAreas) {
      const edge = (side: 'top' | 'bottom' | 'left' | 'right') => {
        const isRows = side === 'top' || side === 'bottom';
        const from = { top: a.r0, bottom: a.r1 + 1, left: a.c0, right: a.c1 + 1 }[side];
        const done = (to: number) => {
          const next = { r0: a.r0, c0: a.c0, r1: a.r1, c1: a.c1 };
          if (side === 'top') next.r0 = to;
          if (side === 'bottom') next.r1 = to - 1;
          if (side === 'left') next.c0 = to;
          if (side === 'right') next.c1 = to - 1;
          if (next.r0 > next.r1 || next.c0 > next.c1) return;
          onAreas(
            pv.areas.map((x, j) => (j === ai ? next : { r0: x.r0, c0: x.c0, r1: x.r1, c1: x.c1 })),
          );
        };
        const [x1, y1, x2, y2] = {
          top: [box.x, box.y, box.x + box.w, box.y],
          bottom: [box.x, box.y + box.h, box.x + box.w, box.y + box.h],
          left: [box.x, box.y, box.x, box.y + box.h],
          right: [box.x + box.w, box.y, box.x + box.w, box.y + box.h],
        }[side];
        return (
          <line
            key={`edge-${ai}-${side}`}
            className={isRows ? css.handleRow : css.handleCol}
            x1={x1}
            y1={y1}
            x2={x2}
            y2={y2}
            data-testid={`print-area-edge-${ai}-${side}`}
            onPointerDown={startDrag(isRows, ai, from, done)}
          >
            <title>{t('sheetViews.areaEdge')}</title>
          </line>
        );
      };
      shapes.push(edge('top'), edge('bottom'), edge('left'), edge('right'));
    }
    for (const b of a.rowBreaks) {
      const i = a.rows.findIndex((s) => s[0] === b.at);
      const r = rows[i];
      if (!r) continue;
      shapes.push(line(`r-${ai}-${b.at}`, true, b.at, b.manual, box.x, r.y, box.x + box.w, r.y));
    }
    for (const b of a.colBreaks) {
      const i = a.cols.findIndex((s) => s[0] === b.at);
      const c = cols[i];
      if (!c) continue;
      shapes.push(line(`c-${ai}-${b.at}`, false, b.at, b.manual, c.x, box.y, c.x, box.y + box.h));
    }
  });
  return (
    <svg
      ref={svgRef}
      className={css.overlay}
      width={width}
      height={height}
      data-testid="page-break-preview"
      aria-label={t('sheetViews.breakPreview')}
    >
      <path className={css.veil} d={veil} fillRule="evenodd" />
      {shapes}
      {drag &&
        (drag.rows ? (
          <line className={css.guide} x1={ox} y1={drag.y} x2={width} y2={drag.y} />
        ) : (
          <line className={css.guide} x1={drag.x} y1={oy} x2={drag.x} y2={height} />
        ))}
    </svg>
  );
}

// ------------------------------------------------------------------ page layout view

/** One printed page, drawn when it scrolls into view. */
function LayoutPage({
  host,
  glyphs,
  sheet,
  index,
  page,
  scale,
  revision,
  onCell,
  onHeaderFooter,
  active,
  fail,
}: {
  host: SheetHost;
  glyphs: GlyphStore;
  sheet: number;
  index: number;
  page: PageLayoutPage;
  scale: number;
  revision: number;
  onCell: (row: number, col: number) => void;
  onHeaderFooter: () => void;
  /** The active cell, outlined when this page prints it. */
  active?: { row: number; col: number };
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [visible, setVisible] = useState(typeof IntersectionObserver === 'undefined');
  useEffect(() => {
    const el = wrap.current;
    if (!el || typeof IntersectionObserver === 'undefined') return;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) setVisible(true);
    });
    io.observe(el);
    return () => io.disconnect();
  }, []);
  useEffect(() => {
    if (!visible) return;
    let live = true;
    void (async () => {
      const r = await host.exec<PrintPreviewInfo>({ cmd: 'printPreview', sheet, page: index });
      const el = canvas.current;
      const ctx = el?.getContext('2d');
      if (!live || !r.page || !el || !ctx) return;
      await glyphs.ensure(r.page);
      if (!live) return;
      const dpr = window.devicePixelRatio || 1;
      el.width = Math.round(r.page.width * scale * dpr);
      el.height = Math.round(r.page.height * scale * dpr);
      renderPage(ctx, r.page, glyphs, scale * dpr);
      el.dataset.rendered = String(revision);
    })().catch(fail);
    return () => {
      live = false;
    };
  }, [host, glyphs, sheet, index, scale, revision, visible, fail]);
  // The active cell's box on this page.
  const [mark, setMark] = useState<ScreenRect | null>(null);
  const cells = page.cells;
  const on =
    !!active &&
    !!cells &&
    active.row >= cells.r0 &&
    active.row <= cells.r1 &&
    active.col >= cells.c0 &&
    active.col <= cells.c1;
  const ar = active?.row ?? -1;
  const ac = active?.col ?? -1;
  useEffect(() => {
    if (!on) {
      setMark(null);
      return;
    }
    let live = true;
    void host
      .exec<ScreenRect | null>({ cmd: 'pageCellRect', sheet, page: index, row: ar, col: ac })
      .then((r) => live && setMark(r))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [host, sheet, index, on, ar, ac, revision]);
  const [ml, mr, mt, mb, mh, mf] = page.margins;
  const w = page.width * scale;
  const h = page.height * scale;
  const click = (e: ReactMouseEvent<HTMLDivElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    const x = (e.clientX - box.left) / scale;
    const y = (e.clientY - box.top) / scale;
    void host
      .exec<{ row: number; col: number } | null>({ cmd: 'pageHit', sheet, page: index, x, y })
      .then((hit) => hit && onCell(hit.row, hit.col))
      .catch(fail);
  };
  return (
    <div
      ref={wrap}
      className={css.page}
      style={{ width: w, height: h }}
      data-testid={`page-layout-page-${index}`}
      onClick={click}
    >
      <canvas ref={canvas} style={{ width: w, height: h }} />
      {mark && (
        <div
          className={css.activeCell}
          data-testid={`page-layout-active-${index}`}
          style={{
            left: mark.x * scale,
            top: mark.y * scale,
            width: mark.w * scale,
            height: mark.h * scale,
          }}
        />
      )}
      <div
        className={css.margins}
        style={{ left: ml * scale, top: mt * scale, right: mr * scale, bottom: mb * scale }}
      />
      <button
        type="button"
        className={css.hfArea}
        style={{
          left: ml * scale,
          right: mr * scale,
          top: mh * scale,
          height: Math.max(8, (mt - mh) * scale),
        }}
        title={t('sheetViews.headerArea')}
        aria-label={t('sheetViews.headerArea')}
        data-testid={`page-layout-header-${index}`}
        onClick={(e) => {
          e.stopPropagation();
          onHeaderFooter();
        }}
      />
      <button
        type="button"
        className={css.hfArea}
        style={{
          left: ml * scale,
          right: mr * scale,
          bottom: mf * scale,
          height: Math.max(8, (mb - mf) * scale),
        }}
        title={t('sheetViews.footerArea')}
        aria-label={t('sheetViews.footerArea')}
        data-testid={`page-layout-footer-${index}`}
        onClick={(e) => {
          e.stopPropagation();
          onHeaderFooter();
        }}
      />
    </div>
  );
}

/** Page layout view: the sheet's printed pages one below the other. */
export function PageLayoutView({
  host,
  sheet,
  revision,
  zoom,
  onCell,
  onHeaderFooter,
  onKeyDown,
  regionRef,
  active,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  revision: number;
  zoom: number;
  onCell: (row: number, col: number) => void;
  onHeaderFooter: () => void;
  /** Keys while the pages have the focus (shortcuts, typing into the selected cell). */
  onKeyDown?: (e: ReactKeyboardEvent<HTMLDivElement>) => void;
  /** Receives the focusable pages element (the editor returns the focus to it). */
  regionRef?: MutableRefObject<HTMLDivElement | null>;
  /** The active cell (outlined on its page). */
  active?: { row: number; col: number };
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [pages, setPages] = useState<PageLayoutPage[]>([]);
  const glyphs = useMemo(() => new GlyphStore(host), [host]);
  useEffect(() => {
    let live = true;
    void host
      .exec<{ pages: PageLayoutPage[] }>({ cmd: 'pageLayout', sheet })
      .then((r) => live && setPages(r.pages))
      .catch(fail);
    return () => {
      live = false;
    };
  }, [host, sheet, revision, fail]);
  const scale = PX_PER_PT * zoom;
  return (
    <div
      className={css.layout}
      data-testid="page-layout-view"
      role="region"
      aria-label={t('sheetViews.layoutView')}
      tabIndex={0}
      ref={regionRef}
      onKeyDown={onKeyDown}
    >
      {pages.map((p, i) => (
        <LayoutPage
          key={i}
          host={host}
          glyphs={glyphs}
          sheet={sheet}
          index={i}
          page={p}
          scale={scale}
          revision={revision}
          onCell={onCell}
          onHeaderFooter={onHeaderFooter}
          active={active}
          fail={fail}
        />
      ))}
    </div>
  );
}

// ------------------------------------------------------------------ dialogs

/** What the editor shows on the active sheet (saved with a custom view). */
export interface CurrentView {
  zoom: number;
  active: [number, number];
  sqref: string;
  topLeft?: [number, number];
}

/** Custom views: save the current view under a name, show or delete one. */
export function CustomViewsDialog({
  host,
  exec,
  sheet,
  current,
  onApplied,
  onClose,
}: {
  host: SheetHost;
  exec: Exec;
  sheet: number;
  current: () => CurrentView;
  onApplied: (r: AppliedCustomView) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [list, setList] = useState<CustomViewInfo[]>([]);
  const [name, setName] = useState('');
  const [print, setPrint] = useState(true);
  const [hidden, setHidden] = useState(true);
  const [error, setError] = useState('');
  const reload = useCallback(async () => {
    setList(await host.exec<CustomViewInfo[]>({ cmd: 'customViews' }));
  }, [host]);
  useEffect(() => {
    let live = true;
    void host
      .exec<CustomViewInfo[]>({ cmd: 'customViews' })
      .then((l) => live && setList(l))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [host]);
  const add = async () => {
    const n = name.trim();
    if (!n) {
      setError(t('sheetViews.nameRequired'));
      return;
    }
    setError('');
    const c = current();
    const r = await exec<{ guid: string }>({
      cmd: 'addCustomView',
      name: n,
      sheet,
      includePrintSettings: print,
      includeHiddenRowCol: hidden,
      zoom: Math.round(c.zoom * 100),
      active: c.active,
      sqref: c.sqref,
      ...(c.topLeft ? { topLeft: c.topLeft } : {}),
    });
    if (r) {
      setName('');
      await reload();
    }
  };
  const show = async (guid: string) => {
    const r = await exec<AppliedCustomView>({ cmd: 'applyCustomView', guid });
    if (r) {
      onApplied(r);
      onClose();
    }
  };
  const remove = async (guid: string) => {
    if (await exec({ cmd: 'deleteCustomView', guid })) await reload();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetViews.customTitle')}
      testId="custom-views"
      width={460}
      footer={
        <button type="button" className={styles.btn} onClick={onClose}>
          {t('sheetViews.close')}
        </button>
      }
    >
      <div className={css.dialog}>
        <div className={css.listTitle}>{t('sheetViews.saved')}</div>
        {list.length === 0 ? (
          <p className={css.empty}>{t('sheetViews.none')}</p>
        ) : (
          <ul className={css.list}>
            {list.map((v, i) => (
              <li key={v.guid} data-testid={`custom-view-item-${i}`}>
                <span className={css.itemName}>{v.name}</span>
                <button
                  type="button"
                  className={styles.btn}
                  data-testid={`custom-view-show-${i}`}
                  onClick={() => void show(v.guid)}
                >
                  {t('sheetViews.show')}
                </button>
                <button
                  type="button"
                  className={styles.btn}
                  data-testid={`custom-view-delete-${i}`}
                  onClick={() => void remove(v.guid)}
                >
                  {t('sheetViews.delete')}
                </button>
              </li>
            ))}
          </ul>
        )}
        <label className={css.field}>
          {t('sheetViews.name')}
          <input
            data-testid="custom-view-name"
            value={name}
            maxLength={255}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void add();
            }}
          />
        </label>
        <label className={css.check}>
          <input
            type="checkbox"
            data-testid="custom-view-print"
            checked={print}
            onChange={(e) => setPrint(e.target.checked)}
          />
          {t('sheetViews.includePrint')}
        </label>
        <label className={css.check}>
          <input
            type="checkbox"
            data-testid="custom-view-hidden"
            checked={hidden}
            onChange={(e) => setHidden(e.target.checked)}
          />
          {t('sheetViews.includeHidden')}
        </label>
        {error && (
          <p className={css.error} role="alert">
            {error}
          </p>
        )}
        <button
          type="button"
          className={styles.btn}
          data-testid="custom-view-add"
          onClick={() => void add()}
        >
          {t('sheetViews.add')}
        </button>
        <p className={css.note}>{t('sheetViews.sessionNote')}</p>
      </div>
    </Dialog>
  );
}

/** Temporary sheet views kept on this device: keep the current one, show or delete one. */
export function TempViewsDialog({
  views,
  onKeep,
  onShow,
  onDelete,
  onClose,
}: {
  views: TempView[];
  onKeep: (name: string) => void;
  onShow: (v: TempView) => void;
  onDelete: (name: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const keep = () => {
    onKeep(name.trim() || t('sheetViews.tempDefault', { n: views.length + 1 }));
    setName('');
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetViews.tempTitle')}
      testId="temp-views"
      width={440}
      footer={
        <button type="button" className={styles.btn} onClick={onClose}>
          {t('sheetViews.close')}
        </button>
      }
    >
      <div className={css.dialog}>
        <p className={css.note}>{t('sheetViews.tempHint')}</p>
        {views.length === 0 ? (
          <p className={css.empty}>{t('sheetViews.none')}</p>
        ) : (
          <ul className={css.list}>
            {views.map((v, i) => (
              <li key={v.name} data-testid={`temp-view-item-${i}`}>
                <span className={css.itemName}>{v.name}</span>
                <button
                  type="button"
                  className={styles.btn}
                  data-testid={`temp-view-show-${i}`}
                  onClick={() => {
                    onShow(v);
                    onClose();
                  }}
                >
                  {t('sheetViews.show')}
                </button>
                <button
                  type="button"
                  className={styles.btn}
                  data-testid={`temp-view-delete-${i}`}
                  onClick={() => onDelete(v.name)}
                >
                  {t('sheetViews.delete')}
                </button>
              </li>
            ))}
          </ul>
        )}
        <label className={css.field}>
          {t('sheetViews.name')}
          <input
            data-testid="temp-view-name"
            value={name}
            maxLength={100}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <button type="button" className={styles.btn} data-testid="temp-view-keep" onClick={keep}>
          {t('sheetViews.tempSave')}
        </button>
      </div>
    </Dialog>
  );
}
