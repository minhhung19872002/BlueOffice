import type { SlideAnimInfo, SlideEffectPlay, SlideEffectTrack } from '@blueoffice/platform';
import { rectsPolygon } from './transitionFx';

/**
 * Keyframes of an animation effect from its behaviours (ECMA-376 Part 1, §19.5, read by the engine
 * into tracks): visibility sets, fades and opacity, position (`ppt_x` / `ppt_y`), size (`ppt_w` /
 * `ppt_h`, animScale), rotation (`style.rotation`, animRot), motion paths and the §19.5.3 animEffect
 * filters as clip polygons inside the shape's box. The track values are sampled into one keyframe
 * list for the shape's layer (a full-slide layer, so translations are fractions of the slide).
 * Colour changes (animClr) are not shown. Unit-tested.
 */

type Frames = Record<string, string | number>[];
type Rect = [number, number, number, number];

/** Progress 0..1 of a track at `ms` (repeats and reversals); `null` before it starts. */
export function trackProgress(t: SlideEffectTrack, ms: number): number | null {
  if (ms < t.beginMs) return null;
  if (t.durMs <= 0) return 1;
  const run = t.autoRev ? 2 * t.durMs : t.durMs;
  const total = run * t.repeat;
  const local = Math.min(ms - t.beginMs, total);
  let p = local >= total ? (t.autoRev ? 0 : 1) : (local % run) / t.durMs;
  if (t.autoRev && p > 1) p = 2 - p;
  return Math.min(1, Math.max(0, p));
}

function ended(t: SlideEffectTrack, ms: number): boolean {
  return ms >= t.beginMs + (t.autoRev ? 2 : 1) * t.durMs * t.repeat;
}

/** Value of keyframes at `p` (linear, or discrete steps). */
function keyValue(keys: [number, number][], p: number, discrete: boolean): number {
  if (keys.length === 0) return 0;
  const first = keys[0] ?? [0, 0];
  if (p <= first[0]) return first[1];
  for (let i = 1; i < keys.length; i++) {
    const [t1, v1] = keys[i] ?? [1, 0];
    const [t0, v0] = keys[i - 1] ?? [0, 0];
    if (p <= t1) {
      if (discrete) return v0;
      return t1 > t0 ? v0 + ((v1 - v0) * (p - t0)) / (t1 - t0) : v1;
    }
  }
  return (keys.at(-1) ?? first)[1];
}

/** Point along a polyline at `p` (by length). */
function along(points: [number, number][], p: number): [number, number] {
  if (points.length === 0) return [0, 0];
  if (points.length === 1) return points[0] ?? [0, 0];
  const seg: number[] = [];
  let total = 0;
  for (let i = 1; i < points.length; i++) {
    const a: [number, number] = points[i - 1] ?? [0, 0];
    const b: [number, number] = points[i] ?? [0, 0];
    const d = Math.hypot(b[0] - a[0], b[1] - a[1]);
    seg.push(d);
    total += d;
  }
  let want = p * total;
  for (let i = 0; i < seg.length; i++) {
    const d = seg[i] ?? 0;
    if (want <= d || i === seg.length - 1) {
      const a: [number, number] = points[i] ?? [0, 0];
      const b: [number, number] = points[i + 1] ?? [0, 0];
      const k = d > 0 ? Math.min(1, want / d) : 1;
      return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
    }
    want -= d;
  }
  return points.at(-1) ?? [0, 0];
}

const clamp = (v: number) => Math.min(1, Math.max(0, v));

/**
 * The shown part of the shape box `[x, y, w, h]` (slide fractions) for §19.5.3 filter `filter` at
 * progress `p` of an entrance (an exit shows the part of `1 − p`): rectangles of a fixed count per
 * filter so the polygons interpolate.
 */
