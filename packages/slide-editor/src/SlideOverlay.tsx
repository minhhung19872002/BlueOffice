import type {
  HandleKind,
  SlideBox,
  SlideCommand,
  SlideEditResult,
  SlidePreview,
  SlideShapeInfo,
  SlideTextEdit,
  SlideTransform,
} from '@blueoffice/platform';
import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';
import type { SlideController } from './controller';
import { fromEditorText, toEditorText } from './editing';
import styles from './SlideEditor.module.css';

/** Text being edited inside a shape. */
export interface TextEditing {
  id: number;
  text: string;
  rect: SlideBox;
  /** Font size in points (for the editor). */
  size: number;
  /** Slide point where editing started (caret goes there). */
  at?: { x: number; y: number };
}

/** Is a slide point inside a (rotated) box? */
export function inBox(b: SlideBox, x: number, y: number, tol = 0): boolean {
  const a = (-b.rot * Math.PI) / 180;
  const dx = x - b.cx;
  const dy = y - b.cy;
  const lx = dx * Math.cos(a) - dy * Math.sin(a);
  const ly = dx * Math.sin(a) + dy * Math.cos(a);
  return Math.abs(lx) <= b.w / 2 + tol && Math.abs(ly) <= b.h / 2 + tol;
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
  onStartEdit: (shape: SlideShapeInfo, at?: { x: number; y: number }) => void;
  /** Leave text editing (Escape, click outside the shape). */
  onExitEdit: () => void;
  /** Keyboard shortcut inside the text (Ctrl+B, …); true when handled. */
  onTextShortcut?: (e: KeyboardEvent) => boolean;
  label: string;
  editorLabel: string;
  /** Tooltips of adjust handles and connector ends. */
  adjustLabel?: string;
  connectorLabel?: string;
  /** Accessible name of the table cell editor. */
  cellLabel?: string;
  /** Collaborators' selected objects on this slide (live collaboration). */
  peers?: { clientId: number; name: string; color: string; rect: SlideBox; label: string }[];
}

