import type {
  SlideSmartArtColors,
  SlideSmartArtItem,
  SlideSmartArtLayout,
  SlideSmartArtStyle,
  SmartArtPreview,
} from '@blueoffice/platform';
import { Popover, type RibbonGroup, type RibbonItem } from '@blueoffice/ui';
import {
  ArrowDown,
  ArrowLeftRight,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  LayoutGrid,
  ListIndentDecrease,
  ListIndentIncrease,
  Paintbrush,
  Palette,
  PanelLeft,
  RotateCcw,
  Shapes,
  SquarePlus,
  Type,
  X,
} from 'lucide-react';
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from 'react';
import { editOutline, SMARTART_LIBRARY, type SmartArtNodeOp } from './SmartArtDialog';
import styles from './SmartArtTools.module.css';

/**
 * The SmartArt Design tools shared by BlueDoc, BlueSheet and BlueSlide (CORE-SMART-004): the text
 * pane (a pane beside the diagram, opened from the arrow on its frame) and the SmartArt Design
 * ribbon tab — Add Shape (after / before / above / below / assistant), Promote / Demote, Move Up /
 * Down, Right to Left, the layout gallery with live preview, Change Colours, SmartArt Styles, Reset
 * Graphic and Convert (to shapes; to text in BlueSlide). Every action is one `setSmartArt` (or
 * conversion) command of the app's engine: one undo step.
 */

/** The diagram the tools act on. */
export interface SmartArtState {
  layout: SlideSmartArtLayout;
  items: SlideSmartArtItem[];
  colors: SlideSmartArtColors;
  style: SlideSmartArtStyle;
  rtl?: boolean;
}

/** A change of the diagram (absent fields keep their value). */
export type SmartArtPatch = Partial<SmartArtState>;

/** Actions on the current shape of a diagram (the node the text pane / canvas has). */
export type SmartArtAction =
  | 'addAfter'
  | 'addBefore'
  | 'addAbove'
  | 'addBelow'
  | 'addAssistant'
  | 'promote'
  | 'demote'
  | 'up'
  | 'down'
  | 'remove';

export const SMARTART_ADD_ACTIONS: SmartArtAction[] = [
  'addAfter',
  'addBefore',
  'addAbove',
  'addBelow',
  'addAssistant',
];

const OUTLINE_OP: Partial<Record<SmartArtAction, SmartArtNodeOp>> = {
  promote: 'promote',
  demote: 'demote',
  up: 'up',
  down: 'down',
  remove: 'remove',
};

/** Levels as the engine reads them: the first node at the top, no node more than one deeper. */
function clamp(items: SlideSmartArtItem[]): SlideSmartArtItem[] {
  let prev = -1;
  return items.map((i) => {
    const level = Math.max(0, Math.min(i.level, prev + 1));
    prev = level;
    return level === i.level ? i : { ...i, level };
  });
}

/** The index after node `i` and its sub-nodes. */
function subtreeEnd(items: SlideSmartArtItem[], i: number): number {
  const level = items[i]?.level ?? 0;
  let j = i + 1;
  while (j < items.length && (items[j]?.level ?? 0) > level) j++;
  return j;
}

/** Can `action` apply to node `node` (`null`: the whole graphic is selected, no shape)? */
export function canSmartArtAction(
  s: Pick<SmartArtState, 'items' | 'layout'>,
  node: number | null,
  action: SmartArtAction,
): boolean {
  const items = clamp(s.items);
  const cur = node === null ? undefined : items[node];
  switch (action) {
    case 'addAfter':
    case 'addBefore':
      return true;
    case 'addAbove':
    case 'addBelow':
      return !!cur;
    case 'addAssistant':
      // Organisation charts only, below a shape.
      return !!cur && s.layout.startsWith('hierarchy');
    case 'remove':
      return !!cur && items.length > 1;
    default: {
      const op = OUTLINE_OP[action];
      if (!cur || !op || node === null) return false;
      const r = editOutline(items, node, op);
      return JSON.stringify(r.items) !== JSON.stringify(items);
    }
  }
}

