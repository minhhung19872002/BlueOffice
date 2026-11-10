import {
  FakeTranslationProvider,
  type SlideCommand,
  type SlideEditResult,
  type SlideShapeInfo,
} from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import {
  applyDeckTexts,
  editedSelection,
  joinLines,
  readDeckTexts,
  spliceEdited,
  textLines,
  translateDeckTexts,
  translateSelectedShapes,
  translateShapeTexts,
} from '../src/translate';

const shape = (id: number, text: string | null, kind: SlideShapeInfo['kind'] = 'shape') =>
  ({
    id,
    name: `s${id}`,
    kind,
    rect: { cx: 0, cy: 0, w: 1, h: 1, rot: 0 },
    text,
    placeholder: null,
    lockAspect: false,
    editableText: kind === 'shape',
  }) as unknown as SlideShapeInfo;

/** A fake deck: slides of shapes + notes, the current slide, a command log. */
function fakeDeck(slides: { shapes: SlideShapeInfo[]; notes: string }[]) {
  let slide = 0;
  const log: SlideCommand[] = [];
  const run = async (c: SlideCommand): Promise<SlideEditResult> => {
    log.push(c);
    const cur = slides[slide]!;
    if (c.cmd === 'goTo') slide = c.slide;
    if (c.cmd === 'setText') {
      const s = cur.shapes.find((x) => x.id === c.id);
      if (s) s.text = c.text;
    }
    if (c.cmd === 'setNotes') slides[c.slide ?? slide]!.notes = c.text;
    const now = slides[slide]!;
    return {
      slide,
      selection: c.cmd === 'selectAll' ? now.shapes : [],
      notes: now.notes,
    } as unknown as SlideEditResult;
  };
  return { run, log, slides, current: () => slide };
}

describe('BlueSlide translation', () => {
  it('keeps paragraph and line breaks', async () => {
    const t = 'Tiêu đề\nDòng một\vDòng hai';
    const { lines, seps } = textLines(t);
    expect(lines).toEqual(['Tiêu đề', 'Dòng một', 'Dòng hai']);
    expect(joinLines(lines, seps)).toBe(t);
    const out = await translateShapeTexts(new FakeTranslationProvider(), [t], 'vi', 'en');
    expect(out).toEqual(['[en] Tiêu đề\n[en] Dòng một\v[en] Dòng hai']);
  });

  it('splices the translation into the edited text', async () => {
    const edit = {
      textEdit: { id: 7, start: 4, end: 8, text: 'Xin chào bạn', caret: null, rects: [], rect: {} },
    } as unknown as SlideEditResult;
    expect(editedSelection(edit)).toBe('chào');
    const cmds: SlideCommand[] = [];
    const run = async (c: SlideCommand) => {
      cmds.push(c);
      return null;
    };
    await spliceEdited(run, edit, 'hello', true);
    await spliceEdited(run, edit, 'hello', false);
    expect(cmds).toEqual([
      { cmd: 'setText', id: 7, text: 'Xin hello bạn', sel: [9, 9], cell: undefined },
      { cmd: 'setText', id: 7, text: 'Xin chào hello bạn', sel: [14, 14], cell: undefined },
    ]);
  });

  it('translates the selected shapes in place (not pictures or tables)', async () => {
    const d = fakeDeck([
      {
        shapes: [shape(1, 'Xin chào các bạn'), shape(2, null, 'picture'), shape(3, 'a', 'table')],
        notes: '',
      },
    ]);
    const n = await translateSelectedShapes(
      d.run,
      d.slides[0]!.shapes,
      new FakeTranslationProvider(),
      'vi',
      'en',
    );
    expect(n).toBe(1);
    expect(d.slides[0]!.shapes[0]!.text).toBe('[en] Xin chào các bạn');
    expect(d.slides[0]!.shapes[2]!.text).toBe('a');
  });

  it('reads, translates and writes the whole deck with notes, keeping slide count', async () => {
    const d = fakeDeck([
      {
        shapes: [shape(1, 'Giới thiệu'), shape(2, 'The quick brown fox')],
        notes: 'Ghi chú cho người trình bày',
      },
      { shapes: [shape(5, 'Kết luận và cảm ơn')], notes: '' },
    ]);
    await d.run({ cmd: 'goTo', slide: 1 });
    const texts = await readDeckTexts(d.run, 2, 1, [5]);
    // Back where it was.
    expect(d.current()).toBe(1);
    expect(d.log.at(-1)).toEqual({ cmd: 'select', ids: [5] });
    const fake = new FakeTranslationProvider();
    const { deck, count } = await translateDeckTexts(texts, fake, 'auto', 'en');
    expect(count).toBe(3);
    await applyDeckTexts(d.run, deck);
    expect(d.slides).toHaveLength(2);
    expect(d.slides[0]!.shapes.map((s) => s.text)).toEqual([
      '[en] Giới thiệu',
      'The quick brown fox',
    ]);
    expect(d.slides[0]!.notes).toBe('[en] Ghi chú cho người trình bày');
    expect(d.slides[1]!.shapes[0]!.text).toBe('[en] Kết luận và cảm ơn');
  });

  it('cancels before writing anything', async () => {
    const d = fakeDeck([{ shapes: [shape(1, 'Xin chào các bạn')], notes: '' }]);
    const fake = new FakeTranslationProvider();
    fake.gate = new Promise(() => {});
    const ac = new AbortController();
    const p = translateSelectedShapes(d.run, d.slides[0]!.shapes, fake, 'vi', 'en', {
      signal: ac.signal,
    });
    ac.abort();
    await expect(p).rejects.toMatchObject({ code: 'cancelled' });
    expect(d.log.filter((c) => c.cmd === 'setText')).toEqual([]);
  });
});
