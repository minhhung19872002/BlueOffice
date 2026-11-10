import { AppWindow, PanelLeft, PanelRight, X } from 'lucide-react';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from 'react';
import styles from './TaskPanes.module.css';

/**
 * Task panes (CORE-UI-006): one host shared by BlueDoc, BlueSheet and BlueSlide.
 * Panes dock left or right (several panes in one dock become tabs), float as a
 * window inside the app frame, resize by a splitter, and remember where they were
 * per app. F6 cycles focus between the app and the panes; Escape returns to the app.
 */

export type TaskPaneSide = 'left' | 'right';
export type TaskPaneDock = TaskPaneSide | 'float';

/** A floating pane's box, in CSS px relative to the host frame. */
export interface TaskPaneRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface TaskPanePlacement {
  dock: TaskPaneDock;
  /** Side the pane goes back to when docked again. */
  side: TaskPaneSide;
  /** Last floating box. */
  rect?: TaskPaneRect;
}

/** Remembered layout of an app's panes. */
export interface TaskPaneLayout {
  panes: Record<string, TaskPanePlacement>;
  width: Record<TaskPaneSide, number>;
  /** Selected tab of each dock. */
  active: Partial<Record<TaskPaneSide, string>>;
}

/** A pane the editor currently shows. */
export interface TaskPaneDef {
  id: string;
  title: string;
  icon?: ReactNode;
  content: ReactNode;
  /** Where the pane opens the first time (default right). */
  defaultDock?: TaskPaneSide;
  /** Test id of the pane's close button (default `taskpane-close`). */
  closeTestId?: string;
}

/** UI strings (the caller translates them). */
export interface TaskPaneLabels {
  dockLeft: string;
  dockRight: string;
  float: string;
  close: string;
  resize: string;
  move: string;
  tabs: string;
}

export interface TaskPaneStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const TASK_PANE_MIN_WIDTH = 180;
export const TASK_PANE_MAX_WIDTH = 640;
export const TASK_PANE_DEFAULT_WIDTH = 300;
const MIN_FLOAT = { w: 200, h: 140 };
const KEY_STEP = 16;

export function emptyTaskPaneLayout(): TaskPaneLayout {
  return {
    panes: {},
    width: { left: TASK_PANE_DEFAULT_WIDTH, right: TASK_PANE_DEFAULT_WIDTH },
    active: {},
  };
}

export const taskPaneStorageKey = (app: string) => `bo.taskPanes.${app}`;

const clampWidth = (w: number) =>
  Math.round(Math.min(TASK_PANE_MAX_WIDTH, Math.max(TASK_PANE_MIN_WIDTH, w)));

const isSide = (v: unknown): v is TaskPaneSide => v === 'left' || v === 'right';
const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

/** Reads a stored layout, dropping anything malformed. */
export function parseTaskPaneLayout(raw: unknown): TaskPaneLayout {
  const out = emptyTaskPaneLayout();
  if (!raw || typeof raw !== 'object') return out;
  const r = raw as Record<string, unknown>;
  const width = r.width as Record<string, unknown> | undefined;
  if (width && typeof width === 'object') {
    if (isNum(width.left)) out.width.left = clampWidth(width.left);
    if (isNum(width.right)) out.width.right = clampWidth(width.right);
  }
  const active = r.active as Record<string, unknown> | undefined;
  if (active && typeof active === 'object') {
    if (typeof active.left === 'string') out.active.left = active.left;
    if (typeof active.right === 'string') out.active.right = active.right;
  }
  const panes = r.panes as Record<string, unknown> | undefined;
  if (panes && typeof panes === 'object') {
    for (const [id, v] of Object.entries(panes)) {
      if (!v || typeof v !== 'object') continue;
      const p = v as Record<string, unknown>;
      const dock = p.dock === 'float' || isSide(p.dock) ? p.dock : null;
      if (!dock) continue;
      const side = isSide(p.side) ? p.side : dock === 'float' ? 'right' : dock;
      const placement: TaskPanePlacement = { dock, side };
      const rect = p.rect as Record<string, unknown> | undefined;
      if (rect && isNum(rect.x) && isNum(rect.y) && isNum(rect.w) && isNum(rect.h)) {
        placement.rect = {
          x: rect.x,
          y: rect.y,
          w: Math.max(MIN_FLOAT.w, rect.w),
          h: Math.max(MIN_FLOAT.h, rect.h),
        };
      }
      out.panes[id] = placement;
    }
  }
  return out;
}