export function filterRects(filter: string, p: number, [x, y, w, h]: Rect): Rect[] {
  const box = (u0: number, v0: number, u1: number, v1: number): Rect => [
    x + w * clamp(u0),
    y + h * clamp(v0),
    x + w * clamp(u1),
    y + h * clamp(v1),
  ];
  const [name, sub = ''] = filter.replace(')', '').split('(');
  switch (name) {
    case 'wipe':
      return [
        sub === 'down'
          ? box(0, 0, 1, p)
          : sub === 'right'
            ? box(0, 0, p, 1)
            : sub === 'left'
              ? box(1 - p, 0, 1, 1)
              : box(0, 1 - p, 1, 1),
      ];
    case 'slide':
      // The part that has slid into the box from that side.
      return [
        sub === 'fromTop'
          ? box(0, 0, 1, p)
          : sub === 'fromLeft'
            ? box(0, 0, p, 1)
            : sub === 'fromRight'
              ? box(1 - p, 0, 1, 1)
              : box(0, 1 - p, 1, 1),
      ];
    case 'blinds': {
      const n = 6;
      return Array.from({ length: n }, (_, i) =>
        sub === 'vertical' ? box(i / n, 0, (i + p) / n, 1) : box(0, i / n, 1, (i + p) / n),
      );
    }
    case 'randombar': {
      const n = 12;
      const order = [5, 2, 9, 0, 7, 11, 3, 8, 1, 10, 4, 6];
      return Array.from({ length: n }, (_, i) => {
        const on = (order[i] ?? 0) < p * n;
        return sub === 'vertical'
          ? box(i / n, 0, on ? (i + 1) / n : i / n, 1)
          : box(0, i / n, 1, on ? (i + 1) / n : i / n);
      });
    }
    case 'checkerboard': {
      const [c, r] = [6, 4];
      const out: Rect[] = [];
      for (let j = 0; j < r; j++) {
        for (let i = 0; i < c; i++) {
          const q = clamp((p - ((i + j) % 2) * 0.5) * 2);
          out.push(
            sub === 'down'
              ? box(i / c, j / r, (i + 1) / c, (j + q) / r)
              : box(i / c, j / r, (i + q) / c, (j + 1) / r),
          );
        }
      }
      return out;
    }
    case 'barn': {
      // In: from both edges inwards; out: from the middle outwards.
      const inward = sub.startsWith('in');
      const vert = sub.endsWith('Vertical');
      if (inward) {
        return vert
          ? [box(0, 0, p / 2, 1), box(1 - p / 2, 0, 1, 1)]
          : [box(0, 0, 1, p / 2), box(0, 1 - p / 2, 1, 1)];
      }
      return vert
        ? [box(0.5 - p / 2, 0, 0.5, 1), box(0.5, 0, 0.5 + p / 2, 1)]
        : [box(0, 0.5 - p / 2, 1, 0.5), box(0, 0.5, 1, 0.5 + p / 2)];
    }
    case 'box':
    case 'circle':
    case 'diamond':
    case 'plus':
      // Grows from the centre (in) or closes in from the edges (out), as a box.
      return sub === 'out'
        ? [
            box(0, 0, 1, p / 2),
            box(0, 1 - p / 2, 1, 1),
            box(0, 0, p / 2, 1),
            box(1 - p / 2, 0, 1, 1),
          ]
        : [
            box(0.5 - p / 2, 0.5 - p / 2, 0.5 + p / 2, 0.5 + p / 2),
            box(0.5, 0.5, 0.5, 0.5),
            box(0.5, 0.5, 0.5, 0.5),
            box(0.5, 0.5, 0.5, 0.5),
          ];
    case 'strips': {
      // Diagonal steps from a corner.
      const n = 8;
      const fromRight = sub.endsWith('Left');
      const fromBottom = sub.startsWith('up');
      return Array.from({ length: n }, (_, i) => {
        const q = clamp(p * 2 - i / n);
        const [u0, u1] = fromRight ? [1 - q, 1] : [0, q];
        const [v0, v1] = fromBottom ? [1 - (i + 1) / n, 1 - i / n] : [i / n, (i + 1) / n];
        return box(u0, v0, u1, v1);
      });
    }
    case 'wheel':
    case 'wedge':
    case 'dissolve':
    default: {
      // Pieces in a fixed scattered order.
      const [c, r] = [8, 6];
      const n = c * r;
      return Array.from({ length: n }, (_, k) => {
        const rank = (k * 29) % n;
        const [i, j] = [k % c, Math.floor(k / c)];
        return rank < p * n
          ? box(i / c, j / r, (i + 1) / c, (j + 1) / r)
          : box(i / c, j / r, i / c, j / r);
      });
    }
  }
}

/**
 * Keyframes of effect `a` for its layer on a `width` × `height` slide, sampled from its tracks
 * (`null` when the effect carries none).
 */