interface Drag {
  mode: 'move' | 'resize' | 'rotate' | 'marquee' | 'adjust' | 'cxnEnd' | 'cells';
  handle?: HandleKind;
  /** Adjust handle index / connector end. */
  index?: number;
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
/** Snap distance of a connector end to a connection site, CSS pixels. */
const SITE_PX = 12;

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
  onExitEdit,
  onTextShortcut,
  label,
  editorLabel,
  adjustLabel = '',
  connectorLabel = '',
  cellLabel = '',
  peers = [],
}: SlideOverlayProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const drag = useRef<Drag | null>(null);
  const textDrag = useRef<{ busy: boolean; pending: { x: number; y: number } | null } | null>(null);
  const [hitSeq, setHitSeq] = useState(0);
  // Live drags of adjust handles / connector ends: one command in flight, latest wins.
  const live = useRef<{ busy: boolean; pending: SlideCommand | null; active: boolean }>({
    busy: false,
    pending: null,
    active: false,
  });
  const runLive = (cmd: SlideCommand) => {
    const l = live.current;
    if (l.busy) {
      l.pending = cmd;
      return;
    }
    l.busy = true;
    void controller.run(cmd).then(() => {
      l.busy = false;
      const next = l.pending;
      l.pending = null;
      if (next) runLive(next);
    });
  };
  const [marquee, setMarquee] = useState<{ x: number; y: number; w: number; h: number } | null>(
    null,
  );
  const [cellEdit, setCellEdit] = useState<{ row: number; col: number } | null>(null);
  const table = edit?.table;
  const tableFrame = table && edit?.selection.find((s) => s.id === table.id)?.rect;
  const textEdit: SlideTextEdit | null =
    editing && edit?.textEdit?.id === editing.id ? edit.textEdit : null;
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

  /** Caret to a slide point of the edited shape; coalesced while dragging. */
  const hitText = (x: number, y: number, extend: boolean) => {
    const td = textDrag.current;
    if (td?.busy) {
      td.pending = { x, y };
      return;
    }
    if (td) td.busy = true;
    void controller.run({ cmd: 'textHit', x, y, extend }).then(() => {
      setHitSeq((s) => s + 1);
      const t = textDrag.current;
      if (!t) return;
      t.busy = false;
      const next = t.pending;
      t.pending = null;
      if (next) hitText(next.x, next.y, true);
    });
  };

  const onPointerDown = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (e.button !== 0) return;
    const p = toPt(e);
    if (editing) {
      e.preventDefault();
      if (inBox(editing.rect, p.x, p.y, 2 / scale)) {
        svgRef.current?.setPointerCapture?.(e.pointerId);
        textDrag.current = { busy: false, pending: null };
        hitText(p.x, p.y, e.shiftKey);
      } else {
        onExitEdit();
      }
      return;
    }
    e.preventDefault();
    svgRef.current?.setPointerCapture?.(e.pointerId);
    // Table editing: cells are selected with the pointer, Shift/drag extends.
    if (table && tableFrame) {
      if (inBox(tableFrame, p.x, p.y)) {
        setCellEdit(null);
        drag.current = { mode: 'cells', sx: p.x, sy: p.y, started: false, ready: true, last: null };
        void controller.run({ cmd: 'tableHit', x: p.x, y: p.y, extend: e.shiftKey });
        return;
      }
      setCellEdit(null);
      void controller.run({ cmd: 'tableExit' });
    }
    const target = e.target as Element;
    const adj = target.getAttribute('data-adjust');
    const cxn = target.getAttribute('data-cxn-end');
    if (adj !== null || cxn !== null) {
      drag.current = {
        mode: adj !== null ? 'adjust' : 'cxnEnd',
        index: Number(adj ?? cxn),
        sx: p.x,
        sy: p.y,
        started: false,
        ready: true,
        last: null,
      };
      live.current.active = true;
      return;
    }
    const handle = target.getAttribute('data-handle') as HandleKind | null;
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
    void controller
      .run({ cmd: 'hitTest', x: p.x, y: p.y })
      .then((h) => {
        // Empty space: a drag draws a selection box (marquee).
        if (h?.hits && h.hits.length === 0) {
          if (!extend) void controller.run({ cmd: 'select', ids: [] });
          if (drag.current === d) {
            d.mode = 'marquee';
            d.ready = true;
          }
          return null;
        }
        return controller.run({ cmd: 'click', x: p.x, y: p.y, extend });
      })
      .then((r) => {
        if (drag.current !== d || d.mode === 'marquee') return;
        if (!r || extend || r.selection.length === 0) drag.current = null;
        else d.ready = true;
      });
  };

  const onPointerMove = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (editing) {
      if (textDrag.current) {
        const p = toPt(e);
        hitText(p.x, p.y, true);
      }
      return;
    }
    const d = drag.current;
    if (!d || !d.ready) return;
    const p = toPt(e);
    if (!d.started && Math.hypot(p.x - d.sx, p.y - d.sy) * (scale || 1) < DRAG_THRESHOLD_PX) {
      return;
    }
    d.started = true;
    if (d.mode === 'cells') {
      runLive({ cmd: 'tableHit', x: p.x, y: p.y, extend: true });
      return;
    }
    if (d.mode === 'adjust') {
      runLive({ cmd: 'adjust', index: d.index ?? 0, x: p.x, y: p.y, merge: true });
      return;
    }
    if (d.mode === 'cxnEnd') {
      runLive({
        cmd: 'connectorEnd',
        end: d.index === 1 ? 1 : 0,
        x: p.x,
        y: p.y,
        merge: true,
        tolerance: SITE_PX / (scale || 1),
      });
      return;
    }
    if (d.mode === 'marquee') {
      setMarquee({
        x: Math.min(d.sx, p.x),
        y: Math.min(d.sy, p.y),
        w: Math.abs(p.x - d.sx),
        h: Math.abs(p.y - d.sy),
      });
      return;
    }
    d.last = transformOf(d, p.x, p.y, e.shiftKey);
    controller.preview(d.last, !e.altKey, SNAP_PX / scale);
  };

  const onPointerUp = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (textDrag.current) {
      textDrag.current = null;
      svgRef.current?.releasePointerCapture?.(e.pointerId);
      return;
    }
    const d = drag.current;
    drag.current = null;
    svgRef.current?.releasePointerCapture?.(e.pointerId);
    live.current.active = false;
    if (!d?.started || d.mode === 'cells') return;
    const p = toPt(e);
    if (d.mode === 'adjust') {
      runLive({ cmd: 'adjust', index: d.index ?? 0, x: p.x, y: p.y, merge: true });
      return;
    }
    if (d.mode === 'cxnEnd') {
      runLive({
        cmd: 'connectorEnd',
        end: d.index === 1 ? 1 : 0,
        x: p.x,
        y: p.y,
        merge: true,
        tolerance: SITE_PX / (scale || 1),
      });
      return;
    }
    if (d.mode === 'marquee') {
      setMarquee(null);
      const rect = {
        x: Math.min(d.sx, p.x),
        y: Math.min(d.sy, p.y),
        w: Math.abs(p.x - d.sx),
        h: Math.abs(p.y - d.sy),
      };
      void controller.run({ cmd: 'selectRect', rect, extend: e.shiftKey });
      return;
    }
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
    if (editing) return;
    const sel = edit?.selection ?? [];
    const one = sel.length === 1 ? sel[0] : undefined;
    if (!one) return;
    const p = toPt(e);
    if (one.kind === 'table') {
      // First double-click enters the table, the next one edits the cell.
      void controller.run({ cmd: 'tableHit', x: p.x, y: p.y }).then((r) => {
        const t = r?.table;
        if (t) setCellEdit({ row: t.sel[0], col: t.sel[1] });
      });
      return;
    }
    if (one.kind === 'group') {
      void controller.run({ cmd: 'click', x: p.x, y: p.y, deep: true });
    } else if (one.editableText) {
      onStartEdit(one, p);
    }
  };

  const hs = HANDLE_PX / scale;
  const showHandles = !preview && !editing && !table && edit?.bounds;
  const caret = textEdit?.caret;
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
          setMarquee(null);
          controller.cancelPreview();
        }}
        onDoubleClick={onDoubleClick}
      >
        {edit?.placeholders.map((b, i) => boxRect(b, styles.placeholder, `ph${i}`))}
        {peers.map((p) => (
          <g key={`peer${p.clientId}`} data-testid="peer-selection" aria-label={p.label}>
            <title>{p.label}</title>
            {boxRect(p.rect, styles.peer, undefined, { style: { stroke: p.color } })}
            <text
              className={styles.peerName}
              x={p.rect.cx - p.rect.w / 2}
              y={p.rect.cy - p.rect.h / 2 - 3 / scale}
              style={{ fill: p.color, fontSize: 11 / scale }}
            >
              {p.name}
            </text>
          </g>
        ))}
        {editing
          ? boxRect(textEdit?.rect ?? editing.rect, styles.editingFrame, 'edit', {
              'data-testid': 'text-editing-frame',
            })
          : edit?.selection.map((s) =>
              boxRect(s.rect, styles.selected, s.id, { 'data-testid': 'selection-box' }),
            )}
        {table?.cells.map((cell) => {
          const [r0, c0, r1, c1] = table.sel;
          const inside = cell.row >= r0 && cell.row <= r1 && cell.col >= c0 && cell.col <= c1;
          return boxRect(
            cell.rect,
            inside ? styles.cellSel : styles.cell,
            `c${cell.row}-${cell.col}`,
            {
              'data-testid': inside ? 'table-cell-selected' : 'table-cell',
            },
          );
        })}
        {textEdit?.rects.map((b, i) =>
          boxRect(b, styles.textSel, `ts${i}`, { 'data-testid': 'text-selection' }),
        )}
        {caret && (
          <line
            className={styles.caret}
            data-testid="text-caret"
            x1={caret[0]}
            y1={caret[1]}
            x2={caret[2]}
            y2={caret[3]}
            vectorEffect="non-scaling-stroke"
          />
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
        {showHandles &&
          edit.sites?.map(([x, y], i) => (
            <path
              key={`site${i}`}
              className={styles.site}
              d={`M${x - hs / 3} ${y - hs / 3}L${x + hs / 3} ${y + hs / 3}M${x - hs / 3} ${y + hs / 3}L${x + hs / 3} ${y - hs / 3}`}
              vectorEffect="non-scaling-stroke"
            />
          ))}
        {showHandles &&
          edit.connector?.ends.map(([x, y], i) => (
            <circle
              key={`cxn${i}`}
              className={edit.connector?.attached[i] ? styles.cxnAttached : styles.cxnEnd}
              data-cxn-end={i}
              data-testid={`connector-end-${i}`}
              cx={x}
              cy={y}
              r={hs / 2}
              vectorEffect="non-scaling-stroke"
            >
              <title>{connectorLabel}</title>
            </circle>
          ))}
        {showHandles &&
          edit.adjust.map((h) => (
            <path
              key={`adj${h.index}`}
              className={styles.adjust}
              data-adjust={h.index}
              data-testid={`adjust-${h.index}`}
              d={`M${h.x} ${h.y - hs / 2}L${h.x + hs / 2} ${h.y}L${h.x} ${h.y + hs / 2}L${h.x - hs / 2} ${h.y}Z`}
              vectorEffect="non-scaling-stroke"
            >
              <title>{adjustLabel}</title>
            </path>
          ))}
        {preview?.boxes.map((b, i) => boxRect(b, styles.ghost, `g${i}`))}
        {marquee && (
          <rect
            className={styles.marquee}
            data-testid="marquee"
            x={marquee.x}
            y={marquee.y}
            width={marquee.w}
            height={marquee.h}
            vectorEffect="non-scaling-stroke"
          />
        )}
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
      {table && cellEdit && (
        <CellEditor
          key={`${table.id}/${cellEdit.row}/${cellEdit.col}`}
          table={table}
          at={cellEdit}
          scale={scale}
          label={cellLabel}
          controller={controller}
          onMove={setCellEdit}
          onClose={() => setCellEdit(null)}
        />
      )}
      {editing && (
        <TextEditor
          key={editing.id}
          editing={editing}
          textEdit={textEdit}
          hitSeq={hitSeq}
          scale={scale}
          label={editorLabel}
          controller={controller}
          onExit={onExitEdit}
          onShortcut={onTextShortcut}
        />
      )}
    </div>
  );
}