function defaultStorage(): TaskPaneStorage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    // Storage blocked (private window, sandboxed frame).
    return null;
  }
}

export function loadTaskPaneLayout(
  app: string,
  storage: TaskPaneStorage | null = defaultStorage(),
): TaskPaneLayout {
  try {
    const v = storage?.getItem(taskPaneStorageKey(app));
    return v ? parseTaskPaneLayout(JSON.parse(v)) : emptyTaskPaneLayout();
  } catch {
    return emptyTaskPaneLayout();
  }
}

export function saveTaskPaneLayout(
  app: string,
  layout: TaskPaneLayout,
  storage: TaskPaneStorage | null = defaultStorage(),
) {
  try {
    storage?.setItem(taskPaneStorageKey(app), JSON.stringify(layout));
  } catch {
    // Not remembered; the layout still applies for this session.
  }
}

/** Where a pane is shown. */
export function taskPaneDock(
  layout: TaskPaneLayout,
  def: Pick<TaskPaneDef, 'id' | 'defaultDock'>,
): TaskPaneDock {
  return layout.panes[def.id]?.dock ?? def.defaultDock ?? 'right';
}

/** Docks `id` on a side (and selects its tab) or floats it in `rect`. */
export function dockTaskPane(
  layout: TaskPaneLayout,
  id: string,
  dock: TaskPaneDock,
  rect?: TaskPaneRect,
  defaultSide: TaskPaneSide = 'right',
): TaskPaneLayout {
  const prev = layout.panes[id];
  const side = isSide(dock) ? dock : (prev?.side ?? defaultSide);
  const placement: TaskPanePlacement = { dock, side };
  const keep = rect ?? prev?.rect;
  if (keep) placement.rect = keep;
  return {
    ...layout,
    panes: { ...layout.panes, [id]: placement },
    active: isSide(dock) ? { ...layout.active, [dock]: id } : layout.active,
  };
}

export function resizeTaskPaneDock(
  layout: TaskPaneLayout,
  side: TaskPaneSide,
  width: number,
): TaskPaneLayout {
  return { ...layout, width: { ...layout.width, [side]: clampWidth(width) } };
}

export function activateTaskPane(
  layout: TaskPaneLayout,
  side: TaskPaneSide,
  id: string,
): TaskPaneLayout {
  return layout.active[side] === id
    ? layout
    : { ...layout, active: { ...layout.active, [side]: id } };
}

/** Room the open docked panes take on each side of the work area. */
export function taskPaneInsets(
  layout: TaskPaneLayout,
  panes: readonly Pick<TaskPaneDef, 'id' | 'defaultDock'>[],
): Record<TaskPaneSide, number> {
  const out = { left: 0, right: 0 };
  for (const p of panes) {
    const d = taskPaneDock(layout, p);
    if (d !== 'float') out[d] = layout.width[d];
  }
  return out;
}

/** Keeps a floating box inside the frame (`bounds` 0 × 0: not measured, left as is). */
export function clampTaskPaneRect(r: TaskPaneRect, bounds: { w: number; h: number }): TaskPaneRect {
  let w = Math.max(MIN_FLOAT.w, r.w);
  let h = Math.max(MIN_FLOAT.h, r.h);
  if (bounds.w <= 0 || bounds.h <= 0) return { x: r.x, y: r.y, w, h };
  w = Math.min(w, Math.max(MIN_FLOAT.w, bounds.w));
  h = Math.min(h, Math.max(MIN_FLOAT.h, bounds.h));
  const x = Math.min(Math.max(0, r.x), Math.max(0, bounds.w - w));
  const y = Math.min(Math.max(0, r.y), Math.max(0, bounds.h - h));
  return { x: Math.round(x), y: Math.round(y), w: Math.round(w), h: Math.round(h) };
}

/** Layout state of one app's panes, remembered in `storage` (default: local storage). */
export function useTaskPaneLayout(
  app: string,
  storage?: TaskPaneStorage | null,
): [TaskPaneLayout, (next: TaskPaneLayout) => void] {
  const [store] = useState(() => (storage === undefined ? defaultStorage() : storage));
  const [layout, setLayout] = useState(() => loadTaskPaneLayout(app, store));
  const update = useCallback(
    (next: TaskPaneLayout) => {
      setLayout(next);
      saveTaskPaneLayout(app, next, store);
    },
    [app, store],
  );
  return [layout, update];
}

