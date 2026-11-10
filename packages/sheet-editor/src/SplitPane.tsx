/**
 * The second pane of a split worksheet (CORE-UI-007): the same sheet rendered by the engine
 * for its own scroll position and zoom, with the selection drawn on it. Clicking a cell
 * selects it; typing and commands go to the editor as in the main pane.
 */
import { GlyphStore, ImageStore, PX_PER_PT, renderPage } from '@blueoffice/canvas-host';
import { dragSplit, type SheetHost, type SheetRange, type SheetView } from '@blueoffice/platform';
import { useEffect, useMemo, useRef, useState } from 'react';
import styles from './SheetEditor.module.css';
import type { SheetSplit } from './sheetSplit';

type Rect = { x: number; y: number; w: number; h: number };

export interface SplitPaneProps {
  host: SheetHost;
  glyphs: GlyphStore;
  images?: ImageStore | null;
  sheet: number;
  split: SheetSplit;
  /** Size of the sheet content in points (scroll range). */
  content: { w: number; h: number };
  /** Bumped after every change of the workbook. */
  revision: number;
  selection: { range: SheetRange; active: { row: number; col: number } };
  labels: { pane: string; bar: string };
  onChange: (change: Partial<Pick<SheetSplit, 'ratio' | 'scroll' | 'zoom'>>) => void;
  /** The bar was dropped at an edge or double-clicked. */
  onRemove: () => void;
  onSelect: (cell: { row: number; col: number }, extend: boolean) => void;
  fail: (e: unknown) => void;
}

export function SplitPane(p: SplitPaneProps) {
  const { host, glyphs, images, sheet, split, revision, selection } = p;
  const scroller = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const seq = useRef(0);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [rects, setRects] = useState<{ range: Rect | null; active: Rect | null }>({
    range: null,
    active: null,
  });
  const scale = PX_PER_PT * split.zoom;
  const horizontal = split.orientation === 'horizontal';
  const cb = useRef(p);
  useEffect(() => {
    cb.current = p;
  });

  const view: SheetView = useMemo(
    () => ({
      sheet,
      scrollX: split.scroll.x,
      scrollY: split.scroll.y,
      width: size.w / scale,
      height: size.h / scale,
    }),
    [sheet, split.scroll.x, split.scroll.y, size, scale],
  );

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    // The pane opens at the cell the split was made at.
    el.scrollTo(cb.current.split.scroll.x * scale, cb.current.split.scroll.y * scale);
    return () => ro.disconnect();
    // Only on mount: later scrolling is the user's.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (size.w <= 0 || size.h <= 0) return;
    const n = ++seq.current;
    const frame = requestAnimationFrame(() => {
      void (async () => {
        const page = await host.render(view);
        await Promise.all([glyphs.ensure(page), images?.ensure(page)]);
        const c = canvas.current;
        const ctx = c?.getContext('2d');
        if (n !== seq.current || !c || !ctx) return;
        const dpr = window.devicePixelRatio || 1;
        c.width = Math.round(size.w * dpr);
        c.height = Math.round(size.h * dpr);
        c.style.width = `${size.w}px`;
        c.style.height = `${size.h}px`;
        renderPage(ctx, page, glyphs, scale * dpr, images ?? undefined);
        c.dataset.rendered = String(n);
      })().catch((e: unknown) => cb.current.fail(e));
    });
    return () => cancelAnimationFrame(frame);
  }, [host, glyphs, images, view, size, scale, revision]);

  // The selection, as far as this pane shows it.
  const { range, active } = selection;
  useEffect(() => {
    let live = true;
    const px = (r: Rect | null) =>
      r && { x: r.x * scale, y: r.y * scale, w: r.w * scale, h: r.h * scale };
    void Promise.all([
      host.rangeRect(view, range.r0, range.c0, range.r1, range.c1),
      host.rangeRect(view, active.row, active.col, active.row, active.col),
    ])
      .then(([r, a]) => live && setRects({ range: px(r), active: px(a) }))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [host, view, scale, revision, range.r0, range.c0, range.r1, range.c1, active.row, active.col]);

  const total = () => {
    const box = scroller.current?.parentElement?.parentElement?.getBoundingClientRect();
    return box ?? null;
  };

  return (
    <>
      <div
        role="separator"
        aria-orientation={horizontal ? 'horizontal' : 'vertical'}
        aria-label={p.labels.bar}
        aria-valuenow={Math.round(split.ratio * 100)}
        tabIndex={0}
        data-testid="sheet-split-bar"
        className={styles.splitBar}
        data-orientation={split.orientation}
        onDoubleClick={p.onRemove}
        onKeyDown={(e) => {
          const back = horizontal ? 'ArrowUp' : 'ArrowLeft';
          const forth = horizontal ? 'ArrowDown' : 'ArrowRight';
          const step = e.key === back ? -0.05 : e.key === forth ? 0.05 : 0;
          if (!step) return;
          e.preventDefault();
          e.stopPropagation();
          p.onChange({ ratio: split.ratio + step });
        }}
        onPointerDown={(e) => {
          e.preventDefault();
          e.currentTarget.setPointerCapture(e.pointerId);
        }}
        onPointerMove={(e) => {
          if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
          const box = total();
          if (!box) return;
          const ratio = horizontal
            ? dragSplit(box.height, e.clientY - box.top)
            : dragSplit(box.width, e.clientX - box.left);
          if (ratio === null) p.onRemove();
          else p.onChange({ ratio });
        }}
      />
      <div
        className={styles.splitPane}
        style={{ flex: `${1 - split.ratio} 1 0` }}
        role="region"
        aria-label={p.labels.pane}
      >
        <div
          ref={scroller}
          className={styles.scroller}
          data-testid="sheet-split-scroller"
          onScroll={(e) => {
            const el = e.currentTarget;
            p.onChange({ scroll: { x: el.scrollLeft / scale, y: el.scrollTop / scale } });
          }}
          onWheel={(e) => {
            if (!e.ctrlKey) return;
            const z = Math.min(4, Math.max(0.25, split.zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1)));
            p.onChange({ zoom: Math.round(z * 100) / 100 });
          }}
        >
          <div className={styles.sticky}>
            <canvas
              ref={canvas}
              data-testid="sheet-split-canvas"
              aria-hidden="true"
              onMouseDown={(e) => {
                if (e.button !== 0) return;
                const box = e.currentTarget.getBoundingClientRect();
                const x = (e.clientX - box.left) / scale;
                const y = (e.clientY - box.top) / scale;
                const extend = e.shiftKey;
                void host
                  .hitTest(view, x, y)
                  .then((hit) => {
                    if (hit.kind === 'cell' || hit.kind === 'filterButton')
                      p.onSelect({ row: hit.row, col: hit.col }, extend);
                  })
                  .catch(() => {});
                // The keyboard stays with the editor's input.
                e.preventDefault();
              }}
            />
            {rects.range && (
              <div
                className={styles.selection}
                data-testid="sheet-split-selection"
                style={{
                  left: rects.range.x,
                  top: rects.range.y,
                  width: rects.range.w,
                  height: rects.range.h,
                }}
              />
            )}
            {rects.active && (
              <div
                className={styles.splitActive}
                style={{
                  left: rects.active.x,
                  top: rects.active.y,
                  width: rects.active.w,
                  height: rects.active.h,
                }}
              />
            )}
          </div>
          <div style={{ width: p.content.w * scale, height: p.content.h * scale }} />
        </div>
      </div>
    </>
  );
}
