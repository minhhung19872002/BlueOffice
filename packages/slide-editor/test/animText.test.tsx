// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  EngineHost,
  PageDisplayList,
  SlideAnimInfo,
  SlideCommand,
  SlideEditResult,
  SlideShowInfo,
} from '@blueoffice/platform';
import { GlyphStore, ImageStore, renderPage, type CanvasLike } from '@blueoffice/canvas-host';
import {
  AnimationPane,
  MotionPathOverlay,
  anchors,
  drawnPath,
  effectsAt,
  formatMotionPath,
  morphLayers,
  motionPathSvg,
  moveAnchor,
  parseMotionPath,
  pieceOrder,
  playFrames,
  tintAfter,
  tintFrames,
  tintsShown,
} from '../src';

afterEach(cleanup);

const page = (n = 0): PageDisplayList => ({ width: 1000, height: 500, commands: n ? [] : [] });

const anim = (a: Partial<SlideAnimInfo>): SlideAnimInfo => ({
  index: 0,
  shapeId: 4,
  shapeName: 'A',
  effect: 'fade',
  class: 'entr',
  direction: null,
  trigger: 'onClick',
  delayMs: 0,
  durMs: 1000,
  click: 1,
  startMs: 0,
  endMs: 1000,
  rect: [400, 200, 100, 50],
  layer: 1,
  ...a,
});

const t = { beginMs: 0, durMs: 1000, repeat: 1, autoRev: false, hold: true };

describe('colour changes (§19.5.2 animClr)', () => {
  // source: the colour drawing fades in over the plain layer with the behaviour's progress
  // (opaque drawings blend linearly: the RGB interpolation of clrSpc rgb) and stays when it holds.
  it('fades the colour drawing in and keeps it', () => {
    const a = anim({
      class: 'emph',
      effect: 'fillColor',
      tint: page(),
      play: {
        durMs: 1000,
        tracks: [
          { ...t, beginMs: 200, durMs: 800, kind: 'color', attr: 'fillcolor', to: 0x00b050 },
        ],
      },
    });
    const f = tintFrames(a, 10)!;
    expect(f[0]?.opacity).toBe(0);
    // At 600 ms: half of the 800 ms change from 200 ms.
    expect(f[6]?.opacity).toBe(0.5);
    expect(f[10]?.opacity).toBe(1);
    expect(tintAfter(a)).toBe(1);
    // Not held: the colour goes back.
    expect(
      tintAfter({ ...a, play: { durMs: 1000, tracks: [{ ...a.play!.tracks[0]!, hold: false }] } }),
    ).toBe(0);
    // No target colour, or no drawing: nothing to show.
    expect(tintFrames({ ...a, tint: undefined })).toBeNull();
    const show: SlideShowInfo = {
      slide: 0,
      width: 1000,
      height: 500,
      transition: null,
      layers: [
        { ids: [], backdrop: true, page: page() },
        { ids: [4], backdrop: false, page: page() },
      ],
      animations: [a],
      clicks: 1,
      media: [],
    };
    expect(tintsShown(show, 0)).toEqual({ 0: false });
    expect(tintsShown(show, 1)).toEqual({ 0: true });
    expect(effectsAt(show, 1)[0]?.tint?.length).toBe(25);
  });
});

describe('text word by word / letter by letter (§19.5 iterate)', () => {
  const pieces = [
    { word: 0, letter: 0 },
    { word: 0, letter: 1 },
    { word: 1, letter: 2 },
  ];

  // source: piece k starts (its word's or letter's rank) × the step; backwards reverses the order.
  it('starts each piece after the previous one', () => {
    expect(pieceOrder(anim({ textBy: 'wd', textStepMs: 100 }), pieces)).toEqual([0, 0, 100]);
    expect(pieceOrder(anim({ textBy: 'lt', textStepMs: 50 }), pieces)).toEqual([0, 50, 100]);
    expect(pieceOrder(anim({ textBy: 'lt', textStepMs: 50, textBackwards: true }), pieces)).toEqual(
      [100, 50, 0],
    );
  });

  // source: a piece turns about its own centre; the move is the shape's (formulas of the shape).
  it('plays the effect on every piece in its own box', () => {
    const play = {
      durMs: 1000,
      tracks: [
        { ...t, kind: 'rotate' as const, by: 360 },
        {
          ...t,
          kind: 'anim' as const,
          attr: 'ppt_y',
          keys: [
            [0, 1.25],
            [1, 0.45],
          ] as [number, number][],
          discrete: false,
        },
      ],
    };
    const a = anim({ class: 'emph', play, textBy: 'lt', textStepMs: 100, endMs: 1200 });
    const f = playFrames(a, 1000, 500, 24, [100, 50, 20, 10])!;
    // Centre of the letter (110, 55) on a 1000 × 500 slide.
    expect(f[0]?.transformOrigin).toBe('11% 11%');
    // From 0.8 of the slide below the shape (y 1.25 − 0.45).
    expect(String(f[0]?.transform)).toContain('translate(0%, 80%)');
    const show: SlideShowInfo = {
      slide: 0,
      width: 1000,
      height: 500,
      transition: null,
      layers: [
        { ids: [], backdrop: true, page: page() },
        {
          ids: [4],
          backdrop: false,
          page: page(),
          text: {
            body: page(),
            words: 2,
            pieces: pieces.map((p, k) => ({
              ...p,
              page: page(),
              rect: [100 + 20 * k, 50, 20, 10] as [number, number, number, number],
              text: 'abc'[k] ?? '',
            })),
          },
        },
      ],
      animations: [a],
      clicks: 1,
      media: [],
    };
    const e = effectsAt(show, 1)[0]!;
    // One piece's run (the effect's duration), each letter 100 ms after the previous one.
    expect(e.duration).toBe(1000);
    expect(e.pieces?.map((p) => p.delay)).toEqual([0, 100, 200]);
    expect(e.pieces?.[1]?.frames[0]?.transformOrigin).toBe('13% 11%');
  });
});

