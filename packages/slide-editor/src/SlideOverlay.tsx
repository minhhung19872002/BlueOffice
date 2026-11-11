import type {
  HandleKind,
  SlideBox,
  SlideCommand,
  SlideEditResult,
  SlidePreview,
  SlideRect,
  SlideShapeBoxes,
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
import {
  boxHandles,
  constrainMove,
  drawnRect,
  fromEditorText,
  hitShape,
  paragraphRange,
  snapTo,
  toEditorText,
  wordRange,
} from './editing';
import { nearestSite } from './editPoints';
import shapeStyles from './shapeFormat.module.css';
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
  /** Table cell `[row, col]` whose text is edited (the rect is the cell's box). */
  cell?: [number, number];
  /** Initial text selection (UTF-16 offsets) when the engine placed the caret. */
  sel?: [number, number];
  /** Editing started from the keyboard on the selected shape (Enter, F2): all its text selected. */
  selectAll?: boolean;
  /** Editing started by typing on the selected shape: the typed text, added at the end. */
  insert?: string;
}

const sameCell = (a?: [number, number], b?: [number, number]) =>
  (!a && !b) || (!!a && !!b && a[0] === b[0] && a[1] === b[1]);

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
  /** Double-click on an embedded / linked object (CORE-EMBED-001). */
  onActivateObject?: () => void;
  /** Leave text editing (Escape, click outside the shape). */
  onExitEdit: () => void;
  /** Keyboard shortcut inside the text (Ctrl+B, …); true when handled. */
  onTextShortcut?: (e: KeyboardEvent) => boolean;
  /** Ctrl+Enter in the edited text: the next text placeholder, or a new slide. */
  onNextPlaceholder?: () => void;
  /** Mouse-up after selecting text in the edited shape (viewport point; mini toolbar). */
  onTextSelectEnd?: (at: { x: number; y: number }) => void;
  label: string;
  editorLabel: string;
  /** Tooltips of adjust handles and connector ends. */
  adjustLabel?: string;
  connectorLabel?: string;
  /** Start editing the text of a table cell (result of `cellTextHit` / `cellNext`). */
  onStartCellEdit?: (te: SlideTextEdit) => void;
  /** Collaborators' selected objects on this slide (live collaboration). */
  peers?: { clientId: number; name: string; color: string; rect: SlideBox; label: string }[];
  /**
   * Drawing a new object (Insert › Shapes / Text box): a click puts it at its default size,
   * a drag at the dragged size; `onDraw` gets its box (slide points).
   */
  drawing?: 'shape' | 'text' | null;
  onDraw?: (rect: SlideRect) => void;
  /** The format painter is loaded: a click on a shape (or a text selection) applies it. */
  painting?: boolean;
  onPaint?: () => void;
  /** Gridlines shown every `step` points; drawn objects snap to them. */
  grid?: { show: boolean; step: number } | null;
  /** Drawing guides (vertical lines at `x`, horizontal at `y`, slide points); draggable. */
  guides?: { x: number[]; y: number[] } | null;
  /** A guide was dragged (`to` null: dragged off the slide, removed). */
  onGuideMove?: (axis: 'x' | 'y', index: number, to: number | null) => void;
  /** Labels of a guide's position while it is dragged. */
  guideLabel?: (points: number) => string;
}

