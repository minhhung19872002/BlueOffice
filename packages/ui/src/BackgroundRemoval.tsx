import { Eraser, Plus, Undo2 } from 'lucide-react';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from 'react';
import { Dialog } from './Dialog';
import styles from './BackgroundRemoval.module.css';

/**
 * "Remove background" mode of the picture tools (CORE-PIC-002), shared by BlueDoc, BlueSheet
 * and BlueSlide: the picture with the proposed removal tinted (drawn by the engine), tools to
 * mark areas to keep or to remove (pointer strokes, or the keyboard: arrows move the cursor,
 * Space / Enter marks), then "Discard changes" or "Keep changes". The segmentation runs in the
 * engine (graph cut); a host [`SegmentationProvider`] may supply the starting estimate.
 * Mirrors `bo_engine::picture_bg`.
 */

/** A stroke: points as fractions (0..1) of the picture. */
export interface BackgroundMarkValue {
  keep: boolean;
  points: [number, number][];
}

/** Starting estimate of a segmentation provider: one byte per pixel (≥ 128 = keep), base64. */
export interface BackgroundPriorValue {
  width: number;
  height: number;
  data: string;
}

/** Settings of a background removal (a type alias: it is intersected into commands). */
export type RemovalSpecValue = {
  /** Rectangle holding the subject (left, top, right, bottom fractions). */
  rect?: [number, number, number, number];
  marks?: BackgroundMarkValue[];
  prior?: BackgroundPriorValue;
  /** Previews also return the working picture (for a segmentation provider). */
  source?: boolean;
};

/** A preview from the engine. */
export interface BackgroundPreviewValue {
  /** PNG base64 of the (reduced) picture with the removal tinted. */
  image: string;
  width: number;
  height: number;
  /** Share kept, 1/1000 %. */
  kept: number;
  ms: number;
  /** The working picture itself, PNG base64 (asked for with `source`). */
  source?: string;
}

/**
 * Supplies the starting estimate of the segmentation (e.g. a licensed salient-object model run
 * locally by the host, CORE-PIC-002). Without `prior` — or when it resolves to `null` — the
 * engine starts from the rectangle.
 */
export interface SegmentationProvider {
  id: string;
  /** Estimate for the working picture (`image`: PNG base64, `width` × `height` pixels). */
  prior?: (picture: {
    image: string;
    width: number;
    height: number;
  }) => Promise<BackgroundPriorValue | null>;
}

/** The provider BlueOffice ships: the engine's graph cut, started from the rectangle. */
export const graphCutProvider: SegmentationProvider = { id: 'graph-cut' };

export interface BackgroundRemovalLabels {
  removeBackground: string;
  markKeep: string;
  markRemove: string;
  undoMark: string;
  discard: string;
  keepChanges: string;
  computing: string;
  kept: (percent: string) => string;
  canvas: string;
}

/** Keyboard cursor step (fraction of the picture); Shift multiplies it by 5. */
const STEP = 0.02;
/** Radius of a stroke as a fraction of the longer side (`bo_segment::MARK_RADIUS`). */
export const MARK_RADIUS = 1 / 80;