export function playFrames(
  a: SlideAnimInfo,
  width: number,
  height: number,
  steps = 24,
): Frames | null {
  const play: SlideEffectPlay | undefined = a.play;
  if (!play || play.tracks.length === 0) return null;
  const W = width || 1;
  const H = height || 1;
  const [rx, ry, rw, rh] = a.rect ?? [0, 0, W, H];
  const box: Rect = [rx / W, ry / H, rw / W, rh / H];
  const base = { x: box[0] + box[2] / 2, y: box[1] + box[3] / 2, w: box[2], h: box[3] };
  const dur = Math.max(1, play.durMs, a.endMs - a.startMs);
  const sets = play.tracks.filter((t) => t.kind === 'set' && t.attr === 'style.visibility');
  // An entrance starts hidden until its first visibility set; anything else starts shown.
  const startVisible = a.class !== 'entr' || sets.length === 0;
  const filter = play.tracks.find(
    (t): t is Extract<SlideEffectTrack, { kind: 'filter' }> =>
      t.kind === 'filter' && !t.filter.startsWith('fade'),
  );
  const frames: Frames = [];
  for (let k = 0; k <= steps; k++) {
    const ms = (dur * k) / steps;
    let visible = startVisible;
    for (const t of sets) {
      if (t.kind === 'set' && ms >= t.beginMs) visible = t.value !== 'hidden';
    }
    let opacity = visible ? 1 : 0;
    let [dx, dy, sx, sy, rot] = [0, 0, 1, 1, 0];
    let clip: string | null = null;
    for (const t of play.tracks) {
      const p = trackProgress(t, ms);
      const active = p !== null && (!ended(t, ms) || t.hold);
      switch (t.kind) {
        case 'filter': {
          // Before an entering filter starts the shape is not shown yet.
          const q = p ?? 0;
          const shown = t.entering ? q : 1 - q;
          if (t.filter.startsWith('fade')) {
            if (p !== null || t.entering) opacity *= shown;
          } else if (t === filter) {
            clip = rectsPolygon(
              filterRects(t.filter, shown, [box[0], box[1], box[2], box[3]]).map(
                ([x0, y0, x1, y1]): Rect => [x0, y0, x1, y1],
              ),
            );
          }
          break;
        }
        case 'anim': {
          if (!active || p === null) break;
          const v = keyValue(t.keys, p, t.discrete);
          if (t.attr === 'ppt_x') dx += v - base.x;
          else if (t.attr === 'ppt_y') dy += v - base.y;
          else if (t.attr === 'ppt_w') sx *= base.w > 0 ? v / base.w : 1;
          else if (t.attr === 'ppt_h') sy *= base.h > 0 ? v / base.h : 1;
          else if (t.attr === 'style.rotation' || t.attr === 'r') rot += v;
          else if (t.attr === 'style.opacity') opacity *= clamp(v);
          break;
        }
        case 'set':
          if (t.attr === 'style.opacity' && p !== null) opacity *= clamp(Number(t.value) || 0);
          break;
        case 'motion': {
          if (!active || p === null) break;
          const [mx, my] = along(t.points, p);
          dx += mx;
          dy += my;
          break;
        }
        case 'rotate':
          if (active && p !== null) rot += t.by * p;
          break;
        case 'scale':
          if (active && p !== null) {
            sx *= t.from[0] + (t.to[0] - t.from[0]) * p;
            sy *= t.from[1] + (t.to[1] - t.from[1]) * p;
          }
          break;
        default:
          break;
      }
    }
    const pct = (v: number) => `${Math.round(v * 100000) / 1000}%`;
    const frame: Record<string, string | number> = {
      offset: k / steps,
      opacity: Math.round(opacity * 1000) / 1000,
      transform: `translate(${pct(dx)}, ${pct(dy)}) rotate(${Math.round(rot * 100) / 100}deg) scale(${Math.round(sx * 10000) / 10000}, ${Math.round(sy * 10000) / 10000})`,
      transformOrigin: `${pct(base.x)} ${pct(base.y)}`,
    };
    if (clip) frame.clipPath = clip;
    frames.push(frame);
  }
  // A filter that starts late still needs a clip on every frame (same polygon shape).
  if (filter && frames.some((f) => !f.clipPath)) {
    const full = rectsPolygon(filterRects(filter.filter, 1, box));
    for (const f of frames) if (!f.clipPath) f.clipPath = full;
  }
  return frames;
}
