// @vitest-environment happy-dom
import type { SlideInkObject, SlideInkStrokeIn } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DrawLayer } from '../src/draw/DrawLayer';
import { DRAW_COMMANDS, applyDrawCommand, isChecked } from '../src/draw/drawTab';
import {
  INITIAL_DRAW,
  MIN_STEP,
  REPLAY_GAP,
  REPLAY_SAMPLE_MS,
  RULER_THICKNESS,
  StrokeRecorder,
  brushOf,
  edgeDistance,
  previewWidth,
  projectOnEdge,
  replayTimeline,
  snapAngle,
  snapsToRuler,
  touches,
  visibleAt,
  type DrawState,
} from '../src/draw/inkTools';

const SLIDE = { width: 960, height: 540 };

afterEach(cleanup);

function stroke(points: [number, number, number][], times?: number[]) {
  return {
    points,
    ...(times ? { times } : {}),
    color: '000000',
    opacity: 1,
    highlighter: false,
    rectTip: false,
  };
}

describe('draw tools state', () => {
  it('switches tools, presets and the ruler', () => {
    let s: DrawState = INITIAL_DRAW;
    s = applyDrawCommand(s, 'draw.highlighter', SLIDE);
    expect(brushOf(s)).toEqual({ color: 'FFE600', width: 14, highlighter: true });
    s = applyDrawCommand(s, 'draw.width.bold', SLIDE);
    expect(brushOf(s).width).toBe(22);
    expect(isChecked(s, 'draw.width.bold')).toBe(true);
    // A pen colour picks the pen; its width is still the pen's own.
    s = applyDrawCommand(s, 'draw.color.red', SLIDE);
    expect(s.tool).toBe('pen');
    expect(brushOf(s)).toEqual({ color: 'C8261E', width: 3, highlighter: false });
    expect(isChecked(s, 'draw.color.red')).toBe(true);
    expect(isChecked(s, 'draw.highlighter')).toBe(false);
    // The ruler appears in the middle of the slide and turns by 15°.
    s = applyDrawCommand(s, 'draw.ruler', SLIDE);
    expect(s.ruler).toEqual({ x: 480, y: 270, angle: 0 });
    s = applyDrawCommand(s, 'draw.rulerLeft', SLIDE);
    expect(s.ruler?.angle).toBe(-15);
    expect(isChecked(s, 'draw.ruler')).toBe(true);
    s = applyDrawCommand(s, 'draw.ruler', SLIDE);
    expect(s.ruler).toBeNull();
    expect(new Set(DRAW_COMMANDS).size).toBe(DRAW_COMMANDS.length);
  });

  it('widens with pressure like the engine', () => {
    // width · (0.5 + f).
    expect(previewWidth(2, 0.25)).toBe(1.5);
    expect(previewWidth(2, 1)).toBe(3);
    expect(previewWidth(2, undefined)).toBe(2);
  });
});

describe('ruler', () => {
  it('projects on the drawing edge', () => {
    // At 0° the edge is the upper side: y = 100 − 56/2 = 72.
    const r = { x: 100, y: 100, angle: 0 };
    expect(RULER_THICKNESS).toBe(56);
    expect(edgeDistance(r, [150, 80])).toBeCloseTo(-8);
    expect(projectOnEdge(r, [150, 80])).toEqual([150, 72]);
    expect(snapsToRuler(r, [150, 80])).toBe(true);
    expect(snapsToRuler(r, [150, 40])).toBe(false);
    // At 90° the ruler runs down the slide and its edge is x = 100 + 28 = 128.
    const v = { x: 100, y: 100, angle: 90 };
    const p = projectOnEdge(v, [120, 130]);
    expect(p[0]).toBeCloseTo(128);
    expect(p[1]).toBeCloseTo(130);
  });

  it('snaps the angle to 15° steps within 2°', () => {
    expect(snapAngle(44)).toBe(45);
    expect(snapAngle(40)).toBe(40);
    expect(snapAngle(190)).toBe(-170);
    expect(snapAngle(-361)).toBe(0);
  });

  it('keeps a stroke on the edge', () => {
    const r = { x: 100, y: 100, angle: 0 };
    const rec = new StrokeRecorder([60, 80], 1000, null, r);
    expect(rec.onRuler).toBe(true);
    rec.extend([90, 60], 1016, null);
    expect(rec.points).toEqual([
      [60, 72],
      [90, 72],
    ]);
  });
});

