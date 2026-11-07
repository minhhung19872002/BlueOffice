import type {
  HandleKind,
  SlideBox,
  SlideEditResult,
  SlidePreview,
  SlideShapeInfo,
  SlideTransform,
} from '@blueoffice/platform';
import {
  useEffect,
  useRef,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';
import type { SlideController } from './controller';
import { fromEditorText, toEditorText } from './editing';
import styles from './SlideEditor.module.css';

/** Text being edited in the overlay textarea. */
export interface TextEditing {
  id: number;
  text: string;
  rect: SlideBox;
  /** Font size in points (for the editor). */
  size: number;
}

export interface SlideOverlayProps {
  /** Slide size in points. */
  slide: { width: number; height: number };
  /** Displayed width in CSS pixels. */
  width: number;
  edit: SlideEditResult | null;
  preview: SlidePreview | null;
  controller: SlideController;
  editing: TextEditing | null;
  onStartEdit: (shape: SlideShapeInfo) => void;
  onCommitText: (id: number, text: string) => void;
  label: string;
  editorLabel: string;
}

interface Drag {
  mode: 'move' | 'resize' | 'rotate';
  handle?: HandleKind;
  sx: number;
  sy: number;
  started: boolean;
  ready: boolean;
  last: SlideTransform | null;
}

/** Snap tolerance and handle size in CSS pixels. */
const SNAP_PX = 6;
const HANDLE_PX = 9;
const DRAG_THRESHOLD_PX = 3;

function boxRect(b: SlideBox, className: string | undefined, key?: string | number, extra = {}) {
  return (
    <rect
      key={key}
      className={className}
      x={b.cx - b.w / 2}
      y={b.cy - b.h / 2}
      width={Math.max(b.w, 0)}
      height={Math.max(b.h, 0)}
      transform={b.rot ? `rotate(${b.rot} ${b.cx} ${b.cy})` : undefined}
      vectorEffect="non-scaling-stroke"
      {...extra}
    />
  );
}

/**
 * Selection layer over the main slide canvas: outlines, resize/rotation
 * handles, drag previews with smart-guide lines and the text editor. All
 * geometry comes from the engine (slide points); the SVG viewBox is the slide.
 */
export function SlideOverlay({
  slide,
  width,
  edit,
  preview,
  controller,
  editing,
  onStartEdit,
  onCommitText,
  label,
  editorLabel,
}: SlideOverlayProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const drag = useRef<Drag | null>(null);
  const scale = slide.width > 0 ? width / slide.width : 1;
  const height = slide.height * scale;

  const toPt = (e: { clientX: number; clientY: number }) => {
    const r = svgRef.current?.getBoundingClientRect();
    if (!r || r.width === 0 || r.height === 0) return { x: 0, y: 0 };
    return {
      x: ((e.clientX - r.left) / r.width) * slide.width,
      y: ((e.clientY - r.top) / r.height) * slide.height,
    };
  };

  const transformOf = (d: Drag, x: number, y: number, shift: boolean): SlideTransform => {
    if (d.mode === 'rotate') return { kind: 'rotate', x, y, step: shift };
    if (d.mode === 'resize') {
      const single = edit?.selection.length === 1 ? edit.selection[0] : undefined;
      return {
        kind: 'resize',
        handle: d.handle ?? 'se',
        dx: x - d.sx,
        dy: y - d.sy,
        keepAspect: shift !== (single?.lockAspect ?? false),
      };
    }
    return { kind: 'move', dx: x - d.sx, dy: y - d.sy };
  };

  const onPointerDown = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (editing || e.button !== 0) return;
    e.preventDefault();
    svgRef.current?.setPointerCapture?.(e.pointerId);
    const p = toPt(e);
    const handle = (e.target as Element).getAttribute('data-handle') as HandleKind | null;
    if (handle) {
      drag.current = {
        mode: handle === 'rot' ? 'rotate' : 'resize',
        handle,
        sx: p.x,
        sy: p.y,
        started: false,
        ready: true,
        last: null,
      };
      return;
    }
    const d: Drag = { mode: 'move', sx: p.x, sy: p.y, started: false, ready: false, last: null };
    drag.current = d;
    const extend = e.shiftKey;
    void controller.run({ cmd: 'click', x: p.x, y: p.y, extend }).then((r) => {
      if (drag.current !== d) return;
      if (!r || extend || r.selection.length === 0) drag.current = null;
      else d.ready = true;
    });
  };

  const onPointerMove = (e: ReactPointerEvent<SVGSVGElement>) => {
    const d = drag.current;
    if (!d || !d.ready) return;
    const p = toPt(e);
    if (!d.started && Math.hypot(p.x - d.sx, p.y - d.sy) * scale < DRAG_THRESHOLD_PX) return;
    d.started = true;
    d.last = transformOf(d, p.x, p.y, e.shiftKey);
    controller.preview(d.last, !e.altKey, SNAP_PX / scale);
  };

  const onPointerUp = (e: ReactPointerEvent<SVGSVGElement>) => {
    const d = drag.current;
    drag.current = null;
    svgRef.current?.releasePointerCapture?.(e.pointerId);
    if (!d?.started) return;
    const p = toPt(e);
    const t = d.ready ? transformOf(d, p.x, p.y, e.shiftKey) : d.last;
    controller.cancelPreview();
    if (t) {
      void controller.run({
        cmd: 'transform',
        transform: t,
        snap: !e.altKey,
        tolerance: SNAP_PX / scale,
      });
    }
  };

  const onDoubleClick = (e: ReactMouseEvent<SVGSVGElement>) => {
    const sel = edit?.selection ?? [];
    const one = sel.length === 1 ? sel[0] : undefined;
    if (!one) return;
    if (one.kind === 'group') {
      const p = toPt(e);
      void controller.run({ cmd: 'click', x: p.x, y: p.y, deep: true });
    } else if (one.editableText) {
      onStartEdit(one);
    }
  };

  const hs = HANDLE_PX / scale;
  const showHandles = !preview && !editing && edit?.bounds;
  const rotHandle = edit?.handles.find((h) => h.kind === 'rot');
  const topCenter = edit?.handles.find((h) => h.kind === 'n');

  return (
    <div className={styles.overlay} style={{ width, height }}>
      <svg
        ref={svgRef}
        className={styles.overlaySvg}
        data-testid="slide-overlay"
        role="application"
        aria-label={label}
        width={width}
        height={height}
        viewBox={`0 0 ${slide.width} ${slide.height}`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => {
          drag.current = null;
          controller.cancelPreview();
        }}
        onDoubleClick={onDoubleClick}
      >
        {edit?.placeholders.map((b, i) => boxRect(b, styles.placeholder, `ph${i}`))}
        {edit?.selection.map((s) =>
          boxRect(s.rect, styles.selected, s.id, { 'data-testid': 'selection-box' }),
        )}
        {showHandles && edit.selection.length > 1 && boxRect(edit.bounds!, styles.bounds, 'b')}
        {showHandles && rotHandle && topCenter && (
          <line
            className={styles.rotLine}
            x1={topCenter.x}
            y1={topCenter.y}
            x2={rotHandle.x}
            y2={rotHandle.y}
            vectorEffect="non-scaling-stroke"
          />
        )}
        {showHandles &&
          edit.handles.map((h) =>
            h.kind === 'rot' ? (
              <circle
                key={h.kind}
                className={styles.handle}
                data-handle={h.kind}
                data-testid={`handle-${h.kind}`}
                cx={h.x}
                cy={h.y}
                r={hs / 2}
                vectorEffect="non-scaling-stroke"
              />
            ) : (
              <rect
                key={h.kind}
                className={styles.handle}
                data-handle={h.kind}
                data-testid={`handle-${h.kind}`}
                x={h.x - hs / 2}
                y={h.y - hs / 2}
                width={hs}
                height={hs}
                style={{ cursor: `${h.kind}-resize` }}
                vectorEffect="non-scaling-stroke"
              />
            ),
          )}
        {preview?.boxes.map((b, i) => boxRect(b, styles.ghost, `g${i}`))}
        {preview?.guides.map((g, i) =>
          g.axis === 'v' ? (
            <line
              key={`l${i}`}
              className={styles.guide}
              data-testid="smart-guide"
              x1={g.pos}
              x2={g.pos}
              y1={g.from}
              y2={g.to}
              vectorEffect="non-scaling-stroke"
            />
          ) : (
            <line
              key={`l${i}`}
              className={styles.guide}
              data-testid="smart-guide"
              x1={g.from}
              x2={g.to}
              y1={g.pos}
              y2={g.pos}
              vectorEffect="non-scaling-stroke"
            />
          ),
        )}
      </svg>
      {editing && (
        <TextEditor
          key={editing.id}
          editing={editing}
          scale={scale}
          label={editorLabel}
          onCommit={(text) => onCommitText(editing.id, text)}
        />
      )}
    </div>
  );
}

