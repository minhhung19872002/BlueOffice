/**
 * Pure presenting logic (SLIDE-SHOW-001): playing order of a show (setup range,
 * custom shows, hidden slides), navigation through it (loop, custom shows started
 * by hyperlinks, returning to the linking slide), kiosk inactivity restart,
 * rehearsal clock, zoom rectangle → stage transform and black / white screen.
 * No DOM; unit-tested.
 */
import type { SlideCustomShow, SlideLinkAction, SlideShowSetup } from '@blueoffice/platform';

// ------------------------------------------------------------------ playing order

export interface OrderOptions {
  setup?: SlideShowSetup | null;
  customShows?: SlideCustomShow[];
  /** Slides left out of the show (`sld@show="0"`). */
  hidden?: number[];
  /** Play this custom show instead of the setup's range. */
  custom?: number | null;
}

/**
 * The slides a show plays, in order (0-based). A custom show plays its list as
 * is (hidden slides included: they were picked by name); the deck or a range
 * of slide numbers (§19.2.1.38 sldRg, 1-based) leave out hidden slides.
 */
export function playOrder(count: number, o: OrderOptions = {}): number[] {
  const valid = (i: number) => Number.isInteger(i) && i >= 0 && i < count;
  const customId = o.custom ?? (o.setup?.range.kind === 'custom' ? o.setup.range.id : null);
  if (customId !== null && customId !== undefined) {
    const cs = o.customShows?.find((s) => s.id === customId);
    if (cs) return cs.slides.filter(valid);
  }
  const hidden = new Set(o.hidden ?? []);
  let from = 0;
  let to = count - 1;
  if (o.setup?.range.kind === 'range') {
    from = Math.max(0, o.setup.range.from - 1);
    to = Math.min(count - 1, o.setup.range.to - 1);
  }
  const out: number[] = [];
  for (let i = from; i <= to; i++) if (!hidden.has(i)) out.push(i);
  return out;
}

/**
 * Where "from the current slide" starts in `order`: the slide's first place, else the
 * first later slide of the order, else the beginning.
 */
export function startAt(order: number[], slide: number): number {
  const i = order.indexOf(slide);
  if (i >= 0) return i;
  const later = order.findIndex((s) => s > slide);
  return later >= 0 ? later : 0;
}

// ------------------------------------------------------------------ navigation

export type Enter = 'transition' | 'click' | 'jump';

/** Position in a running show: `at` = place in the order (`order.length`: end screen). */
export interface RunPos {
  at: number;
  slide: number;
  click: number;
  enter: Enter;
}

export interface ShowRun {
  order: number[];
  /** Orders to come back to after custom shows started with "return". */
  back: { order: number[]; at: number }[];
  /** Start again after the last slide (setup loop, kiosk). */
  loop: boolean;
  /** No animations: slides show with every effect played, without transitions. */
  still: boolean;
  /** Place of the previously shown slide (`lastViewed` hyperlinks). */
  prev: number | null;
}

export function newRun(order: number[], opts: { loop?: boolean; still?: boolean } = {}): ShowRun {
  return { order, back: [], loop: !!opts.loop, still: !!opts.still, prev: null };
}

export type RunAction = 'next' | 'prev' | 'first' | 'last';

function arrive(run: ShowRun, at: number, clicks: (s: number) => number, enter: Enter): RunPos {
  const slide = run.order[at] ?? 0;
  if (run.still) return { at, slide, click: clicks(slide), enter: 'jump' };
  return { at, slide, click: 0, enter };
}

/** The end screen after the last slide. */
function end(run: ShowRun, count: number): RunPos {
  return { at: run.order.length, slide: count, click: 0, enter: 'jump' };
}

/**
 * Next / previous / first / last. Clicks play the slide's effects first; after
 * the last slide come the slide a custom show returns to, the first slide (loop)
 * or the end screen.
 */