describe('stroke recorder', () => {
  it('records pressure (pen only), times and skips tiny moves', () => {
    const pen = new StrokeRecorder([10, 10], 500, 0.3, null);
    expect(pen.extend([10 + MIN_STEP / 2, 10], 508, 0.4)).toBe(false);
    expect(pen.extend([20, 10], 516, 0.9)).toBe(true);
    const s = pen.toStroke({ color: 'FF0000', width: 3, highlighter: false });
    expect(s).toEqual({
      points: [
        [10, 10],
        [20, 10],
      ],
      pressure: [0.3, 0.9],
      times: [0, 16],
      color: 'FF0000',
      width: 3,
    } satisfies SlideInkStrokeIn);
    const mouse = new StrokeRecorder([0, 0], 0, null, null);
    mouse.extend([5, 0], 7, null);
    const m = mouse.toStroke({ color: 'FFE600', width: 14, highlighter: true });
    expect(m.pressure).toBeUndefined();
    expect(m.highlighter).toBe(true);
  });
});

describe('eraser preview', () => {
  it('touches a stroke within radius plus half its width', () => {
    const s = stroke([
      [0, 0, 4],
      [10, 0, 4],
    ]);
    // Distance 7 ≤ 6 + 2.
    expect(touches(s, [5, 7], 6)).toBe(true);
    expect(touches(s, [5, 9], 6)).toBe(false);
  });
});

describe('ink replay timeline', () => {
  it('uses recorded times and shortens pauses', () => {
    const objects: SlideInkObject[] = [
      {
        id: 2,
        name: 'Ink 2',
        rect: [0, 0, 10, 10],
        strokes: [
          stroke(
            [
              [0, 0, 1],
              [1, 0, 1],
            ],
            [5000, 5040],
          ),
        ],
      },
      {
        id: 3,
        name: 'Ink 3',
        rect: [0, 0, 10, 10],
        strokes: [
          stroke(
            [
              [0, 0, 1],
              [1, 1, 1],
              [2, 2, 1],
            ],
            [1000, 1010, 1030],
          ),
        ],
      },
    ];
    const { strokes, total } = replayTimeline(objects);
    // Ordered by first time: Ink 3 (1000..1030) then Ink 2 (5000..5040);
    // the 3970 ms wait is shortened to 1000 ms.
    expect(strokes.map((r) => r.stroke.points.length)).toEqual([3, 2]);
    expect(strokes[0]!.offsets).toEqual([0, 10, 30]);
    expect(strokes[1]!.start).toBe(30 + 1000);
    expect(total).toBe(1030 + 40);
    expect(visibleAt(strokes, 15)).toEqual([2, 0]);
    expect(visibleAt(strokes, 1050)).toEqual([3, 1]);
  });

  it('paces strokes without times', () => {
    const objects: SlideInkObject[] = [
      {
        id: 2,
        name: 'a',
        rect: [0, 0, 1, 1],
        strokes: [
          stroke([
            [0, 0, 1],
            [1, 0, 1],
            [2, 0, 1],
          ]),
          stroke([[0, 0, 1]]),
        ],
      },
    ];
    const { strokes, total } = replayTimeline(objects);
    expect(strokes[0]!.offsets).toEqual([0, REPLAY_SAMPLE_MS, 2 * REPLAY_SAMPLE_MS]);
    expect(strokes[1]!.start).toBe(2 * REPLAY_SAMPLE_MS + REPLAY_GAP);
    expect(total).toBe(2 * REPLAY_SAMPLE_MS + REPLAY_GAP);
  });
});