interface TextEditorProps {
  editing: TextEditing;
  scale: number;
  label: string;
  onCommit: (text: string) => void;
}

/**
 * Plain `<textarea>` over the shape. It is uncontrolled so the browser/OS IME
 * (Telex, VNI) composes freely; the text is committed on blur or Escape —
 * never while a composition is in progress.
 */
function TextEditor({ editing, scale, label, onCommit }: TextEditorProps) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const composing = useRef(false);
  const done = useRef(false);
  const b = editing.rect;
  const minH = Math.max(b.h, editing.size * 1.6);

  useEffect(() => {
    const ta = ref.current;
    if (!ta) return;
    ta.focus();
    ta.setSelectionRange(ta.value.length, ta.value.length);
  }, []);

  const commit = () => {
    if (done.current || !ref.current) return;
    done.current = true;
    onCommit(fromEditorText(ref.current.value));
  };

  return (
    <textarea
      ref={ref}
      className={styles.textEditor}
      data-testid="slide-text-editor"
      aria-label={label}
      spellCheck={false}
      defaultValue={toEditorText(editing.text)}
      style={{
        left: (b.cx - b.w / 2) * scale,
        top: (b.cy - minH / 2) * scale,
        width: Math.max(b.w * scale, 40),
        height: minH * scale,
        fontSize: Math.max(editing.size * scale, 8),
        transform: b.rot ? `rotate(${b.rot}deg)` : undefined,
      }}
      onCompositionStart={() => {
        composing.current = true;
      }}
      onCompositionEnd={() => {
        composing.current = false;
      }}
      onBlur={commit}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (composing.current || e.nativeEvent.isComposing) return;
        if (e.key === 'Escape') {
          e.preventDefault();
          commit();
        }
      }}
    />
  );
}
