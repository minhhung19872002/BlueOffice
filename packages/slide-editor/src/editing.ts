import type {
  HandleKind,
  SlideBox,
  SlideCommand,
  SlideHandle,
  SlideRect,
  SlideShapeBoxes,
  SlideTransform,
} from '@blueoffice/platform';

/** Line break inside a paragraph in the engine's editing format (§21.1.2.2.1 br). */
export const ENGINE_BR = '\u000b';
/** How a line break is shown in the text editor (a textarea line break that is not a paragraph). */
export const EDITOR_BR = ' ';

/** Engine text → textarea value. */
export function toEditorText(text: string): string {
  return text.split(ENGINE_BR).join(EDITOR_BR);
}

/** Textarea value → engine text. */
export function fromEditorText(text: string): string {
  return text.replace(/\r\n?/g, '\n').split(EDITOR_BR).join(ENGINE_BR);
}

/**
 * Keyboard steps in points. A plain arrow moves by one step of the default grid
 * (1/12 inch); Ctrl makes it a fine step.
 */
export const GRID_STEP = 6;
export const FINE_STEP = 1;
/** Alt+Left / Alt+Right turn the selection by this many degrees (1° with Ctrl). */
export const ROTATE_STEP = 15;

/** Nudge step in points (Ctrl = fine step). */
export function nudgeStep(fine: boolean): number {
  return fine ? FINE_STEP : GRID_STEP;
}

/** Modifier keys of an arrow key press. */
export interface ArrowMods {
  shift?: boolean;
  ctrl?: boolean;
  alt?: boolean;
}

/** What the arrow keys need to know about the selection (rotation needs its centre). */
export interface ArrowTarget {
  cx: number;
  cy: number;
  rot: number;
}

/**
 * Arrow key → command for the selected objects, the way presentation programs do it:
 * arrows move by a grid step (Ctrl: fine step), Shift+arrows resize (→ wider, ← narrower,
 * ↑ taller, ↓ shorter; Ctrl: fine step) and Alt+← / Alt+→ rotate by 15° (`one` is the
 * single selected object; several objects are not rotated from the keyboard).
 */
export function nudgeCommand(
  key: string,
  mods: ArrowMods = {},
  one?: ArrowTarget | null,
): SlideCommand | null {
  const s = nudgeStep(!!mods.ctrl);
  const d: Record<string, [number, number]> = {
    ArrowLeft: [-1, 0],
    ArrowRight: [1, 0],
    ArrowUp: [0, -1],
    ArrowDown: [0, 1],
  };
  const v = d[key];
  if (!v) return null;
  if (mods.alt) {
    if (!one || v[0] === 0) return null;
    const turn = (mods.ctrl ? 1 : ROTATE_STEP) * v[0];
    const a = ((one.rot + turn) * Math.PI) / 180;
    // The rotation handle sits above the centre: a pointer there gives the angle.
    const r = 100;
    return {
      cmd: 'transform',
      transform: { kind: 'rotate', x: one.cx + r * Math.sin(a), y: one.cy - r * Math.cos(a) },
      snap: false,
    };
  }
  if (mods.shift) {
    // The right / bottom edge moves, the top-left corner stays.
    const transform: SlideTransform =
      v[0] !== 0
        ? { kind: 'resize', handle: 'e', dx: v[0] * s, dy: 0 }
        : { kind: 'resize', handle: 's', dx: 0, dy: -v[1] * s };
    return { cmd: 'transform', transform, snap: false };
  }
  return {
    cmd: 'transform',
    transform: { kind: 'move', dx: v[0] * s, dy: v[1] * s },
    snap: false,
  };
}

/**
 * A drag with Shift held moves along one axis only: the smaller of the two
 * offsets is dropped.
 */
export function constrainMove(dx: number, dy: number, shift: boolean): { dx: number; dy: number } {
  if (!shift) return { dx, dy };
  return Math.abs(dx) >= Math.abs(dy) ? { dx, dy: 0 } : { dx: 0, dy };
}

/** A key that types a character (starts text editing on a selected shape). */
export function isTypingKey(e: {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
}): boolean {
  return e.key.length === 1 && e.key !== ' ' && !e.ctrlKey && !e.metaKey && !e.altKey;
}

/** Normalise a colour input value (`#rrggbb`) to the engine's `RRGGBB`. */
export function hexOf(value: string): string | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(value.trim());
  return m ? m[1]!.toUpperCase() : null;
}

/** The sizes of the font size list, in points. */
export const FONT_SIZES = [
  8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 44, 48, 54, 60, 66, 72, 80, 88, 96,
];

/**
 * "Increase / decrease font size": the next / previous size of the list (44 → 48, not
 * 46); past the list it goes on in multiples of 10 points, below it by 1 point.
 */
export function steppedSize(size: number, up: boolean): number {
  const first = FONT_SIZES[0]!;
  const last = FONT_SIZES[FONT_SIZES.length - 1]!;
  if (up) {
    const next = FONT_SIZES.find((v) => v > size + 1e-6);
    return next ?? Math.min(4000, (size >= last ? Math.floor(size / 10) * 10 : last) + 10);
  }
  if (size > last + 10) return Math.ceil(size / 10) * 10 - 10;
  const prev = [...FONT_SIZES].reverse().find((v) => v < size - 1e-6);
  return prev ?? Math.max(1, Math.ceil(size) - 1 < first ? Math.ceil(size) - 1 : first);
}

