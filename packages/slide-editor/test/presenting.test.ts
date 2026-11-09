import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_SHOW_SETUP, type SlideCustomShow } from '@blueoffice/platform';
import {
  InactivityTimer,
  RehearsalClock,
  dragBox,
  kioskAllows,
  newRun,
  playOrder,
  runGoto,
  runLink,
  runStep,
  startAt,
  toggleCover,
  unzoom,
  zoomAt,
  zoomTransform,
  type RunPos,
  type ShowRun,
} from '../src/presenting';
import { InkLayer, segmentDistance, strokeHit, strokePath } from '../src/ink';
import { showAction } from '../src/show';

const shows: SlideCustomShow[] = [
  { id: 0, name: 'Ngắn', slides: [3, 0] },
  { id: 4, name: 'Phụ lục', slides: [5, 6, 9] },
];

describe('playing order', () => {
  it('plays every slide but the hidden ones', () => {
    expect(playOrder(5)).toEqual([0, 1, 2, 3, 4]);
    expect(playOrder(5, { hidden: [1, 3] })).toEqual([0, 2, 4]);
  });

  it('plays a range of slide numbers (1-based, clamped)', () => {
    const setup = { ...DEFAULT_SHOW_SETUP, range: { kind: 'range' as const, from: 2, to: 9 } };
    // Slides 2..5 of 5 → indices 1..4, hidden slide 3 (index 2) left out.
    expect(playOrder(5, { setup, hidden: [2] })).toEqual([1, 3, 4]);
  });

  it('plays a custom show in its own order, hidden slides included', () => {
    const setup = { ...DEFAULT_SHOW_SETUP, range: { kind: 'custom' as const, id: 0 } };
    expect(playOrder(5, { setup, customShows: shows, hidden: [3] })).toEqual([3, 0]);
    // Started explicitly; slides past the end are left out.
    expect(playOrder(7, { custom: 4, customShows: shows })).toEqual([5, 6]);
    // Unknown show: the whole deck.
    expect(playOrder(3, { custom: 7, customShows: shows })).toEqual([0, 1, 2]);
  });

  it('starts from the current slide, or the next one in the order', () => {
    expect(startAt([1, 3, 4], 3)).toBe(1);
    expect(startAt([1, 3, 4], 2)).toBe(1);
    expect(startAt([1, 3, 4], 9)).toBe(0);
  });
});

describe('navigation in a playing order', () => {
  const clicks = (s: number) => (s === 3 ? 2 : 0);
  const at = (run: ShowRun, i: number): RunPos => ({
    at: i,
    slide: run.order[i]!,
    click: 0,
    enter: 'transition',
  });

  it('plays clicks, then the next slide of the order, then the end screen', () => {
    const run = newRun([3, 0]);
    let p = at(run, 0);
    let r = runStep(run, p, 'next', 5, clicks);
    expect(r.pos).toEqual({ at: 0, slide: 3, click: 1, enter: 'click' });
    p = { ...r.pos, click: 2 };
    r = runStep(run, p, 'next', 5, clicks);
    expect(r.pos).toEqual({ at: 1, slide: 0, click: 0, enter: 'transition' });
    r = runStep(r.run, r.pos, 'next', 5, clicks);
    // End screen: slide = count.
    expect(r.pos).toEqual({ at: 2, slide: 5, click: 0, enter: 'jump' });
    // Back from the end: the last slide with its effects played.
    r = runStep(r.run, r.pos, 'prev', 5, clicks);
    expect(r.pos).toEqual({ at: 1, slide: 0, click: 0, enter: 'jump' });
    r = runStep(r.run, r.pos, 'prev', 5, clicks);
    expect(r.pos).toEqual({ at: 0, slide: 3, click: 2, enter: 'jump' });
  });

  it('loops to the first slide', () => {
    const run = newRun([1, 2], { loop: true });
    const r = runStep(run, at(run, 1), 'next', 3, clicks);
    expect(r.pos).toEqual({ at: 0, slide: 1, click: 0, enter: 'transition' });
  });

  it('without animation shows each slide with every effect done, no transition', () => {
    const run = newRun([0, 3], { still: true });
    const r = runStep(run, { at: 0, slide: 0, click: 0, enter: 'jump' }, 'next', 5, clicks);
    expect(r.pos).toEqual({ at: 1, slide: 3, click: 2, enter: 'jump' });
  });

  it('a hyperlink plays a custom show and comes back to its slide', () => {
    const run = newRun([0, 1, 2]);
    const p = at(run, 1);
    const go = runLink(run, p, { kind: 'customShow', id: 0, return: true }, 5, clicks, shows);
    if (go === 'end') throw new Error('end');
    expect(go.run.order).toEqual([3, 0]);
    expect(go.pos).toEqual({ at: 0, slide: 3, click: 0, enter: 'transition' });
    let r = runStep(go.run, { ...go.pos, click: 2 }, 'next', 5, clicks);
    expect(r.pos.slide).toBe(0);
    // After the custom show's last slide: back to slide 1 (place 1), effects played.
    r = runStep(r.run, r.pos, 'next', 5, clicks);
    expect(r.run.order).toEqual([0, 1, 2]);
    expect(r.pos).toEqual({ at: 1, slide: 1, click: 0, enter: 'transition' });
    r = runStep(r.run, r.pos, 'next', 5, clicks);
    expect(r.pos.slide).toBe(2);
  });

  it('a custom show without return ends the show after its last slide', () => {
    const run = newRun([0, 1, 2]);
    const go = runLink(run, at(run, 0), { kind: 'customShow', id: 4 }, 7, clicks, shows);
    if (go === 'end') throw new Error('end');
    let r = runStep(go.run, go.pos, 'next', 7, clicks);
    expect(r.pos.slide).toBe(6);
    r = runStep(r.run, r.pos, 'next', 7, clicks);
    expect(r.pos).toEqual({ at: 2, slide: 7, click: 0, enter: 'jump' });
  });

  it('jump links, the last viewed slide and the slide grid', () => {
    const run = newRun([0, 1, 2, 3]);
    expect(runLink(run, at(run, 0), { kind: 'end' }, 4, clicks, shows)).toBe('end');
    const g = runGoto(run, at(run, 0), 2, clicks);
    expect(g.pos).toEqual({ at: 2, slide: 2, click: 0, enter: 'transition' });
    const back = runLink(g.run, g.pos, { kind: 'lastViewed' }, 4, clicks, shows);
    if (back === 'end') throw new Error('end');
    expect(back.pos.slide).toBe(0);
    const last = runLink(run, at(run, 0), { kind: 'last' }, 4, clicks, shows);
    if (last === 'end') throw new Error('end');
    expect(last.pos).toEqual({ at: 3, slide: 3, click: 2, enter: 'jump' });
    const to = runLink(run, at(run, 0), { kind: 'slide', slide: 2 }, 4, clicks, shows);
    if (to === 'end') throw new Error('end');
    expect(to.pos.slide).toBe(2);
  });
});

