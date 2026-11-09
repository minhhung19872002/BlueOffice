/**
 * Draw tab logic (CORE-INK-001), no DOM: tool and preset state, stroke
 * recording with pen pressure, the ruler (strokes snap to its edge), and the
 * replay timeline. Presets, widths and the ruler behaviour are BlueSlide's own
 * choices. Coordinates are slide points.
 */
import type { SlideInkObject, SlideInkStrokeIn, SlideInkStrokeOut } from '@blueoffice/platform';

export type DrawTool = 'select' | 'pen' | 'highlighter' | 'eraserStroke' | 'eraserPoint';

type Pt = [number, number];

/** Pen colour presets (`RRGGBB`). */
export const PEN_COLORS = {
  black: '1B1B1B',
  blue: '1F5FBF',
  red: 'C8261E',
  green: '1E8C3A',
  orange: 'E07B00',
  purple: '7A3DB8',
} as const;
/** Highlighter colour presets. */
export const HIGHLIGHTER_COLORS = {
  yellow: 'FFE600',
  lime: '9BE22D',
  pink: 'FF6FB5',
  aqua: '35D4F0',
} as const;
export type PenColor = keyof typeof PEN_COLORS;
export type HighlighterColor = keyof typeof HIGHLIGHTER_COLORS;

/** Widths in points per tool. */
export const WIDTHS = {
  pen: { fine: 1.5, medium: 3, bold: 6 },
  highlighter: { fine: 8, medium: 14, bold: 22 },
} as const;
export type WidthPreset = 'fine' | 'medium' | 'bold';

/** Eraser radius in points. */
export const ERASER_RADIUS = 6;
/** Pointer movements closer than this (points) are not recorded. */
export const MIN_STEP = 0.75;
/** A stroke starting within this distance (points) of the ruler edge follows it. */
export const RULER_SNAP = 18;
/** Ruler size in points (its own choice). */
export const RULER_LENGTH = 520;
export const RULER_THICKNESS = 56;

export interface DrawState {
  tool: DrawTool;
  penColor: PenColor;
  highlighterColor: HighlighterColor;
  penWidth: WidthPreset;
  highlighterWidth: WidthPreset;
  ruler: Ruler | null;
}

export const INITIAL_DRAW: DrawState = {
  tool: 'select',
  penColor: 'black',
  highlighterColor: 'yellow',
  penWidth: 'medium',
  highlighterWidth: 'medium',
  ruler: null,
};

/** Colour and width of the active pen or highlighter. */
export function brushOf(s: DrawState): { color: string; width: number; highlighter: boolean } {
  if (s.tool === 'highlighter') {
    return {
      color: HIGHLIGHTER_COLORS[s.highlighterColor],
      width: WIDTHS.highlighter[s.highlighterWidth],
      highlighter: true,
    };
  }
  return { color: PEN_COLORS[s.penColor], width: WIDTHS.pen[s.penWidth], highlighter: false };
}

// --------------------------------------------------------------------- ruler

/** A ruler: centre and angle (degrees, clockwise on the slide). */
export interface Ruler {
  x: number;
  y: number;
  angle: number;
}

/** Unit vector along the ruler. */
function along(r: Ruler): Pt {
  const a = (r.angle * Math.PI) / 180;
  return [Math.cos(a), Math.sin(a)];
}

/**
 * The drawing edge of the ruler: the long side at `−thickness/2` across it
 * (the upper edge at angle 0). Returns its origin and direction.
 */
export function rulerEdge(r: Ruler): { o: Pt; d: Pt } {
  const d = along(r);
  // Normal pointing to the "upper" side: (sin, −cos).
  const n: Pt = [d[1], -d[0]];
  return { o: [r.x + (n[0] * RULER_THICKNESS) / 2, r.y + (n[1] * RULER_THICKNESS) / 2], d };
}

/** Signed distance of `p` from the edge line (positive away from the ruler). */
export function edgeDistance(r: Ruler, p: Pt): number {
  const { o, d } = rulerEdge(r);
  // Normal (sin, −cos) points away from the ruler body.
  return (p[0] - o[0]) * d[1] - (p[1] - o[1]) * d[0];
}

/** `p` projected on the ruler edge. */
export function projectOnEdge(r: Ruler, p: Pt): Pt {
  const { o, d } = rulerEdge(r);
  const t = (p[0] - o[0]) * d[0] + (p[1] - o[1]) * d[1];
  return [o[0] + t * d[0], o[1] + t * d[1]];
}

/** Does a stroke starting at `p` follow the ruler? */
export function snapsToRuler(r: Ruler | null, p: Pt): boolean {
  if (!r) return false;
  const { o, d } = rulerEdge(r);
  const t = (p[0] - o[0]) * d[0] + (p[1] - o[1]) * d[1];
  return Math.abs(edgeDistance(r, p)) <= RULER_SNAP && Math.abs(t) <= RULER_LENGTH / 2;
}

/** Is `p` on the ruler body (dragging moves it)? */
export function onRuler(r: Ruler, p: Pt): boolean {
  const d = along(r);
  const dx = p[0] - r.x;
  const dy = p[1] - r.y;
  const t = dx * d[0] + dy * d[1];
  const n = dx * d[1] - dy * d[0];
  return Math.abs(t) <= RULER_LENGTH / 2 && Math.abs(n) <= RULER_THICKNESS / 2;
}

/** Angle (degrees, −180..180) of the pointer around the ruler centre (rotation knob). */
export function angleTo(r: Ruler, p: Pt): number {
  return (Math.atan2(p[1] - r.y, p[0] - r.x) * 180) / Math.PI;
}