/**
 * Apply an action of the SmartArt Design tab to the outline at node `node` (black-box Office
 * behaviour): Add Shape After / Before puts an empty shape at the same level after the node (and its
 * sub-shapes) / right before it — at the end / start of the top level when no shape is selected;
 * Above puts a new shape in its place with the node (and its sub-shapes) one level down under it;
 * Below adds an empty last child; Assistant an empty assistant first under it. Promote / Demote /
 * Move Up / Down take the node's sub-shapes along. Returns the new outline and the shape selected
 * afterwards (the new one), or `null` when the action does not apply.
 */
export function smartArtAction(
  items: SlideSmartArtItem[],
  node: number | null,
  action: SmartArtAction,
): { items: SlideSmartArtItem[]; node: number } | null {
  const src = clamp(items);
  const cur = node === null ? undefined : src[node];
  const blank = (level: number, assistant = false): SlideSmartArtItem =>
    assistant ? { text: '', level, assistant } : { text: '', level };
  if (node === null || !cur) {
    if (action === 'addAfter') return { items: [...src, blank(0)], node: src.length };
    if (action === 'addBefore') return { items: [blank(0), ...src], node: 0 };
    return null;
  }
  const end = subtreeEnd(src, node);
  switch (action) {
    case 'addAfter':
      return {
        items: [...src.slice(0, end), blank(cur.level), ...src.slice(end)],
        node: end,
      };
    case 'addBefore':
      return { items: [...src.slice(0, node), blank(cur.level), ...src.slice(node)], node };
    case 'addAbove': {
      const moved = src.slice(node, end).map((i) => ({ ...i, level: i.level + 1 }));
      return {
        items: [...src.slice(0, node), blank(cur.level), ...moved, ...src.slice(end)],
        node,
      };
    }
    case 'addBelow':
      return {
        items: [...src.slice(0, end), blank(cur.level + 1), ...src.slice(end)],
        node: end,
      };
    case 'addAssistant':
      return {
        items: [...src.slice(0, node + 1), blank(cur.level + 1, true), ...src.slice(node + 1)],
        node: node + 1,
      };
    case 'remove':
      if (src.length < 2) return null;
      break;
    default:
      break;
  }
  const op = OUTLINE_OP[action];
  if (!op) return null;
  const r = editOutline(src, node, op);
  if (JSON.stringify(r.items) === JSON.stringify(src)) return null;
  return { items: r.items, node: r.index };
}

// --------------------------------------------------------------------------------- text pane

/** Strings of the text pane. */
export interface SmartArtPaneLabels {
  title: string;
  close: string;
  /** Accessible name of the line of node `n` (1-based). */
  node: (n: number) => string;
  /** Shown in an empty line. */
  placeholder: string;
}

/**
 * The outline after a key of the text pane on line `i` with the caret at `caret` (Office text pane):
 * Enter splits the line into a new shape at the same level; Tab / Shift+Tab demote / promote the
 * shape (with its sub-shapes); Backspace at the start of an empty line deletes the shape (at the
 * start of a line with text, the text joins the line above); Delete at the end of a line joins the
 * line below. Returns the outline, the line and caret to go to, or `null` (the key is not handled).
 */
export function paneKey(
  items: SlideSmartArtItem[],
  i: number,
  caret: number,
  key: 'Enter' | 'Tab' | 'ShiftTab' | 'Backspace' | 'Delete',
  selection = 0,
): { items: SlideSmartArtItem[]; line: number; caret: number } | null {
  const cur = items[i];
  if (!cur) return null;
  switch (key) {
    case 'Enter': {
      const before = cur.text.slice(0, caret);
      const after = cur.text.slice(caret + selection);
      const next = [...items];
      next[i] = { ...cur, text: before };
      next.splice(i + 1, 0, { text: after, level: cur.level });
      return { items: next, line: i + 1, caret: 0 };
    }
    case 'Tab':
    case 'ShiftTab': {
      const r = smartArtAction(items, i, key === 'Tab' ? 'demote' : 'promote');
      return r ? { items: r.items, line: r.node, caret } : { items, line: i, caret };
    }
    case 'Backspace': {
      if (caret !== 0 || selection !== 0) return null;
      if (cur.text === '') {
        if (items.length < 2) return null;
        const next = clamp(items.filter((_, k) => k !== i));
        const line = Math.max(0, i - 1);
        return { items: next, line, caret: i > 0 ? (next[line]?.text.length ?? 0) : 0 };
      }
      const prev = items[i - 1];
      if (!prev) return null;
      const next = [...items];
      next[i - 1] = { ...prev, text: prev.text + cur.text };
      next.splice(i, 1);
      return { items: clamp(next), line: i - 1, caret: prev.text.length };
    }
    case 'Delete': {
      const below = items[i + 1];
      if (caret !== cur.text.length || selection !== 0 || !below) return null;
      const next = [...items];
      next[i] = { ...cur, text: cur.text + below.text };
      next.splice(i + 1, 1);
      return { items: clamp(next), line: i, caret };
    }
  }
}