const WORD_CHAR = /[\p{L}\p{N}\p{M}_'’]/u;

/** The word around offset `at` (double-click): `[start, end]`, empty when `at` is not in a word. */
export function wordRange(text: string, at: number): [number, number] {
  let a = Math.max(0, Math.min(at, text.length));
  let z = a;
  while (a > 0 && WORD_CHAR.test(text[a - 1]!)) a--;
  while (z < text.length && WORD_CHAR.test(text[z]!)) z++;
  return [a, z];
}

/** The paragraph around offset `at` (triple-click), without its paragraph mark. */
export function paragraphRange(text: string, at: number): [number, number] {
  const p = Math.max(0, Math.min(at, text.length));
  const a = text.lastIndexOf('\n', p - 1) + 1;
  const z = text.indexOf('\n', p);
  return [a, z < 0 ? text.length : z];
}

/** Is a slide point inside a (rotated) box, `tol` points around it included? */
export function boxContains(b: SlideBox, x: number, y: number, tol = 0): boolean {
  const a = (-b.rot * Math.PI) / 180;
  const dx = x - b.cx;
  const dy = y - b.cy;
  const lx = dx * Math.cos(a) - dy * Math.sin(a);
  const ly = dx * Math.sin(a) + dy * Math.cos(a);
  return Math.abs(lx) <= b.w / 2 + tol && Math.abs(ly) <= b.h / 2 + tol;
}

/**
 * The top-level shape a click at a slide point hits, from the engine's `shapeBoxes`: the
 * topmost one with a box (a group: one of its shapes' boxes) around the point — the engine's
 * own hit test, run in the editor so the selection is drawn before the engine answers.
 */
export function hitShape(boxes: SlideShapeBoxes, x: number, y: number): number | null {
  for (let i = boxes.shapes.length - 1; i >= 0; i--) {
    const s = boxes.shapes[i]!;
    if (s.boxes.some((b) => boxContains(b, x, y, boxes.tolerance))) return s.id;
  }
  return null;
}

/** Distance of the rotation handle above the top edge, points (as the engine places it). */
export const ROT_HANDLE_OFFSET = 18;

/** The eight sizing handles and the rotation handle of a box (the engine's `handles`). */
export function boxHandles(b: SlideBox): SlideHandle[] {
  const r = (b.rot * Math.PI) / 180;
  const at = (lx: number, ly: number) => ({
    x: b.cx + lx * Math.cos(r) - ly * Math.sin(r),
    y: b.cy + lx * Math.sin(r) + ly * Math.cos(r),
  });
  const kinds: [HandleKind, number, number][] = [
    ['nw', -1, -1],
    ['n', 0, -1],
    ['ne', 1, -1],
    ['e', 1, 0],
    ['se', 1, 1],
    ['s', 0, 1],
    ['sw', -1, 1],
    ['w', -1, 0],
  ];
  const out: SlideHandle[] = kinds.map(([kind, sx, sy]) => ({
    kind,
    ...at((sx * b.w) / 2, (sy * b.h) / 2),
  }));
  out.push({ kind: 'rot', ...at(0, -b.h / 2 - ROT_HANDLE_OFFSET) });
  return out;
}

/** Default size of a shape inserted with a click (no drag), points (1 inch). */
export const CLICK_SHAPE_SIZE = 72;
/** Width of a text box inserted with a click, points (its height follows its text). */
export const CLICK_TEXT_WIDTH = 144;
export const CLICK_TEXT_HEIGHT = 30;

/**
 * Box of a shape drawn on the slide from `a` to `b` (slide points): a click (no drag) puts a
 * default-size shape with its top-left corner at the point, Shift keeps the drag square, a
 * text box drawn by dragging takes the dragged width.
 */
export function drawnRect(
  a: { x: number; y: number },
  b: { x: number; y: number } | null,
  opts: { square?: boolean; text?: boolean } = {},
): SlideRect {
  if (!b) {
    return opts.text
      ? { x: a.x, y: a.y, w: CLICK_TEXT_WIDTH, h: CLICK_TEXT_HEIGHT }
      : { x: a.x, y: a.y, w: CLICK_SHAPE_SIZE, h: CLICK_SHAPE_SIZE };
  }
  let w = Math.abs(b.x - a.x);
  let h = Math.abs(b.y - a.y);
  if (opts.square) w = h = Math.max(w, h);
  const x = b.x < a.x ? a.x - w : a.x;
  const y = b.y < a.y ? a.y - h : a.y;
  if (opts.text) h = Math.max(h, CLICK_TEXT_HEIGHT);
  return { x, y, w: Math.max(w, 1), h: Math.max(h, 1) };
}

/**
 * Shift+F3: the next case of a text — lower case → UPPER CASE → Each Word Capitalised → lower
 * case (text in any other case goes to UPPER CASE first).
 */
export function nextCase(text: string, lang?: string): string {
  const upper = text.toLocaleUpperCase(lang);
  const lower = text.toLocaleLowerCase(lang);
  const title = lower.replace(
    /(^|[^\p{L}\p{N}'’])(\p{L})/gu,
    (_, a: string, c: string) => a + c.toLocaleUpperCase(lang),
  );
  if (text === lower && text !== upper) return upper;
  if (text === upper && text !== lower) return title;
  if (text === title) return lower;
  return upper;
}

/** `v` snapped to the nearest multiple of `step` when within `tol`, else unchanged. */
export function snapTo(v: number, step: number, tol: number): number {
  const s = Math.round(v / step) * step;
  return Math.abs(s - v) <= tol ? s : v;
}