describe('kiosk', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('restarts after the inactivity period, input restarts the wait', () => {
    vi.useFakeTimers();
    const idle = vi.fn();
    const t = new InactivityTimer(300_000, idle);
    t.start();
    vi.advanceTimersByTime(299_999);
    expect(idle).not.toHaveBeenCalled();
    t.poke();
    vi.advanceTimersByTime(299_999);
    expect(idle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(idle).toHaveBeenCalledTimes(1);
    // Left alone: restarts again one period later.
    vi.advanceTimersByTime(300_000);
    expect(idle).toHaveBeenCalledTimes(2);
    t.stop();
    vi.advanceTimersByTime(900_000);
    expect(idle).toHaveBeenCalledTimes(2);
    expect(t.running).toBe(false);
  });

  it('only Esc works at a kiosk', () => {
    expect(kioskAllows(showAction('Escape'))).toBe(true);
    expect(kioskAllows(showAction('ArrowRight'))).toBe(false);
    expect(kioskAllows(showAction(' '))).toBe(false);
  });
});

describe('rehearsal clock', () => {
  it('times each slide in ms, summing a slide shown twice, without pauses', () => {
    const c = new RehearsalClock(1000);
    c.enter(0, 1000);
    // Slide 1: 4.5 s.
    c.enter(1, 5500);
    expect(c.current(7000)).toBe(1500);
    // A 10 s pause on slide 2 does not count.
    c.pause(7000);
    expect(c.current(12_000)).toBe(1500);
    c.resume(17_000);
    // Slide 2: 1.5 s + 2.25 s = 3.75 s.
    c.enter(0, 19_250);
    // Back on slide 1 for 0.75 s: 4.5 + 0.75 = 5.25 s.
    expect(c.total(20_000)).toBe(9000);
    expect(c.finish(20_000)).toEqual([
      { slide: 0, ms: 5250 },
      { slide: 1, ms: 3750 },
    ]);
  });

  it('repeats the current slide', () => {
    const c = new RehearsalClock(0);
    c.enter(2, 0);
    c.repeat(8000);
    expect(c.finish(11_000)).toEqual([{ slide: 2, ms: 3000 }]);
  });
});