/** Where the pane goes: beside the diagram's frame (viewport px). */
export interface SmartArtPaneAnchor {
  left: number;
  top: number;
  width: number;
  height: number;
}

const PANE_WIDTH = 240;

/** Pane position: left of the frame, else right of it, inside the window. */
export function panePosition(a: SmartArtPaneAnchor | null, vw: number, vh: number): CSSProperties {
  if (!a) return { left: 16, top: Math.max(8, vh / 4) };
  const left =
    a.left - PANE_WIDTH - 12 >= 4
      ? a.left - PANE_WIDTH - 12
      : Math.min(vw - PANE_WIDTH - 4, a.left + a.width + 12);
  return { left: Math.max(4, left), top: Math.max(4, Math.min(a.top, vh - 260)) };
}

/** Typing commits after this pause (one undo step per burst of typing). */
export const PANE_COMMIT_MS = 700;

/**
 * The text pane: one line per shape, indented by level, edited in place. Every structural key
 * (Enter, Tab, Shift+Tab, Backspace / Delete joining lines) is committed at once; typing is committed
 * after a pause, when the pane loses focus or when it closes.
 */
export function SmartArtTextPane({
  items,
  node,
  onNode,
  onCommit,
  onClose,
  anchor,
  labels,
}: {
  items: SlideSmartArtItem[];
  /** The current shape (its line has the caret). */
  node: number | null;
  onNode: (node: number) => void;
  /** Apply a new outline (one undo step); `node`: the line with the caret. */
  onCommit: (items: SlideSmartArtItem[], node: number) => void;
  onClose: () => void;
  /** Where the diagram is on screen (read when the pane opens and when the window resizes). */
  anchor: () => SmartArtPaneAnchor | null;
  labels: SmartArtPaneLabels;
}) {
  const [draft, setDraft] = useState(items);
  const draftRef = useRef(items);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const dirty = useRef(false);
  const lineRefs = useRef<(HTMLInputElement | null)[]>([]);
  const focusTo = useRef<{ line: number; caret: number } | null>(null);
  const commitRef = useRef(onCommit);
  const nodeRef = useRef(node);
  useLayoutEffect(() => {
    commitRef.current = onCommit;
    nodeRef.current = node;
  });
  // The engine's outline replaces the draft unless typing is waiting to be committed.
  const key = JSON.stringify(items);
  useEffect(() => {
    if (dirty.current) return;
    draftRef.current = items;
    setDraft(items);
    // `key` stands for `items`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const flush = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    if (!dirty.current) return;
    dirty.current = false;
    commitRef.current(draftRef.current, nodeRef.current ?? 0);
  };
  useEffect(() => flush, []);
  useLayoutEffect(() => {
    const f = focusTo.current;
    if (!f) return;
    focusTo.current = null;
    const el = lineRefs.current[f.line];
    if (el) {
      el.focus();
      el.setSelectionRange(f.caret, f.caret);
    }
  });
  const set = (next: SlideSmartArtItem[], commit: boolean, line: number, caret?: number) => {
    draftRef.current = next;
    setDraft(next);
    if (caret !== undefined) focusTo.current = { line, caret };
    if (line !== nodeRef.current) onNode(line);
    nodeRef.current = line;
    if (commit) {
      if (timer.current) clearTimeout(timer.current);
      timer.current = null;
      dirty.current = false;
      commitRef.current(next, line);
    } else {
      dirty.current = true;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(flush, PANE_COMMIT_MS);
    }
  };
  const [place, setPlace] = useState<CSSProperties>({ left: 16, top: 120 });
  const anchorRef = useRef(anchor);
  useLayoutEffect(() => {
    anchorRef.current = anchor;
  });
  useLayoutEffect(() => {
    const at = () =>
      setPlace(panePosition(anchorRef.current(), window.innerWidth, window.innerHeight));
    at();
    window.addEventListener('resize', at);
    return () => window.removeEventListener('resize', at);
  }, []);
  return (
    <section
      className={styles.pane}
      style={{ ...place, width: PANE_WIDTH }}
      aria-label={labels.title}
      data-testid="smartart-pane"
      data-f6-region
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) flush();
      }}
    >
      <header className={styles.head}>
        <span>{labels.title}</span>
        <button
          type="button"
          className={styles.close}
          aria-label={labels.close}
          title={labels.close}
          data-testid="smartart-pane-close"
          onClick={() => {
            flush();
            onClose();
          }}
        >
          <X size={14} aria-hidden />
        </button>
      </header>
      <ul className={styles.lines}>
        {draft.map((it, i) => (
          <li
            key={i}
            className={styles.line}
            style={{ paddingLeft: 4 + it.level * 16 }}
            data-level={it.level}
            data-assistant={it.assistant ? 'true' : undefined}
          >
            <span className={styles.bullet} aria-hidden="true">
              {it.level === 0 ? '•' : '–'}
            </span>
            <input
              ref={(el) => {
                lineRefs.current[i] = el;
              }}
              className={i === node ? `${styles.input} ${styles.current}` : styles.input}
              value={it.text}
              placeholder={labels.placeholder}
              aria-label={labels.node(i + 1)}
              data-testid={`smartart-pane-line-${i}`}
              spellCheck={false}
              onFocus={() => {
                if (nodeRef.current !== i) {
                  nodeRef.current = i;
                  onNode(i);
                }
              }}
              onChange={(e) => {
                const next = [...draftRef.current];
                const was = next[i];
                if (!was) return;
                next[i] = { ...was, text: e.target.value };
                set(next, false, i);
              }}
              onKeyDown={(e) => {
                const el = e.currentTarget;
                const caret = el.selectionStart ?? 0;
                const sel = (el.selectionEnd ?? caret) - caret;
                if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
                  const to = i + (e.key === 'ArrowUp' ? -1 : 1);
                  const target = lineRefs.current[to];
                  if (target) {
                    e.preventDefault();
                    target.focus();
                    const c = Math.min(caret, target.value.length);
                    target.setSelectionRange(c, c);
                  }
                  return;
                }
                if (e.key === 'Escape') {
                  e.preventDefault();
                  flush();
                  onClose();
                  return;
                }
                const k =
                  e.key === 'Tab'
                    ? e.shiftKey
                      ? 'ShiftTab'
                      : 'Tab'
                    : e.key === 'Enter' && !e.shiftKey
                      ? 'Enter'
                      : e.key === 'Backspace'
                        ? 'Backspace'
                        : e.key === 'Delete'
                          ? 'Delete'
                          : null;
                if (!k) return;
                const r = paneKey(draftRef.current, i, caret, k, sel);
                if (!r) {
                  if (k === 'Tab' || k === 'ShiftTab' || k === 'Enter') e.preventDefault();
                  return;
                }
                e.preventDefault();
                set(r.items, true, r.line, r.caret);
              }}
            />
          </li>
        ))}
      </ul>
    </section>
  );
}

