/**
 * Pointer layer of the Draw tab (CORE-INK-001), over the slide while a pen,
 * highlighter or eraser is active: records strokes (pen pressure from pointer
 * events), shows them live, previews the eraser and holds the ruler. Each
 * finished stroke or eraser drag is handed to the editor as one command.
 */
import type { SlideEraseMode, SlideInkObject, SlideInkStrokeIn } from '@blueoffice/platform';
import {
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';
import styles from './draw.module.css';
import {
  ERASER_RADIUS,
  RULER_LENGTH,
  RULER_SNAP,
  RULER_THICKNESS,
  StrokeRecorder,
  angleTo,
  brushOf,
  edgeDistance,
  onRuler as isOnRuler,
  snapAngle,
  touches,
  type DrawState,
  type Ruler,
} from './inkTools';

type Pt = [number, number];

export interface DrawLayerLabels {
  layer: string;
  ruler: string;
  rulerAngle: string;
}

export interface DrawLayerProps {
  /** Slide size in points. */
  slide: { width: number; height: number };
  state: DrawState;
  /** Ink of the slide (eraser preview). */
  ink: SlideInkObject[];
  onStroke: (s: SlideInkStrokeIn) => void;
  onErase: (points: Pt[], radius: number, mode: SlideEraseMode) => void;
  onRuler: (r: Ruler) => void;
  /** A finished lasso drag (slide points; CORE-INK-002). */
  onLasso?: (path: Pt[]) => void;
  /** Ink picked with the lasso (outlined). */
  picked?: SlideInkObject[];
  labels: DrawLayerLabels;
}

type Drag =
  | { kind: 'stroke'; rec: StrokeRecorder; id: number }
  | { kind: 'erase'; path: Pt[]; id: number }
  | { kind: 'lasso'; path: Pt[]; id: number }
  | { kind: 'moveRuler'; from: Pt; start: Ruler; id: number }
  | { kind: 'rotateRuler'; start: Ruler; a0: number; id: number };

/** Rotation knob position (right end of the ruler). */
function knob(r: Ruler): Pt {
  const a = (r.angle * Math.PI) / 180;
  const d = RULER_LENGTH / 2 - 14;
  return [r.x + Math.cos(a) * d, r.y + Math.sin(a) * d];
}

export function DrawLayer({
  slide,
  state,
  ink,
  onStroke,
  onErase,
  onRuler,
  onLasso,
  picked = [],
  labels,
}: DrawLayerProps) {
  const ref = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  /** What the drag shows (a copy of the drag state, for rendering). */
  const [view, setView] = useState<{
    stroke: Pt[] | null;
    erase: Pt[] | null;
    lasso: Pt[] | null;
  }>({
    stroke: null,
    erase: null,
    lasso: null,
  });
  const [cursor, setCursor] = useState<Pt | null>(null);
  const redraw = () => {
    const d = drag.current;
    setView({
      stroke: d?.kind === 'stroke' ? d.rec.points.map((p) => [p[0], p[1]] as Pt) : null,
      erase: d?.kind === 'erase' ? [...d.path] : null,
      lasso: d?.kind === 'lasso' ? [...d.path] : null,
    });
  };
  const erasing = state.tool === 'eraserStroke' || state.tool === 'eraserPoint';
  const ruler = state.ruler;

  const toSlide = (e: { clientX: number; clientY: number }): Pt => {
    const r = ref.current?.getBoundingClientRect();
    if (!r || r.width <= 0 || r.height <= 0) return [0, 0];
    return [
      ((e.clientX - r.left) * slide.width) / r.width,
      ((e.clientY - r.top) * slide.height) / r.height,
    ];
  };
  // Pen input carries pressure; a mouse reports a constant 0.5 while pressed.
  const pressureOf = (e: ReactPointerEvent) => (e.pointerType === 'pen' ? e.pressure : null);

  const down = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || drag.current) return;
    const p = toSlide(e);
    const id = e.pointerId;
    ref.current?.setPointerCapture?.(id);
    e.preventDefault();
    if (ruler) {
      const k = knob(ruler);
      if (Math.hypot(p[0] - k[0], p[1] - k[1]) <= 12) {
        drag.current = { kind: 'rotateRuler', start: ruler, a0: angleTo(ruler, p), id };
        return;
      }
      // The body away from the drawing edge moves the ruler.
      if (isOnRuler(ruler, p) && edgeDistance(ruler, p) < -RULER_SNAP) {
        drag.current = { kind: 'moveRuler', from: p, start: ruler, id };
        return;
      }
    }
    if (erasing) {
      drag.current = { kind: 'erase', path: [p], id };
    } else if (state.tool === 'lasso') {
      drag.current = { kind: 'lasso', path: [p], id };
    } else {
      drag.current = {
        kind: 'stroke',
        rec: new StrokeRecorder(p, e.timeStamp, pressureOf(e), ruler),
        id,
      };
    }
    redraw();
  };

  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    const p = toSlide(e);
    if (erasing) setCursor(p);
    const d = drag.current;
    if (!d || d.id !== e.pointerId) return;
    switch (d.kind) {
      case 'stroke': {
        // Coalesced events give the full pen resolution where supported.
        const native = e.nativeEvent as PointerEvent;
        const list =
          typeof native.getCoalescedEvents === 'function' ? native.getCoalescedEvents() : [];
        const samples = list.length > 0 ? list : [native];
        let any = false;
        for (const s of samples) {
          const pressure = e.pointerType === 'pen' ? s.pressure : null;
          if (d.rec.extend(toSlide(s), s.timeStamp, pressure)) any = true;
        }
        if (any) redraw();
        break;
      }
      case 'erase':
      case 'lasso':
        d.path.push(p);
        redraw();
        break;
      case 'moveRuler':
        onRuler({ ...d.start, x: d.start.x + p[0] - d.from[0], y: d.start.y + p[1] - d.from[1] });
        break;
      case 'rotateRuler':
        onRuler({ ...d.start, angle: snapAngle(d.start.angle + angleTo(d.start, p) - d.a0) });
        break;
    }
  };

  const up = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d || d.id !== e.pointerId) return;
    drag.current = null;
    if (d.kind === 'stroke') onStroke(d.rec.toStroke(brushOf(state)));
    if (d.kind === 'erase') {
      onErase(d.path, ERASER_RADIUS, state.tool === 'eraserPoint' ? 'point' : 'stroke');
    }
    if (d.kind === 'lasso') onLasso?.(d.path);
    redraw();
  };

  const cancel = () => {
    drag.current = null;
    redraw();
  };

  const rulerKey = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (!ruler) return;
    const step = e.shiftKey ? 15 : 1;
    if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') {
      onRuler({ ...ruler, angle: snapAngle(ruler.angle - step) });
      e.preventDefault();
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowUp') {
      onRuler({ ...ruler, angle: snapAngle(ruler.angle + step) });
      e.preventDefault();
    }
  };

  const brush = brushOf(state);
  const live = view.stroke;
  const erasePath = view.erase;
  const doomed =
    erasePath && state.tool === 'eraserStroke'
      ? ink.flatMap((o) =>
          o.strokes.filter((s) => erasePath.some((p) => touches(s, p, ERASER_RADIUS))),
        )
      : [];

  return (
    <div
      ref={ref}
      className={`${styles.layer} ${erasing ? styles.layerEraser : ''}`}
      role="application"
      aria-label={labels.layer}
      data-testid="draw-layer"
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={cancel}
      onPointerLeave={() => setCursor(null)}
    >
      <svg
        className={styles.svg}
        viewBox={`0 0 ${slide.width} ${slide.height}`}
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        {ruler && (
          <g transform={`translate(${ruler.x} ${ruler.y}) rotate(${ruler.angle})`}>
            <rect
              className={styles.ruler}
              x={-RULER_LENGTH / 2}
              y={-RULER_THICKNESS / 2}
              width={RULER_LENGTH}
              height={RULER_THICKNESS}
              rx={3}
            />
            {/* Ticks every 0.5 cm along the drawing edge (1 cm = 72 / 2.54 pt). */}
            {Array.from({ length: Math.floor(RULER_LENGTH / (36 / 2.54)) + 1 }, (_, i) => {
              const x = -RULER_LENGTH / 2 + (i * 36) / 2.54;
              const long = i % 2 === 0;
              return (
                <line
                  key={i}
                  className={styles.rulerTick}
                  x1={x}
                  x2={x}
                  y1={-RULER_THICKNESS / 2}
                  y2={-RULER_THICKNESS / 2 + (long ? 10 : 5)}
                />
              );
            })}
            <text className={styles.rulerLabel} x={0} y={6}>
              {`${Math.round(((ruler.angle % 360) + 360) % 360)}°`}
            </text>
            <circle className={styles.knob} cx={RULER_LENGTH / 2 - 14} cy={0} r={7} />
          </g>
        )}
        {live && live.length > 0 && (
          <polyline
            data-testid="draw-live"
            points={live.map((p) => `${p[0]},${p[1]}`).join(' ')}
            fill="none"
            stroke={`#${brush.color}`}
            strokeOpacity={brush.highlighter ? 0.5 : 1}
            strokeWidth={brush.width}
            strokeLinecap={brush.highlighter ? 'square' : 'round'}
            strokeLinejoin="round"
          />
        )}
        {doomed.map((s, i) => (
          <polyline
            key={i}
            className={styles.doomed}
            points={s.points.map((p) => `${p[0]},${p[1]}`).join(' ')}
          />
        ))}
        {view.lasso && view.lasso.length > 1 && (
          <polygon
            className={styles.lasso}
            data-testid="draw-lasso"
            points={view.lasso.map((p) => `${p[0]},${p[1]}`).join(' ')}
          />
        )}
        {picked.map((o) => (
          <rect
            key={o.id}
            className={styles.picked}
            data-testid="draw-picked"
            x={o.rect[0]}
            y={o.rect[1]}
            width={o.rect[2]}
            height={o.rect[3]}
          />
        ))}
        {erasing && cursor && (
          <circle className={styles.eraser} cx={cursor[0]} cy={cursor[1]} r={ERASER_RADIUS} />
        )}
      </svg>
      {ruler && (
        <div
          className={styles.rulerFocus}
          tabIndex={0}
          role="slider"
          aria-label={labels.ruler}
          aria-valuetext={`${labels.rulerAngle} ${Math.round(ruler.angle)}°`}
          aria-valuenow={Math.round(ruler.angle)}
          aria-valuemin={-180}
          aria-valuemax={180}
          onKeyDown={rulerKey}
        />
      )}
    </div>
  );
}
