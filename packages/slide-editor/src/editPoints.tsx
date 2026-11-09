/**
 * Point editor (CORE-SHAPE-001): drawn over the slide while "Edit points" is on for the selected
 * freeform. Anchor points and Bézier control points are dragged with the pointer; clicking the
 * outline adds a point; the context menu (or the Delete key) removes a point, changes its type
 * (corner / smooth / symmetric) or makes a segment straight or curved. Geometry and coordinates
 * come from the engine (`freeform` in the edit result, slide points); every change is an
 * `editPoint` command (a drag is one undo step).
 */
import type {
  SlideCommand,
  SlideEditResult,
  SlideFreeformInfo,
  SlidePointEdit,
  SlidePointType,
} from '@blueoffice/platform';
import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';
import styles from './shapeFormat.module.css';

/** Anchor / control point size and outline hit width, CSS pixels. */
const POINT_PX = 8;
const HIT_PX = 10;
const DRAG_THRESHOLD_PX = 3;

export interface EditPointsLabels {
  overlay: string;
  point: (n: number) => string;
  handle: string;
  addHint: string;
  deletePoint: string;
  corner: string;
  smooth: string;
  symmetric: string;
  straight: string;
  curved: string;
}

export interface EditPointsOverlayProps {
  /** Slide size in points. */
  slide: { width: number; height: number };
  /** Displayed width in CSS pixels. */
  width: number;
  freeform: SlideFreeformInfo;
  run: (cmd: SlideCommand) => Promise<SlideEditResult | null>;
  /** Leave point editing (Escape, click outside the shape). */
  onExit: () => void;
  labels: EditPointsLabels;
}

/** A point of the overlay: anchor (`side` undefined) or control point. */
interface Target {
  path: number;
  point: number;
  side?: 0 | 1;
}

/** Index of the segment whose middle is nearest to a point (context menu on the outline). */
export function nearestSegment(
  freeform: SlideFreeformInfo,
  x: number,
  y: number,
): { path: number; segment: number; curved: boolean } | null {
  let best: { path: number; segment: number; curved: boolean; d: number } | null = null;
  freeform.paths.forEach((p, path) =>
    p.segments.forEach((s, segment) => {
      const d = Math.hypot(s.mid[0] - x, s.mid[1] - y);
      if (!best || d < best.d) best = { path, segment, curved: s.curved, d };
    }),
  );
  if (!best) return null;
  const { path, segment, curved } = best;
  return { path, segment, curved };
}

/** Nearest connection site within `tol` of a point (connector snapping indicator). */
export function nearestSite(
  sites: readonly [number, number][],
  x: number,
  y: number,
  tol: number,
): [number, number] | null {
  let best: [number, number] | null = null;
  let bd = tol;
  for (const s of sites) {
    const d = Math.hypot(s[0] - x, s[1] - y);
    if (d <= bd) {
      bd = d;
      best = s;
    }
  }
  return best;
}

/** The `editPoint` edit for dragging a target to (x, y). */
export function dragEdit(t: Target, x: number, y: number): SlidePointEdit {
  return t.side === undefined
    ? { kind: 'move', path: t.path, point: t.point, x, y }
    : { kind: 'handle', path: t.path, point: t.point, side: t.side, x, y };
}

interface Drag {
  target: Target;
  sx: number;
  sy: number;
  started: boolean;
}

interface Menu {
  /** Position in CSS pixels inside the overlay. */
  left: number;
  top: number;
  point?: Target;
  segment?: { path: number; segment: number; curved: boolean };
}

