import { DictationSession } from '@blueoffice/editor';
import {
  FakeDictationProvider,
  type SlideCommand,
  type SlideEditResult,
  type SlideTextEdit,
} from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { slideDictationTarget } from '../src/dictation';

/** The edited text box: `setText` replaces its text and caret (one undo step each). */
function box(text: string, at = text.length) {
  let te: SlideTextEdit = {
    id: 7,
    start: at,
    end: at,
    caret: null,
    rects: [],
    rect: { cx: 50, cy: 20, w: 100, h: 40, rot: 0 },
    text,
  };
  const sent: SlideCommand[] = [];
  const result = () => ({ textEdit: te }) as unknown as SlideEditResult;
  let editing = true;
  const tg = slideDictationTarget({
    run: async (c) => {
      sent.push(c);
      if (c.cmd === 'setText' && c.sel)
        te = { ...te, text: c.text, start: c.sel[0], end: c.sel[1] };
      return result();
    },
    latest: () => (editing ? result() : null),
    idle: async () => {},
  });
  return {
    tg,
    sent,
    text: () => te.text,
    stopEditing: () => {
      editing = false;
    },
  };
}

describe('BlueSlide dictation', () => {
  it('writes each phrase into the edited text at the caret, one setText per phrase', async () => {
    const b = box('Tiêu đề: ', 9);
    const p = new FakeDictationProvider();
    const s = new DictationSession(p, b.tg, 'vi-VN');
    await s.start();
    p.final('kế hoạch quý một xuống dòng');
    p.final('mục tiêu');
    await s.written();
    // A line break inside the paragraph is `\v` in the editing format.
    expect(b.text()).toBe('Tiêu đề: kế hoạch quý một\vMục tiêu');
    expect(b.sent.map((c) => c.cmd)).toEqual(['setText', 'setText']);
    expect(b.sent[0]).toMatchObject({ id: 7, sel: [26, 26] });
    expect(b.sent.every((c) => c.cmd === 'setText' && !c.merge)).toBe(true);
  });

  it('has nowhere to write without a text being edited', async () => {
    const b = box('');
    b.stopEditing();
    expect(await b.tg.context()).toBeNull();
  });
});
