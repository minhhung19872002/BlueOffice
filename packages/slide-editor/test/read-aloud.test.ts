import { ReadAloudSession } from '@blueoffice/editor';
import {
  FakeSpeechProvider,
  type SlideCommand,
  type SlideEditResult,
  type SlideShapeInfo,
} from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { paragraphsOf, readingOrder, slideReadRequest } from '../src/readAloud';

const VI = { id: 'vi', name: 'An', lang: 'vi-VN', local: true, default: true };
const settings = { rate: 1, pitch: 1, volume: 1, voice: null, fallbackLang: 'vi-VN' };

const shape = (id: number, text: string, cx: number, cy: number): SlideShapeInfo => ({
  id,
  name: `s${id}`,
  kind: 'shape',
  rect: { cx, cy, w: 100, h: 40, rot: 0 },
  text,
  placeholder: null,
  lockAspect: false,
  editableText: true,
});

function source(edit: Partial<SlideEditResult>, all: SlideShapeInfo[] = []) {
  const sent: SlideCommand[] = [];
  const run = async (c: SlideCommand) => {
    sent.push(c);
    if (c.cmd === 'selectAll') return { selection: all } as unknown as SlideEditResult;
    return null;
  };
  return { sent, run, edit: edit as SlideEditResult };
}

describe('BlueSlide read aloud', () => {
  it('splits edited text into paragraphs with offsets', () => {
    expect(paragraphsOf('ab\ncd\n')).toEqual([
      { text: 'ab', base: 0 },
      { text: 'cd', base: 3 },
      { text: '', base: 6 },
    ]);
  });

  it('orders shapes top to bottom then left to right', () => {
    const order = readingOrder([
      shape(1, 'c', 300, 200),
      shape(2, 'a', 300, 50),
      shape(3, 'b', 100, 200),
      shape(4, '  ', 0, 0),
    ]).map((s) => s.id);
    expect(order).toEqual([2, 3, 1]);
  });

  it('selects the word being spoken in the edited text (UTF-16 offsets)', async () => {
    // Paragraph 2 starts at 9 ("Tiêu đề" is 7 units + "\n" → 8; "😀" makes the first line 9).
    const text = 'Tiêu đề😀\nNội dung chính';
    const src = source({
      textEdit: {
        id: 7,
        start: 10,
        end: 10,
        caret: null,
        rects: [],
        rect: { cx: 0, cy: 0, w: 0, h: 0, rot: 0 },
        text,
      },
    });
    const req = await slideReadRequest({ ...src, withNotes: false, fallbackLang: 'vi-VN' });
    expect(req.unit).toBe(1);
    expect(req.offset).toBe(0);
    const p = new FakeSpeechProvider([VI]);
    const s = new ReadAloudSession(p, req.units, settings, { onWord: (w) => req.onWord?.(w) });
    await s.start(req.unit, req.offset);
    p.step(); // "Nội"
    p.step(); // "dung"
    expect(src.sent).toEqual([
      { cmd: 'textSelect', id: 7, start: 10, end: 13, cell: undefined },
      { cmd: 'textSelect', id: 7, start: 14, end: 18, cell: undefined },
    ]);
    req.onStop?.();
    expect(src.sent.at(-1)).toEqual({
      cmd: 'textSelect',
      id: 7,
      start: 10,
      end: 10,
      cell: undefined,
    });
  });

  it('reads only the selected part of the edited text', async () => {
    const text = 'một hai\nba bốn';
    const src = source({
      textEdit: {
        id: 1,
        start: 4,
        end: 10,
        caret: null,
        rects: [],
        rect: { cx: 0, cy: 0, w: 0, h: 0, rot: 0 },
        text,
      },
    });
    const req = await slideReadRequest({ ...src, withNotes: false, fallbackLang: 'vi-VN' });
    expect(req.units.map((u) => u.range)).toEqual([
      { start: 4, end: 7 },
      { start: 0, end: 2 },
    ]);
  });

  it('reads every shape of the slide and the notes, selecting each shape', async () => {
    const all = [shape(5, 'Hai', 0, 300), shape(6, 'Một', 0, 100)];
    const src = source({ selection: [], notes: 'Ghi chú' }, all);
    const req = await slideReadRequest({ ...src, withNotes: true, fallbackLang: 'vi-VN' });
    expect(req.units.map((u) => u.text)).toEqual(['Một', 'Hai', 'Ghi chú']);
    const p = new FakeSpeechProvider([VI]);
    const s = new ReadAloudSession(p, req.units, settings, { onUnit: (i) => req.onUnit?.(i) });
    await s.start();
    p.run();
    req.onStop?.();
    expect(src.sent).toEqual([
      { cmd: 'selectAll' },
      { cmd: 'select', ids: [] },
      { cmd: 'select', ids: [6] },
      { cmd: 'select', ids: [5] },
      { cmd: 'select', ids: [] },
      { cmd: 'select', ids: [] },
    ]);
  });

  it('reads the selected shapes without notes unless asked', async () => {
    const src = source({ selection: [shape(9, 'Chỉ cái này', 0, 0)], notes: 'Ghi chú' });
    const req = await slideReadRequest({ ...src, withNotes: false, fallbackLang: 'vi-VN' });
    expect(req.units.map((u) => u.text)).toEqual(['Chỉ cái này']);
    req.onStop?.();
    expect(src.sent).toEqual([{ cmd: 'select', ids: [9] }]);
  });
});