export function EditPointsOverlay({
  slide,
  width,
  freeform,
  run,
  onExit,
  labels,
}: EditPointsOverlayProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const layerRef = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  const [selected, setSelected] = useState<Target | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  // Live drags: one command in flight, the latest pending one wins.
  const live = useRef<{ busy: boolean; pending: SlideCommand | null }>({
    busy: false,
    pending: null,
  });
  const scale = slide.width > 0 ? width / slide.width : 1;
  const height = slide.height * scale;
  const ps = POINT_PX / scale;

  useEffect(() => {
    layerRef.current?.focus();
  }, []);

  const runLive = (cmd: SlideCommand) => {
    const l = live.current;
    if (l.busy) {
      l.pending = cmd;
      return;
    }
    l.busy = true;
    void run(cmd).finally(() => {
      l.busy = false;
      const next = l.pending;
      l.pending = null;
      if (next) runLive(next);
    });
  };

  const edit = (e: SlidePointEdit) => void run({ cmd: 'editPoint', edit: e });

  const toPt = (e: { clientX: number; clientY: number }) => {
    const r = svgRef.current?.getBoundingClientRect();
    if (!r || r.width === 0 || r.height === 0) return { x: 0, y: 0 };
    return {
      x: ((e.clientX - r.left) / r.width) * slide.width,
      y: ((e.clientY - r.top) / r.height) * slide.height,
    };
  };

  const targetOf = (el: Element): Target | null => {
    const a = el.getAttribute('data-point');
    const h = el.getAttribute('data-control');
    const v = a ?? h;
    if (v === null) return null;
    const [path, point, side] = v.split(':').map(Number);
    if (path === undefined || point === undefined) return null;
    return { path, point, side: h !== null ? (side === 1 ? 1 : 0) : undefined };
  };

  const onPointerDown = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (e.button !== 0) return;
    setMenu(null);
    e.preventDefault();
    const p = toPt(e);
    const el = e.target as Element;
    const target = targetOf(el);
    if (target) {
      svgRef.current?.setPointerCapture?.(e.pointerId);
      drag.current = { target, sx: p.x, sy: p.y, started: false };
      if (target.side === undefined) setSelected(target);
      return;
    }
    if (el.getAttribute('data-outline') !== null) {
      edit({ kind: 'add', x: p.x, y: p.y, tolerance: HIT_PX / scale });
      return;
    }
    onExit();
  };

  const onPointerMove = (e: ReactPointerEvent<SVGSVGElement>) => {
    const d = drag.current;
    if (!d) return;
    const p = toPt(e);
    if (!d.started && Math.hypot(p.x - d.sx, p.y - d.sy) * scale < DRAG_THRESHOLD_PX) return;
    // The first step of a drag starts a new undo step; the next ones join it.
    const merge = d.started;
    d.started = true;
    runLive({ cmd: 'editPoint', edit: dragEdit(d.target, p.x, p.y), merge });
  };

  const onPointerUp = (e: ReactPointerEvent<SVGSVGElement>) => {
    const d = drag.current;
    drag.current = null;
    svgRef.current?.releasePointerCapture?.(e.pointerId);
    if (!d?.started) return;
    const p = toPt(e);
    runLive({ cmd: 'editPoint', edit: dragEdit(d.target, p.x, p.y), merge: true });
  };

  const onContextMenu = (e: ReactMouseEvent<SVGSVGElement>) => {
    e.preventDefault();
    const r = layerRef.current?.getBoundingClientRect();
    const left = r ? e.clientX - r.left : 0;
    const top = r ? e.clientY - r.top : 0;
    const el = e.target as Element;
    const target = targetOf(el);
    if (target && target.side === undefined) {
      setSelected(target);
      setMenu({ left, top, point: target });
      return;
    }
    if (el.getAttribute('data-outline') !== null) {
      const p = toPt(e);
      const segment = nearestSegment(freeform, p.x, p.y);
      if (segment) setMenu({ left, top, segment });
    }
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      if (menu) setMenu(null);
      else onExit();
      return;
    }
    if ((e.key === 'Delete' || e.key === 'Backspace') && selected) {
      e.preventDefault();
      e.stopPropagation();
      edit({ kind: 'delete', path: selected.path, point: selected.point });
      setSelected(null);
    }
  };

  const setType = (t: Target, pointType: SlidePointType) => {
    setMenu(null);
    edit({ kind: 'pointType', path: t.path, point: t.point, pointType });
  };

  return (
    <div
      ref={layerRef}
      className={styles.layer}
      tabIndex={-1}
      data-testid="edit-points"
      onKeyDown={onKeyDown}
    >
      <svg
        ref={svgRef}
        className={styles.svg}
        role="application"
        aria-label={labels.overlay}
        width={width}
        height={height}
        viewBox={`0 0 ${slide.width} ${slide.height}`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => {
          drag.current = null;
        }}
        onContextMenu={onContextMenu}
      >
        {freeform.paths.map((p, pi) => (
          <g key={`o${pi}`}>
            <path className={styles.outline} d={p.d} vectorEffect="non-scaling-stroke" />
            <path
              className={styles.outlineHit}
              d={p.d}
              data-outline={pi}
              data-testid={`edit-points-outline-${pi}`}
              style={{ strokeWidth: HIT_PX / scale }}
            >
              <title>{labels.addHint}</title>
            </path>
          </g>
        ))}
        {freeform.paths.map((p, pi) =>
          p.points.map((pt, k) =>
            pt.handles.map((h, side) =>
              h ? (
                <g key={`h${pi}-${k}-${side}`}>
                  <line
                    className={styles.handleLine}
                    x1={pt.x}
                    y1={pt.y}
                    x2={h[0]}
                    y2={h[1]}
                    vectorEffect="non-scaling-stroke"
                  />
                  <circle
                    className={styles.handle}
                    cx={h[0]}
                    cy={h[1]}
                    r={ps / 2.5}
                    data-control={`${pi}:${k}:${side}`}
                    data-testid={`edit-control-${pi}-${k}-${side}`}
                    vectorEffect="non-scaling-stroke"
                  >
                    <title>{labels.handle}</title>
                  </circle>
                </g>
              ) : null,
            ),
          ),
        )}
        {freeform.paths.map((p, pi) =>
          p.points.map((pt, k) => {
            const on = selected?.path === pi && selected.point === k;
            return (
              <rect
                key={`a${pi}-${k}`}
                className={on ? styles.anchorSelected : styles.anchor}
                x={pt.x - ps / 2}
                y={pt.y - ps / 2}
                width={ps}
                height={ps}
                data-point={`${pi}:${k}`}
                data-kind={pt.kind}
                data-testid={`edit-point-${pi}-${k}`}
                vectorEffect="non-scaling-stroke"
              >
                <title>{labels.point(k + 1)}</title>
              </rect>
            );
          }),
        )}
      </svg>
      <span className={styles.hint}>{labels.addHint}</span>
      {menu && (
        <div
          className={styles.context}
          role="menu"
          data-testid="edit-points-menu"
          style={{ left: menu.left, top: menu.top }}
        >
          {menu.point && (
            <>
              <button
                type="button"
                role="menuitem"
                className={styles.entry}
                data-testid="edit-points-delete"
                onClick={() => {
                  const t = menu.point;
                  setMenu(null);
                  setSelected(null);
                  if (t) edit({ kind: 'delete', path: t.path, point: t.point });
                }}
              >
                {labels.deletePoint}
              </button>
              {(['corner', 'smooth', 'symmetric'] as const).map((k) => (
                <button
                  key={k}
                  type="button"
                  role="menuitemradio"
                  aria-checked={
                    freeform.paths[menu.point?.path ?? 0]?.points[menu.point?.point ?? 0]?.kind ===
                    k
                  }
                  className={styles.entry}
                  data-testid={`edit-points-${k}`}
                  onClick={() => menu.point && setType(menu.point, k)}
                >
                  {labels[k]}
                </button>
              ))}
            </>
          )}
          {menu.segment && (
            <>
              {([false, true] as const).map((curved) => (
                <button
                  key={String(curved)}
                  type="button"
                  role="menuitemradio"
                  aria-checked={menu.segment?.curved === curved}
                  className={styles.entry}
                  data-testid={`edit-points-${curved ? 'curved' : 'straight'}`}
                  onClick={() => {
                    const s = menu.segment;
                    setMenu(null);
                    if (s) edit({ kind: 'segment', path: s.path, segment: s.segment, curved });
                  }}
                >
                  {curved ? labels.curved : labels.straight}
                </button>
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}
