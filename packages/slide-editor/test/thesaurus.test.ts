import type { SlideCommand, SlideEditResult } from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { slideReplaceWord, slideWordAtCaret, wordInText } from '../src/thesaurus';

const editing = (text: string, start: number, end = start) =>
  ({
    textEdit: { id: 7, start, end, caret: null, rects: [], rect: {}, text },
  }) as unknown as SlideEditResult;

describe('BlueSlide thesaurus helpers', () => {
  it('finds the word at the caret or a one-word selection', () => {
    expect(wordInText('Fast cars\vrun', 2, 2)).toEqual({ from: 0, to: 4, word: 'Fast' });
    expect(wordInText('Fast cars', 5, 9)?.word).toBe('cars');
    expect(wordInText('Fast cars', 0, 9)).toBeNull();
    expect(slideWordAtCaret(editing('a big deal', 4))?.word).toBe('big');
    expect(slideWordAtCaret(null)).toBeNull();
  });

  it('splices the replacement into the shape text with one setText', async () => {
    const sent: SlideCommand[] = [];
    let edit = editing('A Big deal', 4);
    const run = async (c: SlideCommand) => {
      sent.push(c);
      if (c.cmd === 'setText') edit = editing(c.text, c.sel?.[0] ?? 0);
      return edit;
    };
    expect(await slideReplaceWord(run, 'Big', 'Large')).toBe(true);
    expect(sent).toEqual([
      { cmd: 'state' },
      { cmd: 'setText', id: 7, text: 'A Large deal', sel: [7, 7], cell: undefined },
    ]);
    // The word is no longer there: nothing is sent.
    expect(await slideReplaceWord(run, 'Big', 'Huge')).toBe(false);
    expect(sent.filter((c) => c.cmd === 'setText')).toHaveLength(1);
  });
});
