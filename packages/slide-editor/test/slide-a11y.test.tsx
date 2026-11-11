// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, render, renderHook, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { EngineHost, SlideA11yShape, SlideShapeInfo } from '@blueoffice/platform';
import {
  SlideStageMirror,
  ThumbDescription,
  selectionAnnouncement,
  shapeSummary,
  slideTitle,
  thumbLabel,
  useSlideOutline,
} from '../src/SlideA11y';

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = (k: string, o?: Record<string, unknown>) => String(i18n.t(k, o));

const SHAPES: SlideA11yShape[] = [
  { id: 2, name: 'Title 1', kind: 'shape', placeholder: 'title', text: 'Quarterly\u000bresults' },
  { id: 3, name: 'Content 2', kind: 'shape', placeholder: 'body', text: 'Sales up\nCosts down' },
  { id: 4, name: 'Picture 3', kind: 'picture', alt: 'Bar chart of sales' },
  { id: 5, name: 'Chart 4', kind: 'chart' },
  { id: 6, name: 'Diagram 5', kind: 'smartart', alt: 'Process in three steps' },
];

const shapeInfo = (id: number, kind: SlideShapeInfo['kind'], text: string | null) =>
  ({ id, name: `Shape ${id}`, kind, text }) as SlideShapeInfo;

describe('BlueSlide accessibility', () => {
  it('summarises slides and shapes', () => {
    expect(slideTitle(SHAPES)).toBe('Quarterly results');
    expect(thumbLabel(0, SHAPES, t)).toBe('Slide 1: Quarterly results');
    expect(thumbLabel(4, undefined, t)).toBe('Slide 5');
    expect(shapeSummary(SHAPES[1]!, t)).toBe('Sales up Costs down');
    expect(shapeSummary(SHAPES[2]!, t)).toBe('picture: Bar chart of sales');
    expect(shapeSummary(SHAPES[3]!, t)).toBe('chart: Chart 4');
    expect(shapeSummary(SHAPES[4]!, t)).toBe('SmartArt diagram: Process in three steps');
    expect(selectionAnnouncement([], t)).toBe('');
    expect(selectionAnnouncement([shapeInfo(3, 'shape', 'Sales up')], t)).toBe('Sales up selected');
    expect(selectionAnnouncement([shapeInfo(4, 'picture', null)], t)).toBe(
      'picture Shape 4 selected',
    );
    expect(
      selectionAnnouncement([shapeInfo(3, 'shape', 'a'), shapeInfo(4, 'picture', null)], t),
    ).toBe('2 objects selected');
  });

  it('mirrors the slide in reading order and announces the selection', () => {
    const { rerender } = render(
      <I18nextProvider i18n={i18n}>
        <SlideStageMirror index={0} shapes={SHAPES} selection={[]} />
        <ThumbDescription id="d0" shapes={SHAPES} />
      </I18nextProvider>,
    );
    const region = screen.getByRole('region', { name: 'Content of slide 1' });
    expect(region.querySelector('h2')?.textContent).toBe('Quarterly results');
    expect(region.querySelector('p')?.textContent).toBe('Sales up Costs down');
    expect(
      [...region.querySelectorAll('[role="img"]')].map((e) => e.getAttribute('aria-label')),
    ).toEqual([
      'picture: Bar chart of sales',
      'chart: Chart 4',
      'SmartArt diagram: Process in three steps',
    ]);
    // Reading order: title, body, then the graphics.
    expect([...region.children].map((e) => e.getAttribute('data-shape'))).toEqual([
      '2',
      '3',
      '4',
      '5',
      '6',
    ]);
    expect(document.getElementById('d0')?.textContent).toBe(
      'Sales up Costs down. picture: Bar chart of sales. chart: Chart 4. SmartArt diagram: Process in three steps',
    );
    const live = screen.getByTestId('slide-a11y-live');
    expect(live.textContent).toBe('');
    rerender(
      <I18nextProvider i18n={i18n}>
        <SlideStageMirror index={0} shapes={SHAPES} selection={[shapeInfo(4, 'picture', null)]} />
      </I18nextProvider>,
    );
    expect(screen.getByTestId('slide-a11y-live').textContent).toBe('picture Shape 4 selected');
  });

  it('reads the outline of the deck from the engine', async () => {
    const calls: unknown[] = [];
    const engine = {
      slideCommand: async (cmd: unknown) => {
        calls.push(cmd);
        return {
          slides: [
            { index: 0, shapes: SHAPES },
            { index: 1, shapes: [] },
          ],
        };
      },
    } as unknown as EngineHost;
    const { result } = renderHook(() => useSlideOutline(engine, 2, 1));
    await waitFor(() => expect(result.current).toHaveLength(2));
    expect(calls).toEqual([{ cmd: 'slideOutline', from: 0, to: 1 }]);
    expect(result.current[0]?.[0]?.text).toBe('Quarterly\u000bresults');
  });
});
