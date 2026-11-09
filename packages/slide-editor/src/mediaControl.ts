/**
 * Slide show playback of a clip's options (SLIDE-MEDIA-001), independent of React and of the
 * real media element: trim window, fade-in / fade-out volume ramps, bookmarks reached, loop,
 * rewind, and clips that keep playing across slides. WebVTT cue parsing for caption files.
 *
 * Times of the playback options are in ms ([MS-PPTX] ST_UniversalTimeOffset); a media
 * element's `currentTime` / `duration` are in seconds (HTML).
 */
import type { SlideMediaBookmark, SlideMediaPlayback } from '@blueoffice/platform';

/** What the controller uses of an `HTMLMediaElement`. */
export interface MediaLike {
  /** Seconds. */
  currentTime: number;
  /** Seconds; NaN while unknown. */
  readonly duration: number;
  /** 0..1. */
  volume: number;
  muted: boolean;
  readonly paused: boolean;
  play(): Promise<void> | void;
  pause(): void;
}

/** The options the controller needs (a subset of `SlideMediaPlayback`). */
export type PlaybackOptions = Pick<
  SlideMediaPlayback,
  | 'trimStartMs'
  | 'trimEndMs'
  | 'fadeInMs'
  | 'fadeOutMs'
  | 'bookmarks'
  | 'volume'
  | 'mute'
  | 'loop'
  | 'rewind'
>;

/** End of the trimmed window (ms); `Infinity` while the duration is unknown. */
export function trimEnd(p: Pick<PlaybackOptions, 'trimEndMs'>, durationMs: number): number {
  return Number.isFinite(durationMs) ? Math.max(0, durationMs - p.trimEndMs) : Infinity;
}

/**
 * Volume factor at media time `t` (ms) from the fades: rises linearly from 0 at the trim start
 * over `fadeInMs`, falls linearly to 0 at the trim end over `fadeOutMs`; 1 in between.
 */
export function fadeGain(t: number, p: PlaybackOptions, durationMs: number): number {
  const start = p.trimStartMs;
  const end = trimEnd(p, durationMs);
  let g = 1;
  if (p.fadeInMs > 0) g = Math.min(g, (t - start) / p.fadeInMs);
  if (p.fadeOutMs > 0 && Number.isFinite(end)) g = Math.min(g, (end - t) / p.fadeOutMs);
  return Math.max(0, Math.min(1, g));
}

/** Element volume (0..1) at media time `t` (ms). */
export function volumeAt(t: number, p: PlaybackOptions, durationMs: number): number {
  if (p.mute) return 0;
  return (Math.max(0, Math.min(100, p.volume)) / 100) * fadeGain(t, p, durationMs);
}

/** Bookmarks passed when playback moved forward from `from` to `to` (ms): `from < time <= to`. */
export function crossedBookmarks(
  from: number,
  to: number,
  bookmarks: SlideMediaBookmark[],
): SlideMediaBookmark[] {
  if (!(to > from)) return [];
  return bookmarks.filter((b) => b.timeMs > from && b.timeMs <= to);
}

export interface ControllerHooks {
  /** A bookmark was reached (its effects start). */
  onBookmark?: (name: string) => void;
  /** Playing (`true`) or stopped (`false`). */
  onPlaying?: (playing: boolean) => void;
}

/** Ticks (ms) of the controller; small enough for smooth fades. */
export const TICK_MS = 40;

/**
 * Plays one clip with its options: starts at the trim start, ramps the volume, reports the
 * bookmarks it passes, and at the trim end loops, or stops (and rewinds when asked).
 */
export class MediaController {
  private timer: ReturnType<typeof setInterval> | null = null;
  private last = 0;
  private playing = false;

  constructor(
    readonly el: MediaLike,
    private opts: PlaybackOptions,
    private readonly hooks: ControllerHooks = {},
  ) {}

  /** Options changed (the slide was edited). */
  update(opts: PlaybackOptions): void {
    this.opts = opts;
  }

  get isPlaying(): boolean {
    return this.playing;
  }

  private durationMs(): number {
    const d = this.el.duration;
    return Number.isFinite(d) ? d * 1000 : NaN;
  }

  private seekMs(ms: number): void {
    this.el.currentTime = ms / 1000;
    this.last = ms;
  }

  /** Play from the trim start. */
  start(): void {
    this.seekMs(this.opts.trimStartMs);
    this.resume();
  }

  /** Play from where the clip is (outside the trimmed window: from its start). */
  resume(): void {
    const t = this.el.currentTime * 1000;
    if (t < this.opts.trimStartMs || t >= trimEnd(this.opts, this.durationMs())) {
      this.seekMs(this.opts.trimStartMs);
    } else {
      this.last = t;
    }
    this.el.muted = this.opts.mute;
    this.el.volume = volumeAt(this.last, this.opts, this.durationMs());
    const p = this.el.play();
    if (p && typeof (p as Promise<void>).catch === 'function') {
      (p as Promise<void>).catch(() => {});
    }
    this.setPlaying(true);
    if (this.timer === null) this.timer = setInterval(() => this.tick(), TICK_MS);
  }

  /** Pause where it is. */
  pause(): void {
    this.el.pause();
    this.halt();
  }

  /** Play or pause (a click on the clip). */
  toggle(): void {
    if (this.playing) this.pause();
    else this.resume();
  }

