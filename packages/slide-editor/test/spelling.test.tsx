// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  EngineHost,
  SlideCommand,
  SlideIssue,
  SlideSpellMark,
  SlideSpellTarget,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createRef, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SlideController } from '../src/controller';
import { SlideSpellDialog, SlideSpellLayer, onMark, wavePath } from '../src/spelling';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const issue = (
  target: SlideSpellTarget,
  unit: number,
  word: string,
  start: number,
  extra: Partial<SlideIssue> = {},
): SlideIssue => ({
  target,
  slide: target.slide,
  unit,
  para: 0,
  bodyFrom: start,
  bodyTo: start + word.length,
  start,
  end: start + word.length,
  from: start,
  to: start + word.length,
  word,
  suggestions: ['document'],
  before: '',
  after: '',
  ...extra,
});

/** A fake engine answering the slide spelling commands from a list of issues. */
function fakeEngine(initial: SlideIssue[]) {
  let issues = [...initial];
  const sent: SlideCommand[] = [];
  const slideCommand = vi.fn(async (c: SlideCommand) => {
    sent.push(c);
    switch (c.cmd) {
      case 'slideSpellCheck':
        return { slideSpell: { issues, unchecked: [], languages: ['vi', 'en'] } };
      case 'slideSpellAt':
        return { slideSpell: { hit: issues[0] ?? null } };
      case 'slideSpellReplace':
        issues = issues.filter(
          (i) =>
            !c.edits.some(
              (e) => JSON.stringify(e.target) === JSON.stringify(i.target) && e.start === i.start,
            ),
        );
        return { slide: 0, selection: [], dirty: [] };
      default:
        return { slideSpell: { ok: true } };
    }
  });
  const engine = { slideCommand } as unknown as EngineHost;
  return { engine, sent };
}

describe('BlueSlide spelling', () => {
  it('draws wavy lines and finds the word above a mark', () => {
    expect(wavePath([0, 10], [8, 10], 1, 2)).toBe(
      'M0.00,10.00L2.00,11.00L4.00,10.00L6.00,11.00L8.00,10.00',
    );
    const m: SlideSpellMark = { from: [10, 50], to: [60, 50], h: 20, id: 2 };
    expect(onMark(m, 30, 40)).toBe(true); // on the word
    expect(onMark(m, 30, 20)).toBe(false); // above the line of text
    expect(onMark(m, 80, 40)).toBe(false); // past the end
    // Text turned 90° clockwise: the line runs down and the text lies at its +x side.
    const r: SlideSpellMark = { from: [100, 0], to: [100, 50], h: 20, id: 3 };
    expect(onMark(r, 110, 25)).toBe(true);
    expect(onMark(r, 90, 25)).toBe(false);
  });

  it('walks the deck: shapes, tables and notes; replace all is one transaction', async () => {
    const shape: SlideSpellTarget = { kind: 'shape', slide: 0, id: 2 };
    const cell: SlideSpellTarget = { kind: 'cell', slide: 0, id: 5, row: 1, col: 0 };
    const notes: SlideSpellTarget = { kind: 'notes', slide: 1 };
    const { engine, sent } = fakeEngine([
      issue(shape, 0, 'documnet', 4),
      issue(cell, 1, 'documnet', 2),
      issue(notes, 2, 'documnet', 0),
    ]);
    const onResult = vi.fn();
    const controller = new SlideController(engine, onResult, () => {});
    const onShow = vi.fn();
    const onChanged = vi.fn();
    wrap(
      <SlideSpellDialog
        engine={engine}
        controller={controller}
        onClose={() => {}}
        onShow={onShow}
        onChanged={onChanged}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('spellcheck-where').textContent).toBe('Slide 1'));
    expect(onShow).toHaveBeenCalledWith(expect.objectContaining({ target: shape }));
    // Skip the shape, then the table cell comes.
    fireEvent.click(screen.getByTestId('spellcheck-ignore-once'));
    await waitFor(() =>
      expect(screen.getByTestId('spellcheck-where').textContent).toBe('Slide 1, table'),
    );
    fireEvent.click(screen.getByTestId('spellcheck-change-all'));
    await waitFor(() => expect(screen.getByTestId('spellcheck-done')).toBeTruthy());
    const replaces = sent.filter((c) => c.cmd === 'slideSpellReplace');
    expect(replaces).toHaveLength(1);
    expect(replaces[0]).toMatchObject({
      edits: [
        { target: shape, para: 0, start: 4, end: 12, text: 'document', word: 'documnet' },
        { target: cell, para: 0, start: 2, end: 10, text: 'document', word: 'documnet' },
        { target: notes, para: 0, start: 0, end: 8, text: 'document', word: 'documnet' },
      ],
    });
    // The edit went through the controller (the editor sees the result) and marks refresh.
    expect(onResult).toHaveBeenCalled();
    expect(onChanged).toHaveBeenCalled();
  });

  it('opens the corrections of a marked word on a right click', async () => {
    const shape: SlideSpellTarget = { kind: 'shape', slide: 0, id: 2 };
    const { engine, sent } = fakeEngine([issue(shape, 0, 'documnet', 4)]);
    const controller = new SlideController(
      engine,
      () => {},
      () => {},
    );
    const box = createRef<HTMLDivElement>();
    wrap(
      <div ref={box}>
        <SlideSpellLayer
          engine={engine}
          controller={controller}
          container={box}
          slide={0}
          size={{ width: 960, height: 540 }}
          width={960}
          marks={[{ from: [100, 200], to: [180, 200], h: 24, id: 2 }]}
          onChanged={() => {}}
        />
      </div>,
    );
    expect(screen.getAllByTestId('slide-spell-mark')).toHaveLength(1);
    // happy-dom lays nothing out: the layer covers the slide at 1 px per point.
    const svg = screen.getByTestId('slide-spell-layer');
    svg.getBoundingClientRect = () =>
      ({
        left: 0,
        top: 0,
        right: 960,
        bottom: 540,
        width: 960,
        height: 540,
        x: 0,
        y: 0,
      }) as DOMRect;
    // Off the word: the browser keeps its own menu.
    fireEvent.contextMenu(box.current!, { clientX: 400, clientY: 400 });
    expect(sent.some((c) => c.cmd === 'slideSpellAt')).toBe(false);
    fireEvent.contextMenu(box.current!, { clientX: 140, clientY: 190 });
    const menu = await screen.findByTestId('spell-menu');
    expect(sent).toContainEqual({ cmd: 'slideSpellAt', slide: 0, x: 140, y: 190 });
    fireEvent.click(menu.querySelector('button')!);
    await waitFor(() =>
      expect(sent).toContainEqual({
        cmd: 'slideSpellReplace',
        edits: [{ target: shape, para: 0, start: 4, end: 12, text: 'document', word: 'documnet' }],
      }),
    );
  });
});
