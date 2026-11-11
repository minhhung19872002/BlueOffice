// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  EngineHost,
  PageDisplayList,
  SlideAnimInfo,
  SlideCommand,
  SlideEditResult,
  SlideMorphOutline,
  SlideShowInfo,
} from '@blueoffice/platform';
import type { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import {
  AnimationPane,
  buildChoices,
  dropIndex,
  effectsAt,
  hasBgOption,
  morphLayers,
  outlineFrame,
  partVisibility,
  tintsShown,
  visibility,
} from '../src';

afterEach(cleanup);

const i18n = createI18n('vi-VN');

const page = (): PageDisplayList => ({ width: 1000, height: 500, commands: [] });

const anim = (a: Partial<SlideAnimInfo>): SlideAnimInfo => ({
  index: 0,
  shapeId: 4,
  shapeName: 'A',
  effect: 'fade',
  class: 'entr',
  direction: null,
  trigger: 'onClick',
  delayMs: 0,
  durMs: 500,
  click: 1,
  startMs: 0,
  endMs: 500,
  rect: [100, 100, 200, 50],
  layer: 1,
  ...a,
});

const show = (animations: SlideAnimInfo[], parts = false): SlideShowInfo => ({
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
      ...(parts
        ? {
            parts: {
              pages: [page(), page(), page()],
              rects: [null, [100, 100, 200, 20], [100, 120, 200, 20]],
            },
          }
        : {}),
    },
  ],
  animations,
  clicks: Math.max(0, ...animations.map((a) => a.click)),
  media: [],
});

describe('builds in the show (§19.5 bldP / bldGraphic)', () => {
  // source: a paragraph build — the layer stays shown, each piece starts hidden (its first
  // effect is an entrance) and shows from its own click; the background piece of a build without
  // a background step is always shown.
  it('shows pieces of a layer click by click', () => {
    const s = show(
      [anim({ index: 0, click: 1, parts: [1] }), anim({ index: 1, click: 2, parts: [2] })],
      true,
    );
    expect(visibility(s, -1)).toEqual([true, true]);
    expect(partVisibility(s, -1)).toEqual({ '1:1': false, '1:2': false });
    expect(partVisibility(s, 1)).toEqual({ '1:1': true, '1:2': false });
    expect(partVisibility(s, 2)).toEqual({ '1:1': true, '1:2': true });
    const fx = effectsAt(s, 2);
    expect(fx).toHaveLength(1);
    expect(fx[0]?.parts).toEqual([2]);
  });

  // source: hide after animation, hide on next click (hidden once the next click starts) and
  // rewind (an entrance hides again, an exit shows again; no colour kept).
  it('hides after the effect, at the next click, and rewinds', () => {
    const s = show([
      anim({ index: 0, click: 1, after: 'hideOnClick' }),
      anim({ index: 1, click: 2, layer: 0, class: 'exit', rewind: true }),
    ]);
    // Done with click 1: shown; click 2 has begun: hidden.
    expect(visibility(s, 1)[1]).toBe(true);
    expect(visibility(s, 1, true)[1]).toBe(false);
    expect(visibility(s, 2)[1]).toBe(false);
    // The rewound exit leaves its layer shown.
    expect(visibility(s, 2)[0]).toBe(true);
    const hide = show([anim({ after: 'hide' })]);
    expect(visibility(hide, 1)[1]).toBe(false);
    const tinted = show([
      anim({
        rewind: true,
        tint: page(),
        play: {
          durMs: 500,
          tracks: [
            {
              beginMs: 500,
              durMs: 1,
              repeat: 1,
              autoRev: false,
              hold: true,
              kind: 'color',
              attr: 'style.color',
              to: 0x808080,
            },
          ],
        },
      }),
    ]);
    expect(tintsShown(tinted, 1)).toEqual({ 0: false });
    // The sound of an effect goes with it.
    const sound = show([
      anim({ sound: { stop: false, part: '/ppt/media/audio1.wav', name: 'a.wav', loop: false } }),
    ]);
    expect(effectsAt(sound, 1)[0]?.sound).toBe('/ppt/media/audio1.wav');
  });
});