interface TextEditorProps {
  editing: TextEditing;
  textEdit: SlideTextEdit | null;
  /** Bumped after a caret placement with the pointer. */
  hitSeq: number;
  scale: number;
  label: string;
  controller: SlideController;
  onExit: () => void;
  onShortcut?: (e: KeyboardEvent) => boolean;
}

/**
 * Invisible `<textarea>` over the shape: it receives typing and the browser/OS
 * IME (Telex, VNI) while the engine draws the text, the caret and the selection
 * on the canvas. Every change is sent as `setText` (merged into one undo step);
 * the textarea is uncontrolled and never rewritten during a composition.
 */
function TextEditor({
  editing,
  textEdit,
  hitSeq,
  scale,
  label,
  controller,
  onExit,
  onShortcut,
}: TextEditorProps) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const composing = useRef(false);
  const inflight = useRef(0);
  const b = editing.rect;
  const minH = Math.max(b.h, editing.size * 1.6);

  const selection = (): [number, number] => {
    const ta = ref.current;
    return ta ? [ta.selectionStart, ta.selectionEnd] : [0, 0];
  };

  const sendSelect = () => {
    if (composing.current) return;
    const [start, end] = selection();
    void controller.run({ cmd: 'textSelect', id: editing.id, start, end });
  };

  const sendText = () => {
    const ta = ref.current;
    if (!ta) return;
    inflight.current += 1;
    void controller
      .run({
        cmd: 'setText',
        id: editing.id,
        text: fromEditorText(ta.value),
        merge: true,
        sel: selection(),
      })
      .finally(() => {
        inflight.current -= 1;
      });
  };

  useEffect(() => {
    const ta = ref.current;
    if (!ta) return;
    ta.focus();
    ta.setSelectionRange(ta.value.length, ta.value.length);
    const n = ta.value.length;
    void controller.run({ cmd: 'textSelect', id: editing.id, start: n, end: n });
    if (editing.at) {
      void controller.run({ cmd: 'textHit', x: editing.at.x, y: editing.at.y }).then((r) => {
        const te = r?.textEdit;
        if (te && ref.current) ref.current.setSelectionRange(te.start, te.end);
      });
    }
    // Runs once per edited shape (the component is keyed by the shape id).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The engine changed the text (undo, redo, formatting): follow it.
  const engineText = textEdit?.text;
  useEffect(() => {
    const ta = ref.current;
    if (!ta || engineText === undefined || composing.current || inflight.current > 0) return;
    const shown = toEditorText(engineText);
    if (ta.value !== shown) {
      ta.value = shown;
      if (textEdit) ta.setSelectionRange(textEdit.start, textEdit.end);
    }
  }, [engineText, textEdit]);

  // A pointer placed the caret on the canvas.
  useEffect(() => {
    const ta = ref.current;
    if (!ta || !textEdit || hitSeq === 0) return;
    ta.focus();
    ta.setSelectionRange(
      Math.min(textEdit.start, textEdit.end),
      Math.max(textEdit.start, textEdit.end),
      textEdit.end < textEdit.start ? 'backward' : 'forward',
    );
    // Only after a new caret placement.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hitSeq]);

  const onKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    e.stopPropagation();
    if (composing.current || e.nativeEvent.isComposing) return;
    const mod = e.ctrlKey || e.metaKey;
    if (e.key === 'Escape') {
      e.preventDefault();
      onExit();
      return;
    }
    if (e.key === 'Tab') {
      // Tab / Shift+Tab: list level of the selected paragraphs (§21.1.2.2.7 pPr@lvl).
      e.preventDefault();
      void controller.run({
        cmd: 'formatText',
        format: { kind: 'level', delta: e.shiftKey ? -1 : 1 },
      });
      return;
    }
    if (mod && !e.altKey && (e.key === 'z' || e.key === 'y' || e.key === 'Z')) {
      e.preventDefault();
      const redo = e.key === 'y' || (e.key === 'Z' && e.shiftKey) || (e.key === 'z' && e.shiftKey);
      void controller.run({ cmd: redo ? 'redo' : 'undo' });
      return;
    }
    if (mod && onShortcut?.(e.nativeEvent)) e.preventDefault();
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
        sendText();
      }}
      onInput={sendText}
      onSelect={sendSelect}
      onKeyUp={(e) => {
        if (e.key.startsWith('Arrow') || e.key === 'Home' || e.key === 'End') sendSelect();
      }}
      onKeyDown={onKeyDown}
    />
  );
}