/** Ruler angle rounded to whole degrees, snapping to multiples of 15° within 2°. */
export function snapAngle(a: number): number {
  let v = ((((a + 180) % 360) + 360) % 360) - 180;
  const k = Math.round(v / 15) * 15;
  if (Math.abs(v - k) <= 2) v = k;
  // + 0 turns -0 into 0.
  return Math.round(v) + 0;
}

// ----------------------------------------------------------------- recording

/** Records one stroke: points, pressure (pen input only) and times. */
export class StrokeRecorder {
  readonly points: Pt[] = [];
  readonly pressure: number[] = [];
  readonly times: number[] = [];
  private readonly withPressure: boolean;
  private readonly ruler: Ruler | null;

  /**
   * `pressure` is used for pen input only (a mouse reports 0.5 while pressed);
   * with a `ruler` the stroke follows its edge.
   */
  constructor(
    p: Pt,
    t0: number,
    pressure: number | null,
    ruler: Ruler | null,
    private readonly start = t0,
  ) {
    this.withPressure = pressure !== null;
    this.ruler = ruler && snapsToRuler(ruler, p) ? ruler : null;
    this.push(p, t0, pressure);
  }

  get onRuler(): boolean {
    return this.ruler !== null;
  }

  private push(p: Pt, t: number, pressure: number | null): void {
    const q = this.ruler ? projectOnEdge(this.ruler, p) : p;
    this.points.push([q[0], q[1]]);
    this.times.push(Math.max(0, Math.round(t - this.start)));
    if (this.withPressure) this.pressure.push(Math.min(1, Math.max(0, pressure ?? 0.5)));
  }

  /** Add a sample; false when it is too close to the last one. */
  extend(p: Pt, t: number, pressure: number | null): boolean {
    const q = this.ruler ? projectOnEdge(this.ruler, p) : p;
    const last = this.points[this.points.length - 1]!;
    if (Math.hypot(q[0] - last[0], q[1] - last[1]) < MIN_STEP) return false;
    this.push(p, t, pressure);
    return true;
  }

  /** The `addInk` payload. */
  toStroke(brush: { color: string; width: number; highlighter: boolean }): SlideInkStrokeIn {
    return {
      points: this.points.map((p) => [p[0], p[1]]),
      ...(this.withPressure ? { pressure: [...this.pressure] } : {}),
      times: [...this.times],
      color: brush.color,
      width: brush.width,
      ...(brush.highlighter ? { highlighter: true } : {}),
    };
  }
}

/** Width at a sample of the live preview: the engine's curve `width · (0.5 + f)`. */
export function previewWidth(width: number, f: number | undefined): number {
  return f === undefined ? width : width * (0.5 + Math.min(1, Math.max(0, f)));
}

// ------------------------------------------------------------------- erasing

function segDist(p: Pt, a: Pt, b: Pt): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  const t =
    len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2));
  return Math.hypot(a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
}

/** Does the eraser at `p` touch a stroke (preview of the stroke eraser)? */
export function touches(s: SlideInkStrokeOut, p: Pt, radius: number): boolean {
  const pts = s.points;
  if (pts.length === 1) {
    const q = pts[0]!;
    return Math.hypot(q[0] - p[0], q[1] - p[1]) <= radius + q[2] / 2;
  }
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1]!;
    const b = pts[i]!;
    if (segDist(p, [a[0], a[1]], [b[0], b[1]]) <= radius + Math.max(a[2], b[2]) / 2) return true;
  }
  return false;
}

// -------------------------------------------------------------------- replay

/** One stroke of the replay with its start and per-sample offsets (ms). */
export interface ReplayStroke {
  stroke: SlideInkStrokeOut;
  start: number;
  /** Offset of each sample from `start`. */
  offsets: number[];
}

/** Pause between strokes (ms) and time per sample when none was recorded. */
export const REPLAY_GAP = 250;
export const REPLAY_SAMPLE_MS = 8;
/** Longest pause kept from recorded times (ms). */
export const REPLAY_MAX_GAP = 1000;

/**
 * Strokes in drawing order with their timing: recorded times (`T`) are used
 * within a stroke; strokes follow each other with a short pause (recorded
 * absolute times only order them, long waits are shortened).
 */
export function replayTimeline(objects: SlideInkObject[]): {
  strokes: ReplayStroke[];
  total: number;
} {
  const all = objects.flatMap((o, oi) => o.strokes.map((s, si) => ({ s, key: [oi, si] as const })));
  // Order: by first recorded time when every stroke has one, else z-order.
  const timed = all.every((a) => a.s.times && a.s.times.length === a.s.points.length);
  if (timed) all.sort((a, b) => a.s.times![0]! - b.s.times![0]!);
  const strokes: ReplayStroke[] = [];
  let t = 0;
  let prevEnd: number | null = null;
  for (const { s } of all) {
    const offsets =
      s.times && s.times.length === s.points.length
        ? s.times.map((v) => v - s.times![0]!)
        : s.points.map((_, i) => i * REPLAY_SAMPLE_MS);
    if (timed && prevEnd !== null) {
      const gap = s.times![0]! - prevEnd;
      t += Math.min(REPLAY_MAX_GAP, Math.max(REPLAY_GAP / 2, gap));
    } else if (strokes.length > 0) {
      t += REPLAY_GAP;
    }
    strokes.push({ stroke: s, start: t, offsets });
    t += offsets[offsets.length - 1] ?? 0;
    prevEnd = timed ? s.times![s.times!.length - 1]! : null;
  }
  return { strokes, total: t };
}

/** Number of samples of each stroke visible at time `t` (ms). */
export function visibleAt(timeline: ReplayStroke[], t: number): number[] {
  return timeline.map((r) => {
    if (t < r.start) return 0;
    let n = 0;
    while (n < r.offsets.length && r.start + r.offsets[n]! <= t) n++;
    return n;
  });
}