describe('animation pane: builds, after animation, sound, drag and drop', () => {
  const build = (kind: 'text' | 'chart' | 'diagram', value: string) => ({
    kind,
    value,
    reverse: false,
    bg: false,
    head: true,
    levels: 2,
  });

  it('lists the build choices of each kind of shape', () => {
    expect(buildChoices(build('text', 'whole'))).toEqual(['whole', 'allAtOnce', 'p1', 'p2']);
    expect(buildChoices(build('chart', 'series'))).toContain('categoryEl');
    expect(buildChoices(build('diagram', 'one'))).toEqual([
      'whole',
      'allAtOnce',
      'one',
      'lvlAtOnce',
      'lvlOne',
    ]);
    expect(hasBgOption(build('text', 'p1'))).toBe(true);
    expect(hasBgOption(build('chart', 'allAtOnce'))).toBe(false);
    expect(hasBgOption(build('diagram', 'one'))).toBe(false);
    expect(dropIndex(0, 2, 3)).toBe(2);
    expect(dropIndex(1, 1, 3)).toBeNull();
    expect(dropIndex(0, 5, 3)).toBeNull();
  });

  function pane(animations: SlideAnimInfo[], pickSound?: () => Promise<null>) {
    const run = vi.fn<(c: SlideCommand) => void>();
    const edit = {
      slide: 0,
      selection: [{ id: 4 }],
      animations,
      objects: [],
    } as unknown as SlideEditResult;
    render(
      <I18nextProvider i18n={i18n}>
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
          pickSound={pickSound}
        />
      </I18nextProvider>,
    );
    return run;
  }

  it('sets the build, the order and the background step', () => {
    const run = pane([
      anim({
        index: 0,
        build: build('text', 'p1'),
        target: { kind: 'paragraphs', start: 0, end: 1 },
      }),
      anim({
        index: 1,
        build: { ...build('text', 'p1'), head: false },
        target: { kind: 'paragraphs', start: 2, end: 2 },
      }),
    ]);
    // The build's options on its first effect only; each effect names its paragraphs.
    expect(screen.queryByTestId('anim-build-1')).toBeNull();
    expect(screen.getByTestId('anim-part-0').textContent).toContain('đoạn 1–2');
    expect(screen.getByTestId('anim-part-1').textContent).toContain('đoạn 3');
    const sel = screen.getByTestId('anim-build-0') as HTMLSelectElement;
    expect(sel.value).toBe('p1');
    expect(Array.from(sel.options).map((o) => o.textContent)).toContain('Theo đoạn đến cấp 2');
    fireEvent.change(sel, { target: { value: 'p2' } });
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, build: 'p2' });
    fireEvent.click(screen.getByTestId('anim-build-reverse-0'));
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, buildReverse: true });
    fireEvent.click(screen.getByTestId('anim-build-bg-0'));
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, buildBg: true });
  });

  it('sets after animation, rewind and the effect sound', () => {
    const run = pane([anim({ index: 0, after: 'dim', dimColor: 'A0A0A0' })]);
    expect((screen.getByTestId('anim-after-0') as HTMLSelectElement).value).toBe('dim');
    expect((screen.getByTestId('anim-dim-color-0') as HTMLInputElement).value).toBe('#a0a0a0');
    fireEvent.change(screen.getByTestId('anim-dim-color-0'), { target: { value: '#112233' } });
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, dimColor: '112233' });
    fireEvent.change(screen.getByTestId('anim-after-0'), { target: { value: 'hideOnClick' } });
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, after: 'hideOnClick' });
    fireEvent.click(screen.getByTestId('anim-rewind-0'));
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, rewind: true });
    // A built-in sound: a WAV of its own, named after it.
    fireEvent.change(screen.getByTestId('anim-sound-0'), { target: { value: 'chime' } });
    const last = run.mock.calls.at(-1)?.[0] as Extract<SlideCommand, { cmd: 'setAnimation' }>;
    expect(last.soundName).toBe('chime.wav');
    expect(atob(last.soundData ?? '').startsWith('RIFF')).toBe(true);
    fireEvent.change(screen.getByTestId('anim-sound-0'), { target: { value: 'none' } });
    expect(run).toHaveBeenLastCalledWith({ cmd: 'setAnimation', index: 0, sound: 'none' });
  });

  it('reorders effects by dragging them', () => {
    const run = pane([
      anim({ index: 0, shapeName: 'A' }),
      anim({ index: 1, shapeName: 'B' }),
      anim({ index: 2, shapeName: 'C' }),
    ]);
    const item = (i: number) => screen.getByTestId(`anim-item-${i}`);
    const data = new Map<string, string>();
    const dataTransfer = {
      setData: (k: string, v: string) => data.set(k, v),
      getData: (k: string) => data.get(k) ?? '',
      effectAllowed: '',
      dropEffect: '',
    };
    expect(item(0).getAttribute('draggable')).toBe('true');
    fireEvent.dragStart(item(0), { dataTransfer });
    fireEvent.dragOver(item(2), { dataTransfer });
    expect(item(2).className).toMatch(/animDropTarget/);
    fireEvent.drop(item(2), { dataTransfer });
    expect(run).toHaveBeenLastCalledWith({ cmd: 'moveAnimation', from: 0, to: 2 });
    // Dropped on itself: nothing moves.
    run.mockClear();
    fireEvent.dragStart(item(1), { dataTransfer });
    fireEvent.drop(item(1), { dataTransfer });
    expect(run).not.toHaveBeenCalled();
  });
});

describe('morph of outlines point by point', () => {
  const o: SlideMorphOutline = {
    from: [
      {
        rings: [
          [
            [0, 0],
            [10, 0],
            [10, 10],
          ],
        ],
        closed: [true],
        fill: [255, 0, 0, 255],
        stroke: [[0, 0, 0, 255], 1],
      },
    ],
    to: [
      {
        rings: [
          [
            [20, 0],
            [40, 0],
            [40, 20],
          ],
        ],
        closed: [true],
        fill: [0, 0, 255, 255],
      },
    ],
  };

  // source: linear interpolation of every point, the fill (RGB) and the line, which fades out
  // where the new shape has none.
  it('interpolates points, fill and line', () => {
    expect(outlineFrame(o, 0)[0]?.d).toBe('M 0 0 L 10 0 L 10 10 Z');
    const mid = outlineFrame(o, 0.5)[0]!;
    expect(mid.d).toBe('M 10 0 L 25 0 L 25 15 Z');
    expect(mid.fill).toBe('rgba(128, 0, 128, 1)');
    expect(mid.stroke).toBe('rgba(0, 0, 0, 0.502)');
    expect(outlineFrame(o, 1)[0]?.stroke).toBe('rgba(0, 0, 0, 0)');
    // The overlay draws the outlines under the pair's text.
    const layers = morphLayers(
      {
        fromBackdrop: page(),
        toBackdrop: page(),
        pairs: [
          {
            from: [5, 5, 10, 10, 0],
            to: [30, 10, 20, 20, 0],
            fromPage: page(),
            toPage: page(),
            outline: o,
          },
        ],
        gone: [],
        new: [],
        option: 'byObject',
      },
      1000,
      500,
    );
    expect(layers.map((l) => (l.outline ? 'outline' : 'page'))).toEqual([
      'page',
      'page',
      'outline',
      'page',
      'page',
    ]);
  });
});