describe('zoom', () => {
  const stage = { width: 800, height: 450 };

  it('fills the stage with the dragged rectangle', () => {
    // 200 × 112.5 rectangle at (200, 100): scale min(800/200, 450/112.5) = 4; its
    // centre (300, 156.25) goes to (400, 225): x = 400 − 4·300 = −800, y = 225 − 4·156.25 = −400.
    const z = zoomTransform({ x: 200, y: 100, w: 200, h: 112.5 }, stage);
    expect(z).toEqual({ scale: 4, x: -800, y: -400 });
    // The rectangle's top-left corner lands on the stage's top-left corner.
    expect(4 * 200 + z.x).toBe(0);
    expect(4 * 100 + z.y).toBe(0);
  });

  it('keeps the slide covering the stage near its edges', () => {
    // 400 × 225 at the top-left corner: scale 2, centre (100, 52.5) → x = 400 − 200 = 200 → clamped to 0.
    expect(zoomTransform({ x: -100, y: -60, w: 400, h: 225 }, stage)).toEqual({
      scale: 2,
      x: 0,
      y: 0,
    });
    // Bottom-right: x ≥ 800 − 1600 = −800, y ≥ 450 − 900 = −450.
    expect(zoomTransform({ x: 700, y: 400, w: 400, h: 225 }, stage)).toEqual({
      scale: 2,
      x: -800,
      y: -450,
    });
  });

  it('zooms around a clicked point and maps stage points back to the slide', () => {
    const z = zoomAt({ x: 400, y: 225 }, 2, stage);
    // 400 × 225 box centred on the centre: x = 400 − 2·400 = −400, y = 225 − 2·225 = −225.
    expect(z).toEqual({ scale: 2, x: -400, y: -225 });
    // Stage centre → slide centre (960 × 540 slide on an 800 px stage: k = 1.2).
    expect(unzoom({ x: 400, y: 225 }, z, 1.2)).toEqual({ x: 480, y: 270 });
    expect(dragBox({ x: 50, y: 80 }, { x: 10, y: 20 })).toEqual({ x: 10, y: 20, w: 40, h: 60 });
  });
});

describe('black / white screen', () => {
  it('toggles with B and W', () => {
    expect(showAction('b')).toBe('black');
    expect(showAction('W')).toBe('white');
    expect(showAction(',')).toBe('white');
    expect(toggleCover('none', 'black')).toBe('black');
    expect(toggleCover('black', 'black')).toBe('none');
    expect(toggleCover('black', 'white')).toBe('white');
    expect(toggleCover('white', 'white')).toBe('none');
  });
});

describe('ink', () => {
  it('measures distances to segments', () => {
    // (5, 5) is 5 above the segment (0,0)–(10,0); (13, 4) is 5 from its end (10, 0).
    expect(segmentDistance([5, 5], [0, 0], [10, 0])).toBe(5);
    expect(segmentDistance([13, 4], [0, 0], [10, 0])).toBe(5);
    expect(
      strokeHit(
        {
          points: [
            [0, 0],
            [10, 0],
          ],
          color: '000000',
          width: 2,
        },
        [5, 5],
        4,
      ),
    ).toBe(true);
    expect(
      strokeHit(
        {
          points: [
            [0, 0],
            [10, 0],
          ],
          color: '000000',
          width: 2,
        },
        [5, 5],
        3.9,
      ),
    ).toBe(false);
  });

  it('draws, erases and clears strokes per slide', () => {
    const ink = new InkLayer();
    expect(ink.isEmpty()).toBe(true);
    ink.begin(1, 'pen', 'D7261E', [10, 10]);
    ink.extend([10.2, 10.2]);
    ink.extend([20, 10]);
    ink.extend([30, 20]);
    ink.end();
    ink.begin(1, 'highlighter', 'FFE600', [100, 100]);
    ink.extend([200, 100]);
    ink.end();
    ink.begin(3, 'pen', 'D7261E', [5, 5]);
    ink.end();
    // The 0.28 pt move is skipped.
    expect(ink.strokes(1)[0]!.points).toEqual([
      [10, 10],
      [20, 10],
      [30, 20],
    ]);
    expect(ink.strokes(1)[1]).toMatchObject({ width: 14, highlighter: true });
    expect(strokePath(ink.strokes(1)[0]!)).toBe('M10 10L20 10L30 20');
    expect(strokePath(ink.strokes(3)[0]!)).toBe('M5 5L5 5');
    // Erasing at (150, 106): 6 from the highlighter (half width 7) → removed.
    expect(ink.erase(1, [150, 106], 0)).toBe(true);
    expect(ink.strokes(1)).toHaveLength(1);
    expect(ink.erase(1, [150, 106], 0)).toBe(false);
    expect(ink.toCommand().map((s) => [s.slide, s.strokes.length])).toEqual([
      [1, 1],
      [3, 1],
    ]);
    ink.clear(3);
    expect(ink.toCommand()).toHaveLength(1);
    ink.clear();
    expect(ink.isEmpty()).toBe(true);
  });
});