export function runStep(
  run: ShowRun,
  pos: RunPos,
  action: RunAction,
  count: number,
  clicks: (slide: number) => number,
): { run: ShowRun; pos: RunPos } {
  const n = run.order.length;
  const moved = (r: ShowRun, p: RunPos) => ({
    run: p.at !== pos.at || r.order !== run.order ? { ...r, prev: pos.at } : r,
    pos: p,
  });
  if (n === 0) return { run, pos: end(run, count) };
  switch (action) {
    case 'first':
      return moved(run, arrive(run, 0, clicks, 'jump'));
    case 'last':
      return moved(run, {
        ...arrive(run, n - 1, clicks, 'jump'),
        click: clicks(run.order[n - 1]!),
      });
    case 'prev': {
      if (pos.at >= n) {
        return moved(run, {
          ...arrive(run, n - 1, clicks, 'jump'),
          click: clicks(run.order[n - 1]!),
        });
      }
      if (!run.still && pos.click > 0)
        return { run, pos: { ...pos, click: pos.click - 1, enter: 'jump' } };
      if (pos.at === 0) return { run, pos: { ...pos, enter: 'jump' } };
      return moved(run, {
        ...arrive(run, pos.at - 1, clicks, 'jump'),
        click: clicks(run.order[pos.at - 1]!),
      });
    }
    case 'next': {
      if (pos.at >= n) return { run, pos: { ...pos, enter: 'jump' } };
      if (!run.still && pos.click < clicks(pos.slide)) {
        return { run, pos: { ...pos, click: pos.click + 1, enter: 'click' } };
      }
      if (pos.at + 1 < n) return moved(run, arrive(run, pos.at + 1, clicks, 'transition'));
      const back = run.back[run.back.length - 1];
      if (back) {
        const r: ShowRun = { ...run, order: back.order, back: run.back.slice(0, -1) };
        const slide = back.order[back.at] ?? 0;
        return moved(r, {
          at: back.at,
          slide,
          click: clicks(slide),
          enter: run.still ? 'jump' : 'transition',
        });
      }
      if (run.loop) return moved(run, arrive(run, 0, clicks, 'transition'));
      return moved(run, end(run, count));
    }
  }
}

/** Jump to place `at` of the order (slide grid). */
export function runGoto(
  run: ShowRun,
  pos: RunPos,
  at: number,
  clicks: (slide: number) => number,
): { run: ShowRun; pos: RunPos } {
  const i = Math.max(0, Math.min(at, run.order.length - 1));
  return { run: { ...run, prev: pos.at }, pos: arrive(run, i, clicks, 'transition') };
}

/**
 * Follow a hyperlink (§21.1.2.3.5): custom shows (with or without coming back),
 * slides of the order, jumps. `end` = the show ends.
 */
export function runLink(
  run: ShowRun,
  pos: RunPos,
  action: SlideLinkAction,
  count: number,
  clicks: (slide: number) => number,
  customShows: SlideCustomShow[],
): { run: ShowRun; pos: RunPos } | 'end' {
  switch (action.kind) {
    case 'none':
      return { run, pos };
    case 'end':
      return 'end';
    case 'next':
      return runStep(run, { ...pos, click: clicks(pos.slide) }, 'next', count, clicks);
    case 'prev':
      return runStep(run, { ...pos, click: 0 }, 'prev', count, clicks);
    case 'first':
    case 'last':
      return runStep(run, pos, action.kind, count, clicks);
    case 'lastViewed':
      return run.prev === null || run.prev >= run.order.length
        ? { run, pos }
        : runGoto(run, pos, run.prev, clicks);
    case 'slide': {
      const at = run.order.indexOf(action.slide);
      return at < 0 ? { run, pos } : runGoto(run, pos, at, clicks);
    }
    case 'customShow': {
      const cs = customShows.find((s) => s.id === action.id);
      const order = cs?.slides.filter((i) => i >= 0 && i < count) ?? [];
      if (order.length === 0) return { run, pos };
      const r: ShowRun = {
        ...run,
        order,
        back: action.return ? [...run.back, { order: run.order, at: pos.at }] : [],
        prev: pos.at,
      };
      return { run: r, pos: arrive(r, 0, clicks, 'transition') };
    }
  }
}

// ------------------------------------------------------------------ kiosk

type Timers = {
  set: (fn: () => void, ms: number) => ReturnType<typeof setTimeout>;
  clear: (id: ReturnType<typeof setTimeout>) => void;
};

/**
 * Kiosk restart (§19.2.1.17 kiosk@restart): after `ms` without input the show
 * starts again. `poke` on every pointer move, click or key.
 */
export class InactivityTimer {
  private id: ReturnType<typeof setTimeout> | null = null;
  constructor(
    private readonly ms: number,
    private readonly onIdle: () => void,
    private readonly timers: Timers = {
      set: (fn, ms) => setTimeout(fn, ms),
      clear: (id) => clearTimeout(id),
    },
  ) {}

  start(): void {
    this.poke();
  }

  poke(): void {
    this.stop();
    if (this.ms <= 0) return;
    this.id = this.timers.set(() => {
      this.id = null;
      this.onIdle();
      // Keeps watching: a show left alone restarts again after the next period.
      this.poke();
    }, this.ms);
  }

  stop(): void {
    if (this.id !== null) this.timers.clear(this.id);
    this.id = null;
  }

  get running(): boolean {
    return this.id !== null;
  }
}

/** Kiosk: only Esc ends the show; clicks and keys do not move it (timings and links do). */
export function kioskAllows(action: string | null): boolean {
  return action === 'exit';
}

// ------------------------------------------------------------------ rehearsal

/**
 * Rehearse timings: time spent on each slide (summed when a slide comes back),
 * with pause / resume and "repeat this slide".
 */
