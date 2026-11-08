import type { SlideAnimInfo, SlideShowInfo } from '@blueoffice/platform';
import { playFrames } from './effectFx';

/**
 * Slide show playback, without the DOM: which layers are visible at a click
 * step, the keyframes of each effect and of the slide transition. The show
 * stage applies them with the Web Animations API. Unit-tested.
 *
 * A slide arrives as layers (ECMA-376 Part 1, §19.5 targets top-level shapes):
 * the backdrop plus one transparent full-slide layer per animated shape, so an
 * effect is a fade, a move or a clip of its layer.
 */

/** Keyframes of one effect (CSS properties of the layer). */
export type Frames = Record<string, string | number>[];

export interface PlannedEffect {
  layer: number;
  frames: Frames;
  /** ms after the click. */
  delay: number;
  duration: number;
  /** Visibility once the effect is done (`null` = unchanged). */
  visibleAfter: boolean | null;
}

/** Final visibility of an effect's shape (`null` = emphasis / path: unchanged). */
function after(a: SlideAnimInfo): boolean | null {
  if (a.class === 'entr') return true;
  if (a.class === 'exit') return false;
  return null;
}

/**
 * Visibility of every layer once the effects of clicks `0..=click` are done
 * (`click = -1`: before the slide's own effects). A layer whose first effect is
 * an entrance starts hidden.
 */
export function visibility(show: SlideShowInfo, click: number): boolean[] {
  const vis = show.layers.map(() => true);
  const seen = new Set<number>();
  for (const a of show.animations) {
    if (a.layer == null || seen.has(a.layer)) continue;
    seen.add(a.layer);
    if (a.class === 'entr') vis[a.layer] = false;
  }
  for (const a of show.animations) {
    if (a.layer == null || a.click > click) continue;
    const v = after(a);
    if (v !== null) vis[a.layer] = v;
  }
  return vis;
}

const pct = (v: number) => `${Math.round(v * 10000) / 100}%`;

/** Keyframes of one effect, for a slide of `width` × `height` points. */
export function effectFrames(a: SlideAnimInfo, width: number, height: number): Frames {
  // Effects with behaviours read by the engine play them (§19.5); the rest below.
  const sampled = playFrames(a, width, height);
  if (sampled) return sampled;
  const exit = a.class === 'exit';
  const order = (f: Frames): Frames => (exit ? [...f].reverse() : f);
  const [x, y, w, h] = a.rect ?? [0, 0, width, height];
  const W = width || 1;
  const H = height || 1;
  if (a.class !== 'entr' && a.class !== 'exit') {
    // Emphasis, motion path, other: a short pulse (the effect itself is kept in the file).
    return [{ opacity: 1 }, { opacity: 0.35 }, { opacity: 1 }];
  }
  switch (a.effect) {
    case 'appear':
      return order([
        { opacity: 0, offset: 0 },
        { opacity: 0, offset: 0.99 },
        { opacity: 1, offset: 1 },
      ]);
    case 'flyIn': {
      const from =
        a.direction === 'l'
          ? `translate(${pct(-(x + w) / W)}, 0)`
          : a.direction === 'r'
            ? `translate(${pct((W - x) / W)}, 0)`
            : a.direction === 't'
              ? `translate(0, ${pct(-(y + h) / H)})`
              : `translate(0, ${pct((H - y) / H)})`;
      return order([
        { transform: from, opacity: 1 },
        { transform: 'translate(0, 0)', opacity: 1 },
      ]);
    }
    case 'wipe': {
      // inset(top right bottom left), revealing the shape box from `direction`.
      const top = y / H;
      const left = x / W;
      const bottom = (H - y - h) / H;
      const right = (W - x - w) / W;
      const box = (t: number, r: number, b: number, l: number) =>
        `inset(${pct(t)} ${pct(r)} ${pct(b)} ${pct(l)})`;
      const start =
        a.direction === 't'
          ? box(top, right, 1 - top, left)
          : a.direction === 'l'
            ? box(top, 1 - left, bottom, left)
            : a.direction === 'r'
              ? box(top, right, bottom, 1 - right)
              : box(1 - bottom, right, bottom, left);
      return order([
        { clipPath: start, opacity: 1 },
        { clipPath: box(top, right, bottom, left), opacity: 1 },
      ]);
    }
    // Fade and every effect BlueSlide does not model.
    default:
      return order([{ opacity: 0 }, { opacity: 1 }]);
  }
}

/** Effects started by click step `click` (0 = with the slide). */
export function effectsAt(show: SlideShowInfo, click: number): PlannedEffect[] {
  return show.animations
    .filter((a) => a.click === click && a.layer != null)
    .map((a) => ({
      layer: a.layer ?? 0,
      frames: effectFrames(a, show.width, show.height),
      delay: a.startMs,
      duration: Math.max(1, a.endMs - a.startMs),
      visibleAfter: after(a),
    }));
}

/** When the effects of a click step are all done (ms). */
export function stepDuration(show: SlideShowInfo, click: number): number {
  return show.animations.filter((a) => a.click === click).reduce((m, a) => Math.max(m, a.endMs), 0);
}

export { transitionPlan, type TransitionPlan } from './transitionFx';

/** Position in the show: slide and click steps done (0 = the slide's own effects ran). */
export interface ShowPos {
  slide: number;
  click: number;
  /** How the stage reaches this position: play the transition, play one click, or jump. */
  enter: 'transition' | 'click' | 'jump';
}

/**
 * Next / previous position. `clicks(i)` = click steps of slide `i` (unknown =
 * 0); `slide === count` is the black end screen.
 */
export function advance(
  pos: ShowPos,
  dir: 'next' | 'prev',
  count: number,
  clicks: (slide: number) => number,
): ShowPos {
  if (dir === 'next') {
    if (pos.slide >= count) return { ...pos, enter: 'jump' };
    if (pos.click < clicks(pos.slide))
      return { slide: pos.slide, click: pos.click + 1, enter: 'click' };
    return { slide: pos.slide + 1, click: 0, enter: 'transition' };
  }
  if (pos.slide >= count)
    return { slide: Math.max(0, count - 1), click: clicks(count - 1), enter: 'jump' };
  if (pos.click > 0) return { slide: pos.slide, click: pos.click - 1, enter: 'jump' };
  if (pos.slide === 0) return { slide: 0, click: 0, enter: 'jump' };
  return { slide: pos.slide - 1, click: clicks(pos.slide - 1), enter: 'jump' };
}