export function BackgroundRemovalMode({
  open,
  labels,
  initial,
  provider = graphCutProvider,
  preview,
  onKeep,
  onDiscard,
}: {
  open: boolean;
  labels: BackgroundRemovalLabels;
  /** Settings of an earlier removal (resumed). */
  initial?: RemovalSpecValue | null;
  provider?: SegmentationProvider;
  preview: (spec: RemovalSpecValue) => Promise<BackgroundPreviewValue | null>;
  onKeep: (spec: RemovalSpecValue) => void;
  onDiscard: () => void;
}) {
  const [tool, setTool] = useState<'keep' | 'remove'>('keep');
  const [marks, setMarks] = useState<BackgroundMarkValue[]>(initial?.marks ?? []);
  const [drawing, setDrawing] = useState<BackgroundMarkValue | null>(null);
  // The last preview and the settings it answers (busy while they differ from the current ones).
  const [answered, setAnswered] = useState<string | null>(null);
  const [shownPreview, setShownPreview] = useState<BackgroundPreviewValue | null>(null);
  const [cursor, setCursor] = useState<[number, number]>([0.5, 0.5]);
  // `undefined` while the provider works out its estimate.
  const [prior, setPrior] = useState<BackgroundPriorValue | null | undefined>(() =>
    provider.prior ? undefined : null,
  );
  const seq = useRef(0);
  const area = useRef<HTMLDivElement>(null);
  const rect = initial?.rect;

  // The provider's estimate, once per opening (from the working picture the engine returns).
  useEffect(() => {
    let live = true;
    const ask = provider.prior;
    if (!ask) return;
    void preview({ ...(rect ? { rect } : {}), marks: [], source: true })
      .then((p) => (p?.source ? ask({ image: p.source, width: p.width, height: p.height }) : null))
      .then((p) => live && setPrior(p))
      .catch(() => live && setPrior(null));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider]);

  const spec = useCallback(
    (m: BackgroundMarkValue[]): RemovalSpecValue => ({
      ...(rect ? { rect } : {}),
      marks: m,
      ...(prior ? { prior } : {}),
    }),
    [rect, prior],
  );

  // A new preview whenever the marks change (stale answers are dropped).
  const key = prior === undefined ? '' : JSON.stringify(spec(marks));
  useEffect(() => {
    if (prior === undefined) return;
    const n = ++seq.current;
    void preview(spec(marks)).then((p) => {
      if (n !== seq.current) return;
      setAnswered(key);
      if (p) setShownPreview(p);
    });
  }, [key, marks, prior, preview, spec]);
  const shown = shownPreview;
  const busy = answered !== key || prior === undefined;

  const at = (e: PointerEvent<HTMLDivElement>): [number, number] => {
    const r = e.currentTarget.getBoundingClientRect();
    const f = (v: number) => Math.max(0, Math.min(1, v));
    return [f((e.clientX - r.left) / (r.width || 1)), f((e.clientY - r.top) / (r.height || 1))];
  };
  const onDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture?.(e.pointerId);
    setDrawing({ keep: tool === 'keep', points: [at(e)] });
  };
  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!drawing) return;
    const p = at(e);
    const last = drawing.points[drawing.points.length - 1]!;
    if (Math.hypot(p[0] - last[0], p[1] - last[1]) < 0.004) return;
    setDrawing({ ...drawing, points: [...drawing.points, p] });
  };
  const onUp = () => {
    if (!drawing) return;
    setMarks((m) => [...m, drawing]);
    setDrawing(null);
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? STEP * 5 : STEP;
    const move: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    };
    const d = move[e.key];
    if (d) {
      e.preventDefault();
      setCursor(([x, y]) => [
        Math.max(0, Math.min(1, x + d[0])),
        Math.max(0, Math.min(1, y + d[1])),
      ]);
    } else if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      setMarks((m) => [...m, { keep: tool === 'keep', points: [cursor] }]);
    }
  };

  const aspect = shown ? shown.width / Math.max(1, shown.height) : 4 / 3;
  const strokes = drawing ? [...marks, drawing] : marks;
  const width = Math.round(Math.min(560, 380 * aspect));
  const stroke = MARK_RADIUS * 2 * Math.max(1, 1 / aspect);
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onDiscard()}
      title={labels.removeBackground}
      testId="picture-bg-dialog"
      width={Math.max(360, width + 48)}
      footer={
        <>
          <button
            type="button"
            data-testid="picture-bg-keep"
            disabled={busy && !shown}
            onClick={() => onKeep(spec(marks))}
          >
            {labels.keepChanges}
          </button>
          <button type="button" data-testid="picture-bg-discard" onClick={onDiscard}>
            {labels.discard}
          </button>
        </>
      }
    >
      <div className={styles.tools} role="toolbar" aria-label={labels.removeBackground}>
        <button
          type="button"
          aria-pressed={tool === 'keep'}
          data-testid="picture-bg-mark-keep"
          className={styles.tool}
          onClick={() => setTool('keep')}
        >
          <Plus size={16} aria-hidden="true" /> {labels.markKeep}
        </button>
        <button
          type="button"
          aria-pressed={tool === 'remove'}
          data-testid="picture-bg-mark-remove"
          className={styles.tool}
          onClick={() => setTool('remove')}
        >
          <Eraser size={16} aria-hidden="true" /> {labels.markRemove}
        </button>
        <button
          type="button"
          data-testid="picture-bg-undo-mark"
          className={styles.tool}
          disabled={marks.length === 0}
          onClick={() => setMarks((m) => m.slice(0, -1))}
        >
          <Undo2 size={16} aria-hidden="true" /> {labels.undoMark}
        </button>
      </div>
      <div
        ref={area}
        className={styles.area}
        style={{ width, aspectRatio: String(aspect) }}
        tabIndex={0}
        role="application"
        aria-label={labels.canvas}
        aria-busy={busy}
        data-testid="picture-bg-area"
        data-marks={marks.length}
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={onUp}
        onKeyDown={onKey}
      >
        {shown && (
          <img
            className={styles.picture}
            src={`data:image/png;base64,${shown.image}`}
            alt=""
            draggable={false}
            data-testid="picture-bg-preview"
          />
        )}
        <svg
          className={styles.overlay}
          viewBox="0 0 1 1"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          {strokes.map((m, i) => (
            <polyline
              key={i}
              points={m.points.map((p) => p.join(',')).join(' ')}
              className={m.keep ? styles.keep : styles.remove}
              strokeWidth={stroke}
            />
          ))}
          <circle
            className={styles.cursor}
            cx={cursor[0]}
            cy={cursor[1]}
            r={stroke}
            data-testid="picture-bg-cursor"
          />
        </svg>
      </div>
      <p className={styles.status} aria-live="polite" data-testid="picture-bg-status">
        {busy || !shown ? labels.computing : labels.kept((shown.kept / 1000).toFixed(0))}
      </p>
    </Dialog>
  );
}
