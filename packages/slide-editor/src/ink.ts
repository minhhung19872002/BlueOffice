/**
 * Ink drawn over the slides during a show (pen, highlighter), in slide points.
 * Kept at the end of the show as freeform shapes (`keepInk` command) or
 * discarded. No DOM; unit-tested.
 */
import type { SlideInkStroke } from '@blueoffice/platform';

export type PresenterTool = 'pointer' | 'pen' | 'highlighter' | 'laser' | 'eraser' | 'zoom';

/** Line widths in points (BlueSlide's own choice). */
export const PEN_WIDTH = 2;
export const HIGHLIGHTER_WIDTH = 14;
/** Default colours (`RRGGBB`): a red pen, a yellow highlighter. */
export const PEN_COLOR = 'D7261E';
export const HIGHLIGHTER_COLOR = 'FFE600';

type Pt = [number, number];

/** Distance from `p` to the segment `a`–`b`. */
export function segmentDistance(p: Pt, a: Pt, b: Pt): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  const t =
    len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2));
  const x = a[0] + t * dx - p[0];
  const y = a[1] + t * dy - p[1];
  return Math.hypot(x, y);
}

/** Does the stroke pass within `radius` of `p` (its own half width included)? */
export function strokeHit(s: SlideInkStroke, p: Pt, radius: number): boolean {
  const r = radius + s.width / 2;
  const pts = s.points;
  if (pts.length === 1) return Math.hypot(pts[0]![0] - p[0], pts[0]![1] - p[1]) <= r;
  for (let i = 1; i < pts.length; i++) {
    if (segmentDistance(p, pts[i - 1]!, pts[i]!) <= r) return true;
  }
  return false;
}

export class InkLayer {
  private readonly bySlide = new Map<number, SlideInkStroke[]>();
  private live: { slide: number; stroke: SlideInkStroke } | null = null;
  private version = 0;
  /** Time of each point of a stroke (ms), while a clock is set (recording a show). */
  private readonly times = new WeakMap<SlideInkStroke, number[]>();
  private clock: (() => number) | null = null;

  /** Time stamps for new points (SLIDE-REC-001); `null`: strokes are not timed. */
  setClock(clock: (() => number) | null): void {
    this.clock = clock;
  }

  /** Changes on every edit (React keys / redraws). */
  get rev(): number {
    return this.version;
  }

  begin(slide: number, tool: 'pen' | 'highlighter', color: string, p: Pt): void {
    const stroke: SlideInkStroke = {
      points: [p],
      color,
      width: tool === 'pen' ? PEN_WIDTH : HIGHLIGHTER_WIDTH,
      ...(tool === 'highlighter' ? { highlighter: true } : {}),
    };
    const list = this.bySlide.get(slide) ?? [];
    list.push(stroke);
    this.bySlide.set(slide, list);
    if (this.clock) this.times.set(stroke, [this.clock()]);
    this.live = { slide, stroke };
    this.version++;
  }

  /** Add a point to the stroke being drawn (points closer than 0.5 pt are skipped). */
  extend(p: Pt): void {
    const s = this.live?.stroke;
    if (!s) return;
    const last = s.points[s.points.length - 1]!;
    if (Math.hypot(last[0] - p[0], last[1] - p[1]) < 0.5) return;
    s.points.push(p);
    const t = this.times.get(s);
    if (t && this.clock) t.push(Math.max(t[t.length - 1] ?? 0, this.clock()));
    this.version++;
  }

  /** Strokes of `slide` with the time of each point where they were timed. */
  timed(slide: number): (SlideInkStroke & { times?: number[] })[] {
    return this.strokes(slide).map((s) => {
      const t = this.times.get(s);
      return t && t.length === s.points.length ? { ...s, times: [...t] } : { ...s };
    });
  }

  end(): void {
    this.live = null;
  }

  get drawing(): boolean {
    return this.live !== null;
  }

  /** Remove the strokes of `slide` passing near `p`; true when any was removed. */
  erase(slide: number, p: Pt, radius: number): boolean {
    const list = this.bySlide.get(slide);
    if (!list) return false;
    const keep = list.filter((s) => !strokeHit(s, p, radius));
    if (keep.length === list.length) return false;
    if (keep.length) this.bySlide.set(slide, keep);
    else this.bySlide.delete(slide);
    this.version++;
    return true;
  }

  /** Remove the ink of one slide, or of every slide. */
  clear(slide?: number): void {
    if (slide === undefined) this.bySlide.clear();
    else this.bySlide.delete(slide);
    this.live = null;
    this.version++;
  }

  strokes(slide: number): SlideInkStroke[] {
    return this.bySlide.get(slide) ?? [];
  }

  isEmpty(): boolean {
    for (const l of this.bySlide.values()) if (l.length) return false;
    return true;
  }

  /** The `keepInk` payload, slides in order. */
  toCommand(): { slide: number; strokes: SlideInkStroke[] }[] {
    return [...this.bySlide.entries()]
      .filter(([, l]) => l.length > 0)
      .sort((a, b) => a[0] - b[0])
      .map(([slide, strokes]) => ({
        slide,
        strokes: strokes.map((s) => ({ ...s, points: s.points.map((p) => [p[0], p[1]] as Pt) })),
      }));
  }
}

/** SVG path data of a stroke (slide points). */
export function strokePath(s: SlideInkStroke): string {
  const [first, ...rest] = s.points;
  if (!first) return '';
  const pts = rest.length ? rest : [first];
  return `M${first[0]} ${first[1]}` + pts.map((p) => `L${p[0]} ${p[1]}`).join('');
}