interface CellEditorProps {
  table: NonNullable<SlideEditResult['table']>;
  at: { row: number; col: number };
  scale: number;
  label: string;
  controller: SlideController;
  onMove: (at: { row: number; col: number }) => void;
  onClose: () => void;
}

/**
 * Text of one table cell, edited in a textarea over the cell; every change is
 * sent live (one undo step), Tab / Shift+Tab move to the next / previous cell,
 * Escape closes. Composition (IME) text is sent once composed.
 */
function CellEditor({ table, at, scale, label, controller, onMove, onClose }: CellEditorProps) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const composing = useRef(false);
  const cell = table.cells.find((c) => c.row === at.row && c.col === at.col);
  const text = table.sel[0] === at.row && table.sel[1] === at.col ? table.text : '';
  useEffect(() => {
    const ta = ref.current;
    if (!ta) return;
    ta.focus();
    ta.setSelectionRange(ta.value.length, ta.value.length);
  }, []);
  if (!cell) return null;
  const b = cell.rect;
  const send = () => {
    if (!ref.current || composing.current) return;
    void controller.run({
      cmd: 'setCellText',
      row: at.row,
      col: at.col,
      text: fromEditorText(ref.current.value),
      merge: true,
    });
  };
  const go = (delta: number) => {
    const order = [...table.cells].sort((a, z) => a.row - z.row || a.col - z.col);
    const i = order.findIndex((c) => c.row === at.row && c.col === at.col);
    const next = order[(i + delta + order.length) % order.length];
    if (!next) return;
    void controller
      .run({ cmd: 'tableSelect', id: table.id, r0: next.row, c0: next.col })
      .then(() => onMove({ row: next.row, col: next.col }));
  };
  return (
    <textarea
      ref={ref}
      className={styles.cellEditor}
      data-testid="table-cell-editor"
      aria-label={label}
      spellCheck={false}
      defaultValue={toEditorText(text)}
      style={{
        left: (b.cx - b.w / 2) * scale,
        top: (b.cy - b.h / 2) * scale,
        width: Math.max(b.w * scale, 30),
        height: Math.max(b.h * scale, 18),
        fontSize: Math.max(14 * scale, 8),
        transform: b.rot ? `rotate(${b.rot}deg)` : undefined,
      }}
      onCompositionStart={() => {
        composing.current = true;
      }}
      onCompositionEnd={() => {
        composing.current = false;
        send();
      }}
      onInput={send}
      onBlur={onClose}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (composing.current || e.nativeEvent.isComposing) return;
        if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
        } else if (e.key === 'Tab') {
          e.preventDefault();
          go(e.shiftKey ? -1 : 1);
        }
      }}
    />
  );
}