describe('motion paths (§19.5.4)', () => {
  // source: absolute and relative commands read into absolute points; written back with E.
  it('parse, move points and write back', () => {
    const segs = parseMotionPath('M 0 0 l 0.1 0 C 0.2 0 0.2 0.1 0.3 0.1 E');
    expect(segs).toEqual([
      { c: 'M', pts: [[0, 0]] },
      { c: 'L', pts: [[0.1, 0]] },
      {
        c: 'C',
        pts: [
          [0.2, 0],
          [0.2, 0.1],
          [0.3, 0.1],
        ],
      },
    ]);
    expect(anchors(segs).map((x) => x.pt)).toEqual([
      [0, 0],
      [0.1, 0],
      [0.3, 0.1],
    ]);
    // Moving the corner at 0.1 moves the curve's first control along.
    const moved = moveAnchor(segs, 1, 0, 0.05);
    expect(formatMotionPath(moved)).toBe('M 0 0 L 0.1 0.05 C 0.2 0.05 0.2 0.1 0.3 0.1 E');
    expect(formatMotionPath(parseMotionPath('M 0 0 L 1 0 Z'))).toBe('M 0 0 L 1 0 Z');
    expect(motionPathSvg(parseMotionPath('M 0 0 L 0.5 0.5 E'), [100, 50], 1000, 500)).toBe(
      'M 100 50 L 600 300',
    );
  });

  // source: a drawn path is relative to its first point, in slide fractions; jitter is dropped.
  it('turns drawn points into a path', () => {
    expect(
      drawnPath(
        [
          [100, 100],
          [100.5, 100],
          [200, 150],
          [300, 100],
        ],
        1000,
        500,
      ),
    ).toBe('M 0 0 L 0.1 0.1 L 0.2 0 E');
    expect(drawnPath([[1, 1]], 1000, 500)).toBeNull();
  });

  it('draws a path with clicks and edits points by dragging', () => {
    const run = vi.fn<(c: SlideCommand) => void>();
    const onDrawn = vi.fn();
    const labels = { path: (n: string) => n, point: (n: number) => `p${n}`, drawHint: 'draw' };
    const { rerender } = render(
      <MotionPathOverlay
        slide={{ width: 1000, height: 500 }}
        width={1000}
        paths={[]}
        editing={null}
        drawing
        run={run}
        onDrawn={onDrawn}
        labels={labels}
      />,
    );
    const svg = screen.getByTestId('motion-paths');
    for (const [x, y] of [
      [100, 100],
      [300, 200],
    ]) {
      fireEvent.pointerDown(svg, { clientX: x, clientY: y });
      fireEvent.pointerUp(svg, { clientX: x, clientY: y });
    }
    fireEvent.keyDown(svg, { key: 'Enter' });
    expect(run).toHaveBeenCalledWith({
      cmd: 'addAnimation',
      effect: 'customPath',
      path: 'M 0 0 L 0.2 0.2 E',
    });
    expect(onDrawn).toHaveBeenCalled();
    run.mockClear();
    rerender(
      <MotionPathOverlay
        slide={{ width: 1000, height: 500 }}
        width={1000}
        paths={[
          anim({
            index: 2,
            class: 'path',
            effect: 'customPath',
            path: 'M 0 0 L 0.1 0 E',
            rect: [0, 0, 100, 100],
          }),
        ]}
        editing={2}
        drawing={false}
        run={run}
        onDrawn={onDrawn}
        labels={labels}
      />,
    );
    // From the shape's centre (50, 50) to 0.1 of the slide right.
    expect(screen.getByTestId('motion-path-2').getAttribute('d')).toBe('M 50 50 L 150 50');
    const end = screen.getByTestId('motion-point-2-1');
    fireEvent.pointerDown(end, { clientX: 150, clientY: 50, pointerId: 1 });
    fireEvent.pointerMove(end, { clientX: 150, clientY: 100, pointerId: 1 });
    fireEvent.pointerUp(end, { clientX: 150, clientY: 100, pointerId: 1 });
    expect(run).toHaveBeenCalledWith({ cmd: 'setAnimation', index: 2, path: 'M 0 0 L 0.1 0.1 E' });
  });
});

