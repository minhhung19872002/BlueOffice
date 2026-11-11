import type { SlideCommand, SlideEditResult, SlideShapeInfo } from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { FONT_SIZES, paragraphRange, steppedSize, wordRange } from '../src/editing';
import { findInText, findNext, replaceAll, replaceHit, replaceInText } from '../src/findReplace';

const plain = { matchCase: false, wholeWord: false };

/** A fake deck: per slide, shapes `[id, text]`; answers the commands the search uses. */
function fakeDeck(slides: [number, string][][]) {
  let current = 0;
  let selected: number[] = [];
  const log: SlideCommand[] = [];
  const shape = (id: number, text: string): SlideShapeInfo => ({
    id,
    name: `s${id}`,
    kind: 'shape',
    rect: { cx: 0, cy: 0, w: 10, h: 10, rot: 0 },
    text,
    placeholder: null,
    lockAspect: false,
    editableText: true,
  });
  const result = (): SlideEditResult =>
    ({
      slide: current,
      selection: slides[current]!.filter(([id]) => selected.includes(id)).map(([id, t]) =>
        shape(id, t),
      ),
      history: log.filter((c) => c.cmd === 'setText').length,
    }) as unknown as SlideEditResult;
  const run = async (cmd: SlideCommand) => {
    log.push(cmd);
    if (cmd.cmd === 'goTo') {
      current = cmd.slide;
      selected = [];
    } else if (cmd.cmd === 'selectAll') selected = slides[current]!.map(([id]) => id);
    else if (cmd.cmd === 'select') selected = cmd.ids;
    else if (cmd.cmd === 'setText') {
      const s = slides[current]!.find(([id]) => id === cmd.id);
      if (s) s[1] = cmd.text;
    }
    return result();
  };
  return { run, log, slides, current: () => current, selected: () => selected };
}

describe('find and replace', () => {
  it('finds every match, case and whole words on request', () => {
    expect(findInText('Doanh thu, doanh nghiệp', 'doanh', plain)).toEqual([
      [0, 5],
      [11, 16],
    ]);
    expect(findInText('Doanh thu, doanh nghiệp', 'doanh', { ...plain, matchCase: true })).toEqual([
      [11, 16],
    ]);
    // "an" inside "ban" is not a whole word; Vietnamese letters count as letters.
    expect(findInText('ban an toàn, bán', 'an', { ...plain, wholeWord: true })).toEqual([[4, 6]]);
    expect(findInText('aaa', 'aa', plain)).toEqual([[0, 2]]);
    expect(findInText('abc', '', plain)).toEqual([]);
  });

  it('replaces every match and counts', () => {
    expect(replaceInText('a-b-a', 'a', 'xy', plain)).toEqual({ text: 'xy-b-xy', count: 2 });
    expect(replaceInText('abc', 'z', 'q', plain)).toEqual({ text: 'abc', count: 0 });
  });

  it('find next walks shapes and slides in order and wraps at the end', async () => {
    const d = fakeDeck([
      [
        [1, 'one fox'],
        [2, 'fox and fox'],
      ],
      [[3, 'nothing']],
      [[4, 'last fox']],
    ]);
    const seen: string[] = [];
    let hit = null;
    for (let i = 0; i < 5; i++) {
      hit = await findNext(d.run, 3, 0, hit, 'fox', plain);
      seen.push(hit ? `${hit.slide}/${hit.id}@${hit.start}` : 'none');
    }
    // Slide 1: shape 1, shape 2 twice; slide 3; then wrapped to the first again.
    expect(seen).toEqual(['0/1@4', '0/2@0', '0/2@8', '2/4@5', '0/1@4']);
    // The shape of the match is selected on its slide.
    expect(d.current()).toBe(0);
    expect(d.selected()).toEqual([1]);
    expect(await findNext(d.run, 3, 1, null, 'zebra', plain)).toBeNull();
    expect(d.current()).toBe(1);
  });

  it('replace changes the hit, replace all changes the deck as one undo step', async () => {
    const d = fakeDeck([[[1, 'fox fox']], [[2, 'a fox']]]);
    const hit = await findNext(d.run, 2, 0, null, 'fox', plain);
    expect(await replaceHit(d.run, hit!, 'fox', 'cat', plain)).toBe(true);
    expect(d.slides[0]![0]![1]).toBe('cat fox');
    // The text changed under a stale hit: nothing is replaced.
    expect(await replaceHit(d.run, hit!, 'fox', 'cat', plain)).toBe(false);
    d.log.length = 0;
    expect(await replaceAll(d.run, 2, 1, 'fox', 'dog', plain)).toBe(2);
    expect(d.slides.map((s) => s[0]![1])).toEqual(['cat dog', 'a dog']);
    expect(d.log.at(-1)).toEqual({ cmd: 'mergeHistory', since: 0 });
    expect(d.current()).toBe(1);
  });
});

describe('font size steps', () => {
  it('grow and shrink follow the font size list', () => {
    expect(steppedSize(44, true)).toBe(48);
    expect(steppedSize(18, true)).toBe(20);
    expect(steppedSize(10, true)).toBe(10.5);
    expect(steppedSize(19, true)).toBe(20);
    expect(steppedSize(48, false)).toBe(44);
    expect(steppedSize(19, false)).toBe(18);
    // Past the list: multiples of 10 points.
    expect(steppedSize(96, true)).toBe(100);
    expect(steppedSize(110, true)).toBe(120);
    expect(steppedSize(120, false)).toBe(110);
    expect(steppedSize(100, false)).toBe(96);
    expect(steppedSize(8, false)).toBe(7);
    expect(steppedSize(1, false)).toBe(1);
    // Every list size steps to its neighbours.
    for (let i = 1; i < FONT_SIZES.length; i++) {
      expect(steppedSize(FONT_SIZES[i - 1]!, true)).toBe(FONT_SIZES[i]);
      expect(steppedSize(FONT_SIZES[i]!, false)).toBe(FONT_SIZES[i - 1]);
    }
  });
});

describe('double-click and triple-click ranges', () => {
  const text = 'Mục tiêu năm 2026\nTăng doanh thu\u000b15%';
  it('a double-click takes the word around the caret (Vietnamese letters included)', () => {
    expect(wordRange(text, 5)).toEqual([4, 8]);
    expect(text.slice(...wordRange(text, 5))).toBe('tiêu');
    expect(text.slice(...wordRange(text, 0))).toBe('Mục');
    expect(text.slice(...wordRange(text, 17))).toBe('2026');
    // Between two spaces there is no word.
    expect(wordRange('a  b', 2)).toEqual([2, 2]);
  });
  it('a triple-click takes the paragraph, line breaks inside it included', () => {
    expect(paragraphRange(text, 3)).toEqual([0, 17]);
    expect(text.slice(...paragraphRange(text, 20))).toBe('Tăng doanh thu\u000b15%');
    expect(paragraphRange(text, 17)).toEqual([0, 17]);
    expect(paragraphRange('', 0)).toEqual([0, 0]);
  });
});