interface Drag {
  mode: 'move' | 'resize' | 'rotate' | 'marquee' | 'adjust' | 'cxnEnd' | 'cells' | 'draw' | 'guide';
  /** Guide being dragged: axis and index. */
  guide?: ['x' | 'y', number];
  handle?: HandleKind;
  /** Adjust handle index / connector end. */
  index?: number;
  sx: number;
  sy: number;
  started: boolean;
  ready: boolean;
  last: SlideTransform | null;
  /** Ctrl was down on a selected shape: a click without a drag takes it out of the selection. */
  toggle?: { x: number; y: number };
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
  onActivateObject,
  onExitEdit,
  onTextShortcut,
  onNextPlaceholder,
  onTextSelectEnd,
  label,
  editorLabel,
  adjustLabel = '',
  connectorLabel = '',
  onStartCellEdit,
  peers = [],
  drawing = null,
  onDraw,
  painting = false,
  onPaint,
  grid = null,
  guides = null,
  onGuideMove,
  guideLabel,
}: SlideOverlayProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const drag = useRef<Drag | null>(null);
  const textDrag = useRef<{ busy: boolean; pending: { x: number; y: number } | null } | null>(null);
  const [hitSeq, setHitSeq] = useState(0);
  // Text editing that started while a click was still on its way (double-click).
  const editingNow = useRef(false);
  // Bumped by a double-click: clicks still in flight from it are dropped.
  const gesture = useRef(0);
  useEffect(() => {
    editingNow.current = !!editing;
  }, [editing]);
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
  // Connection site a dragged connector end will snap to (CORE-SHAPE-001).
  const [snapSite, setSnapSite] = useState<[number, number] | null>(null);
  // The boxes of the slide's shapes as a click hits them (`shapeBoxes`): a click is hit-tested
  // here and its selection drawn at once, the engine's own click confirms it.
  const hitBoxes = useRef<{ slide: number; boxes: SlideShapeBoxes } | null>(null);
  const boxesAsked = useRef(false);
  const ensureBoxes = () => {
    if (hitBoxes.current || boxesAsked.current || !edit) return;
    boxesAsked.current = true;
    void controller.query({ cmd: 'shapeBoxes' }).then((r) => {
      boxesAsked.current = false;
      if (r?.shapeBoxes) hitBoxes.current = { slide: r.slide, boxes: r.shapeBoxes };
    });
  };
  // Any change of the slide makes the boxes stale.
  useEffect(() => {
    if (!edit) return;
    const h = hitBoxes.current;
    if (h && (h.slide !== edit.slide || edit.dirty.length > 0 || edit.slidesChanged)) {
      hitBoxes.current = null;
    }
  }, [edit]);
  // The selection as drawn before the engine answered a click.
  const [opt, setOpt] = useState<{ ids: number[]; boxes: SlideBox[] } | null>(null);
  // The box being drawn (Insert › Shapes / Text box).
  const [drawBox, setDrawBox] = useState<SlideRect | null>(null);
  // Position of the guide being dragged.
  const [guideAt, setGuideAt] = useState<{ axis: 'x' | 'y'; index: number; pos: number } | null>(
    null,
  );
  const table = edit?.table;
  const tableFrame = table && edit?.selection.find((s) => s.id === table.id)?.rect;
  const textEdit: SlideTextEdit | null =
    editing && edit?.textEdit?.id === editing.id && sameCell(edit.textEdit.cell, editing.cell)
      ? edit.textEdit
      : null;
  /** Edit the text of the table cell at a slide point. */
  const editCellAt = (x: number, y: number) =>
    void controller.run({ cmd: 'cellTextHit', x, y }).then((r) => {
      const te = r?.textEdit;
      if (te?.cell) onStartCellEdit?.(te);
    });
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

  /** A drawn point snapped to the grid (snap to grid is on by default, as in the reference). */
  const snapped = (q: { x: number; y: number }) => {
    const step = grid?.step ?? 6;
    return { x: snapTo(q.x, step, step / 2), y: snapTo(q.y, step, step / 2) };
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
    // Shift keeps the move on one axis.
    return { kind: 'move', ...constrainMove(x - d.sx, y - d.sy, shift) };
  };

  /** Ctrl+drag: a copy of the selection lands at the drop point, as one undo step. */
  const dropCopy = async (t: SlideTransform, snap: boolean) => {
    if (t.kind !== 'move') return;
    const since = (await controller.run({ cmd: 'historyLen' }))?.history;
    const before = edit?.selection[0]?.rect;
    const dup = await controller.run({ cmd: 'duplicate' });
    const after = dup?.selection[0]?.rect;
    if (!dup || dup.error || !before || !after) return;
    // The copy was placed a little off the original: move it by what is left.
    await controller.run({
      cmd: 'transform',
      transform: {
        kind: 'move',
        dx: t.dx - (after.cx - before.cx),
        dy: t.dy - (after.cy - before.cy),
      },
      snap,
      tolerance: SNAP_PX / scale,
    });
    if (since !== undefined) await controller.run({ cmd: 'mergeHistory', since });
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
    // Keyboard focus to the slide (shortcuts act on shapes, not on the slide panel).
    if (!editing) (svgRef.current?.closest('[tabindex]') as HTMLElement | null)?.focus();
    const p = toPt(e);
    if (editing) {
      e.preventDefault();
      if (inBox(editing.rect, p.x, p.y, 2 / scale)) {
        svgRef.current?.setPointerCapture?.(e.pointerId);
        textDrag.current = { busy: false, pending: null };
        hitText(p.x, p.y, e.shiftKey);
      } else if (editing.cell && tableFrame && inBox(tableFrame, p.x, p.y)) {
        // Another cell of the same table.
        editCellAt(p.x, p.y);
      } else {
        onExitEdit();
      }
      return;
    }
    e.preventDefault();
    svgRef.current?.setPointerCapture?.(e.pointerId);
    if (drawing) {
      // Insert › Shapes / Text box: a click or a drag on the slide places the new object.
      drag.current = { mode: 'draw', sx: p.x, sy: p.y, started: false, ready: true, last: null };
      return;
    }
    const guideEl = (e.target as Element).getAttribute('data-guide');
    if (guideEl) {
      const [axis, i] = guideEl.split(':') as ['x' | 'y', string];
      drag.current = {
        mode: 'guide',
        guide: [axis, Number(i)],
        sx: p.x,
        sy: p.y,
        started: false,
        ready: true,
        last: null,
      };
      return;
    }
    // Table editing: cells are selected with the pointer, Shift/drag extends.
    if (table && tableFrame) {
      if (inBox(tableFrame, p.x, p.y)) {
        drag.current = { mode: 'cells', sx: p.x, sy: p.y, started: false, ready: true, last: null };
        void controller.run({ cmd: 'tableHit', x: p.x, y: p.y, extend: e.shiftKey });
        return;
      }
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
    // Shift or Ctrl + click adds to / takes from the selection.
    const ctrl = e.ctrlKey || e.metaKey;
    const extend = e.shiftKey || ctrl;
    const g = gesture.current;
    const selectedIds = new Set((edit?.selection ?? []).map((x) => x.id));
    const known = hitBoxes.current?.slide === edit?.slide ? hitBoxes.current?.boxes : undefined;
    if (known) {
      // The hit test runs here: the selection is drawn on this mouse-down, the engine's
      // `click` (same rule) confirms it a moment later.
      const hit = hitShape(known, p.x, p.y);
      const boxOf = (id: number) => known.shapes.find((x) => x.id === id)?.rect;
      if (hit === null) {
        // Empty space: nothing selected (unless extending); a drag draws a selection box.
        d.mode = 'marquee';
        d.ready = true;
        if (!extend) {
          setOpt({ ids: [], boxes: [] });
          void controller.run({ cmd: 'select', ids: [] }).then(() => setOpt(null));
        }
        return;
      }
      if (ctrl && selectedIds.has(hit)) {
        // Ctrl on a selected shape: a drag copies the selection, a plain click takes it out.
        d.toggle = { x: p.x, y: p.y };
        d.ready = true;
        return;
      }
      let ids = [...selectedIds];
      if (!extend) {
        if (!selectedIds.has(hit)) ids = [hit];
      } else if (selectedIds.has(hit)) {
        ids = ids.filter((id) => id !== hit);
      } else {
        ids.push(hit);
      }
      const shown = ids.map((id) => boxOf(id) ?? edit?.selection.find((x) => x.id === id)?.rect);
      if (shown.every((b): b is SlideBox => !!b)) setOpt({ ids, boxes: shown });
      // Shift+click only changes the selection; a plain or Ctrl press may start a drag.
      d.ready = !(extend && !ctrl);
      void controller.run({ cmd: 'click', x: p.x, y: p.y, extend }).then((r) => {
        setOpt(null);
        if (drag.current === d && (!r || r.selection.length === 0)) drag.current = null;
        if (r && painting && !extend) onPaint?.();
      });
      return;
    }
    void controller
      .run({ cmd: 'hitTest', x: p.x, y: p.y })
      .then((h) => {
        if (editingNow.current || gesture.current !== g) return null;
        if (ctrl && h?.hits?.some((id) => selectedIds.has(id))) {
          // Ctrl on a selected shape: a drag copies the selection, a plain click toggles it
          // (the button may be up again already).
          if (drag.current !== d) return controller.run({ cmd: 'click', x: p.x, y: p.y, extend });
          d.toggle = { x: p.x, y: p.y };
          return h;
        }
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
        if (r && painting && !extend && r.selection.length > 0) onPaint?.();
        if (drag.current !== d || d.mode === 'marquee') return;
        if (!r || (extend && !ctrl) || r.selection.length === 0) drag.current = null;
        else d.ready = true;
      });
  };

  const onPointerMove = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (!editing) ensureBoxes();
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
    if (d.mode === 'draw') {
      setDrawBox(
        drawnRect(snapped({ x: d.sx, y: d.sy }), snapped(p), {
          square: e.shiftKey,
          text: drawing === 'text',
        }),
      );
      return;
    }
    if (d.mode === 'guide') {
      const [axis, index] = d.guide ?? ['x', 0];
      const pos = axis === 'x' ? p.x : p.y;
      setGuideAt({ axis, index, pos: Math.round(pos * 2) / 2 });
      return;
    }
    if (d.mode === 'cells') {
      runLive({ cmd: 'tableHit', x: p.x, y: p.y, extend: true });
      return;
    }
    if (d.mode === 'adjust') {
      runLive({ cmd: 'adjust', index: d.index ?? 0, x: p.x, y: p.y, merge: true });
      return;
    }
    if (d.mode === 'cxnEnd') {
      setSnapSite(nearestSite(edit?.sites ?? [], p.x, p.y, SITE_PX / (scale || 1)));
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
      // The format painter goes onto the text just selected.
      if (painting) onPaint?.();
      else onTextSelectEnd?.({ x: e.clientX, y: e.clientY });
      return;
    }
    const d = drag.current;
    drag.current = null;
    svgRef.current?.releasePointerCapture?.(e.pointerId);
    live.current.active = false;
    setSnapSite(null);
    if (d && !d.started && d.toggle) {
      void controller.run({ cmd: 'click', x: d.toggle.x, y: d.toggle.y, extend: true });
      return;
    }
    if (d?.mode === 'draw') {
      const p = toPt(e);
      setDrawBox(null);
      const a = snapped({ x: d.sx, y: d.sy });
      onDraw?.(
        drawnRect(a, d.started ? snapped(p) : null, {
          square: e.shiftKey,
          text: drawing === 'text',
        }),
      );
      return;
    }
    if (d?.mode === 'guide' && d.guide) {
      const p = toPt(e);
      setGuideAt(null);
      if (!d.started) return;
      const [axis, i] = d.guide;
      const pos = axis === 'x' ? p.x : p.y;
      const max = axis === 'x' ? slide.width : slide.height;
      // Dragged off the slide: the guide is removed.
      onGuideMove?.(axis, i, pos < 0 || pos > max ? null : Math.round(pos * 2) / 2);
      return;
    }
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
    if (t && d.mode === 'move' && (e.ctrlKey || e.metaKey)) {
      void dropCopy(t, !e.altKey);
    } else if (t) {
      void controller.run({
        cmd: 'transform',
        transform: t,
        snap: !e.altKey,
        tolerance: SNAP_PX / scale,
      });
    }
  };

  /** In the edited text: a double-click selects the word, a triple-click the paragraph. */
  const selectAround = (e: ReactMouseEvent<SVGSVGElement>, paragraph: boolean) => {
    if (!editing) return;
    const p = toPt(e);
    if (!inBox(editing.rect, p.x, p.y, 2 / scale)) return;
    void controller
      .run({ cmd: 'textHit', x: p.x, y: p.y })
      .then((r) => {
        const te = r?.textEdit;
        if (!te) return null;
        const [start, end] = (paragraph ? paragraphRange : wordRange)(te.text, te.start);
        if (start === end) return null;
        return controller.run({ cmd: 'textSelect', id: te.id, start, end, cell: te.cell });
      })
      .then((r) => {
        if (r) setHitSeq((n) => n + 1);
      });
  };

  const onDoubleClick = (e: ReactMouseEvent<SVGSVGElement>) => {
    if (editing) {
      selectAround(e, false);
      return;
    }
    gesture.current += 1;
    const sel = edit?.selection ?? [];
    const one = sel.length === 1 ? sel[0] : undefined;
    if (!one) return;
    const p = toPt(e);
    if (one.kind === 'table') {
      // Double-click a cell: edit its text on the canvas, caret at the point.
      editCellAt(p.x, p.y);
      return;
    }
    if (one.kind === 'object') {
      onActivateObject?.();
      return;
    }
    if (one.kind === 'group') {
      void controller.run({ cmd: 'click', x: p.x, y: p.y, deep: true });
    } else if (one.editableText || one.kind === 'smartart') {
      // A diagram: the node under the pointer is edited in place, else its text pane opens.
      onStartEdit(one, p);
    }
  };

  const hs = HANDLE_PX / scale;
  const optBox = opt?.boxes.length === 1 ? opt.boxes[0] : undefined;
  const showHandles = !preview && !editing && !table && (opt ? !!optBox : !!edit?.bounds);
  // Before the engine answered a click: the handles of the box drawn here.
  const handles = opt ? (optBox ? boxHandles(optBox) : []) : (edit?.handles ?? []);
  const caret = textEdit?.caret;
  const rotHandle = handles.find((h) => h.kind === 'rot');
  const topCenter = handles.find((h) => h.kind === 'n');

  return (
    <div className={styles.overlay} style={{ width, height }}>
      <svg
        ref={svgRef}
        className={styles.overlaySvg}
        data-testid="slide-overlay"
        data-drawing={drawing ?? undefined}
        data-painting={painting ? 'true' : undefined}
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
        onClick={(e) => {
          if (e.detail === 3) selectAround(e, true);
        }}
      >
        {grid?.show && (
          <GridLines width={slide.width} height={slide.height} step={grid.step * 12} />
        )}
        {guides &&
          [
            ...guides.x.map((v, i) => ['x', i, v] as const),
            ...guides.y.map((v, i) => ['y', i, v] as const),
          ].map(([axis, i, v]) => {
            const moving = guideAt?.axis === axis && guideAt.index === i;
            const pos = moving ? guideAt.pos : v;
            return (
              <g key={`guide-${axis}${i}`}>
                <line
                  className={styles.staticGuide}
                  data-testid="static-guide"
                  data-axis={axis}
                  x1={axis === 'x' ? pos : -slide.width}
                  x2={axis === 'x' ? pos : slide.width * 2}
                  y1={axis === 'y' ? pos : -slide.height}
                  y2={axis === 'y' ? pos : slide.height * 2}
                  vectorEffect="non-scaling-stroke"
                />
                <line
                  className={styles.guideGrip}
                  data-guide={`${axis}:${i}`}
                  x1={axis === 'x' ? pos : 0}
                  x2={axis === 'x' ? pos : slide.width}
                  y1={axis === 'y' ? pos : 0}
                  y2={axis === 'y' ? pos : slide.height}
                  style={{ cursor: axis === 'x' ? 'col-resize' : 'row-resize' }}
                />
                {moving && guideLabel && (
                  <text
                    className={styles.guideTip}
                    x={axis === 'x' ? pos + 4 / scale : 4 / scale}
                    y={axis === 'y' ? pos - 4 / scale : 14 / scale}
                    style={{ fontSize: 11 / scale }}
                  >
                    {guideLabel(pos - (axis === 'x' ? slide.width : slide.height) / 2)}
                  </text>
                )}
              </g>
            );
          })}
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
          : opt
            ? opt.boxes.map((b, i) =>
                boxRect(b, styles.selected, `o${opt.ids[i]}`, { 'data-testid': 'selection-box' }),
              )
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
        {showHandles &&
          !opt &&
          edit?.bounds &&
          edit.selection.length > 1 &&
          boxRect(edit.bounds, styles.bounds, 'b')}
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
          handles.map((h) =>
            h.kind === 'rot' ? (
              <g key={h.kind}>
                <circle
                  className={styles.rotHandle}
                  data-handle={h.kind}
                  data-testid={`handle-${h.kind}`}
                  cx={h.x}
                  cy={h.y}
                  r={hs * 0.6}
                  vectorEffect="non-scaling-stroke"
                />
                {/* A circular arrow inside: the rotation handle, not a sizing one. */}
                <path
                  className={styles.rotArrow}
                  d={`M${h.x + hs * 0.32} ${h.y - hs * 0.12}A${hs * 0.32} ${hs * 0.32} 0 1 1 ${h.x + hs * 0.1} ${h.y - hs * 0.3}`}
                  vectorEffect="non-scaling-stroke"
                />
              </g>
            ) : (
              // Round sizing handles, white with an outline.
              <circle
                key={h.kind}
                className={styles.handle}
                data-handle={h.kind}
                data-testid={`handle-${h.kind}`}
                cx={h.x}
                cy={h.y}
                r={hs / 2}
                style={{ cursor: `${h.kind}-resize` }}
                vectorEffect="non-scaling-stroke"
              />
            ),
          )}
        {showHandles &&
          !opt &&
          edit?.sites?.map(([x, y], i) => (
            <path
              key={`site${i}`}
              className={styles.site}
              d={`M${x - hs / 3} ${y - hs / 3}L${x + hs / 3} ${y + hs / 3}M${x - hs / 3} ${y + hs / 3}L${x + hs / 3} ${y - hs / 3}`}
              vectorEffect="non-scaling-stroke"
            />
          ))}
        {snapSite && (
          <circle
            className={shapeStyles.snapSite}
            data-testid="connector-snap-site"
            cx={snapSite[0]}
            cy={snapSite[1]}
            r={hs}
            vectorEffect="non-scaling-stroke"
          />
        )}
        {showHandles &&
          !opt &&
          edit?.connector?.ends.map(([x, y], i) => (
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
          !opt &&
          edit?.adjust.map((h) => (
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
        {preview?.boxes.map((b, i) =>
          boxRect(b, styles.ghost, `g${i}`, { 'data-testid': 'drag-outline' }),
        )}
        {drawBox && (
          <rect
            className={styles.drawBox}
            data-testid="draw-box"
            x={drawBox.x}
            y={drawBox.y}
            width={drawBox.w}
            height={drawBox.h}
            vectorEffect="non-scaling-stroke"
          />
        )}
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
      {editing && (
        <TextEditor
          key={`${editing.id}/${editing.cell?.join(',') ?? ''}`}
          editing={editing}
          textEdit={textEdit}
          hitSeq={hitSeq}
          scale={scale}
          label={editorLabel}
          controller={controller}
          onExit={onExitEdit}
          onShortcut={onTextShortcut}
          onCell={onStartCellEdit}
          onNextPlaceholder={onNextPlaceholder}
          list={edit?.format.numbering ? 'numbering' : edit?.format.bullets ? 'bullets' : null}
        />
      )}
    </div>
  );
}

/** Gridlines every `step` points, running out from the centre of the slide. */
function GridLines({ width, height, step }: { width: number; height: number; step: number }) {
  if (step <= 0) return null;
  const xs: number[] = [];
  const ys: number[] = [];
  for (let x = width / 2; x > 0; x -= step) xs.push(x);
  for (let x = width / 2 + step; x < width; x += step) xs.push(x);
  for (let y = height / 2; y > 0; y -= step) ys.push(y);
  for (let y = height / 2 + step; y < height; y += step) ys.push(y);
  return (
    <g className={styles.gridLines} data-testid="slide-gridlines" pointerEvents="none">
      {xs.map((x) => (
        <line key={`x${x}`} x1={x} x2={x} y1={0} y2={height} vectorEffect="non-scaling-stroke" />
      ))}
      {ys.map((y) => (
        <line key={`y${y}`} x1={0} x2={width} y1={y} y2={y} vectorEffect="non-scaling-stroke" />
      ))}
    </g>
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
  /** Tab moved to another table cell. */
  onCell?: (te: SlideTextEdit) => void;
  onNextPlaceholder?: () => void;
  /** The list kind of the paragraph with the caret (Backspace at its start removes it). */
  list: 'bullets' | 'numbering' | null;
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
  onCell,
  onNextPlaceholder,
  list,
}: TextEditorProps) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const composing = useRef(false);
  const inflight = useRef(0);
  // Set once the editor is leaving (Escape) or unmounted: a selection event fired while the
  // textarea goes away must not reach the engine after `textExit` (it would reopen text editing
  // there, and a following undo / redo would act inside that text session).
  const closed = useRef(false);
  // The user typed or pressed a key: a late caret placement from the engine (the opening
  // double-click's `textHit`) must not move the selection any more.
  const touched = useRef(false);
  // The engine's text when our last local edit was sent: a late result still carrying it (from
  // a command issued before that edit) must not overwrite what was typed since.
  const staleText = useRef<string | null>(null);
  // The texts sent since the editor opened (the latest last). The engine echoes each of them;
  // the effect that follows the engine may run after a later keystroke was already answered,
  // so an echo of an earlier keystroke must not replace what is in the textarea now.
  const sent = useRef<string[]>([]);
  const engineTextNow = useRef<string | undefined>(textEdit?.text);
  const latestEngineText = textEdit?.text;
  useEffect(() => {
    engineTextNow.current = latestEngineText;
  }, [latestEngineText]);
  useEffect(
    () => () => {
      closed.current = true;
    },
    [],
  );
  const b = editing.rect;
  const minH = Math.max(b.h, editing.size * 1.6);

  const selection = (): [number, number] => {
    const ta = ref.current;
    return ta ? [ta.selectionStart, ta.selectionEnd] : [0, 0];
  };

  const sendSelect = () => {
    if (composing.current || closed.current) return;
    const [start, end] = selection();
    void controller.run({ cmd: 'textSelect', id: editing.id, start, end, cell: editing.cell });
  };

  const sendText = () => {
    const ta = ref.current;
    if (!ta) return;
    touched.current = true;
    if (engineTextNow.current !== undefined && engineTextNow.current !== fromEditorText(ta.value))
      staleText.current = engineTextNow.current;
    inflight.current += 1;
    sent.current = [...sent.current.slice(-63), fromEditorText(ta.value)];
    void controller
      .run({
        cmd: 'setText',
        id: editing.id,
        text: fromEditorText(ta.value),
        merge: true,
        sel: selection(),
        cell: editing.cell,
      })
      .finally(() => {
        inflight.current -= 1;
      });
  };

  useEffect(() => {
    const ta = ref.current;
    if (!ta) return;
    ta.focus();
    if (editing.cell) {
      // The engine already placed the caret (or selected the cell's text).
      const [a, z] = editing.sel ?? [ta.value.length, ta.value.length];
      ta.setSelectionRange(a, z);
      return;
    }
    const n = ta.value.length;
    ta.setSelectionRange(editing.selectAll ? 0 : n, n);
    void controller.run({
      cmd: 'textSelect',
      id: editing.id,
      start: editing.selectAll ? 0 : n,
      end: n,
    });
    if (editing.insert) {
      // The keys that started the editing: typed at the end of the text (over the
      // selection when the text was opened with all of it selected).
      ta.setRangeText(editing.insert, editing.selectAll ? 0 : n, n, 'end');
      sendText();
    }
    if (editing.at) {
      void controller.run({ cmd: 'textHit', x: editing.at.x, y: editing.at.y }).then((r) => {
        const te = r?.textEdit;
        if (te && ref.current && !touched.current) ref.current.setSelectionRange(te.start, te.end);
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
    if (engineText === staleText.current) return;
    const echo = sent.current.lastIndexOf(engineText);
    if (echo >= 0 && echo < sent.current.length - 1) return;
    // Not something typed here (undo, formatting, a replacement): start over.
    if (echo < 0) sent.current = [];
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
    touched.current = true;
    if (composing.current || e.nativeEvent.isComposing) return;
    const mod = e.ctrlKey || e.metaKey;
    // Escape and F2 leave the text: the shape stays selected.
    if (e.key === 'Escape' || (e.key === 'F2' && !mod && !e.altKey && !e.shiftKey)) {
      e.preventDefault();
      closed.current = true;
      onExit();
      return;
    }
    if (
      e.key === 'Enter' &&
      mod &&
      !e.altKey &&
      !e.shiftKey &&
      !editing.cell &&
      onNextPlaceholder
    ) {
      // Ctrl+Enter: on to the next text placeholder (a new slide after the last one).
      e.preventDefault();
      closed.current = true;
      onNextPlaceholder();
      return;
    }
    if (e.key === 'Backspace' && !mod && !e.altKey && list) {
      // Backspace at the start of a bulleted / numbered paragraph first takes the bullet
      // away; the paragraph and its text stay (the next Backspace joins the paragraphs).
      const ta = e.currentTarget;
      const at = ta.selectionStart;
      if (at === ta.selectionEnd && (at === 0 || ta.value[at - 1] === '\n')) {
        e.preventDefault();
        void controller.run({ cmd: 'formatText', format: { kind: list, value: false } });
        return;
      }
    }
    if (e.key === 'Tab' && editing.cell) {
      // Tab / Shift+Tab in a table: next / previous cell, its text selected.
      e.preventDefault();
      void controller.run({ cmd: 'cellNext', back: e.shiftKey }).then((r) => {
        const te = r?.textEdit;
        if (te?.cell) onCell?.(te);
      });
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
      // Undo / redo may bring an earlier text back: follow the engine again.
      staleText.current = null;
      sent.current = [];
      void controller.run({ cmd: redo ? 'redo' : 'undo' });
      return;
    }
    if ((mod || e.key === 'F3') && onShortcut?.(e.nativeEvent)) e.preventDefault();
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
