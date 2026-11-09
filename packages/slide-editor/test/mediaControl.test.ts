import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  MediaCarry,
  MediaController,
  TICK_MS,
  crossedBookmarks,
  fadeGain,
  isWebVtt,
  parseVtt,
  parseVttTime,
  volumeAt,
  type MediaLike,
  type PlaybackOptions,
} from '../src/mediaControl';
import type { SlideAnimInfo, SlideShowInfo } from '@blueoffice/platform';
import { effectFrames, triggerEffects } from '../src/player';

/** A media element whose time moves only when the test says so. */
class FakeMedia implements MediaLike {
  currentTime = 0;
  duration = 10;
  volume = 1;
  muted = false;
  paused = true;
  plays = 0;
  play() {
    this.paused = false;
    this.plays += 1;
    return Promise.resolve();
  }
  pause() {
    this.paused = true;
  }
}

// A 10 s clip trimmed to 1 s … 8 s, fading in over 1 s and out over 2 s, at 80 %.
const opts = (o: Partial<PlaybackOptions> = {}): PlaybackOptions => ({
  trimStartMs: 1000,
  trimEndMs: 2000,
  fadeInMs: 1000,
  fadeOutMs: 2000,
  bookmarks: [
    { name: 'A', timeMs: 3000 },
    { name: 'B', timeMs: 6000 },
  ],
  volume: 80,
  mute: false,
  loop: false,
  rewind: false,
  ...o,
});

describe('fades and bookmarks', () => {
  it('ramps the volume linearly inside the trimmed window', () => {
    const o = opts();
    // Fade in: 0 at the trim start, half way after 0.5 s.
    expect(fadeGain(1000, o, 10_000)).toBe(0);
    expect(fadeGain(1500, o, 10_000)).toBe(0.5);
    expect(fadeGain(5000, o, 10_000)).toBe(1);
    // Fade out: the window ends at 10 − 2 = 8 s; 1 s before it, half way.
    expect(fadeGain(7000, o, 10_000)).toBe(0.5);
    expect(fadeGain(7500, o, 10_000)).toBe(0.25);
    expect(fadeGain(8000, o, 10_000)).toBe(0);
    expect(volumeAt(1500, o, 10_000)).toBeCloseTo(0.4, 10);
    expect(volumeAt(7500, o, 10_000)).toBeCloseTo(0.2, 10);
    expect(volumeAt(5000, opts({ mute: true }), 10_000)).toBe(0);
    // Unknown duration: no fade-out yet.
    expect(fadeGain(7500, o, NaN)).toBe(1);
  });

  it('reports the bookmarks passed going forward', () => {
    const b = opts().bookmarks;
    expect(crossedBookmarks(2000, 3000, b).map((x) => x.name)).toEqual(['A']);
    expect(crossedBookmarks(3000, 7000, b).map((x) => x.name)).toEqual(['B']);
    expect(crossedBookmarks(0, 9000, b).map((x) => x.name)).toEqual(['A', 'B']);
    expect(crossedBookmarks(7000, 2000, b)).toEqual([]);
  });
});

describe('MediaController', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const step = (el: FakeMedia, seconds: number) => {
    el.currentTime = seconds;
    vi.advanceTimersByTime(TICK_MS);
  };

  it('starts at the trim start, fades, fires bookmarks and stops at the trim end', () => {
    const el = new FakeMedia();
    const marks: string[] = [];
    const states: boolean[] = [];
    const c = new MediaController(el, opts(), {
      onBookmark: (n) => marks.push(n),
      onPlaying: (p) => states.push(p),
    });
    c.start();
    expect(el.currentTime).toBe(1);
    expect(el.paused).toBe(false);
    expect(el.volume).toBe(0);
    step(el, 1.5);
    expect(el.volume).toBeCloseTo(0.4, 10);
    step(el, 3.5);
    expect(marks).toEqual(['A']);
    expect(el.volume).toBeCloseTo(0.8, 10);
    step(el, 6);
    expect(marks).toEqual(['A', 'B']);
    step(el, 7.5);
    expect(el.volume).toBeCloseTo(0.2, 10);
    // Past 8 s: paused where it is (no rewind), stopped.
    step(el, 8.2);
    expect(el.paused).toBe(true);
    expect(el.currentTime).toBe(8.2);
    expect(c.isPlaying).toBe(false);
    expect(states).toEqual([true, false]);
    c.dispose();
  });

  it('rewinds after playing', () => {
    const el = new FakeMedia();
    const c = new MediaController(el, opts({ rewind: true }));
    c.start();
    step(el, 8.5);
    expect(el.paused).toBe(true);
    expect(el.currentTime).toBe(1);
    c.dispose();
  });

  it('loops until stopped', () => {
    const el = new FakeMedia();
    const marks: string[] = [];
    const c = new MediaController(el, opts({ loop: true }), { onBookmark: (n) => marks.push(n) });
    c.start();
    step(el, 8.1);
    expect(el.currentTime).toBe(1);
    expect(el.paused).toBe(false);
    expect(c.isPlaying).toBe(true);
    expect(marks).toEqual(['A', 'B']);
    // The next round passes the bookmarks again.
    step(el, 3.2);
    expect(marks).toEqual(['A', 'B', 'A']);
    c.stop();
    expect(el.paused).toBe(true);
    expect(el.currentTime).toBe(1);
    c.dispose();
  });

  it('keeps playback inside the window and toggles on clicks', () => {
    const el = new FakeMedia();
    const c = new MediaController(el, opts());
    c.start();
    // Seeked before the trim start with the controls: back to 1 s.
    step(el, 0.2);
    expect(el.currentTime).toBe(1);
    c.toggle();
    expect(el.paused).toBe(true);
    step(el, 4);
    // Paused: nothing moves.
    expect(c.isPlaying).toBe(false);
    c.toggle();
    expect(el.paused).toBe(false);
    expect(el.currentTime).toBe(4);
    // Resuming after the end starts over.
    c.pause();
    el.currentTime = 9;
    c.resume();
    expect(el.currentTime).toBe(1);
    c.dispose();
  });
});