export interface TaskPaneHostProps {
  layout: TaskPaneLayout;
  onLayout: (next: TaskPaneLayout) => void;
  /** Open panes (the order of the tabs). */
  panes: TaskPaneDef[];
  onClose: (id: string) => void;
  labels: TaskPaneLabels;
  /** Gives the focus back to the document / grid / slide. */
  focusApp?: () => void;
  /**
   * `overlay`: docks are absolutely placed in the positioned parent (the editor
   * offsets its view by {@link taskPaneInsets}); `flex`: docks are flex items of a
   * row container (left first, right last). Floating panes need a positioned parent.
   */
  placement?: 'overlay' | 'flex';
}

type Drag =
  | { kind: 'move' | 'size'; id: string; x0: number; y0: number; rect: TaskPaneRect }
  | { kind: 'split'; side: TaskPaneSide; x0: number; w0: number };

/** Docks, tabs and floating windows of the open task panes. */
export function TaskPaneHost({
  layout,
  onLayout,
  panes,
  onClose,
  labels,
  focusApp,
  placement = 'overlay',
}: TaskPaneHostProps) {
  const layer = useRef<HTMLDivElement>(null);
  const regions = useRef(new Map<string, HTMLElement>());
  const drag = useRef<Drag | null>(null);
  // Live box / width while dragging (committed to the layout on release).
  const [live, setLive] = useState<{
    id?: string;
    rect?: TaskPaneRect;
    side?: TaskPaneSide;
    w?: number;
  }>({});
  // Opening order: a newly opened pane becomes the selected tab of its dock.
  const seen = useRef<string[]>([]);
  useEffect(() => {
    const fresh = panes.filter((p) => !seen.current.includes(p.id));
    if (fresh.length === 0 && seen.current.length === panes.length) return;
    seen.current = panes.map((p) => p.id);
    let next = layout;
    for (const p of fresh) {
      const d = taskPaneDock(next, p);
      if (d !== 'float') next = activateTaskPane(next, d, p.id);
    }
    if (next !== layout) onLayout(next);
  }, [panes, layout, onLayout]);

  const side = (s: TaskPaneSide) => panes.filter((p) => taskPaneDock(layout, p) === s);
  const groups: Record<TaskPaneSide, TaskPaneDef[]> = { left: side('left'), right: side('right') };
  const floats = panes.filter((p) => taskPaneDock(layout, p) === 'float');

  const bounds = () => {
    const r = layer.current?.parentElement?.getBoundingClientRect();
    return { w: r?.width ?? 0, h: r?.height ?? 0 };
  };

  const move = (def: TaskPaneDef, dock: TaskPaneDock) => {
    let rect: TaskPaneRect | undefined;
    if (dock === 'float') {
      const b = bounds();
      const prev = layout.panes[def.id]?.rect;
      const from = taskPaneDock(layout, def);
      const w = from === 'float' ? (prev?.w ?? 320) : layout.width[from];
      rect = clampTaskPaneRect(
        prev ?? {
          x: b.w > 0 ? b.w - w - 24 : 24,
          y: 24,
          w,
          h: b.h > 0 ? Math.min(480, b.h - 48) : 400,
        },
        b,
      );
    }
    onLayout(dockTaskPane(layout, def.id, dock, rect, def.defaultDock ?? 'right'));
  };

  const close = (id: string) => {
    const r = regions.current.get(`float:${id}`);
    const inside = !!r && r.contains(document.activeElement);
    const docked = (['left', 'right'] as const).some((s) =>
      regions.current.get(s)?.contains(document.activeElement),
    );
    onClose(id);
    if (inside || docked) focusApp?.();
  };

  // F6 / Shift+F6: app → panes → app (document capture so editors do not swallow it).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'F6' || e.ctrlKey || e.altKey || e.metaKey) return;
      const list = [...regions.current.entries()]
        .sort(([a], [b]) => regionRank(a) - regionRank(b))
        .map(([, el]) => el)
        .filter((el) => el.isConnected);
      if (list.length === 0) return;
      e.preventDefault();
      e.stopPropagation();
      const at = list.findIndex((el) => el.contains(document.activeElement));
      const n = list.length;
      // Positions 0..n-1 are panes, n is the app.
      const cur = at < 0 ? n : at;
      const next = (cur + (e.shiftKey ? n : 1)) % (n + 1);
      if (next === n) focusApp?.();
      else list[next]?.focus();
    };
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [focusApp]);

  const onRegionKey = (e: ReactKeyboardEvent) => {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      focusApp?.();
    }
  };

  const setRegion = (key: string) => (el: HTMLElement | null) => {
    if (el) regions.current.set(key, el);
    else regions.current.delete(key);
  };

  // Pointer dragging (move / resize a floating window, splitter of a dock).
  const startDrag = (e: ReactPointerEvent<HTMLElement>, d: Drag) => {
    if (e.button !== 0) return;
    e.preventDefault();
    drag.current = d;
    try {
      e.currentTarget.setPointerCapture?.(e.pointerId);
    } catch {
      // Synthetic pointer (tests): capture not available.
    }
  };
  const onDragMove = (e: ReactPointerEvent<HTMLElement>) => {
    const d = drag.current;
    if (!d) return;
    if (d.kind === 'split') {
      const dx = e.clientX - d.x0;
      setLive({ side: d.side, w: clampWidth(d.w0 + (d.side === 'left' ? dx : -dx)) });
      return;
    }
    const dx = e.clientX - d.x0;
    const dy = e.clientY - d.y0;
    const r =
      d.kind === 'move'
        ? { ...d.rect, x: d.rect.x + dx, y: d.rect.y + dy }
        : { ...d.rect, w: d.rect.w + dx, h: d.rect.h + dy };
    setLive({ id: d.id, rect: clampTaskPaneRect(r, bounds()) });
  };
  const onDragEnd = () => {
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    if (d.kind === 'split' && live.w !== undefined && live.side === d.side) {
      onLayout(resizeTaskPaneDock(layout, d.side, live.w));
    } else if (d.kind !== 'split' && live.rect && live.id === d.id) {
      onLayout(dockTaskPane(layout, d.id, 'float', live.rect));
    }
    setLive({});
  };
  const dragHandlers = {
    onPointerMove: onDragMove,
    onPointerUp: onDragEnd,
    onPointerCancel: onDragEnd,
  };

  const buttons = (def: TaskPaneDef, dock: TaskPaneDock) => (
    <span className={styles.buttons}>
      {dock !== 'left' && (
        <button
          type="button"
          className={styles.tool}
          title={labels.dockLeft}
          aria-label={labels.dockLeft}
          data-testid="taskpane-to-left"
          onClick={() => move(def, 'left')}
        >
          <PanelLeft size={14} aria-hidden />
        </button>
      )}
      {dock !== 'right' && (
        <button
          type="button"
          className={styles.tool}
          title={labels.dockRight}
          aria-label={labels.dockRight}
          data-testid="taskpane-to-right"
          onClick={() => move(def, 'right')}
        >
          <PanelRight size={14} aria-hidden />
        </button>
      )}
      {dock !== 'float' && (
        <button
          type="button"
          className={styles.tool}
          title={labels.float}
          aria-label={labels.float}
          data-testid="taskpane-to-float"
          onClick={() => move(def, 'float')}
        >
          <AppWindow size={14} aria-hidden />
        </button>
      )}
      <button
        type="button"
        className={styles.tool}
        title={labels.close}
        aria-label={labels.close}
        data-testid={def.closeTestId ?? 'taskpane-close'}
        onClick={() => close(def.id)}
      >
        <X size={14} aria-hidden />
      </button>
    </span>
  );

  const dockRegion = (s: TaskPaneSide) => {
    const group = groups[s];
    if (group.length === 0) return null;
    const active = group.find((p) => p.id === layout.active[s]) ?? group[group.length - 1]!;
    const width = live.side === s && live.w !== undefined ? live.w : layout.width[s];
    const tabbed = group.length > 1;
    return (
      <section
        key={s}
        ref={setRegion(s)}
        role="complementary"
        aria-label={active.title}
        tabIndex={-1}
        className={`${styles.dock} ${styles[placement]} ${styles[s]}`}
        style={{ width }}
        data-testid={`taskpane-dock-${s}`}
        data-dock={s}
        data-active={active.id}
        onKeyDown={onRegionKey}
      >
        <header className={styles.header}>
          {tabbed ? (
            <div className={styles.tabs} role="tablist" aria-label={labels.tabs}>
              {group.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  role="tab"
                  id={`taskpane-tab-${p.id}`}
                  aria-selected={p.id === active.id}
                  aria-controls={`taskpane-panel-${p.id}`}
                  tabIndex={p.id === active.id ? 0 : -1}
                  className={styles.tab}
                  data-testid={`taskpane-tab-${p.id}`}
                  onClick={() => onLayout(activateTaskPane(layout, s, p.id))}
                  onKeyDown={(e) => {
                    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
                    e.preventDefault();
                    const i = group.indexOf(p);
                    const next =
                      group[(i + (e.key === 'ArrowRight' ? 1 : group.length - 1)) % group.length]!;
                    onLayout(activateTaskPane(layout, s, next.id));
                    const tab = e.currentTarget.parentElement?.querySelector<HTMLElement>(
                      `#taskpane-tab-${CSS.escape(next.id)}`,
                    );
                    requestAnimationFrame(() => tab?.focus());
                  }}
                >
                  {p.icon && <span className={styles.icon}>{p.icon}</span>}
                  {p.title}
                </button>
              ))}
            </div>
          ) : (
            <h2 className={styles.title}>
              {active.icon && <span className={styles.icon}>{active.icon}</span>}
              {active.title}
            </h2>
          )}
          {buttons(active, s)}
        </header>
        {group.map((p) => (
          <div
            key={p.id}
            id={`taskpane-panel-${p.id}`}
            role={tabbed ? 'tabpanel' : undefined}
            aria-labelledby={tabbed ? `taskpane-tab-${p.id}` : undefined}
            hidden={p.id !== active.id}
            className={styles.body}
            data-taskpane={p.id}
          >
            {p.content}
          </div>
        ))}
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label={labels.resize}
          aria-valuenow={width}
          aria-valuemin={TASK_PANE_MIN_WIDTH}
          aria-valuemax={TASK_PANE_MAX_WIDTH}
          tabIndex={0}
          className={styles.splitter}
          data-testid={`taskpane-splitter-${s}`}
          onPointerDown={(e) => startDrag(e, { kind: 'split', side: s, x0: e.clientX, w0: width })}
          {...dragHandlers}
          onKeyDown={(e) => {
            const grow = s === 'left' ? 'ArrowRight' : 'ArrowLeft';
            const shrink = s === 'left' ? 'ArrowLeft' : 'ArrowRight';
            const step = e.shiftKey ? KEY_STEP * 4 : KEY_STEP;
            let w: number | null = null;
            if (e.key === grow) w = width + step;
            else if (e.key === shrink) w = width - step;
            else if (e.key === 'Home') w = TASK_PANE_MIN_WIDTH;
            else if (e.key === 'End') w = TASK_PANE_MAX_WIDTH;
            if (w === null) return;
            e.preventDefault();
            e.stopPropagation();
            onLayout(resizeTaskPaneDock(layout, s, w));
          }}
        />
      </section>
    );
  };

  const floatRegion = (def: TaskPaneDef) => {
    const stored = layout.panes[def.id]?.rect ?? { x: 24, y: 24, w: 320, h: 400 };
    const rect = live.id === def.id && live.rect ? live.rect : stored;
    return (
      <section
        key={def.id}
        ref={setRegion(`float:${def.id}`)}
        role="complementary"
        aria-label={def.title}
        tabIndex={-1}
        className={styles.float}
        style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
        data-testid={`taskpane-float-${def.id}`}
        data-dock="float"
        onKeyDown={onRegionKey}
      >
        <header
          className={`${styles.header} ${styles.handle}`}
          title={labels.move}
          onPointerDown={(e) => {
            if ((e.target as HTMLElement).closest('button')) return;
            startDrag(e, { kind: 'move', id: def.id, x0: e.clientX, y0: e.clientY, rect });
          }}
          onDoubleClick={(e) => {
            if ((e.target as HTMLElement).closest('button')) return;
            move(def, layout.panes[def.id]?.side ?? def.defaultDock ?? 'right');
          }}
          {...dragHandlers}
          data-testid="taskpane-handle"
        >
          <h2 className={styles.title}>
            {def.icon && <span className={styles.icon}>{def.icon}</span>}
            {def.title}
          </h2>
          {buttons(def, 'float')}
        </header>
        <div className={styles.body} data-taskpane={def.id}>
          {def.content}
        </div>
        <div
          className={styles.grip}
          aria-hidden
          data-testid="taskpane-grip"
          onPointerDown={(e) =>
            startDrag(e, { kind: 'size', id: def.id, x0: e.clientX, y0: e.clientY, rect })
          }
          {...dragHandlers}
        />
      </section>
    );
  };

  return (
    <>
      {dockRegion('left')}
      {dockRegion('right')}
      <div ref={layer} className={styles.layer} data-testid="taskpane-layer">
        {floats.map(floatRegion)}
      </div>
    </>
  );
}

/** F6 order: left dock, right dock, then floating windows. */
function regionRank(key: string): number {
  return key === 'left' ? 0 : key === 'right' ? 1 : 2;
}