describe('morph by word / by character', () => {
  it('draws words on canvases of their own box', () => {
    const box: [number, number, number, number] = [10, 20, 30, 40];
    const layers = morphLayers(
      {
        fromBackdrop: page(),
        toBackdrop: page(),
        pairs: [
          {
            from: [25, 40, 30, 40, 0],
            to: [125, 40, 30, 40, 0],
            fromPage: page(),
            toPage: page(),
            clip: [box, [110, 20, 30, 40]],
          },
        ],
        gone: [],
        new: [],
        goneText: [{ page: page(), rect: box }],
        newText: [{ page: page(), rect: [0, 0, 5, 5] }],
        option: 'byWord',
      },
      1000,
      500,
    );
    expect(layers.map((l) => !!l.backdrop)).toEqual([true, true, false, false, false, false]);
    expect(layers[2]?.rect).toEqual(box);
    expect(layers[3]?.rect).toEqual(box);
    expect(layers[4]?.rect).toEqual([110, 20, 30, 40]);
    expect(layers[5]?.rect).toEqual([0, 0, 5, 5]);
  });
});

describe('see-through layers', () => {
  // source: a layer over the backdrop must not paint the page white (only the backdrop is paper).
  it('leaves transparent pages unpainted and draws a part of the page', () => {
    const calls: string[] = [];
    const ctx = new Proxy(
      {},
      {
        get:
          (_t, k) =>
          (...args: unknown[]) =>
            calls.push(`${String(k)}(${args.join(',')})`),
        set: () => true,
      },
    ) as unknown as CanvasLike;
    const glyphs = new GlyphStore({} as unknown as EngineHost);
    renderPage(ctx, { width: 100, height: 50, commands: [] }, glyphs, 2, undefined, {
      transparent: true,
      origin: [10, 5],
    });
    expect(calls).toContain('setTransform(2,0,0,2,-20,-10)');
    expect(calls.some((c) => c.startsWith('fillRect'))).toBe(false);
    calls.length = 0;
    renderPage(ctx, { width: 100, height: 50, commands: [] }, glyphs, 2);
    expect(calls).toContain('fillRect(0,0,100,50)');
    void ImageStore;
  });
});

describe('animation pane: text and path options', () => {
  it('sets text by word, reverses and locks paths, and starts drawing', () => {
    const run = vi.fn<(c: SlideCommand) => void>();
    const onDrawPath = vi.fn();
    const onEditPath = vi.fn();
    const edit = {
      slide: 0,
      selection: [{ id: 4 }],
      animations: [
        anim({ index: 0, textBy: 'wd', textStepMs: 100 }),
        anim({ index: 1, class: 'path', effect: 'customPath', path: 'M 0 0 L 0.1 0 E' }),
      ],
      objects: [],
    } as unknown as SlideEditResult;
    render(
      <AnimationPane
        engine={{} as EngineHost}
        glyphs={{} as GlyphStore}
        images={{} as ImageStore}
        edit={edit}
        slideSize={{ width: 960, height: 540 }}
        revision={0}
        run={run}
        onClose={() => {}}
        previewToken={0}
        onDrawPath={onDrawPath}
        onEditPath={onEditPath}
      />,
    );
    const by = screen.getByTestId('anim-text-by-0') as HTMLSelectElement;
    expect(by.value).toBe('word');
    // 100 ms of 1000 ms.
    expect((screen.getByTestId('anim-text-delay-0') as HTMLInputElement).value).toBe('10');
    fireEvent.change(by, { target: { value: 'letter' } });
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, textBy: 'letter' });
    expect(screen.queryByTestId('anim-text-by-1')).toBeNull();
    fireEvent.click(screen.getByTestId('anim-reverse-path-1'));
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 1, reversePath: true });
    fireEvent.click(screen.getByTestId('anim-lock-path-1'));
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 1, lockPath: true });
    fireEvent.click(screen.getByTestId('anim-edit-path-1'));
    expect(onEditPath).toHaveBeenCalledWith(1);
    fireEvent.change(screen.getByTestId('anim-add'), { target: { value: 'path:customPath' } });
    expect(onDrawPath).toHaveBeenCalled();
  });
});