describe('MediaCarry', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('plays on for the clip span, then stops and releases', () => {
    const carry = new MediaCarry();
    const el = new FakeMedia();
    el.currentTime = 4;
    const release = vi.fn();
    // Clip on slide 2 playing across 3 slides: slides 2, 3, 4.
    carry.adopt('2:5', el, opts(), 2, 3, release);
    expect(el.paused).toBe(false);
    expect(el.currentTime).toBe(4);
    carry.showSlide(3);
    carry.showSlide(4);
    expect(carry.key).toBe('2:5');
    expect(release).not.toHaveBeenCalled();
    carry.showSlide(5);
    expect(carry.key).toBeNull();
    expect(el.paused).toBe(true);
    expect(release).toHaveBeenCalledTimes(1);
  });

  it('stops when the show goes back to the clip slide or the clip ends', () => {
    const carry = new MediaCarry();
    const a = new FakeMedia();
    const ra = vi.fn();
    carry.adopt('1:4', a, opts(), 1, 999, ra);
    carry.showSlide(1);
    expect(ra).toHaveBeenCalledTimes(1);
    const b = new FakeMedia();
    const rb = vi.fn();
    b.currentTime = 7;
    carry.adopt('1:4', b, opts(), 1, 999, rb);
    b.currentTime = 8.5;
    vi.advanceTimersByTime(TICK_MS);
    expect(rb).toHaveBeenCalledTimes(1);
    expect(carry.key).toBeNull();
  });
});

describe('WebVTT', () => {
  it('reads cue times', () => {
    expect(parseVttTime('00:01.000')).toBe(1000);
    expect(parseVttTime('01:02.500')).toBe(62_500);
    expect(parseVttTime('01:00:00.000')).toBe(3_600_000);
    expect(parseVttTime('1:00.000')).toBeNull();
    expect(parseVttTime('00:61.000')).toBeNull();
    const text = [
      'WEBVTT - phụ đề',
      '',
      'NOTE soạn ngày 10/11',
      '',
      'mo-dau',
      '00:00:01.000 --> 00:00:03.500 align:start',
      'Xin chào',
      'các bạn',
      '',
      '01:02.500 --> 01:05.000',
      'Tạm biệt',
      '',
      '00:09.000 --> 00:08.000',
      'ngược',
    ].join('\r\n');
    expect(isWebVtt(text)).toBe(true);
    expect(parseVtt(text)).toEqual([
      { id: 'mo-dau', start: 1000, end: 3500, text: 'Xin chào\ncác bạn' },
      { id: null, start: 62_500, end: 65_000, text: 'Tạm biệt' },
    ]);
    expect(parseVtt('1\n00:00:01,000 --> 00:00:02,000\nSRT')).toBeNull();
    expect(isWebVtt('\uFEFFWEBVTT')).toBe(true);
    expect(isWebVtt('WEBVTTX')).toBe(false);
  });
});

describe('show effects of clips', () => {
  const anim = (a: Partial<SlideAnimInfo>): SlideAnimInfo =>
    ({
      index: 0,
      shapeId: 4,
      shapeName: 'clip',
      effect: 'other',
      class: 'other',
      direction: null,
      trigger: 'onClick',
      delayMs: 0,
      durMs: 1,
      click: 1,
      startMs: 0,
      endMs: 1,
      rect: [0, 0, 10, 10],
      layer: 1,
      ...a,
    }) as SlideAnimInfo;

  it('draws nothing for a media call and splits clicks from bookmarks', () => {
    expect(effectFrames(anim({ mediaCmd: 'playFrom(0.0)' }), 100, 100)).toEqual([]);
    const show = {
      animations: [
        anim({ index: 0, shapeId: 20, triggerShape: 4, layer: 2, class: 'entr', effect: 'fade' }),
        anim({
          index: 1,
          shapeId: 21,
          triggerShape: 4,
          triggerBookmark: 'Mốc 1',
          layer: 3,
          class: 'entr',
          effect: 'fade',
        }),
      ],
      layers: [],
    } as unknown as SlideShowInfo;
    expect(triggerEffects(show, 4).map((e) => e.layer)).toEqual([2]);
    expect(triggerEffects(show, 4, 'Mốc 1').map((e) => e.layer)).toEqual([3]);
    expect(triggerEffects(show, 4, 'khác')).toEqual([]);
  });
});