  /** Stop: pause, back to the trim start. */
  stop(): void {
    this.el.pause();
    this.seekMs(this.opts.trimStartMs);
    this.halt();
  }

  private halt(): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    this.setPlaying(false);
  }

  private setPlaying(p: boolean): void {
    if (this.playing === p) return;
    this.playing = p;
    this.hooks.onPlaying?.(p);
  }

  /** One step of playback (called by the timer; public for tests). */
  tick(): void {
    if (!this.playing) return;
    const o = this.opts;
    const dur = this.durationMs();
    let t = this.el.currentTime * 1000;
    if (t < o.trimStartMs - 1) {
      // Seeked before the window (controls): back into it.
      this.seekMs(o.trimStartMs);
      t = o.trimStartMs;
    }
    const end = trimEnd(o, dur);
    const reached = Math.min(t, end);
    for (const b of crossedBookmarks(this.last, reached, o.bookmarks)) {
      this.hooks.onBookmark?.(b.name);
    }
    this.last = reached;
    if (t >= end) {
      if (o.loop) {
        this.seekMs(o.trimStartMs);
        this.el.volume = volumeAt(o.trimStartMs, o, dur);
        return;
      }
      this.el.pause();
      if (o.rewind) this.seekMs(o.trimStartMs);
      this.halt();
      return;
    }
    this.el.muted = o.mute;
    this.el.volume = volumeAt(t, o, dur);
  }

  dispose(): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
  }
}

/**
 * A clip playing on across slides (`numSld` > 1): the show hands it a detached audio element
 * when the clip's slide is left, and moves it on with each slide shown.
 */
export class MediaCarry {
  private cur: {
    key: string;
    ctrl: MediaController;
    from: number;
    last: number;
    release: () => void;
  } | null = null;

  /** Key of the clip carried on (`slide:shapeId`), if any. */
  get key(): string | null {
    return this.cur?.key ?? null;
  }

  /**
   * Keep `el` playing (from where it is) for the slides after `slide`, up to `slides - 1` of
   * them; `release` frees its resources when it stops.
   */
  adopt(
    key: string,
    el: MediaLike,
    opts: PlaybackOptions,
    slide: number,
    slides: number,
    release: () => void,
  ): void {
    this.stop();
    const ctrl = new MediaController(el, opts, {
      onPlaying: (p) => {
        if (!p && this.cur?.ctrl === ctrl) this.stop();
      },
    });
    this.cur = { key, ctrl, from: slide, last: slide + Math.max(1, slides) - 1, release };
    ctrl.resume();
  }

  /** Slide `slide` is shown: stop when it is outside the clip's span. */
  showSlide(slide: number): void {
    const c = this.cur;
    if (c && (slide <= c.from || slide > c.last)) this.stop();
  }

  stop(): void {
    const c = this.cur;
    if (!c) return;
    this.cur = null;
    c.ctrl.pause();
    c.ctrl.dispose();
    c.release();
  }
}

/** The slide show's carried clip (one at a time). */
export const showMediaCarry = new MediaCarry();

// ---------------------------------------------------------------------------
// WebVTT (W3C WebVTT: The Web Video Text Tracks Format)
// ---------------------------------------------------------------------------

export interface VttCue {
  id: string | null;
  /** ms. */
  start: number;
  /** ms. */
  end: number;
  text: string;
}

/** WebVTT §4.1: optional BOM, `WEBVTT`, then a space, tab, line break or the end. */
export function isWebVtt(text: string): boolean {
  return /^\uFEFF?WEBVTT(?:[ \t\r\n]|$)/.test(text);
}

/** WebVTT timestamp `[hh:]mm:ss.ttt` in ms; `null` when malformed. */
export function parseVttTime(s: string): number | null {
  const m = /^(?:(\d{2,}):)?([0-5]\d):([0-5]\d)\.(\d{3})$/.exec(s.trim());
  if (!m) return null;
  const [, h, mi, se, ms] = m;
  return ((Number(h ?? 0) * 60 + Number(mi)) * 60 + Number(se)) * 1000 + Number(ms);
}

/** Cues of a WebVTT file (NOTE, STYLE and REGION blocks skipped); `null` when not WebVTT. */
export function parseVtt(text: string): VttCue[] | null {
  if (!isWebVtt(text)) return null;
  const blocks = text
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/)
    .slice(1);
  const cues: VttCue[] = [];
  for (const block of blocks) {
    const lines = block.split('\n').filter((l, i) => i > 0 || l.trim() !== '');
    if (lines.length === 0) continue;
    const head = lines[0] ?? '';
    if (/^(NOTE|STYLE|REGION)(?:[ \t]|$)/.test(head)) continue;
    const timingAt = lines.findIndex((l) => l.includes('-->'));
    if (timingAt < 0 || timingAt > 1) continue;
    const [a = '', rest = ''] = (lines[timingAt] ?? '').split('-->');
    const start = parseVttTime(a);
    const end = parseVttTime(rest.trim().split(/[ \t]+/)[0] ?? '');
    if (start === null || end === null || end < start) continue;
    cues.push({
      id: timingAt === 1 ? head : null,
      start,
      end,
      text: lines.slice(timingAt + 1).join('\n'),
    });
  }
  return cues;
}