/** The arrow on the left edge of the selected diagram's frame: shows / hides the text pane. */
export function SmartArtPaneToggle({
  open,
  label,
  style,
  onToggle,
}: {
  open: boolean;
  label: string;
  /** Position (the frame's left edge, vertically centred). */
  style: CSSProperties;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className={styles.toggle}
      style={style}
      aria-label={label}
      aria-pressed={open}
      title={label}
      data-testid="smartart-pane-toggle"
      onMouseDown={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
    >
      {open ? <ChevronRight size={12} aria-hidden /> : <ChevronLeft size={12} aria-hidden />}
    </button>
  );
}

/** A live preview over the diagram's frame (the layout / colours / style under the pointer). */
export function SmartArtLivePreview({ svg, style }: { svg: string; style: CSSProperties }) {
  return (
    <img
      className={styles.live}
      style={style}
      src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`}
      alt=""
      aria-hidden="true"
      data-testid="smartart-live-preview"
      draggable={false}
    />
  );
}

// ------------------------------------------------------------------------------ design tab

/** Strings of the SmartArt Design tab (from i18n). */
export interface SmartArtDesignLabels {
  createGroup: string;
  layoutsGroup: string;
  stylesGroup: string;
  resetGroup: string;
  addShape: string;
  action: (a: SmartArtAction) => string;
  rtl: string;
  textPane: string;
  layouts: string;
  layoutName: (l: SlideSmartArtLayout) => string;
  categoryName: (c: string) => string;
  colors: string;
  colorName: (c: SlideSmartArtColors) => string;
  styles: string;
  styleName: (s: SlideSmartArtStyle) => string;
  reset: string;
  toShapes: string;
  toText: string;
}

/** What the Design tab works with, from the app. */
export interface SmartArtToolsApi {
  /** The selected diagram (`null`: none). */
  state: () => SmartArtState | null;
  /** The current shape of the diagram (`null`: the whole graphic). */
  node: () => number | null;
  setNode: (node: number | null) => void;
  /** Apply a change: one engine command, one undo step. */
  apply: (patch: SmartArtPatch) => void | Promise<void>;
  toShapes: () => void | Promise<void>;
  /** BlueSlide only. */
  toText?: () => void | Promise<void>;
  paneOpen: () => boolean;
  togglePane: () => void;
  /** Library thumbnails (generic sample content) in colours / look. */
  loadPreviews?: (
    colors: SlideSmartArtColors,
    style: SlideSmartArtStyle,
  ) => Promise<SmartArtPreview[]>;
  /** Show the diagram drawn with `patch` over its frame while the pointer is on a gallery item
   * (`null`: end the preview). */
  preview?: (patch: SmartArtPatch | null) => void;
}

/** Colour choices of the Change Colours menu (BlueOffice palettes). */
export const SMARTART_PALETTES: SlideSmartArtColors[] = [
  'colorful',
  'blue',
  'green',
  'orange',
  'purple',
  'gray',
];

/** Swatches of each palette (`bo_pptx::diagram_layout::Palette`). */
export const SMARTART_SWATCHES: Record<string, string[]> = {
  colorful: ['#1F6FB2', '#2E8B57', '#C2571A', '#7B4BA0', '#B8860B'],
  blue: ['#1F6FB2'],
  green: ['#2E8B57'],
  orange: ['#C2571A'],
  purple: ['#7B4BA0'],
  gray: ['#5F6B7A'],
};

/**
 * A guard for answers that arrive out of order (live previews while the pointer moves): `run(p,
 * then)` calls `then` with `p`'s answer only when no later call was made meanwhile.
 */
export function latestOnly(): <T>(p: Promise<T>, then: (v: T) => void) => void {
  let n = 0;
  return (p, then) => {
    const k = ++n;
    p.then(
      (v) => {
        if (k === n) then(v);
      },
      () => undefined,
    );
  };
}

/** Command ids of the SmartArt Design tab. */
export const SMARTART_COMMANDS = {
  action: (a: SmartArtAction) => `smartart.${a}`,
  rtl: 'smartart.rtl',
  textPane: 'smartart.textPane',
  layouts: 'smartart.layouts',
  colors: 'smartart.colors',
  styles: 'smartart.styles',
  reset: 'smartart.reset',
  toShapes: 'smartart.toShapes',
  toText: 'smartart.toText',
} as const;

/** Reset Graphic: the library's default colours and look (layout, text and direction stay). */
export const SMARTART_RESET: SmartArtPatch = { colors: 'colorful', style: 'fill' };

/** Run action `a` of the Add Shape / Promote … buttons through `api`. */
export function runSmartArtAction(api: SmartArtToolsApi, a: SmartArtAction): void {
  const s = api.state();
  if (!s) return;
  const r = smartArtAction(s.items, api.node(), a);
  if (!r) return;
  api.setNode(r.node);
  void api.apply({ items: r.items });
}

/**
 * Register the SmartArt Design commands in an app's registry (`enabled(state)`: a diagram is
 * selected). `api` is read when a command runs.
 */
export function registerSmartArtTools<S>(
  reg: {
    register: (c: {
      id: string;
      run: (s: S) => void | Promise<void>;
      isEnabled?: (s: S) => boolean;
      isChecked?: (s: S) => boolean;
    }) => unknown;
  },
  api: () => SmartArtToolsApi | null,
  enabled: (s: S) => boolean,
): void {
  const can = (a: SmartArtAction) => (s: S) => {
    const x = api();
    const st = x?.state();
    return enabled(s) && !!x && !!st && canSmartArtAction(st, x.node(), a);
  };
  for (const a of [
    ...SMARTART_ADD_ACTIONS,
    'promote',
    'demote',
    'up',
    'down',
  ] as SmartArtAction[]) {
    reg.register({
      id: SMARTART_COMMANDS.action(a),
      isEnabled: can(a),
      run: () => {
        const x = api();
        if (x) runSmartArtAction(x, a);
      },
    });
  }
  reg.register({
    id: SMARTART_COMMANDS.rtl,
    isEnabled: enabled,
    isChecked: () => !!api()?.state()?.rtl,
    run: () => {
      const x = api();
      const st = x?.state();
      if (x && st) void x.apply({ rtl: !st.rtl });
    },
  });
  reg.register({
    id: SMARTART_COMMANDS.textPane,
    isEnabled: enabled,
    isChecked: () => !!api()?.paneOpen(),
    run: () => api()?.togglePane(),
  });
  reg.register({
    id: SMARTART_COMMANDS.reset,
    isEnabled: enabled,
    run: () => void api()?.apply(SMARTART_RESET),
  });
  reg.register({
    id: SMARTART_COMMANDS.toShapes,
    isEnabled: enabled,
    run: () => void api()?.toShapes(),
  });
  reg.register({
    id: SMARTART_COMMANDS.toText,
    isEnabled: (s) => enabled(s) && !!api()?.toText,
    run: () => void api()?.toText?.(),
  });
}

const svgUrl = (svg: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;

/**
 * Ends the live preview when the gallery / menu closes. The app's tools change with every edit
 * result (a preview is one), so only the last ones are kept, not followed by the effect.
 */
function useEndPreview(api: SmartArtToolsApi) {
  const latest = useRef(api);
  useLayoutEffect(() => {
    latest.current = api;
  });
  useEffect(() => () => latest.current.preview?.(null), []);
}

/** The layout gallery: the library by category; pointing at a layout previews it on the diagram. */
function LayoutGallery({
  api,
  labels,
  current,
  close,
}: {
  api: SmartArtToolsApi;
  labels: SmartArtDesignLabels;
  current: SmartArtState;
  close: () => void;
}) {
  const [thumbs, setThumbs] = useState<Map<string, string>>(new Map());
  useEndPreview(api);
  const load = api.loadPreviews;
  useEffect(() => {
    let live = true;
    const c = current.colors === 'file' ? 'colorful' : current.colors;
    const s = current.style === 'file' ? 'fill' : current.style;
    load?.(c, s).then(
      (list) => live && setThumbs(new Map(list.map((p) => [p.layout, p.svg]))),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [load, current.colors, current.style]);
  return (
    <div
      className={styles.gallery}
      role="listbox"
      aria-label={labels.layouts}
      data-testid="smartart-layouts-grid"
    >
      {SMARTART_LIBRARY.map((g) => (
        <div
          key={g.category}
          className={styles.galleryRow}
          role="group"
          aria-label={labels.categoryName(g.category)}
        >
          {g.layouts.map((l) => {
            const svg = thumbs.get(l);
            return (
              <button
                key={l}
                type="button"
                role="option"
                aria-selected={current.layout === l}
                className={styles.galleryItem}
                title={labels.layoutName(l)}
                aria-label={labels.layoutName(l)}
                data-testid={`smartart-gallery-${l}`}
                onPointerEnter={() => api.preview?.({ layout: l })}
                onFocus={() => api.preview?.({ layout: l })}
                onPointerLeave={() => api.preview?.(null)}
                onClick={() => {
                  api.preview?.(null);
                  close();
                  if (l !== current.layout) void api.apply({ layout: l });
                }}
              >
                {svg ? <img src={svgUrl(svg)} alt="" draggable={false} /> : labels.layoutName(l)}
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}

/** A menu of choices previewed on the diagram while pointed at (colours, styles). */
function PreviewMenu<T extends string>({
  api,
  choices,
  current,
  field,
  name,
  swatches,
  close,
  testId,
}: {
  api: SmartArtToolsApi;
  choices: T[];
  current: T;
  field: 'colors' | 'style';
  name: (c: T) => string;
  swatches?: Record<string, string[]>;
  close: () => void;
  testId: string;
}) {
  useEndPreview(api);
  return (
    <div className={styles.menu} role="menu" data-testid={testId}>
      {choices.map((c) => (
        <button
          key={c}
          type="button"
          role="menuitemradio"
          aria-checked={c === current}
          className={styles.menuItem}
          data-testid={`${testId}-${c}`}
          onPointerEnter={() => api.preview?.({ [field]: c })}
          onFocus={() => api.preview?.({ [field]: c })}
          onPointerLeave={() => api.preview?.(null)}
          onClick={() => {
            api.preview?.(null);
            close();
            if (c !== current) void api.apply({ [field]: c });
          }}
        >
          {swatches && (
            <span className={styles.swatches} aria-hidden="true">
              {(swatches[c] ?? []).map((hex, i) => (
                <span key={i} className={styles.swatch} style={{ background: hex }} />
              ))}
            </span>
          )}
          <span>{name(c)}</span>
        </button>
      ))}
    </div>
  );
}

/** A dropdown of the ribbon (Popover) with a label under its icon, like the large buttons. */
function Drop({
  icon,
  label,
  disabled,
  testId,
  children,
}: {
  icon: ReactNode;
  label: string;
  disabled?: boolean;
  testId: string;
  children: (close: () => void) => ReactNode;
}) {
  return (
    <Popover
      trigger={
        <span className={styles.drop}>
          {icon}
          <span>{label}</span>
        </span>
      }
      label={label}
      disabled={disabled}
      testId={testId}
    >
      {children}
    </Popover>
  );
}

/**
 * The groups of the SmartArt Design tab. `mk` builds a ribbon button for a registered command
 * (the app's own helper: enabled / checked state from its registry).
 */
export function smartArtDesignGroups({
  api,
  labels,
  mk,
  enabled,
  icon = 18,
}: {
  api: SmartArtToolsApi;
  labels: SmartArtDesignLabels;
  mk: (id: string, label: string, icon: ReactNode, size?: 'large' | 'small') => RibbonItem;
  /** Is command `id` enabled now? */
  enabled: (id: string) => boolean;
  icon?: number;
}): RibbonGroup[] {
  const st = api.state();
  const small = 16;
  const addMenu: RibbonItem = {
    id: 'smartart.addShape',
    label: labels.addShape,
    enabled: !!st,
    onSelect: () => runSmartArtAction(api, 'addAfter'),
    render: (
      <Drop
        icon={<SquarePlus size={icon} aria-hidden />}
        label={labels.addShape}
        disabled={!st}
        testId="smartart-add-shape"
      >
        {(close) => (
          <div className={styles.menu} role="menu" data-testid="smartart-add-shape-menu">
            {SMARTART_ADD_ACTIONS.map((a) => {
              const id = SMARTART_COMMANDS.action(a);
              return (
                <button
                  key={a}
                  type="button"
                  role="menuitem"
                  className={styles.menuItem}
                  disabled={!enabled(id)}
                  data-command={id}
                  data-testid={`smartart-${a}`}
                  onClick={() => {
                    close();
                    runSmartArtAction(api, a);
                  }}
                >
                  {labels.action(a)}
                </button>
              );
            })}
          </div>
        )}
      </Drop>
    ),
  };
  const layoutItem: RibbonItem = {
    id: SMARTART_COMMANDS.layouts,
    label: labels.layouts,
    enabled: !!st,
    onSelect: () => undefined,
    render: (
      <Drop
        icon={<LayoutGrid size={icon} aria-hidden />}
        label={labels.layouts}
        disabled={!st}
        testId="smartart-layouts"
      >
        {(close) =>
          st ? <LayoutGallery api={api} labels={labels} current={st} close={close} /> : null
        }
      </Drop>
    ),
  };
  const colorsItem: RibbonItem = {
    id: SMARTART_COMMANDS.colors,
    label: labels.colors,
    enabled: !!st,
    onSelect: () => undefined,
    render: (
      <Drop
        icon={<Palette size={icon} aria-hidden />}
        label={labels.colors}
        disabled={!st}
        testId="smartart-colors-menu"
      >
        {(close) =>
          st ? (
            <PreviewMenu
              api={api}
              choices={SMARTART_PALETTES}
              current={st.colors}
              field="colors"
              name={labels.colorName}
              swatches={SMARTART_SWATCHES}
              close={close}
              testId="smartart-colors-list"
            />
          ) : null
        }
      </Drop>
    ),
  };
  const stylesItem: RibbonItem = {
    id: SMARTART_COMMANDS.styles,
    label: labels.styles,
    enabled: !!st,
    onSelect: () => undefined,
    render: (
      <Drop
        icon={<Paintbrush size={icon} aria-hidden />}
        label={labels.styles}
        disabled={!st}
        testId="smartart-styles-menu"
      >
        {(close) =>
          st ? (
            <PreviewMenu
              api={api}
              choices={['fill', 'outline'] as SlideSmartArtStyle[]}
              current={st.style}
              field="style"
              name={labels.styleName}
              close={close}
              testId="smartart-styles-list"
            />
          ) : null
        }
      </Drop>
    ),
  };
  return [
    {
      id: 'smartartCreate',
      label: labels.createGroup,
      items: [
        addMenu,
        mk(
          SMARTART_COMMANDS.action('promote'),
          labels.action('promote'),
          <ListIndentDecrease size={small} aria-hidden />,
          'small',
        ),
        mk(
          SMARTART_COMMANDS.action('demote'),
          labels.action('demote'),
          <ListIndentIncrease size={small} aria-hidden />,
          'small',
        ),
        mk(SMARTART_COMMANDS.rtl, labels.rtl, <ArrowLeftRight size={small} aria-hidden />, 'small'),
        mk(
          SMARTART_COMMANDS.action('up'),
          labels.action('up'),
          <ArrowUp size={small} aria-hidden />,
          'small',
        ),
        mk(
          SMARTART_COMMANDS.action('down'),
          labels.action('down'),
          <ArrowDown size={small} aria-hidden />,
          'small',
        ),
        mk(
          SMARTART_COMMANDS.textPane,
          labels.textPane,
          <PanelLeft size={small} aria-hidden />,
          'small',
        ),
      ],
    },
    { id: 'smartartLayouts', label: labels.layoutsGroup, items: [layoutItem] },
    { id: 'smartartStyles', label: labels.stylesGroup, items: [colorsItem, stylesItem] },
    {
      id: 'smartartReset',
      label: labels.resetGroup,
      // Word / Excel: one button; the presentation app also converts to a bulleted text box.
      items: [
        mk(SMARTART_COMMANDS.reset, labels.reset, <RotateCcw size={icon} aria-hidden />),
        mk(SMARTART_COMMANDS.toShapes, labels.toShapes, <Shapes size={icon} aria-hidden />),
        ...(api.toText
          ? [mk(SMARTART_COMMANDS.toText, labels.toText, <Type size={icon} aria-hidden />)]
          : []),
      ],
    },
  ];
}

/** The Design tab strings from an i18n `t`. */
export function smartArtDesignLabels(t: (k: string, o?: Record<string, unknown>) => string): {
  design: SmartArtDesignLabels;
  pane: SmartArtPaneLabels;
} {
  const k = 'slideEditor.smartart';
  return {
    design: {
      createGroup: t(`${k}.design.create`),
      layoutsGroup: t(`${k}.design.layouts`),
      stylesGroup: t(`${k}.design.styles`),
      resetGroup: t(`${k}.design.reset`),
      addShape: t(`${k}.design.addShape`),
      action: (a) => t(`${k}.design.action.${a}`),
      rtl: t(`${k}.design.rtl`),
      textPane: t(`${k}.textPane`),
      layouts: t(`${k}.design.layoutGallery`),
      layoutName: (l) => t(`${k}.layouts.${l}`),
      categoryName: (c) => t(`${k}.categories.${c}`),
      colors: t(`${k}.design.colors`),
      colorName: (c) => t(`${k}.palette.${c}`),
      styles: t(`${k}.design.styleGallery`),
      styleName: (s) => t(`${k}.styles.${s}`),
      reset: t(`${k}.design.resetGraphic`),
      toShapes: t(`${k}.toShapes`),
      toText: t(`${k}.design.toText`),
    },
    pane: {
      title: t(`${k}.pane.title`),
      close: t(`${k}.pane.close`),
      node: (n) => t(`${k}.pane.node`, { n }),
      placeholder: t(`${k}.pane.placeholder`),
    },
  };
}
