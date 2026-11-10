import { describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '@blueoffice/editor';
import type { SlideCommand, SlideZoomInfo } from '@blueoffice/platform';
import {
  newRun,
  runLink,
  runStep,
  runZoom,
  zoomFrames,
  type RunPos,
  type ShowRun,
} from '../src/presenting';
import { insertCommand, registerZoomCommands, zoomOptions, type ZoomState } from '../src/zoom';

/** SLIDE-ZOOM-001: slide show navigation of zooms (jump, return order), dialog commands, tab. */

const clicks = (s: number) => (s === 1 ? 1 : 0);
const at = (run: ShowRun, i: number): RunPos => ({
  at: i,
  slide: run.order[i]!,
  click: 0,
  enter: 'transition',
});

function walk(run: ShowRun, pos: RunPos, steps: number, count: number) {
  const seen: number[] = [];
  let r = run;
  let p = pos;
  for (let i = 0; i < steps; i++) {
    const n = runStep(r, p, 'next', count, clicks);
    r = n.run;
    p = n.pos;
    seen.push(p.at >= r.order.length ? -1 : p.slide);
  }
  return { seen, run: r, pos: p };
}

describe('zoom navigation', () => {
  // Deck: 0 intro, 1 summary (one click), 2–3 section A, 4 section B, 5 end.
  const order = [0, 1, 2, 3, 4, 5];
  const rect: [number, number, number, number] = [100, 200, 300, 150];

  it('jumps into a section and comes back to the zoom slide after it', () => {
    const run = newRun(order);
    const pos = { ...at(run, 1), click: 1 };
    const r = runZoom(
      run,
      pos,
      { kind: 'zoom', slides: [2, 3], return: true, transitionMs: 800, rect },
      6,
      clicks,
    );
    expect(r.run.order).toEqual([2, 3]);
    expect(r.pos.slide).toBe(2);
    expect(r.pos.zoom).toEqual({ rect, ms: 800, out: false });
    // 3, then back to the summary slide (its click done, zooming out), then on to 2.
    const w = walk(r.run, r.pos, 3, 6);
    expect(w.seen).toEqual([3, 1, 2]);
    const back = runStep(r.run, { ...r.pos, at: 1, slide: 3 }, 'next', 6, clicks);
    expect(back.pos.slide).toBe(1);
    expect(back.pos.click).toBe(1);
    expect(back.pos.zoom).toEqual({ rect, ms: 800, out: true });
    expect(back.run.order).toEqual(order);
  });

  it('two zooms in a row each come back in order', () => {
    const run = newRun(order);
    const first = runZoom(run, at(run, 1), { kind: 'zoom', slides: [4], return: true }, 6, clicks);
    // Back on the summary slide, the second zoom.
    const backed = runStep(first.run, first.pos, 'next', 6, clicks);
    expect(backed.pos.slide).toBe(1);
    const second = runZoom(
      backed.run,
      backed.pos,
      { kind: 'zoom', slides: [2, 3], return: true },
      6,
      clicks,
    );
    expect(walk(second.run, second.pos, 2, 6).seen).toEqual([3, 1]);
    // No zoom transition without a duration (the target's own transition plays).
    expect(second.pos.zoom).toBeUndefined();
  });

  it('a zoom inside a zoomed section returns to the inner zoom, then the outer one', () => {
    const run = newRun(order);
    const outer = runZoom(
      run,
      at(run, 1),
      { kind: 'zoom', slides: [2, 3], return: true },
      6,
      clicks,
    );
    const inner = runZoom(
      outer.run,
      outer.pos,
      { kind: 'zoom', slides: [5], return: true },
      6,
      clicks,
    );
    expect(inner.run.back).toHaveLength(2);
    // 5 → back to 2 (inner zoom's slide) → 3 → back to 1.
    expect(walk(inner.run, inner.pos, 3, 6).seen).toEqual([2, 3, 1]);
  });

  it('without return the show goes on from the target', () => {
    const run = newRun(order);
    const r = runZoom(run, at(run, 1), { kind: 'zoom', slides: [4], return: false }, 6, clicks);
    expect(r.run.order).toEqual(order);
    expect(r.pos.at).toBe(4);
    expect(walk(r.run, r.pos, 2, 6).seen).toEqual([5, -1]);
  });

  it('a hidden target without return plays, then the slides after the zoom', () => {
    const run = newRun([0, 1, 2, 3]);
    const r = runZoom(run, at(run, 1), { kind: 'zoom', slides: [6], return: false }, 7, clicks);
    expect(r.run.order).toEqual([6, 2, 3]);
    expect(walk(r.run, r.pos, 2, 7).seen).toEqual([2, 3]);
  });

  it('runLink routes zoom links; no slides does nothing', () => {
    const run = newRun(order);
    const pos = at(run, 1);
    const r = runLink(run, pos, { kind: 'zoom', slides: [4], return: true }, 6, clicks, []);
    expect(r).not.toBe('end');
    if (r !== 'end') expect(r.pos.slide).toBe(4);
    const none = runZoom(run, pos, { kind: 'zoom', slides: [99], return: true }, 6, clicks);
    expect(none.pos).toBe(pos);
  });

  it('zoom frames grow the target out of the box and back', () => {
    const f = zoomFrames({ rect: [240, 135, 480, 270], ms: 1000, out: false }, 960, 540);
    expect(f.incoming[0]!.transform).toBe('translate(25%, 25%) scale(0.5, 0.5)');
    expect(f.incoming[1]!.transform).toBe('translate(0%, 0%) scale(1, 1)');
    expect(f.outgoing[1]!.transform).toBe('translate(-50%, -50%) scale(2, 2)');
    const o = zoomFrames({ rect: [240, 135, 480, 270], ms: 1000, out: true }, 960, 540);
    expect(o.incoming[0]!.transform).toBe(f.outgoing[1]!.transform);
    expect(o.outgoing[1]!.transform).toBe(f.incoming[0]!.transform);
  });
});

describe('zoom commands', () => {
  it('builds the insert command of the dialog', () => {
    expect(insertCommand('slide', [3, 1, 3], {})).toEqual({
      cmd: 'zoomInsertSlides',
      slides: [1, 3],
    });
    expect(insertCommand('section', [2], {})).toEqual({ cmd: 'zoomInsertSections', sections: [2] });
    expect(insertCommand('summary', [4, 2], { title: 'Tổng quan' })).toEqual({
      cmd: 'zoomInsertSummary',
      slides: [2, 4],
      names: { title: 'Tổng quan' },
    });
    expect(insertCommand('slide', [], {})).toBeNull();
  });

  const info: SlideZoomInfo = {
    id: 7,
    kind: 'slide',
    objects: [
      {
        slides: [2],
        section: null,
        id: '{A}',
        returnToParent: true,
        cover: false,
        transitionMs: null,
        showBg: true,
      },
    ],
  };

  it('reads the options of the selected zoom', () => {
    expect(zoomOptions(info)).toEqual({
      returnToParent: true,
      transition: false,
      durationMs: 1000,
      showBg: true,
      cover: false,
    });
  });

  it('toggles the options of the selected zoom', async () => {
    type S = ZoomState & { hasDeck: boolean };
    const reg = new CommandRegistry<S>();
    const sent: SlideCommand[] = [];
    const openInsert = vi.fn();
    registerZoomCommands(reg, (c) => sent.push(c), { openInsert, pickImage: vi.fn() });
    const s: S = { hasDeck: true, isZoom: true, zoom: info };
    expect(reg.isChecked('zoom.return', s)).toBe(true);
    expect(reg.isChecked('zoom.transition', s)).toBe(false);
    await reg.execute('zoom.return', s);
    await reg.execute('zoom.transition', s);
    await reg.execute('zoom.background', s);
    await reg.execute('insert.zoom.summary', s);
    expect(sent).toEqual([
      { cmd: 'zoomOptions', returnToParent: false },
      { cmd: 'zoomOptions', transition: true },
      { cmd: 'zoomOptions', showBg: false },
    ]);
    expect(openInsert).toHaveBeenCalledWith('summary');
    expect(reg.isEnabled('zoom.return', { hasDeck: true })).toBe(false);
  });
});