describe('DrawLayer', () => {
  const labels = { layer: 'Ink area', ruler: 'Ruler', rulerAngle: 'Angle' };
  const size = (el: HTMLElement) => {
    // 480 × 270 CSS px for a 960 × 540 pt slide: 2 pt per px.
    el.getBoundingClientRect = () =>
      ({ left: 0, top: 0, width: 480, height: 270, right: 480, bottom: 270 }) as DOMRect;
  };

  it('hands a finished stroke over as one command', () => {
    const onStroke = vi.fn();
    render(
      <DrawLayer
        slide={SLIDE}
        state={{ ...INITIAL_DRAW, tool: 'pen' }}
        ink={[]}
        onStroke={onStroke}
        onErase={vi.fn()}
        onRuler={vi.fn()}
        labels={labels}
      />,
    );
    const layer = screen.getByTestId('draw-layer');
    size(layer);
    fireEvent.pointerDown(layer, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerMove(layer, { clientX: 20, clientY: 10, pointerId: 1 });
    fireEvent.pointerMove(layer, { clientX: 30, clientY: 15, pointerId: 1 });
    expect(screen.getByTestId('draw-live')).toBeTruthy();
    fireEvent.pointerUp(layer, { clientX: 30, clientY: 15, pointerId: 1 });
    expect(onStroke).toHaveBeenCalledTimes(1);
    const s = onStroke.mock.calls[0]![0] as SlideInkStrokeIn;
    expect(s.points).toEqual([
      [20, 20],
      [40, 20],
      [60, 30],
    ]);
    expect(s.color).toBe('1B1B1B');
    expect(s.width).toBe(3);
    expect(s.times).toHaveLength(3);
  });

  it('erases along the dragged path', () => {
    const onErase = vi.fn();
    render(
      <DrawLayer
        slide={SLIDE}
        state={{ ...INITIAL_DRAW, tool: 'eraserPoint' }}
        ink={[]}
        onStroke={vi.fn()}
        onErase={onErase}
        onRuler={vi.fn()}
        labels={labels}
      />,
    );
    const layer = screen.getByTestId('draw-layer');
    size(layer);
    fireEvent.pointerDown(layer, { button: 0, clientX: 50, clientY: 50, pointerId: 4 });
    fireEvent.pointerMove(layer, { clientX: 60, clientY: 50, pointerId: 4 });
    fireEvent.pointerUp(layer, { clientX: 60, clientY: 50, pointerId: 4 });
    expect(onErase).toHaveBeenCalledWith(
      [
        [100, 100],
        [120, 100],
      ],
      6,
      'point',
    );
  });

  it('moves and turns the ruler from the keyboard', () => {
    const onRuler = vi.fn();
    render(
      <DrawLayer
        slide={SLIDE}
        state={{ ...INITIAL_DRAW, tool: 'pen', ruler: { x: 480, y: 270, angle: 0 } }}
        ink={[]}
        onStroke={vi.fn()}
        onErase={vi.fn()}
        onRuler={onRuler}
        labels={labels}
      />,
    );
    const ruler = screen.getByRole('slider', { name: 'Ruler' });
    fireEvent.keyDown(ruler, { key: 'ArrowRight', shiftKey: true });
    expect(onRuler).toHaveBeenLastCalledWith({ x: 480, y: 270, angle: 15 });
    // Dragging the body (deep inside, away from the edge) moves it.
    const layer = screen.getByTestId('draw-layer');
    size(layer);
    fireEvent.pointerDown(layer, { button: 0, clientX: 240, clientY: 145, pointerId: 2 });
    fireEvent.pointerMove(layer, { clientX: 250, clientY: 150, pointerId: 2 });
    expect(onRuler).toHaveBeenLastCalledWith({ x: 500, y: 280, angle: 0 });
  });
});