export class RehearsalClock {
  private readonly times = new Map<number, number>();
  private slide: number | null = null;
  private since: number;
  private pausedAt: number | null = null;
  private readonly started: number;
  private pausedTotal = 0;

  constructor(now: number) {
    this.since = now;
    this.started = now;
  }

  /** Time on the current slide so far (ms). */
  current(now: number): number {
    if (this.slide === null) return 0;
    const t = (this.pausedAt ?? now) - this.since;
    return Math.max(0, t);
  }

  /** Time of the whole rehearsal (ms), pauses left out. */
  total(now: number): number {
    const t = (this.pausedAt ?? now) - this.started - this.pausedTotal;
    return Math.max(0, t);
  }

  get paused(): boolean {
    return this.pausedAt !== null;
  }

  private close(now: number) {
    if (this.slide === null) return;
    const t = this.current(now);
    this.times.set(this.slide, (this.times.get(this.slide) ?? 0) + t);
  }

  /** The show moved to `slide`. */
  enter(slide: number, now: number): void {
    if (slide === this.slide) return;
    this.close(now);
    this.slide = slide;
    this.since = this.pausedAt ?? now;
  }

  pause(now: number): void {
    if (this.pausedAt === null) this.pausedAt = now;
  }

  resume(now: number): void {
    if (this.pausedAt === null) return;
    const d = now - this.pausedAt;
    this.pausedTotal += d;
    this.since += d;
    this.pausedAt = null;
  }

  /** Start the current slide's time again. */
  repeat(now: number): void {
    this.since = this.pausedAt ?? now;
  }

  /** Times per slide in ms (whole ms, `advTm`), in slide order. */
  finish(now: number): { slide: number; ms: number }[] {
    this.close(now);
    this.slide = null;
    return [...this.times.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([slide, ms]) => ({ slide, ms: Math.round(ms) }));
  }
}

// ------------------------------------------------------------------ zoom

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** CSS transform of the stage (origin top-left): `translate(x, y) scale(scale)`. */
export interface ZoomTransform {
  scale: number;
  x: number;
  y: number;
}

export const NO_ZOOM: ZoomTransform = { scale: 1, x: 0, y: 0 };
const MAX_ZOOM = 8;

/**
 * Zoom so `rect` (stage px) fills the stage: the largest scale that shows all of
 * it, its centre at the stage's centre, the slide still covering the stage.
 */
export function zoomTransform(rect: Box, stage: { width: number; height: number }): ZoomTransform {
  if (rect.w <= 0 || rect.h <= 0 || stage.width <= 0 || stage.height <= 0) return NO_ZOOM;
  const scale = Math.max(1, Math.min(MAX_ZOOM, stage.width / rect.w, stage.height / rect.h));
  const cx = rect.x + rect.w / 2;
  const cy = rect.y + rect.h / 2;
  const clamp = (v: number, lo: number) => Math.min(0, Math.max(lo, v));
  return {
    scale,
    x: clamp(stage.width / 2 - scale * cx, stage.width - stage.width * scale),
    y: clamp(stage.height / 2 - scale * cy, stage.height - stage.height * scale),
  };
}

/** Zoom by `factor` around a point (a click instead of a dragged rectangle). */
export function zoomAt(
  p: { x: number; y: number },
  factor: number,
  stage: { width: number; height: number },
): ZoomTransform {
  const w = stage.width / factor;
  const h = stage.height / factor;
  return zoomTransform({ x: p.x - w / 2, y: p.y - h / 2, w, h }, stage);
}

/** Stage point → slide point under a zoom (`k` = slide points per stage px at no zoom). */
export function unzoom(
  p: { x: number; y: number },
  z: ZoomTransform,
  k: number,
): { x: number; y: number } {
  return { x: ((p.x - z.x) / z.scale) * k, y: ((p.y - z.y) / z.scale) * k };
}

/** Rectangle between two drag points, normalised. */
export function dragBox(a: { x: number; y: number }, b: { x: number; y: number }): Box {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    w: Math.abs(a.x - b.x),
    h: Math.abs(a.y - b.y),
  };
}

// ------------------------------------------------------------------ screen

export type ScreenCover = 'none' | 'black' | 'white';

/** The run position of a show position (its place in the order, or the slide's first place). */
export function runPosOf(
  p: { slide: number; click: number; enter: Enter; at?: number },
  run: ShowRun,
): RunPos {
  return {
    at: p.at ?? Math.max(0, run.order.indexOf(p.slide)),
    slide: p.slide,
    click: p.click,
    enter: p.enter,
  };
}

/** B / W keys: show that colour, or the slide again when it is already shown. */
export function toggleCover(cur: ScreenCover, want: 'black' | 'white'): ScreenCover {
  return cur === want ? 'none' : want;
}
